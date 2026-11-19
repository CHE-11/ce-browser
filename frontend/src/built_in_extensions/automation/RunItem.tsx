import { useState } from "react";
import { ChevronDown, ChevronRight, ImageIcon, LoaderCircle, RotateCcw, Square } from "lucide-react";
import type { FlowRun } from "@type/browser";
import { Button } from "@/components/Button";
import { stopFlowRun } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import { formatTime, runDuration, runRange, statusTone } from "./flow-model";
import type { RunShot } from "./RunShotDialog";

type RunItemProps = {
  run: FlowRun;
  /** Hide the flow's name, inside that flow's own history. */
  showName: boolean;
  /** The flow still exists, so it can be opened or run again. */
  flowExists: boolean;
  onOpenFlow: (id: string, step?: number) => void;
  onOpenDataset: (id: string) => void;
  onRerun: (run: FlowRun) => void;
  onShowShot: (shot: RunShot) => void;
};

export function RunItem({ run, showName, flowExists, onOpenFlow, onOpenDataset, onRerun, onShowShot }: RunItemProps) {
  const [open, setOpen] = useState(run.status === "running" || run.status === "failed");
  const id = `run-${run.id}`;
  const failed = run.steps.filter((step) => step.status === "failed").length;
  const healed = run.steps.filter((step) => step.status === "healed").length;
  const range = runRange(run);
  const running = run.status === "running";

  return (
    <li id={id} className={cn("rounded-md border bg-zinc-900/60", running ? "border-sky-900" : "border-zinc-800")}>
      <button
        id={`${id}-toggle`}
        type="button"
        aria-expanded={open}
        className={cn("flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm hover:bg-zinc-800/60", focusRow)}
        onClick={() => setOpen(!open)}
      >
        {open ? <ChevronDown id={`${id}-open-icon`} className="size-4 shrink-0 text-zinc-500" aria-hidden /> : <ChevronRight id={`${id}-closed-icon`} className="size-4 shrink-0 text-zinc-500" aria-hidden />}
        <span id={`${id}-name`} className="min-w-0 flex-1 truncate font-medium text-zinc-200">
          {showName ? run.flowName || "Flow" : running && run.current !== null ? `Step ${run.current + 1}: ${run.currentLabel}` : formatTime(run.started)}
        </span>
        {range ? (
          <span id={`${id}-range`} className="rounded bg-zinc-800 px-1.5 py-0.5 text-[11px] text-zinc-400">
            {range}
          </span>
        ) : null}
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
        <span id={`${id}-status`} className={cn("inline-flex w-20 items-center justify-end gap-1 text-xs", statusTone(run.status))}>
          {running ? <LoaderCircle id={`${id}-status-icon`} className="size-3 animate-spin" aria-hidden /> : null}
          {run.status}
        </span>
        <span id={`${id}-when`} className="w-28 text-right text-xs text-zinc-500">
          {formatTime(run.started)} · {runDuration(run)}
        </span>
      </button>
      {open ? (
        <div id={`${id}-details`} className="flex flex-col gap-2 border-t border-zinc-800 px-3 py-2">
          {running && run.current !== null && showName ? (
            <p id={`${id}-current`} className="flex items-center gap-2 text-xs text-sky-300">
              <LoaderCircle id={`${id}-current-icon`} className="size-3 animate-spin" aria-hidden />
              Step {run.current + 1}: {run.currentLabel}
            </p>
          ) : null}
          {run.steps.length ? (
            <ol id={`${id}-steps`} className="flex flex-col gap-1">
              {run.steps.map((step, index) => (
                <li id={`${id}-step-${index}`} key={index} className="flex items-start gap-2 text-xs">
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
                  {step.shot ? (
                    <Button
                      id={`${id}-step-${index}-shot`}
                      size="xs"
                      variant="ghost"
                      className="text-red-300"
                      onClick={() => onShowShot({ runId: run.id, index: step.index, label: step.label })}
                    >
                      <ImageIcon id={`${id}-step-${index}-shot-icon`} className="size-3" aria-hidden />
                      Screenshot
                    </Button>
                  ) : null}
                  {flowExists && (step.status === "failed" || step.status === "healed") ? (
                    <Button id={`${id}-step-${index}-open`} size="xs" variant="ghost" onClick={() => onOpenFlow(run.flowId, step.index)}>
                      Edit step
                    </Button>
                  ) : null}
                </li>
              ))}
            </ol>
          ) : (
            <p id={`${id}-no-steps`} className="text-xs text-zinc-500">
              {running ? "Starting…" : "No steps ran."}
            </p>
          )}
          {run.output ? (
            <pre id={`${id}-output`} className="max-h-48 overflow-auto whitespace-pre-wrap rounded bg-zinc-950 p-2 text-xs text-zinc-300">
              {run.output}
            </pre>
          ) : null}
          <div id={`${id}-actions`} className="flex gap-2">
            {running ? (
              <Button id={`${id}-stop`} size="xs" variant="destructiveOutline" onClick={() => void stopFlowRun(run.id)}>
                <Square id={`${id}-stop-icon`} className="size-3" aria-hidden />
                Stop
              </Button>
            ) : flowExists ? (
              <Button id={`${id}-rerun`} size="xs" variant="outline" onClick={() => onRerun(run)}>
                <RotateCcw id={`${id}-rerun-icon`} className="size-3" aria-hidden />
                Run again
              </Button>
            ) : null}
            {flowExists && showName ? (
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
