import { CornerLeftUp, Rocket, Users } from "lucide-react";
import type { AgentRun } from "@type/browser";
import { Button } from "@/components/Button";
import { cn } from "@/lib/cn";
import { isActiveRun } from "@/lib/use-agent-run";
import { STATUS_TEXT, STATUS_TONE } from "./agent-styles";

type Props = {
  run: AgentRun;
  helpers: AgentRun[];
  onSelect: (id: number) => void;
};

/** Compact run chrome: helpers and parent link. The goal itself is a chat message. */
export function AgentGoalCard({ run, helpers, onSelect }: Props) {
  const active = isActiveRun(run);
  const fullAuto = active && run.autonomy === "full";
  if (!fullAuto && helpers.length === 0 && run.parent === null) {
    return null;
  }

  return (
    <section id="agent-panel-goal" className="flex flex-col gap-2">
      {fullAuto ? (
        <div id="agent-panel-goal-full" className="flex items-center gap-1.5 rounded-lg bg-fuchsia-950/60 px-3 py-1 text-[11px] text-fuchsia-200 ring-1 ring-fuchsia-900/60">
          <Rocket id="agent-panel-goal-full-icon" className="size-3" aria-hidden />
          Full auto. Only payments and uploads ask. Esc stops it.
        </div>
      ) : null}
      {run.parent !== null ? (
        <div id="agent-panel-goal-links" className="flex flex-wrap items-center gap-x-3 gap-y-1">
          <Button id="agent-panel-goal-parent" size="xs" variant="link" onClick={() => onSelect(run.parent!)}>
            <CornerLeftUp id="agent-panel-goal-parent-icon" className="size-3" aria-hidden />
            Back to the main task
          </Button>
        </div>
      ) : null}
      {helpers.length > 0 ? (
        <div id="agent-panel-goal-helpers" className="rounded-lg bg-zinc-900/70 p-2 ring-1 ring-zinc-800">
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
                  className="flex w-full items-start gap-2 rounded px-1.5 py-1 text-left text-xs text-zinc-300 hover:bg-zinc-800"
                  onClick={() => onSelect(helper.id)}
                >
                  <span id={`agent-panel-goal-helper-${helper.id}-dot`} className={cn("mt-1.5 size-1.5 shrink-0 rounded-full", STATUS_TONE[helper.status], isActiveRun(helper) && "animate-pulse")} />
                  <span id={`agent-panel-goal-helper-${helper.id}-body`} className="min-w-0 flex-1">
                    <span id={`agent-panel-goal-helper-${helper.id}-goal`} className="block truncate">
                      {helper.goal}
                    </span>
                    <span id={`agent-panel-goal-helper-${helper.id}-meta`} className="mt-0.5 block text-[10px] text-zinc-500">
                      {[
                        helper.approval ? "Needs you" : STATUS_TEXT[helper.status],
                        `${helper.actions} actions`,
                        helper.model || null,
                        helper.tokensIn + helper.tokensOut > 0 ? `${helper.tokensIn + helper.tokensOut} tok` : null,
                        helper.result ? "Has result" : null,
                      ]
                        .filter(Boolean)
                        .join(" · ")}
                    </span>
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
