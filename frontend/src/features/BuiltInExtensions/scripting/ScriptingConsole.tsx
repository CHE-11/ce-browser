import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { ChevronsDown, ChevronsUp, Maximize2, Minimize2, Trash2 } from "lucide-react";
import type { ScriptLanguage } from "@type/browser";
import { IconButton } from "@/components/IconButton";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";

export type ConsoleLine = {
  id: number;
  kind: "input" | "log" | "result" | "error";
  level: string;
  text: string;
};

type ScriptingConsoleProps = {
  lines: ConsoleLine[];
  busy: boolean;
  disabled: boolean;
  language: ScriptLanguage;
  height: number;
  collapsed: boolean;
  expanded: boolean;
  inputHeight: number;
  inputCollapsed: boolean;
  inputExpanded: boolean;
  onRun: (source: string) => void;
  onClear: () => void;
  onToggleCollapsed: () => void;
  onToggleExpanded: () => void;
  onResize: (delta: number, origin: number) => void;
  onToggleInputCollapsed: () => void;
  onToggleInputExpanded: () => void;
  onResizeInput: (delta: number, origin: number) => void;
};

const levelClass: Record<string, string> = {
  warn: "text-amber-300",
  error: "text-red-300",
  debug: "text-zinc-500",
  result: "text-sky-300",
  input: "text-zinc-400",
};

function trackDrag(event: PointerEvent<HTMLElement>, onDelta: (delta: number) => void) {
  if (event.button !== 0) {
    return;
  }
  event.preventDefault();
  const origin = event.clientY;
  const node = event.currentTarget;
  node.setPointerCapture(event.pointerId);
  const move = (ev: globalThis.PointerEvent) => onDelta(ev.clientY - origin);
  const end = () => {
    node.removeEventListener("pointermove", move);
    node.removeEventListener("pointerup", end);
    node.removeEventListener("pointercancel", end);
  };
  node.addEventListener("pointermove", move);
  node.addEventListener("pointerup", end);
  node.addEventListener("pointercancel", end);
}

/** Log output and a prompt that runs one entry in the page. */
export function ScriptingConsole({
  lines,
  busy,
  disabled,
  language,
  height,
  collapsed,
  expanded,
  inputHeight,
  inputCollapsed,
  inputExpanded,
  onRun,
  onClear,
  onToggleCollapsed,
  onToggleExpanded,
  onResize,
  onToggleInputCollapsed,
  onToggleInputExpanded,
  onResizeInput,
}: ScriptingConsoleProps) {
  const [draft, setDraft] = useState("");
  const [history, setHistory] = useState<string[]>([]);
  const [cursor, setCursor] = useState(-1);
  const end = useRef<HTMLDivElement>(null);
  const inputLabel = language === "css" ? "CSS" : "Expression";

  useEffect(() => {
    end.current?.scrollIntoView({ block: "nearest" });
  }, [lines]);

  const submit = () => {
    const source = draft;
    if (!source.trim() || disabled || busy) {
      return;
    }
    setHistory((items) => [source, ...items.filter((item) => item !== source)].slice(0, 50));
    setCursor(-1);
    setDraft("");
    onRun(source);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      submit();
      return;
    }
    if (event.key === "ArrowUp" && (draft === "" || cursor >= 0)) {
      const next = Math.min(history.length - 1, cursor + 1);
      const item = history[next];
      if (item !== undefined) {
        event.preventDefault();
        setCursor(next);
        setDraft(item);
      }
      return;
    }
    if (event.key === "ArrowDown" && cursor >= 0) {
      event.preventDefault();
      const next = cursor - 1;
      setCursor(next);
      setDraft(next < 0 ? "" : (history[next] ?? ""));
    }
  };

  const shownHeight = collapsed ? 40 : expanded ? Math.max(height, 360) : height;
  const shownInput = inputExpanded ? Math.max(inputHeight, 160) : inputHeight;

  return (
    <section
      id="scripting-console"
      style={{ height: shownHeight }}
      className="flex max-h-[70%] shrink-0 flex-col border-t border-zinc-800 bg-zinc-950"
    >
      <div
        id="scripting-console-resize"
        role="separator"
        aria-orientation="horizontal"
        aria-label="Resize the console"
        aria-valuemin={96}
        aria-valuemax={640}
        aria-valuenow={Math.round(shownHeight)}
        tabIndex={0}
        title="Drag to resize. Up and down arrows change the height. Double-click expands."
        className={cn("h-1.5 shrink-0 cursor-row-resize touch-none hover:bg-sky-500/40 focus-visible:bg-sky-500/40 focus-visible:outline-none", focusRing)}
        onPointerDown={(event) => trackDrag(event, (delta) => onResize(delta, shownHeight))}
        onDoubleClick={onToggleExpanded}
        onKeyDown={(event) => {
          const step = event.shiftKey ? 48 : 16;
          if (event.key === "ArrowUp") {
            event.preventDefault();
            onResize(-step, shownHeight);
          } else if (event.key === "ArrowDown") {
            event.preventDefault();
            onResize(step, shownHeight);
          } else if (event.key === "Home") {
            event.preventDefault();
            onResize(0, 208);
          }
        }}
      />
      <div id="scripting-console-bar" className="flex h-8 shrink-0 items-center gap-1 px-1">
        <IconButton
          id="scripting-console-collapse"
          label={collapsed ? "Expand console" : "Collapse console"}
          aria-expanded={!collapsed}
          onClick={onToggleCollapsed}
        >
          {collapsed ? <ChevronsUp id="scripting-console-collapse-icon" className="size-3.5" aria-hidden /> : <ChevronsDown id="scripting-console-collapse-icon" className="size-3.5" aria-hidden />}
        </IconButton>
        <IconButton
          id="scripting-console-expand"
          label={expanded ? "Shrink console" : "Expand console"}
          aria-pressed={expanded}
          onClick={onToggleExpanded}
        >
          {expanded
            ? <Minimize2 id="scripting-console-expand-icon" className="size-3.5" aria-hidden />
            : <Maximize2 id="scripting-console-expand-icon" className="size-3.5" aria-hidden />}
        </IconButton>
        <span id="scripting-console-label" className="text-xs font-medium text-zinc-400">
          Console
        </span>
        <span id="scripting-console-language" className="text-[11px] text-zinc-600">
          {language}
        </span>
        <span id="scripting-console-spacer" className="flex-1" />
        <IconButton id="scripting-console-clear" label="Clear console" onClick={onClear}>
          <Trash2 id="scripting-console-clear-icon" className="size-3.5" aria-hidden />
        </IconButton>
      </div>
      {collapsed ? null : (
        <>
          <div id="scripting-console-log" className="min-h-0 flex-1 overflow-y-auto px-2 font-mono text-[12px] leading-5">
            {lines.length === 0 ? (
              <p id="scripting-console-empty" className="text-zinc-600">
                Run the script, or type below and press Enter. Shift+Enter inserts a line.
              </p>
            ) : null}
            {lines.map((line) => (
              <p
                id={`scripting-console-line-${line.id}`}
                key={line.id}
                className={cn("whitespace-pre-wrap break-all", levelClass[line.kind === "log" ? line.level : line.kind] ?? "text-zinc-200")}
              >
                {line.kind === "input" ? "> " : ""}
                {line.text}
              </p>
            ))}
            <div id="scripting-console-end" ref={end} />
          </div>
          <div
            id="scripting-console-input-resize"
            role="separator"
            aria-orientation="horizontal"
            aria-label="Resize the expression"
            aria-valuemin={48}
            aria-valuemax={280}
            aria-valuenow={inputCollapsed ? 28 : Math.round(shownInput)}
            tabIndex={0}
            title="Drag to resize the expression. Double-click expands it."
            className={cn("h-1.5 shrink-0 cursor-row-resize touch-none border-t border-zinc-800 hover:bg-sky-500/40 focus-visible:bg-sky-500/40 focus-visible:outline-none", focusRing)}
            onPointerDown={(event) => trackDrag(event, (delta) => onResizeInput(delta, shownInput))}
            onDoubleClick={onToggleInputExpanded}
            onKeyDown={(event) => {
              const step = event.shiftKey ? 32 : 12;
              if (event.key === "ArrowUp") {
                event.preventDefault();
                onResizeInput(-step, shownInput);
              } else if (event.key === "ArrowDown") {
                event.preventDefault();
                onResizeInput(step, shownInput);
              } else if (event.key === "Home") {
                event.preventDefault();
                onResizeInput(0, 64);
              }
            }}
          />
          {inputCollapsed ? (
            <button
              id="scripting-console-input-show"
              type="button"
              className={cn("flex h-7 shrink-0 items-center gap-2 px-2 text-left text-xs text-zinc-400 hover:bg-zinc-900", focusRing)}
              aria-expanded={false}
              onClick={onToggleInputCollapsed}
            >
              <ChevronsUp id="scripting-console-input-show-icon" className="size-3.5" aria-hidden />
              {inputLabel}
            </button>
          ) : (
            <div id="scripting-console-input" style={{ height: shownInput }} className="flex shrink-0 flex-col">
              <div id="scripting-console-input-bar" className="flex h-6 shrink-0 items-center gap-1 px-1">
                <span id="scripting-console-input-label" className="px-1 text-[11px] text-zinc-500">
                  {inputLabel}
                </span>
                <span id="scripting-console-input-spacer" className="flex-1" />
                <IconButton
                  id="scripting-console-input-expand"
                  label={inputExpanded ? "Shrink the expression" : "Expand the expression"}
                  aria-pressed={inputExpanded}
                  onClick={onToggleInputExpanded}
                >
                  {inputExpanded
                    ? <Minimize2 id="scripting-console-input-expand-icon" className="size-3.5" aria-hidden />
                    : <Maximize2 id="scripting-console-input-expand-icon" className="size-3.5" aria-hidden />}
                </IconButton>
                <IconButton id="scripting-console-input-collapse" label="Collapse the expression" aria-expanded onClick={onToggleInputCollapsed}>
                  <ChevronsDown id="scripting-console-input-collapse-icon" className="size-3.5" aria-hidden />
                </IconButton>
              </div>
              <label id="scripting-console-input-field" className="flex min-h-0 flex-1 items-start gap-2 px-2 pb-1.5">
                <span id="scripting-console-prompt" className="pt-1 font-mono text-xs text-zinc-500">
                  {">"}
                </span>
                <textarea
                  id="scripting-console-field"
                  className="h-full min-h-0 flex-1 resize-none bg-transparent font-mono text-[12px] text-zinc-100 outline-none placeholder:text-zinc-600"
                  value={draft}
                  spellCheck={false}
                  disabled={disabled || busy}
                  placeholder={disabled ? "Open a website to run code" : language === "css" ? "CSS to apply" : "Expression or statements"}
                  onChange={(event) => {
                    setCursor(-1);
                    setDraft(event.target.value);
                  }}
                  onKeyDown={onKeyDown}
                />
              </label>
            </div>
          )}
        </>
      )}
    </section>
  );
}
