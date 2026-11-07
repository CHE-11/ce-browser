import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { Bot, Eye, Hand, History, Play, Plus, Settings, Square, Trash2, X } from "lucide-react";
import type { AgentRun, TabInfo } from "@type/browser";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { IconButton } from "@/components/IconButton";
import { activateTab, createTab, navigate, resizeAgentPanel, setAgentPanelWidth, toggleAgentPanel } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { isActiveRun, useAgentRun } from "@/lib/use-agent-run";
import { useAppearanceState, useSettings, useTabs } from "@/lib/use-browser";
import { AgentApproval } from "./AgentApproval";
import { AgentComposer } from "./AgentComposer";
import { AgentError } from "./AgentError";
import { AgentStepList } from "./AgentStepList";

const STATUS_TEXT: Record<AgentRun["status"], string> = {
  running: "Working",
  waiting: "Waiting for you",
  paused: "Paused while you take over",
  done: "Done",
  failed: "Failed",
  stopped: "Stopped",
};

const STATUS_TONE: Record<AgentRun["status"], string> = {
  running: "bg-sky-500",
  waiting: "bg-amber-400",
  paused: "bg-violet-400",
  done: "bg-emerald-500",
  failed: "bg-red-500",
  stopped: "bg-zinc-500",
};

const MIN_PANEL_WIDTH = 280;
const MAX_PANEL_WIDTH = 720;
const DEFAULT_PANEL_WIDTH = 380;
const AI_SETTINGS_URL = "ce://settings?focus=settings-ai";

/** Opens the AI/Agent settings section, reusing a settings tab in this window. */
function openAiSettings(tabs: TabInfo[]) {
  const open = tabs.find((tab) => tab.url.startsWith("ce://settings"));
  if (!open) {
    void createTab(AI_SETTINGS_URL);
    return;
  }
  void activateTab(open.id).then(() => navigate(open.id, AI_SETTINGS_URL));
}

function clampPanelWidth(width: number) {
  return Math.min(MAX_PANEL_WIDTH, Math.max(MIN_PANEL_WIDTH, Math.round(width)));
}

/** The docked column beside the pages where agent runs start, report each step, and ask for approval. */
export function AgentPanelPage() {
  useAppearanceState();
  const settings = useSettings();
  const tabs = useTabs();
  const agent = useAgentRun();
  const { run, runs, activeTab } = agent;
  const active = isActiveRun(run);
  const width = settings?.agentPanelWidth ?? DEFAULT_PANEL_WIDTH;
  const resizing = useRef(false);
  const [resizingPanel, setResizingPanel] = useState(false);

  useEffect(() => {
    document.title = "Agent";
  }, []);

  const stopRef = useRef(agent.stop);
  stopRef.current = agent.stop;
  const escapeRun = active ? run?.id : undefined;
  useEffect(() => {
    if (escapeRun === undefined) {
      return;
    }
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key !== "Escape" || event.ctrlKey || event.altKey || event.metaKey) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      if (!event.repeat) {
        stopRef.current(escapeRun);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [escapeRun]);

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
      id="agent-panel"
      className={cn(
        "relative flex h-screen flex-col border-l border-zinc-800 bg-zinc-950 text-zinc-100",
        resizingPanel && "cursor-col-resize select-none",
      )}
      onContextMenu={(event) => event.preventDefault()}
    >
      <div
        id="agent-panel-resize"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize the agent panel"
        aria-valuemin={MIN_PANEL_WIDTH}
        aria-valuemax={MAX_PANEL_WIDTH}
        aria-valuenow={width}
        tabIndex={0}
        title="Drag to resize. Left and right arrows change the width."
        className={cn(
          "absolute inset-y-0 left-0 z-20 w-1.5 cursor-col-resize touch-none hover:bg-sky-500/40 focus-visible:bg-sky-500/40 focus-visible:outline-none",
          resizingPanel && "bg-sky-500/50",
        )}
        onPointerDown={onResizePointerDown}
        onKeyDown={onResizeKeyDown}
      />
      <header id="agent-panel-header" className="flex h-[var(--toolbar-height)] shrink-0 items-center gap-2 border-b border-zinc-800 pl-3 pr-1.5">
        <Bot id="agent-panel-header-icon" className="size-4 text-sky-400" aria-hidden />
        <h1 id="agent-panel-title" className="text-sm font-semibold">
          Agent
        </h1>
        <IconButton
          id="agent-panel-settings"
          label="AI/Agent settings"
          className="size-7"
          onClick={() => openAiSettings(tabs.windowTabs)}
        >
          <Settings id="agent-panel-settings-icon" aria-hidden />
        </IconButton>
        <div id="agent-panel-header-spacer" className="min-w-0 flex-1" />
        <IconButton id="agent-panel-new" label="New task" className="size-7" onClick={agent.newTask}>
          <Plus id="agent-panel-new-icon" aria-hidden />
        </IconButton>
        <IconButton id="agent-panel-close" label="Close the agent panel" className="size-7" onClick={() => void toggleAgentPanel(false)}>
          <X id="agent-panel-close-icon" aria-hidden />
        </IconButton>
      </header>

      {runs.length > 1 ? (
        <div id="agent-panel-history" className="flex shrink-0 items-center gap-2 border-b border-zinc-800 px-3 py-1.5">
          <History id="agent-panel-history-icon" className="size-3.5 text-zinc-500" aria-hidden />
          <select
            id="agent-panel-history-select"
            className="h-7 min-w-0 flex-1 rounded border border-zinc-800 bg-zinc-900 px-1 text-xs text-zinc-200"
            value={run?.id ?? ""}
            onChange={(event) => agent.select(Number(event.target.value))}
          >
            {run === null ? (
              <option id="agent-panel-history-new" value="">
                New task
              </option>
            ) : null}
            {runs.map((item) => (
              <option id={`agent-panel-history-${item.id}`} key={item.id} value={item.id}>
                {`${item.source === "mcp" ? "MCP · " : ""}${item.goal.slice(0, 60)} · ${STATUS_TEXT[item.status]}`}
              </option>
            ))}
          </select>
          <IconButton id="agent-panel-history-clear" label="Clear finished runs" className="size-7" onClick={agent.clear}>
            <Trash2 id="agent-panel-history-clear-icon" aria-hidden />
          </IconButton>
        </div>
      ) : null}

      <main id="agent-panel-body" className="min-h-0 flex-1 overflow-y-auto px-3 py-3">
        {run ? (
          <div id="agent-panel-run" className="flex flex-col gap-3">
            <section id="agent-panel-goal" className="rounded-lg bg-zinc-900 p-3 ring-1 ring-zinc-800">
              <div id="agent-panel-goal-status" className="mb-1 flex items-center gap-2 text-[11px] text-zinc-400">
                <span id="agent-panel-goal-dot" className={cn("size-2 rounded-full", STATUS_TONE[run.status], active && run.status === "running" && "animate-pulse")} />
                {STATUS_TEXT[run.status]}
                <span id="agent-panel-goal-count" className="ml-auto">
                  {run.source === "mcp" ? `${run.actions} actions` : `${run.actions} / ${run.maxSteps} actions`}
                </span>
              </div>
              <p id="agent-panel-goal-text" className="whitespace-pre-wrap text-sm">
                {run.goal}
              </p>
              <Button id="agent-panel-goal-tab" size="xs" variant="link" className="mt-1" onClick={() => agent.showTab(run.tabId)}>
                <Eye id="agent-panel-goal-tab-icon" className="size-3" aria-hidden />
                Show its tab
              </Button>
            </section>
            <AgentStepList id="agent-panel-steps" steps={run.steps} />
            {run.result && !active ? (
              run.status === "failed" ? (
                <AgentError id="agent-panel-result" text={run.result} />
              ) : (
                <section
                  id="agent-panel-result"
                  className={cn(
                    "whitespace-pre-wrap break-words rounded-lg p-3 text-sm ring-1",
                    run.status === "done" ? "bg-emerald-950/40 ring-emerald-800/60" : "bg-zinc-900 ring-zinc-800",
                  )}
                >
                  {run.result}
                </section>
              )
            ) : null}
          </div>
        ) : (
          <EmptyState
            id="agent-panel-empty"
            icon={Bot}
            title="Give the agent a task"
            hint={`It works in the selected tab${activeTab ? ` (${activeTab.title || activeTab.url})` : ""}, using real clicks and typing. Purchases, sends, and deletes wait for your approval.`}
          />
        )}
      </main>

      <footer id="agent-panel-footer" className="flex shrink-0 flex-col gap-2 border-t border-zinc-800 p-3">
        {agent.error ? <AgentError id="agent-panel-error" text={agent.error} alert /> : null}
        {run?.approval ? (
          <AgentApproval
            approval={run.approval}
            onAnswer={(allow) => agent.answer(run.id, run.approval!.id, allow)}
            onReply={(text) => agent.reply(run.id, text)}
          />
        ) : null}
        {run && active ? (
          <div id="agent-panel-controls" className="flex gap-2">
            {run.status === "paused" ? (
              <Button id="agent-panel-resume" size="sm" variant="primary" className="flex-1" onClick={() => agent.resume(run.id)}>
                <Play id="agent-panel-resume-icon" className="size-3.5" aria-hidden />
                Resume
              </Button>
            ) : run.source === "agent" ? (
              <Button id="agent-panel-take-over" size="sm" variant="outline" className="flex-1" onClick={() => agent.takeOver(run.id)}>
                <Hand id="agent-panel-take-over-icon" className="size-3.5" aria-hidden />
                Take over
              </Button>
            ) : null}
            <Button
              id="agent-panel-stop"
              size="sm"
              variant="destructiveOutline"
              className="flex-1"
              aria-keyshortcuts="Escape"
              title="Stop (Esc)"
              onClick={() => agent.stop(run.id)}
            >
              <Square id="agent-panel-stop-icon" className="size-3.5" aria-hidden />
              Stop
              <kbd id="agent-panel-stop-key" className="rounded border border-red-800 bg-red-950/70 px-1 font-sans text-[10px] leading-4 text-red-200">
                Esc
              </kbd>
            </Button>
          </div>
        ) : null}
        {run && active && run.source === "agent" ? (
          <AgentComposer mode="reply" onSubmit={(text) => agent.reply(run.id, text)} />
        ) : !run || !active ? (
          <AgentComposer
            mode="start"
            disabled={!activeTab}
            suggestions={settings?.agentSuggestions ?? ["Summarize this page and list the key facts"]}
            onSubmit={agent.start}
          />
        ) : null}
      </footer>
    </div>
  );
}
