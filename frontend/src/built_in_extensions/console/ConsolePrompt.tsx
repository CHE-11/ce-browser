import { useEffect, useRef } from "react";
import type * as Monaco from "monaco-editor/editor/editor.api";
import { consoleComplete } from "@/lib/browser-client";

type ConsolePromptProps = {
  tabId: number | null;
  origin: string;
  busy: boolean;
  onRun: (source: string) => void;
};

const HISTORY_LIMIT = 100;
const tabsByModel = new Map<string, () => number | null>();
let providerReady = false;
let workersReady = false;

function historyKey(origin: string) {
  return `ce-console-history:${origin || "blank"}`;
}

function readHistory(origin: string) {
  try {
    const raw = localStorage.getItem(historyKey(origin));
    const parsed = raw ? (JSON.parse(raw) as unknown) : [];
    return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === "string").slice(0, HISTORY_LIMIT) : [];
  } catch {
    return [];
  }
}

function writeHistory(origin: string, items: string[]) {
  localStorage.setItem(historyKey(origin), JSON.stringify(items.slice(0, HISTORY_LIMIT)));
}

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

function suggestOpen(host: HTMLElement) {
  const widget = host.querySelector(".suggest-widget, .parameter-hints-widget");
  if (!widget) {
    return false;
  }
  const style = getComputedStyle(widget);
  return style.display !== "none" && style.visibility !== "hidden";
}

function registerProvider(monaco: typeof Monaco) {
  if (providerReady) {
    return;
  }
  providerReady = true;
  monaco.languages.registerCompletionItemProvider("javascript", {
    triggerCharacters: [".", "$", "_"],
    provideCompletionItems: async (model, position) => {
      const tab = tabsByModel.get(model.uri.toString())?.() ?? null;
      if (tab == null) {
        return { suggestions: [] };
      }
      const offset = model.getOffsetAt(position);
      const head = model.getValue().slice(0, offset);
      const match = head.match(/[A-Za-z_$][\w$]*$/);
      const stem = match?.[0] ?? "";
      let items: string[] = [];
      try {
        items = await consoleComplete(tab, head);
      } catch {
        items = [];
      }
      const startColumn = position.column - stem.length;
      return {
        suggestions: items.map((name) => ({
          label: name,
          kind: monaco.languages.CompletionItemKind.Property,
          insertText: name,
          range: {
            startLineNumber: position.lineNumber,
            startColumn,
            endLineNumber: position.lineNumber,
            endColumn: position.column,
          },
        })),
      };
    },
  });
}

/** A JavaScript prompt. Enter runs it in the page. Up and down walk what you ran here before. */
export function ConsolePrompt({ tabId, origin, busy, onRun }: ConsolePromptProps) {
  const host = useRef<HTMLDivElement>(null);
  const editorRef = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<typeof Monaco | null>(null);
  const historyRef = useRef<string[]>([]);
  const cursor = useRef(-1);
  const draft = useRef("");
  const tabRef = useRef(tabId);
  const busyRef = useRef(busy);
  const runRef = useRef(onRun);
  const originRef = useRef(origin);
  tabRef.current = tabId;
  busyRef.current = busy;
  runRef.current = onRun;
  originRef.current = origin;

  useEffect(() => {
    historyRef.current = readHistory(origin);
    cursor.current = -1;
    draft.current = "";
  }, [origin]);

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
      await import("monaco-editor.css");
      if (cancelled || !host.current) {
        return;
      }
      registerProvider(monaco);
      monaco.editor.defineTheme("console", {
        base: "vs-dark",
        inherit: true,
        rules: [],
        colors: { "editor.background": "#18181b" },
      });
      const model = monaco.editor.createModel("", "javascript", monaco.Uri.parse(`inmemory://console/prompt-${crypto.randomUUID()}.js`));
      tabsByModel.set(model.uri.toString(), () => tabRef.current);
      const editor = monaco.editor.create(host.current, {
        model,
        theme: "console",
        automaticLayout: true,
        minimap: { enabled: false },
        lineNumbers: "off",
        glyphMargin: false,
        folding: false,
        overviewRulerLanes: 0,
        renderLineHighlight: "none",
        scrollBeyondLastLine: false,
        wordWrap: "on",
        fontSize: 13,
        lineHeight: 20,
        fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
        padding: { top: 6, bottom: 6 },
        fixedOverflowWidgets: true,
        scrollbar: { vertical: "auto", horizontal: "hidden", verticalScrollbarSize: 8 },
        placeholder: "JavaScript on this page",
      });
      editorRef.current = editor;
      monacoRef.current = monaco;

      const remember = (source: string) => {
        const next = [source, ...historyRef.current.filter((item) => item !== source)].slice(0, HISTORY_LIMIT);
        historyRef.current = next;
        writeHistory(originRef.current, next);
      };

      const submit = () => {
        const source = editor.getValue();
        if (!source.trim() || busyRef.current || tabRef.current == null) {
          return;
        }
        remember(source);
        cursor.current = -1;
        draft.current = "";
        editor.setValue("");
        runRef.current(source);
      };

      const recall = (step: -1 | 1) => {
        const history = historyRef.current;
        if (step < 0) {
          if (cursor.current === -1) {
            draft.current = editor.getValue();
          }
          const next = Math.min(history.length - 1, cursor.current + 1);
          const item = history[next];
          if (item === undefined) {
            return;
          }
          cursor.current = next;
          editor.setValue(item);
        } else if (cursor.current > 0) {
          cursor.current -= 1;
          editor.setValue(history[cursor.current] ?? "");
        } else if (cursor.current === 0) {
          cursor.current = -1;
          editor.setValue(draft.current);
        } else {
          return;
        }
        const modelNow = editor.getModel();
        const line = modelNow?.getLineCount() ?? 1;
        editor.setPosition({ lineNumber: line, column: (modelNow?.getLineLength(line) ?? 0) + 1 });
      };

      const keys = editor.onKeyDown((event) => {
        if (!host.current || suggestOpen(host.current)) {
          return;
        }
        if (event.keyCode === monaco.KeyCode.Enter && !event.shiftKey && !event.ctrlKey && !event.altKey && !event.metaKey) {
          event.preventDefault();
          event.stopPropagation();
          submit();
          return;
        }
        const position = editor.getPosition();
        const current = editor.getModel();
        if (!position || !current) {
          return;
        }
        if (event.keyCode === monaco.KeyCode.UpArrow && position.lineNumber === 1) {
          event.preventDefault();
          event.stopPropagation();
          recall(-1);
        }
        if (event.keyCode === monaco.KeyCode.DownArrow && position.lineNumber === current.getLineCount()) {
          event.preventDefault();
          event.stopPropagation();
          recall(1);
        }
      });
      dispose = () => {
        tabsByModel.delete(model.uri.toString());
        keys.dispose();
        editor.dispose();
        model.dispose();
      };
    })();
    return () => {
      cancelled = true;
      dispose();
      editorRef.current = null;
    };
  }, []);

  return (
    <div id="console-prompt" className="flex min-h-0 flex-1 bg-zinc-900">
      <span id="console-prompt-mark" className="select-none pl-3 pt-[7px] font-mono text-sm text-sky-400" aria-hidden>
        ›
      </span>
      <div id="console-prompt-editor" ref={host} className="min-h-0 min-w-0 flex-1" />
    </div>
  );
}
