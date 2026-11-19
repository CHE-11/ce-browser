import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react";
import type * as Monaco from "monaco-editor/editor/editor.api";
import type { ScriptLanguage } from "@type/browser";

export type ScriptingEditorHandle = {
  format: () => Promise<void>;
};

type ScriptingEditorProps = {
  scriptKey: string;
  source: string;
  language: ScriptLanguage;
  onChange: (source: string) => void;
};

type Marker = {
  message: string;
  startLine: number;
  startColumn: number;
  endLine: number;
  endColumn: number;
  severity: "error" | "warning";
};

const owners = { biome: "biome", ruff: "ruff" };

let workersReady = false;

function ensureWorkers() {
  if (workersReady) {
    return;
  }
  workersReady = true;
  window.MonacoEnvironment = {
    getWorker(_workerId, label) {
      if (label === "typescript" || label === "javascript") {
        return new Worker(new URL("monaco-editor/language/typescript/ts.worker.js", import.meta.url), { type: "module" });
      }
      return new Worker(new URL("monaco-editor/editor/editor.worker.js", import.meta.url), { type: "module" });
    },
  };
}

function monacoLanguage(language: ScriptLanguage) {
  if (language === "typescript") {
    return "typescript";
  }
  if (language === "python") {
    return "python";
  }
  if (language === "css") {
    return "css";
  }
  return "javascript";
}

function lintPath(language: ScriptLanguage) {
  if (language === "typescript") {
    return "script.ts";
  }
  if (language === "css") {
    return "script.css";
  }
  return "script.js";
}

function registerPython(monaco: typeof Monaco) {
  if (monaco.languages.getLanguages().some((language) => language.id === "python")) {
    return;
  }
  monaco.languages.register({ id: "python" });
  monaco.languages.setMonarchTokensProvider("python", {
    defaultToken: "",
    keywords: [
      "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else",
      "except", "False", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "None",
      "nonlocal", "not", "or", "pass", "raise", "return", "True", "try", "while", "with", "yield",
    ],
    tokenizer: {
      root: [
        [/#.*$/, "comment"],
        [/"""/, "string", "@triple"],
        [/'''/, "string", "@tripleSingle"],
        [/"([^"\\]|\\.)*$/, "string.invalid"],
        [/'([^'\\]|\\.)*$/, "string.invalid"],
        [/"/, "string", "@string"],
        [/'/, "string", "@stringSingle"],
        [/\d[\d_]*(?:\.[\d_]+)?/, "number"],
        [/[a-zA-Z_]\w*/, { cases: { "@keywords": "keyword", "@default": "identifier" } }],
        [/[{}()[\]]/, "@brackets"],
        [/[+\-*/%=<>!&|^~]+/, "operator"],
      ],
      string: [
        [/[^\\"]+/, "string"],
        [/\\./, "string.escape"],
        [/"/, "string", "@pop"],
      ],
      stringSingle: [
        [/[^\\']+/, "string"],
        [/\\./, "string.escape"],
        [/'/, "string", "@pop"],
      ],
      triple: [
        [/[^"]+/, "string"],
        [/"""/, "string", "@pop"],
        [/"/, "string"],
      ],
      tripleSingle: [
        [/[^']+/, "string"],
        [/'''/, "string", "@pop"],
        [/'/, "string"],
      ],
    },
  });
}

async function configureTypeScript() {
  const ts = await import("monaco-editor/languages/features/typescript/register.js");
  const options = {
    target: ts.ScriptTarget.ESNext,
    module: ts.ModuleKind.None,
    allowNonTsExtensions: true,
    allowJs: true,
    checkJs: true,
    strict: false,
    strictNullChecks: true,
    noEmit: true,
    lib: ["es2022", "dom"],
  };
  ts.typescriptDefaults.setCompilerOptions(options);
  ts.javascriptDefaults.setCompilerOptions(options);
  const diagnostics = { noSemanticValidation: false, noSyntaxValidation: false, noSuggestionDiagnostics: false };
  ts.typescriptDefaults.setDiagnosticsOptions(diagnostics);
  ts.javascriptDefaults.setDiagnosticsOptions(diagnostics);
  ts.typescriptDefaults.setEagerModelSync(true);
  ts.javascriptDefaults.setEagerModelSync(true);
}

function lineOf(value: unknown, fallback = 1) {
  if (!value || typeof value !== "object") {
    return fallback;
  }
  const record = value as Record<string, unknown>;
  const line = record.line ?? record.row ?? record.startLine;
  return typeof line === "number" && line > 0 ? line : fallback;
}

function columnOf(value: unknown) {
  if (!value || typeof value !== "object") {
    return 1;
  }
  const record = value as Record<string, unknown>;
  const column = record.column ?? record.col ?? record.startColumn;
  return typeof column === "number" && column > 0 ? column : 1;
}

function markersFrom(value: unknown): Marker[] {
  const list = Array.isArray(value)
    ? value
    : value && typeof value === "object" && Array.isArray((value as { diagnostics?: unknown }).diagnostics)
      ? (value as { diagnostics: unknown[] }).diagnostics
      : [];
  return list.flatMap((item) => {
    if (!item || typeof item !== "object") {
      return [];
    }
    const record = item as Record<string, unknown>;
    const message = String(record.message ?? record.description ?? record.text ?? "").trim();
    if (!message) {
      return [];
    }
    const location = (record.location ?? record.span ?? record.range) as Record<string, unknown> | undefined;
    const start = location?.start ?? record.start_location ?? record.startLocation ?? location;
    const end = location?.end ?? record.end_location ?? record.endLocation ?? start;
    const severity = String(record.severity ?? "error").toLowerCase().includes("warn") ? "warning" : "error";
    return [{
      message,
      startLine: lineOf(start),
      startColumn: columnOf(start),
      endLine: lineOf(end, lineOf(start)),
      endColumn: Math.max(columnOf(end), columnOf(start) + 1),
      severity,
    }];
  });
}

function applyMarkers(monaco: typeof Monaco, model: Monaco.editor.ITextModel, owner: string, markers: Marker[]) {
  monaco.editor.setModelMarkers(
    model,
    owner,
    markers.map((marker) => ({
      message: marker.message,
      severity: marker.severity === "warning" ? monaco.MarkerSeverity.Warning : monaco.MarkerSeverity.Error,
      startLineNumber: marker.startLine,
      startColumn: marker.startColumn,
      endLineNumber: marker.endLine,
      endColumn: marker.endColumn,
    })),
  );
}

type BiomeSession = {
  format: (source: string, filePath: string) => string;
  lint: (source: string, filePath: string) => Marker[];
};

function positionAt(source: string, offset: number) {
  const clipped = Math.max(0, Math.min(offset, source.length));
  let line = 1;
  let column = 1;
  for (let index = 0; index < clipped; index += 1) {
    if (source.charCodeAt(index) === 10) {
      line += 1;
      column = 1;
    } else {
      column += 1;
    }
  }
  return { line, column };
}

function biomeMarkers(diagnostics: unknown, source: string, toCodeUnits: (span: [number, number], text: string) => [number, number]): Marker[] {
  if (!Array.isArray(diagnostics)) {
    return [];
  }
  return diagnostics.flatMap((item) => {
    if (!item || typeof item !== "object") {
      return [];
    }
    const record = item as {
      description?: string;
      severity?: string;
      location?: { span?: [number, number] };
    };
    const message = record.description?.trim() ?? "";
    if (!message) {
      return [];
    }
    const span = record.location?.span;
    const units = span ? toCodeUnits(span, source) : [0, 1];
    const start = positionAt(source, units[0]);
    const end = positionAt(source, units[1]);
    const severity = record.severity === "warning" || record.severity === "information" || record.severity === "hint" ? "warning" : "error";
    return [{
      message,
      startLine: start.line,
      startColumn: start.column,
      endLine: end.line,
      endColumn: Math.max(end.column, start.column + 1),
      severity,
    }];
  });
}

let biomePromise: Promise<BiomeSession | null> | null = null;

function biomeSession() {
  biomePromise ??= (async () => {
    try {
      const wasm = await import("@biomejs/wasm-web");
      await wasm.default();
      const api = await import("@biomejs/js-api/web");
      const biome = new api.Biome();
      const project = biome.openProject();
      try {
        biome.applyConfiguration(project.projectKey, {
          formatter: { enabled: true, indentStyle: "space", indentWidth: 2 },
          linter: { enabled: true, rules: { recommended: true } },
          css: { formatter: { enabled: true }, linter: { enabled: true } },
        });
      } catch {
        // The defaults still format and lint.
      }
      return {
        format(source: string, filePath: string) {
          return biome.formatContent(project.projectKey, source, { filePath }).content;
        },
        lint(source: string, filePath: string) {
          const result = biome.lintContent(project.projectKey, source, { filePath });
          return biomeMarkers(result.diagnostics, source, api.spanInBytesToSpanInCodeUnits);
        },
      } satisfies BiomeSession;
    } catch (error) {
      console.warn("Biome did not start", error);
      return null;
    }
  })();
  return biomePromise;
}

type RuffSession = {
  format: (source: string) => string;
  lint: (source: string) => Marker[];
};

let ruffPromise: Promise<RuffSession | null> | null = null;

function ruffSession() {
  ruffPromise ??= (async () => {
    try {
      const ruff = await import("@astral-sh/ruff-wasm-web");
      await ruff.default();
      const workspace = new ruff.Workspace(
        {
          "line-length": 88,
          "indent-width": 4,
          format: { "indent-style": "space", "quote-style": "double" },
          lint: { select: ["E4", "E7", "E9", "F"] },
        },
        ruff.PositionEncoding.Utf16,
      );
      return {
        format(source: string) {
          return workspace.format(source);
        },
        lint(source: string) {
          return markersFrom(workspace.check(source));
        },
      } satisfies RuffSession;
    } catch (error) {
      console.warn("Ruff did not start", error);
      return null;
    }
  })();
  return ruffPromise;
}

/** Monaco editor with type checking for JavaScript and TypeScript, and lint plus format for every language. */
export const ScriptingEditor = forwardRef<ScriptingEditorHandle, ScriptingEditorProps>(function ScriptingEditor(
  { scriptKey, source, language, onChange },
  ref,
) {
  const host = useRef<HTMLDivElement>(null);
  const [editorReady, setEditorReady] = useState(false);
  const editorRef = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<typeof Monaco | null>(null);
  const onChangeRef = useRef(onChange);
  const languageRef = useRef(language);
  const sourceRef = useRef(source);
  const keyRef = useRef(scriptKey);
  onChangeRef.current = onChange;
  languageRef.current = language;
  sourceRef.current = source;

  useImperativeHandle(ref, () => ({
    async format() {
      const editor = editorRef.current;
      const monaco = monacoRef.current;
      const model = editor?.getModel();
      if (!editor || !monaco || !model) {
        return;
      }
      const current = model.getValue();
      try {
        const next = languageRef.current === "python"
          ? (await ruffSession())?.format(current) ?? current
          : (await biomeSession())?.format(current, lintPath(languageRef.current)) ?? current;
        if (next && next !== current) {
          editor.executeEdits("format", [{ range: model.getFullModelRange(), text: next }]);
          onChangeRef.current(editor.getValue());
        }
      } catch (error) {
        console.warn("Format failed", error);
      }
    },
  }));

  useEffect(() => {
    const node = host.current;
    if (!node) {
      return;
    }
    let cancelled = false;
    let dispose = () => undefined;
    void (async () => {
      ensureWorkers();
      const monaco = await import("monaco-editor/editor/editor.api");
      await import("monaco-editor/languages/definitions/javascript/register.js");
      await import("monaco-editor/languages/definitions/typescript/register.js");
      await import("monaco-editor/languages/definitions/css/register.js");
      await import("monaco-editor.css");
      if (cancelled || !host.current) {
        return;
      }
      registerPython(monaco);
      await configureTypeScript();
      monaco.editor.defineTheme("scripting", {
        base: "vs-dark",
        inherit: true,
        rules: [],
        colors: { "editor.background": "#09090b" },
      });
      const editor = monaco.editor.create(host.current, {
        value: sourceRef.current,
        language: monacoLanguage(languageRef.current),
        theme: "scripting",
        automaticLayout: true,
        minimap: { enabled: false },
        fontSize: 13,
        fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
        tabSize: languageRef.current === "python" ? 4 : 2,
        scrollBeyondLastLine: false,
        wordWrap: "on",
        padding: { top: 8 },
        fixedOverflowWidgets: true,
      });
      editorRef.current = editor;
      monacoRef.current = monaco;
      setEditorReady(true);
      const model = editor.getModel();
      const subscription = editor.onDidChangeModelContent(() => {
        onChangeRef.current(editor.getValue());
      });
      dispose = () => {
        subscription.dispose();
        editor.dispose();
        model?.dispose();
      };
    })();
    return () => {
      cancelled = true;
      dispose();
      editorRef.current = null;
      setEditorReady(false);
    };
  }, []);

  useEffect(() => {
    const editor = editorRef.current;
    const monaco = monacoRef.current;
    const model = editor?.getModel();
    if (!editor || !monaco || !model) {
      return;
    }
    const nextLanguage = monacoLanguage(language);
    if (model.getLanguageId() !== nextLanguage) {
      monaco.editor.setModelLanguage(model, nextLanguage);
    }
    editor.updateOptions({ tabSize: language === "python" ? 4 : 2 });
    if (keyRef.current !== scriptKey || model.getValue() !== source) {
      keyRef.current = scriptKey;
      if (model.getValue() !== source) {
        editor.setValue(source);
      }
    }
  }, [scriptKey, source, language, editorReady]);

  useEffect(() => {
    const editor = editorRef.current;
    const monaco = monacoRef.current;
    const model = editor?.getModel();
    if (!editor || !monaco || !model) {
      return;
    }
    const handle = window.setTimeout(() => {
      const current = model.getValue();
      if (language === "python") {
        applyMarkers(monaco, model, owners.biome, []);
        void ruffSession().then((session) => {
          if (editorRef.current?.getModel() === model) {
            applyMarkers(monaco, model, owners.ruff, session?.lint(current) ?? []);
          }
        });
        return;
      }
      applyMarkers(monaco, model, owners.ruff, []);
      const filePath = lintPath(language);
      void biomeSession().then((session) => {
        if (editorRef.current?.getModel() === model) {
          applyMarkers(monaco, model, owners.biome, session?.lint(current, filePath) ?? []);
        }
      });
    }, 300);
    return () => window.clearTimeout(handle);
  }, [source, language, scriptKey, editorReady]);

  return <div id="scripting-editor-host" ref={host} className="h-full min-h-0 w-full" />;
});
