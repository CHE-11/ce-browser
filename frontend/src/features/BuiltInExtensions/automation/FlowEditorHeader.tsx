import { useState } from "react";
import { AlertTriangle, Circle, Code2, Copy, Download, MoreHorizontal, Pause, Play, Save, Square, Trash2 } from "lucide-react";
import type { Flow, FlowRun, TabInfo } from "@type/browser";
import { Button } from "@/components/Button";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "@/components/ContextMenu";
import { IconButton } from "@/components/IconButton";
import { cn } from "@/lib/cn";
import { inputClassName } from "@/features/BuiltInExtensions/agent/agent-styles";
import { formatWhen, statusTone, untilText } from "./flow-model";

export type EditorMenuAction = "duplicate" | "export" | "playwright" | "delete";

type FlowEditorHeaderProps = {
  flow: Flow;
  dirty: boolean;
  issueCount: number;
  activeRun: FlowRun | undefined;
  nextRun: number | undefined;
  /** Why the scheduler isn't firing triggers now. Empty while it runs. */
  schedulerPaused: string;
  now: number;
  runTab: number | "own";
  tabs: TabInfo[];
  onRunTabChange: (tab: number | "own") => void;
  onNameChange: (name: string) => void;
  onRun: () => void;
  onStop: () => void;
  onSave: () => void;
  onTogglePause: () => void;
  onRecordMore: () => void;
  onShowIssues: () => void;
  onMenu: (action: EditorMenuAction) => void;
};

function statusLine(flow: Flow, activeRun: FlowRun | undefined, nextRun: number | undefined, schedulerPaused: string, now: number) {
  if (activeRun) {
    return {
      tone: "text-sky-400",
      text: activeRun.current !== null ? `Running step ${activeRun.current + 1} of ${flow.steps.length}: ${activeRun.currentLabel}` : "Starting…",
    };
  }
  const parts: string[] = [];
  if (flow.lastRun) {
    parts.push(`Last run ${formatWhen(flow.lastRun)}, ${flow.lastStatus}`);
  }
  if (flow.paused) {
    parts.push("Triggers paused");
  } else if (nextRun) {
    parts.push(`Next run ${untilText(nextRun, now)}`);
  } else if (schedulerPaused && flow.triggers.some((trigger) => trigger.enabled)) {
    parts.push(schedulerPaused);
  }
  return { tone: flow.lastStatus === "failed" ? statusTone("failed") : "text-zinc-500", text: parts.join(" · ") };
}

export function FlowEditorHeader({
  flow,
  dirty,
  issueCount,
  activeRun,
  nextRun,
  schedulerPaused,
  now,
  runTab,
  tabs,
  onRunTabChange,
  onNameChange,
  onRun,
  onStop,
  onSave,
  onTogglePause,
  onRecordMore,
  onShowIssues,
  onMenu,
}: FlowEditorHeaderProps) {
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const status = statusLine(flow, activeRun, nextRun, schedulerPaused, now);
  const act = (action: EditorMenuAction) => {
    setMenu(null);
    onMenu(action);
  };

  return (
    <header id="flow-editor-header" className="sticky top-0 z-10 flex flex-col gap-1 border-b border-zinc-800 bg-zinc-950/95 px-3 pt-3 pb-2 backdrop-blur">
      <div id="flow-editor-header-row" className="flex flex-wrap items-center gap-2">
        <input
          id="flow-editor-name"
          className="min-w-0 flex-1 basis-32 bg-transparent text-lg font-semibold text-zinc-100 outline-none placeholder:text-zinc-600"
          value={flow.name}
          placeholder="Untitled flow"
          onChange={(event) => onNameChange(event.target.value)}
          aria-label="Flow name"
        />
        <select
          id="flow-editor-run-tab"
          className={cn(inputClassName, "min-w-0 w-auto flex-1 basis-40")}
          value={runTab}
          onChange={(event) => onRunTabChange(event.target.value === "own" ? "own" : Number(event.target.value))}
          aria-label="Where to run"
        >
          <option id="flow-editor-run-tab-own" value="own">
            {flow.background ? "In a hidden tab" : "In a new tab"}
          </option>
          {tabs.map((tab) => (
            <option id={`flow-editor-run-tab-${tab.id}`} key={tab.id} value={tab.id}>
              In {tab.title || tab.url}
            </option>
          ))}
        </select>
        {activeRun ? (
          <Button id="flow-editor-stop" size="sm" variant="destructive" onClick={onStop} title="Stop the run">
            <Square id="flow-editor-stop-icon" className="size-3" aria-hidden />
            Stop
          </Button>
        ) : (
          <Button id="flow-editor-run" size="sm" variant="primary" onClick={onRun} title="Run (Ctrl+Enter)">
            <Play id="flow-editor-run-icon" className="size-3.5" aria-hidden />
            Run
          </Button>
        )}
        <Button id="flow-editor-save" size="sm" disabled={!dirty} onClick={onSave} title="Save (Ctrl+S)">
          <Save id="flow-editor-save-icon" className="size-3.5" aria-hidden />
          Save
        </Button>
        <Button id="flow-editor-record" size="sm" variant="ghost" disabled={Boolean(activeRun)} onClick={onRecordMore} title="Record more steps at the end of this flow">
          <Circle id="flow-editor-record-icon" className="size-3 fill-red-500 text-red-500" aria-hidden />
          Record more
        </Button>
        <IconButton id="flow-editor-pause" label={flow.paused ? "Resume triggers" : "Pause triggers"} active={flow.paused} onClick={onTogglePause}>
          {flow.paused ? <Play id="flow-editor-pause-icon" aria-hidden /> : <Pause id="flow-editor-pause-icon" aria-hidden />}
        </IconButton>
        <IconButton
          id="flow-editor-more"
          label="More flow actions"
          aria-haspopup="menu"
          aria-expanded={menu !== null}
          onClick={(event) => {
            const rect = event.currentTarget.getBoundingClientRect();
            setMenu({ x: rect.right - 200, y: rect.bottom + 4 });
          }}
        >
          <MoreHorizontal id="flow-editor-more-icon" aria-hidden />
        </IconButton>
      </div>
      <div id="flow-editor-status-row" className="flex min-h-5 items-center gap-3 text-xs">
        <span id="flow-editor-last" className={cn("min-w-0 flex-1 truncate", status.tone)}>
          {status.text || (flow.id ? "Never run" : "Not saved yet")}
          {dirty && flow.id ? <span className="text-zinc-500"> · Unsaved changes</span> : null}
        </span>
        {issueCount ? (
          <button id="flow-editor-issues" type="button" className="inline-flex items-center gap-1 rounded px-1 text-amber-300 hover:bg-zinc-900" onClick={onShowIssues}>
            <AlertTriangle id="flow-editor-issues-icon" className="size-3.5" aria-hidden />
            {issueCount} {issueCount === 1 ? "problem" : "problems"}
          </button>
        ) : null}
      </div>
      {menu ? (
        <ContextMenu id="flow-editor-menu" label="Flow actions" x={menu.x} y={menu.y} onClose={() => setMenu(null)}>
          <ContextMenuItem id="flow-editor-menu-duplicate" icon={<Copy />} label="Duplicate" disabled={!flow.id} onClick={() => act("duplicate")} />
          <ContextMenuItem id="flow-editor-menu-export" icon={<Download />} label="Export…" disabled={!flow.id} onClick={() => act("export")} />
          <ContextMenuItem id="flow-editor-menu-playwright" icon={<Code2 />} label="Playwright test" onClick={() => act("playwright")} />
          <ContextMenuSeparator id="flow-editor-menu-separator" />
          <ContextMenuItem id="flow-editor-menu-delete" icon={<Trash2 />} label={flow.id ? "Delete…" : "Discard"} danger onClick={() => act("delete")} />
        </ContextMenu>
      ) : null}
    </header>
  );
}
