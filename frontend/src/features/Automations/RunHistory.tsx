import { useState } from "react";
import { ChevronDown, ChevronRight, History, Square, Trash2 } from "lucide-react";
import type { FlowRun } from "@type/browser";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { clearFlowRuns, stopFlowRun } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import { formatWhen, runDuration, statusTone } from "./flow-model";

type RunHistoryProps = {
  runs: FlowRun[];
  flowId?: string;
  onOpenFlow: (id: string) => void;
  onOpenDataset: (id: string) => void;
};

function RunItem({ run, onOpenFlow, onOpenDataset }: { run: FlowRun; onOpenFlow: (id: string) => void; onOpenDataset: (id: string) => void }) {
  const [open, setOpen] = useState(run.status === "running" || run.status === "failed");
  const id = `run-${run.id}`;
  const failed = run.steps.filter((step) => step.status === "failed").length;
  const healed = run.steps.filter((step) => step.status === "healed").length;
  return (
    <li id={id} className="rounded-md border border-zinc-800 bg-zinc-900/60">
      <button
        id={`${id}-toggle`}
        type="button"
        aria-expanded={open}
        className={cn("flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm hover:bg-zinc-800/60", focusRow)}
        onClick={() => setOpen(!open)}
      >
        {open ? <ChevronDown id={`${id}-open-icon`} className="size-4 text-zinc-500" aria-hidden /> : <ChevronRight id={`${id}-closed-icon`} className="size-4 text-zinc-500" aria-hidden />}
        <span id={`${id}-name`} className="min-w-0 flex-1 truncate font-medium text-zinc-200">
          {run.flowName || "Flow"}
        </span>
        <span id={`${id}-trigger`} className="rounded bg-zinc-800 px-1.5 py-0.5 text-[11px] text-zinc-400">
          {run.trigger}
        </span>
        {healed ? (
          <span id={`${id}-healed`} className="text-xs text-amber-300">
            {healed} healed
          </span>
        ) : null}
        {failed ? (
          <span id={`${id}-failed`} className="text-xs text-red-400">
            {failed} failed
          </span>
        ) : null}
        <span id={`${id}-status`} className={cn("w-16 text-right text-xs", statusTone(run.status))}>
          {run.status}
        </span>
        <span id={`${id}-when`} className="w-40 text-right text-xs text-zinc-500">
          {formatWhen(run.started)} · {runDuration(run)}
        </span>
      </button>
      {open ? (
        <div id={`${id}-details`} className="flex flex-col gap-2 border-t border-zinc-800 px-3 py-2">
          {run.steps.length ? (
            <ol id={`${id}-steps`} className="flex flex-col gap-1">
              {run.steps.map((step, index) => (
                <li id={`${id}-step-${index}`} key={index} className="flex gap-2 text-xs">
                  <span id={`${id}-step-${index}-index`} className="w-6 shrink-0 text-right text-zinc-600">
                    {step.index + 1}
                  </span>
                  <span id={`${id}-step-${index}-status`} className={cn("w-14 shrink-0", statusTone(step.status))}>
                    {step.status}
                  </span>
                  <span id={`${id}-step-${index}-label`} className="min-w-0 flex-1 text-zinc-300">
                    {step.label}
                    {step.detail ? (
                      <span id={`${id}-step-${index}-detail`} className="block whitespace-pre-wrap text-zinc-500">
                        {step.detail}
                      </span>
                    ) : null}
                  </span>
                </li>
              ))}
            </ol>
          ) : (
            <p id={`${id}-no-steps`} className="text-xs text-zinc-500">
              No steps ran yet.
            </p>
          )}
          {run.output ? (
            <pre id={`${id}-output`} className="max-h-48 overflow-auto whitespace-pre-wrap rounded bg-zinc-950 p-2 text-xs text-zinc-300">
              {run.output}
            </pre>
          ) : null}
          <div id={`${id}-actions`} className="flex gap-2">
            {run.status === "running" ? (
              <Button id={`${id}-stop`} size="xs" variant="destructiveOutline" onClick={() => void stopFlowRun(run.id)}>
                <Square id={`${id}-stop-icon`} className="size-3" aria-hidden />
                Stop
              </Button>
            ) : null}
            {run.flowId ? (
              <Button id={`${id}-flow`} size="xs" variant="ghost" onClick={() => onOpenFlow(run.flowId)}>
                Open flow
              </Button>
            ) : null}
            {run.datasetId ? (
              <Button id={`${id}-dataset`} size="xs" variant="ghost" onClick={() => onOpenDataset(run.datasetId)}>
                Open dataset
              </Button>
            ) : null}
          </div>
        </div>
      ) : null}
    </li>
  );
}

export function RunHistory({ runs, flowId, onOpenFlow, onOpenDataset }: RunHistoryProps) {
  const shown = (flowId ? runs.filter((run) => run.flowId === flowId) : runs).slice().sort((a, b) => b.started - a.started);
  return (
    <section id="run-history" className="flex min-w-0 flex-1 flex-col overflow-y-auto">
      <header id="run-history-header" className="sticky top-0 z-10 flex items-center gap-2 border-b border-zinc-800 bg-zinc-950/95 px-6 py-3 backdrop-blur">
        <h1 id="run-history-title" className="flex-1 text-lg font-semibold text-zinc-100">
          History
        </h1>
        <Button id="run-history-clear" size="sm" variant="ghost" disabled={!runs.some((run) => run.status !== "running")} onClick={() => void clearFlowRuns()}>
          <Trash2 id="run-history-clear-icon" className="size-3.5" aria-hidden />
          Clear finished
        </Button>
      </header>
      {shown.length === 0 ? (
        <EmptyState id="run-history-empty" icon={History} title="No runs yet" hint="Runs from the editor, schedules, monitors and MCP clients show up here." />
      ) : (
        <ul id="run-history-list" className="flex flex-col gap-2 px-6 py-5">
          {shown.map((run) => (
            <RunItem key={run.id} run={run} onOpenFlow={onOpenFlow} onOpenDataset={onOpenDataset} />
          ))}
        </ul>
      )}
    </section>
  );
}
