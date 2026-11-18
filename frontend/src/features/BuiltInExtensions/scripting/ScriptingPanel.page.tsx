import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AppWindow, ChevronsLeft, ChevronsRight, PanelLeftClose, PanelLeftOpen, Play, Plus, Save, SquareCode, Trash2, X } from "lucide-react";
import type { PageScript, ScriptLanguage, ScriptRun } from "@type/browser";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { Toggle } from "@/components/Toggle";
import {
  deletePageScript,
  closeWindow,
  openScriptingWindow,
  pyodideReady,
  resizeAgentPanel,
  runPageScript,
  savePageScript,
  setAgentPanelWidth,
  toggleScriptingPanel,
} from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { useAppearanceState, useScripting, useSettings, useTabs } from "@/lib/use-browser";
import { siteKey } from "../page-controls/site";
import { compileSource } from "./compile";
import { ScriptingConsole, type ConsoleLine } from "./ScriptingConsole";
import { ScriptingEditor, type ScriptingEditorHandle } from "./ScriptingEditor";

const MIN_PANEL_WIDTH = 280;
const MAX_PANEL_WIDTH = 8192;
const DEFAULT_PANEL_WIDTH = 480;
const LAYOUT_KEY = "ce-scripting-layout";

type Pane = { size: number; collapsed: boolean; expanded: boolean };

type Layout = { sidebar: Pane; console: Pane; input: Pane };

const DEFAULT_LAYOUT: Layout = {
  sidebar: { size: 160, collapsed: false, expanded: false },
  console: { size: 208, collapsed: false, expanded: false },
  input: { size: 72, collapsed: false, expanded: false },
};

function readLayout(): Layout {
  try {
    const raw = localStorage.getItem(LAYOUT_KEY);
    if (!raw) {
      return DEFAULT_LAYOUT;
    }
    const parsed = JSON.parse(raw) as Partial<Layout>;
    return {
      sidebar: { ...DEFAULT_LAYOUT.sidebar, ...parsed.sidebar },
      console: { ...DEFAULT_LAYOUT.console, ...parsed.console },
      input: { ...DEFAULT_LAYOUT.input, ...parsed.input },
    };
  } catch {
    return DEFAULT_LAYOUT;
  }
}

function clampSize(size: number, min: number, max: number) {
  return Math.min(max, Math.max(min, Math.round(size)));
}

function clock(date: Date) {
  const time = date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit", second: "2-digit" });
  return `${time}.${String(date.getMilliseconds()).padStart(3, "0")}`;
}

const fieldClass =
  "h-7 min-w-0 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-xs text-zinc-100 outline-none focus:border-sky-500";

function clampPanelWidth(width: number) {
  return Math.min(MAX_PANEL_WIDTH, Math.max(MIN_PANEL_WIDTH, Math.round(width)));
}

function blank(domain: string): PageScript {
  return {
    id: "",
    name: "",
    domain,
    language: "javascript",
    enabled: true,
    runOnLoad: false,
    source: "",
    compiled: "",
  };
}

function matches(domain: string, script: PageScript) {
  return domain !== "" && (domain === script.domain || domain.endsWith(`.${script.domain}`));
}

let lineId = 1;

function linesFrom(input: string, result: ScriptRun): ConsoleLine[] {
  const lines: ConsoleLine[] = [
    { id: lineId++, kind: "input", level: "input", text: input },
  ];
  for (const log of result.logs) {
    lines.push({ id: lineId++, kind: "log", level: log.level, text: log.text });
  }
  if (result.error) {
    lines.push({ id: lineId++, kind: "error", level: "error", text: result.error });
  } else if (result.value !== null) {
    lines.push({ id: lineId++, kind: "result", level: "result", text: result.value });
  }
  return lines;
}

/** Editor and console for scripts saved against the site in the current tab. */
export function ScriptingPanelPage() {
  useAppearanceState();
  const settings = useSettings();
  const tabs = useTabs();
  const file = useScripting();
  const editor = useRef<ScriptingEditorHandle>(null);
  const width = settings?.agentPanelWidth ?? DEFAULT_PANEL_WIDTH;
  const resizing = useRef(false);
  const [resizingPanel, setResizingPanel] = useState(false);
  const [draft, setDraft] = useState<PageScript | null>(null);
  const [lines, setLines] = useState<ConsoleLine[]>([]);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [runClock, setRunClock] = useState<{ start: Date; stop: Date | null } | null>(null);
  const [pythonReady, setPythonReady] = useState<boolean | null>(null);
  const [layout, setLayout] = useState<Layout>(readLayout);

  const active = tabs.windowTabs.find((tab) => tab.id === tabs.activeId);
  const domain = siteKey(active?.url);
  const scripts = useMemo(
    () => (file?.scripts ?? []).filter((script) => matches(domain, script)),
    [file, domain],
  );

  useEffect(() => {
    document.title = "Scripting";
  }, []);

  useEffect(() => {
    localStorage.setItem(LAYOUT_KEY, JSON.stringify(layout));
  }, [layout]);

  useEffect(() => {
    void pyodideReady().then(setPythonReady).catch(() => setPythonReady(false));
  }, []);

  useEffect(() => {
    if (!domain) {
      setDraft(null);
      return;
    }
    setDraft((current) => {
      if (current && matches(domain, current)) {
        return current;
      }
      return scripts[0] ?? null;
    });
  }, [domain, scripts]);

  useEffect(() => {
    if (!draft?.id || !file) {
      return;
    }
    const saved = file.scripts.find((script) => script.id === draft.id);
    if (!saved) {
      setDraft(domain ? blank(domain) : null);
    }
  }, [file, draft?.id, domain]);

  const select = (script: PageScript) => {
    setStatus(null);
    setDraft(script);
  };

  const persist = async (next: PageScript) => {
    if (!domain) {
      setStatus("Open a website to save a script for it.");
      return null;
    }
    setStatus(null);
    const compiled = await compileSource(next.language, next.source);
    const before = new Set((file?.scripts ?? []).map((script) => script.id));
    const saved = await savePageScript({ ...next, domain, compiled });
    const current = saved.scripts.find((script) => script.id === next.id)
      ?? saved.scripts.find((script) => !before.has(script.id))
      ?? null;
    if (current) {
      setDraft(current);
    }
    return current;
  };

  const runSource = async (language: ScriptLanguage, source: string, expression: boolean, input: string) => {
    if (!active || !domain) {
      setStatus("Open a website to run a script.");
      return;
    }
    const start = new Date();
    setRunClock({ start, stop: null });
    setBusy(true);
    setStatus(language === "python" && pythonReady === false ? "Downloading the Python runtime…" : "Running");
    try {
      const code = language === "typescript" ? await compileSource("typescript", source) : source;
      const runLanguage = language === "python" || language === "css" ? language : "javascript";
      const result = await runPageScript(active.id, runLanguage, code, expression);
      setLines((current) => [...current, ...linesFrom(input, result)]);
      if (language === "python") {
        setPythonReady(true);
      }
      setStatus(result.error ? "Failed" : "Finished");
    } catch (reason: unknown) {
      setLines((current) => [
        ...current,
        ...linesFrom(input, { logs: [], value: null, error: String(reason) }),
      ]);
      setStatus("Failed");
    } finally {
      setRunClock({ start, stop: new Date() });
      setBusy(false);
    }
  };

  const onResizePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || resizing.current) {
      return;
    }
    event.preventDefault();
    resizing.current = true;
    setResizingPanel(true);
    void resizeAgentPanel().finally(() => {
      resizing.current = false;
      setResizingPanel(false);
    });
  };

  const setPane = (key: keyof Layout, patch: Partial<Pane>) => {
    setLayout((current) => ({ ...current, [key]: { ...current[key], ...patch } }));
  };

  const sidebarWidth = layout.sidebar.expanded ? Math.max(layout.sidebar.size, 320) : layout.sidebar.size;
  const poppedOut = getCurrentWindow().label === "popup-scripting";

  const onResizeKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = event.shiftKey ? 64 : 16;
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      void setAgentPanelWidth(clampPanelWidth(width + step));
    } else if (event.key === "ArrowRight") {
      event.preventDefault();
      void setAgentPanelWidth(clampPanelWidth(width - step));
    } else if (event.key === "Home") {
      event.preventDefault();
      void setAgentPanelWidth(DEFAULT_PANEL_WIDTH);
    }
  };

  return (
    <div
      id="scripting-panel"
      className={cn("relative flex h-screen flex-col border-l border-zinc-800 bg-zinc-950 text-zinc-100", resizingPanel && "cursor-col-resize select-none")}
    >
      {poppedOut ? null : (
      <div
        id="scripting-panel-resize"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize the scripting panel"
        aria-valuemin={MIN_PANEL_WIDTH}
        aria-valuemax={MAX_PANEL_WIDTH}
        aria-valuenow={Math.round(width)}
        tabIndex={0}
        title="Drag to resize. Left and right arrows change the width."
        className={cn(
          "absolute inset-y-0 left-0 z-20 w-1.5 cursor-col-resize touch-none hover:bg-sky-500/40 focus-visible:bg-sky-500/40 focus-visible:outline-none",
          focusRing,
          resizingPanel && "bg-sky-500/50",
        )}
        onPointerDown={onResizePointerDown}
        onKeyDown={onResizeKeyDown}
      />
      )}
      <header id="scripting-panel-header" className="flex h-[var(--toolbar-height)] shrink-0 items-center gap-2 border-b border-zinc-800 pl-4 pr-2">
        <SquareCode id="scripting-panel-icon" className="size-4 text-sky-400" aria-hidden />
        <h1 id="scripting-panel-title" className="text-sm font-semibold">
          Scripting
        </h1>
        <span id="scripting-panel-domain" className="min-w-0 flex-1 truncate text-xs text-zinc-500">
          {domain || "No website"}
        </span>
        {pythonReady === false ? (
          <span id="scripting-panel-python" className="shrink-0 text-[11px] text-zinc-500">
            Python downloads on first run
          </span>
        ) : null}
        {poppedOut ? null : (
          <IconButton id="scripting-panel-popout" label="Open in a new window" className="size-7" onClick={() => void openScriptingWindow()}>
            <AppWindow id="scripting-panel-popout-icon" aria-hidden />
          </IconButton>
        )}
        <IconButton
          id="scripting-panel-close"
          label={poppedOut ? "Close this window" : "Close the scripting panel"}
          className="size-7"
          onClick={() => void (poppedOut ? closeWindow() : toggleScriptingPanel(false))}
        >
          <X id="scripting-panel-close-icon" aria-hidden />
        </IconButton>
      </header>
      <div id="scripting-panel-body" className="flex min-h-0 flex-1">
        {layout.sidebar.collapsed ? (
          <div id="scripting-list-rail" className="flex w-8 shrink-0 flex-col items-center border-r border-zinc-800 py-1">
            <IconButton id="scripting-list-expand-rail" label="Expand the script list" aria-expanded={false} onClick={() => setPane("sidebar", { collapsed: false })}>
              <PanelLeftOpen id="scripting-list-expand-rail-icon" className="size-3.5" aria-hidden />
            </IconButton>
          </div>
        ) : (
        <aside id="scripting-list" style={{ width: sidebarWidth }} className="flex max-w-[50%] min-h-0 shrink-0 flex-col gap-1 overflow-y-auto border-r border-zinc-800 p-2">
          <div id="scripting-list-bar" className="flex items-center gap-1">
            <button
              id="scripting-new"
              type="button"
              disabled={!domain}
              className={cn("h-7 min-w-0 flex-1 rounded-md px-2 text-left text-xs text-sky-300 hover:bg-zinc-800 disabled:opacity-40", focusRing)}
              onClick={() => select(blank(domain))}
            >
              <Plus id="scripting-new-icon" className="mr-1 inline size-3" aria-hidden />
              New script
            </button>
            <IconButton
              id="scripting-list-expand"
              label={layout.sidebar.expanded ? "Shrink the script list" : "Expand the script list"}
              aria-pressed={layout.sidebar.expanded}
              onClick={() => setPane("sidebar", { expanded: !layout.sidebar.expanded, collapsed: false })}
            >
              {layout.sidebar.expanded
                ? <ChevronsLeft id="scripting-list-expand-icon" className="size-3.5" aria-hidden />
                : <ChevronsRight id="scripting-list-expand-icon" className="size-3.5" aria-hidden />}
            </IconButton>
            <IconButton id="scripting-list-collapse" label="Collapse the script list" aria-expanded onClick={() => setPane("sidebar", { collapsed: true, expanded: false })}>
              <PanelLeftClose id="scripting-list-collapse-icon" className="size-3.5" aria-hidden />
            </IconButton>
          </div>
          {file === null ? (
            <p id="scripting-loading" className="px-2 py-2 text-xs text-zinc-500">
              Loading…
            </p>
          ) : null}
          {file && scripts.length === 0 ? (
            <p id="scripting-empty" className="px-2 py-2 text-[11px] leading-relaxed text-zinc-500">
              No scripts for this site yet.
            </p>
          ) : null}
          {scripts.map((script) => (
            <button
              id={`scripting-item-${script.id}`}
              key={script.id}
              type="button"
              aria-current={draft?.id === script.id ? "true" : undefined}
              className={cn(
                "flex h-8 items-center gap-1.5 rounded-md px-2 text-left text-xs hover:bg-zinc-800",
                focusRing,
                draft?.id === script.id && "bg-zinc-800",
              )}
              onClick={() => select(script)}
            >
              <span
                id={`scripting-item-${script.id}-state`}
                className={script.enabled ? "size-1.5 rounded-full bg-sky-400" : "size-1.5 rounded-full bg-zinc-600"}
              />
              <span id={`scripting-item-${script.id}-name`} className="min-w-0 flex-1 truncate">
                {script.name || "Script"}
              </span>
            </button>
          ))}
        </aside>
        )}
        {layout.sidebar.collapsed ? null : (
          <div
            id="scripting-list-resize"
            role="separator"
            aria-orientation="vertical"
            aria-label="Resize the script list"
            aria-valuemin={120}
            aria-valuemax={480}
            aria-valuenow={Math.round(sidebarWidth)}
            tabIndex={0}
            title="Drag to resize. Left and right arrows change the width. Double-click expands."
            className={cn("w-1.5 shrink-0 cursor-col-resize touch-none hover:bg-sky-500/40 focus-visible:bg-sky-500/40 focus-visible:outline-none", focusRing)}
            onPointerDown={(event) => {
              if (event.button !== 0) {
                return;
              }
              event.preventDefault();
              const origin = sidebarWidth;
              const start = event.clientX;
              const node = event.currentTarget;
              node.setPointerCapture(event.pointerId);
              const move = (ev: globalThis.PointerEvent) => {
                setPane("sidebar", { expanded: false, collapsed: false, size: clampSize(origin + ev.clientX - start, 120, 480) });
              };
              const end = () => {
                node.removeEventListener("pointermove", move);
                node.removeEventListener("pointerup", end);
                node.removeEventListener("pointercancel", end);
              };
              node.addEventListener("pointermove", move);
              node.addEventListener("pointerup", end);
              node.addEventListener("pointercancel", end);
            }}
            onDoubleClick={() => setPane("sidebar", { expanded: !layout.sidebar.expanded, collapsed: false })}
            onKeyDown={(event) => {
              const step = event.shiftKey ? 32 : 16;
              if (event.key === "ArrowRight") {
                event.preventDefault();
                setPane("sidebar", { expanded: false, size: clampSize(layout.sidebar.size + step, 120, 480) });
              } else if (event.key === "ArrowLeft") {
                event.preventDefault();
                setPane("sidebar", { expanded: false, size: clampSize(layout.sidebar.size - step, 120, 480) });
              } else if (event.key === "Home") {
                event.preventDefault();
                setPane("sidebar", { expanded: false, size: 160 });
              }
            }}
          />
        )}
        <div id="scripting-main" className="flex min-w-0 flex-1 flex-col">
          {draft && domain ? (
            <>
              <div id="scripting-toolbar" className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-zinc-800 px-2 py-1.5">
                <input
                  id="scripting-name"
                  className={`${fieldClass} w-32`}
                  value={draft.name}
                  maxLength={80}
                  placeholder="Name"
                  aria-label="Script name"
                  onChange={(event) => setDraft({ ...draft, name: event.target.value })}
                />
                <select
                  id="scripting-language"
                  className={fieldClass}
                  aria-label="Language"
                  value={draft.language}
                  onChange={(event) => setDraft({ ...draft, language: event.target.value as ScriptLanguage })}
                >
                  <option id="scripting-language-javascript" value="javascript">JavaScript</option>
                  <option id="scripting-language-typescript" value="typescript">TypeScript</option>
                  <option id="scripting-language-python" value="python">Python</option>
                  <option id="scripting-language-css" value="css">CSS</option>
                </select>
                <label id="scripting-enabled" className="flex items-center gap-1.5 text-[11px] text-zinc-400">
                  <Toggle
                    id="scripting-enabled-toggle"
                    size="sm"
                    checked={draft.enabled}
                    aria-label="Enabled"
                    onChange={(enabled) => setDraft({ ...draft, enabled })}
                  />
                  On
                </label>
                <label id="scripting-run-on-load" className="flex items-center gap-1.5 text-[11px] text-zinc-400">
                  <Toggle
                    id="scripting-run-on-load-toggle"
                    size="sm"
                    checked={draft.runOnLoad}
                    aria-label="Run when this site loads"
                    onChange={(runOnLoad) => setDraft({ ...draft, runOnLoad })}
                  />
                  On load
                </label>
                <span id="scripting-toolbar-spacer" className="flex-1" />
                {draft.id ? (
                  <Button
                    id="scripting-delete"
                    size="xs"
                    variant="ghost"
                    onClick={() => {
                      void deletePageScript(draft.id)
                        .then(() => setDraft(blank(domain)))
                        .catch((reason: unknown) => setStatus(String(reason)));
                    }}
                  >
                    <Trash2 id="scripting-delete-icon" className="size-3" aria-hidden />
                    Delete
                  </Button>
                ) : null}
                <Button id="scripting-format" size="xs" variant="ghost" onClick={() => void editor.current?.format()}>
                  Format
                </Button>
                <Button
                  id="scripting-save"
                  size="xs"
                  variant="secondary"
                  onClick={() => {
                    void persist(draft).catch((reason: unknown) => setStatus(String(reason)));
                  }}
                >
                  <Save id="scripting-save-icon" className="size-3" aria-hidden />
                  Save
                </Button>
                <Button
                  id="scripting-run"
                  size="xs"
                  variant="primary"
                  disabled={busy}
                  onClick={() => {
                    void runSource(draft.language, draft.source, false, draft.name || draft.language);
                  }}
                >
                  <Play id="scripting-run-icon" className="size-3" aria-hidden />
                  Run
                </Button>
              </div>
              <div id="scripting-editor" className="min-h-0 flex-1">
                <ScriptingEditor
                  ref={editor}
                  scriptKey={`${draft.id || "new"}:${draft.language}`}
                  source={draft.source}
                  language={draft.language}
                  onChange={(source) => setDraft((current) => (current ? { ...current, source } : current))}
                />
              </div>
            </>
          ) : (
            <div id="scripting-placeholder" className="flex flex-1 items-center justify-center px-6 text-center text-sm text-zinc-500">
              {domain ? "Create a script for this site." : "Open a website, then write a script for it."}
            </div>
          )}
          <ScriptingConsole
            lines={lines}
            busy={busy}
            disabled={!domain}
            language={draft?.language ?? "javascript"}
            height={layout.console.size}
            collapsed={layout.console.collapsed}
            expanded={layout.console.expanded}
            inputHeight={layout.input.size}
            inputCollapsed={layout.input.collapsed}
            inputExpanded={layout.input.expanded}
            onClear={() => setLines([])}
            onToggleCollapsed={() => setPane("console", { collapsed: !layout.console.collapsed, expanded: false })}
            onToggleExpanded={() => setPane("console", { expanded: !layout.console.expanded, collapsed: false })}
            onResize={(delta, origin) => setPane("console", { expanded: false, collapsed: false, size: clampSize(origin - delta, 96, 640) })}
            onToggleInputCollapsed={() => setPane("input", { collapsed: !layout.input.collapsed, expanded: false })}
            onToggleInputExpanded={() => setPane("input", { expanded: !layout.input.expanded, collapsed: false })}
            onResizeInput={(delta, origin) => setPane("input", { expanded: false, collapsed: false, size: clampSize(origin - delta, 48, 280) })}
            onRun={(source) => {
              void runSource(draft?.language ?? "javascript", source, draft?.language !== "css", source);
            }}
          />
        </div>
      </div>
      <footer id="scripting-statusbar" className="flex h-7 shrink-0 items-center gap-4 border-t border-zinc-800 bg-zinc-950 px-3 text-[11px] text-zinc-400">
        <span
          id="scripting-statusbar-message"
          className={cn("min-w-0 flex-1 truncate", status === "Failed" && "text-red-300", status === "Running" && "text-sky-300")}
        >
          {status ?? "Ready"}
        </span>
        <span id="scripting-statusbar-start" className="shrink-0 tabular-nums">
          Start {runClock ? clock(runClock.start) : "—"}
        </span>
        <span id="scripting-statusbar-stop" className="shrink-0 tabular-nums">
          Stop {runClock?.stop ? clock(runClock.stop) : runClock ? "Running" : "—"}
        </span>
      </footer>
    </div>
  );
}
