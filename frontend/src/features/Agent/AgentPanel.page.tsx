import { useEffect } from "react";
import { Bot, Eye, Hand, History, Play, Plus, Square, Trash2, X } from "lucide-react";
import type { AgentRun } from "@type/browser";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { IconButton } from "@/components/IconButton";
import { toggleAgentPanel } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { isActiveRun, useAgentRun } from "@/lib/use-agent-run";
import { useAppearanceState } from "@/lib/use-browser";
import { AgentApproval } from "./AgentApproval";
import { AgentComposer } from "./AgentComposer";
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

/** The docked column beside the pages where agent runs start, report each step, and ask for approval. */
export function AgentPanelPage() {
  useAppearanceState();
  const agent = useAgentRun();
  const { run, runs, activeTab } = agent;
  const active = isActiveRun(run);

  useEffect(() => {
    document.title = "Agent";
  }, []);

  return (
    <div id="agent-panel" className="flex h-screen flex-col border-l border-zinc-800 bg-zinc-950 text-zinc-100" onContextMenu={(event) => event.preventDefault()}>
      <header id="agent-panel-header" className="flex h-10 shrink-0 items-center gap-2 border-b border-zinc-800 pl-3 pr-1.5">
        <Bot id="agent-panel-header-icon" className="size-4 text-sky-400" aria-hidden />
        <h1 id="agent-panel-title" className="flex-1 text-sm font-semibold">
          Agent
        </h1>
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
              <section
                id="agent-panel-result"
                className={cn(
                  "whitespace-pre-wrap rounded-lg p-3 text-sm ring-1",
                  run.status === "done" ? "bg-emerald-950/40 ring-emerald-800/60" : "bg-zinc-900 ring-zinc-800",
                )}
              >
                {run.result}
              </section>
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
        {agent.error ? (
          <p id="agent-panel-error" role="alert" className="text-xs text-red-400">
            {agent.error}
          </p>
        ) : null}
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
            <Button id="agent-panel-stop" size="sm" variant="destructiveOutline" className="flex-1" onClick={() => agent.stop(run.id)}>
              <Square id="agent-panel-stop-icon" className="size-3.5" aria-hidden />
              Stop
            </Button>
          </div>
        ) : null}
        {run && active && run.source === "agent" ? (
          <AgentComposer mode="reply" onSubmit={(text) => agent.reply(run.id, text)} />
        ) : !run || !active ? (
          <AgentComposer mode="start" disabled={!activeTab} onSubmit={agent.start} />
        ) : null}
      </footer>
    </div>
  );
}
