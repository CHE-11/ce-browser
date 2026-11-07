import { useState } from "react";
import { CornerLeftUp, Eye, Rocket, Users } from "lucide-react";
import type { AgentRun } from "@type/browser";
import { Button } from "@/components/Button";
import { autonomyLabel } from "@/lib/agent-autonomy";
import { cn } from "@/lib/cn";
import { isActiveRun } from "@/lib/use-agent-run";
import { STATUS_TEXT, STATUS_TONE } from "./agent-styles";
import { AgentUsage } from "./AgentUsage";

type Props = {
  run: AgentRun;
  helpers: AgentRun[];
  onShowTab: () => void;
  onSelect: (id: number) => void;
};

/** The task, its status, what it used, and its helpers. */
export function AgentGoalCard({ run, helpers, onShowTab, onSelect }: Props) {
  const active = isActiveRun(run);
  const [open, setOpen] = useState(false);
  const long = run.goal.length > 220 || run.goal.split("\n").length > 4;

  return (
    <section id="agent-panel-goal" className="rounded-lg bg-zinc-900 p-3 ring-1 ring-zinc-800">
      {active && run.autonomy === "full" ? (
        <div id="agent-panel-goal-full" className="-mx-3 -mt-3 mb-2 flex items-center gap-1.5 rounded-t-lg bg-fuchsia-950/60 px-3 py-1 text-[11px] text-fuchsia-200 ring-1 ring-fuchsia-900/60">
          <Rocket id="agent-panel-goal-full-icon" className="size-3" aria-hidden />
          Full auto. Only payments and uploads ask. Esc stops it.
        </div>
      ) : null}
      <div id="agent-panel-goal-status" className="mb-1 flex items-center gap-2 text-[11px] text-zinc-400">
        <span id="agent-panel-goal-dot" className={cn("size-2 rounded-full", STATUS_TONE[run.status], active && run.status === "running" && "animate-pulse")} />
        {STATUS_TEXT[run.status]}
        <span id="agent-panel-goal-badges" className="ml-auto flex items-center gap-1">
          {run.source === "mcp" ? (
            <span id="agent-panel-goal-mcp" className="rounded bg-zinc-800 px-1 text-[10px] text-zinc-300">
              MCP
            </span>
          ) : (
            <span id="agent-panel-goal-autonomy" className="rounded bg-zinc-800 px-1 text-[10px] text-zinc-300">
              {autonomyLabel(run.autonomy)}
            </span>
          )}
          {run.background ? (
            <span id="agent-panel-goal-background" className="rounded bg-zinc-800 px-1 text-[10px] text-zinc-300">
              Background
            </span>
          ) : null}
        </span>
      </div>
      <p id="agent-panel-goal-text" className={cn("whitespace-pre-wrap break-words text-sm", long && !open && "line-clamp-4")}>
        {run.goal}
      </p>
      {long ? (
        <Button id="agent-panel-goal-more" size="xs" variant="link" onClick={() => setOpen((value) => !value)}>
          {open ? "Show less" : "Show all"}
        </Button>
      ) : null}
      <AgentUsage id="agent-panel-goal-usage" run={run} />
      <div id="agent-panel-goal-links" className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1">
        <Button id="agent-panel-goal-tab" size="xs" variant="link" onClick={onShowTab}>
          <Eye id="agent-panel-goal-tab-icon" className="size-3" aria-hidden />
          Show its tab
        </Button>
        {run.parent !== null ? (
          <Button id="agent-panel-goal-parent" size="xs" variant="link" onClick={() => onSelect(run.parent!)}>
            <CornerLeftUp id="agent-panel-goal-parent-icon" className="size-3" aria-hidden />
            Back to the main task
          </Button>
        ) : null}
      </div>
      {helpers.length > 0 ? (
        <div id="agent-panel-goal-helpers" className="mt-2 border-t border-zinc-800 pt-2">
          <div id="agent-panel-goal-helpers-title" className="mb-1 flex items-center gap-1 text-[11px] text-zinc-500">
            <Users id="agent-panel-goal-helpers-icon" className="size-3" aria-hidden />
            Helpers
          </div>
          <ul id="agent-panel-goal-helpers-list" className="flex flex-col gap-1">
            {helpers.map((helper) => (
              <li id={`agent-panel-goal-helper-${helper.id}`} key={helper.id}>
                <button
                  id={`agent-panel-goal-helper-${helper.id}-open`}
                  type="button"
                  className="flex w-full items-center gap-2 rounded px-1.5 py-1 text-left text-xs text-zinc-300 hover:bg-zinc-800"
                  onClick={() => onSelect(helper.id)}
                >
                  <span id={`agent-panel-goal-helper-${helper.id}-dot`} className={cn("size-1.5 shrink-0 rounded-full", STATUS_TONE[helper.status], isActiveRun(helper) && "animate-pulse")} />
                  <span id={`agent-panel-goal-helper-${helper.id}-goal`} className="min-w-0 flex-1 truncate">
                    {helper.goal}
                  </span>
                  <span id={`agent-panel-goal-helper-${helper.id}-status`} className="shrink-0 text-[10px] text-zinc-500">
                    {helper.approval ? "Needs you" : STATUS_TEXT[helper.status]}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </section>
  );
}
