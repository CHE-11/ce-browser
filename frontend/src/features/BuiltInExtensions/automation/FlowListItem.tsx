import type { MouseEvent } from "react";
import { Pause } from "lucide-react";
import type { Flow, FlowRun } from "@type/browser";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import { statusDot, untilText } from "./flow-model";

type FlowListItemProps = {
  flow: Flow;
  current: boolean;
  /** The flow's run in progress, if any. */
  running: FlowRun | undefined;
  nextRun: number | undefined;
  now: number;
  onOpen: () => void;
  onContextMenu: (event: MouseEvent<HTMLButtonElement>) => void;
};

function metaText(flow: Flow, running: FlowRun | undefined, nextRun: number | undefined, now: number) {
  if (running) {
    return running.current !== null ? `Running step ${running.current + 1} of ${flow.steps.length}` : "Starting…";
  }
  if (flow.paused) {
    return "Paused";
  }
  if (nextRun) {
    return `Next ${untilText(nextRun, now)}`;
  }
  if (flow.lastStatus) {
    return `Last run ${flow.lastStatus}`;
  }
  return "Never run";
}

export function FlowListItem({ flow, current, running, nextRun, now, onOpen, onContextMenu }: FlowListItemProps) {
  const id = `automations-flow-${flow.id}`;
  const status = running ? "running" : flow.lastStatus;
  return (
    <button
      id={id}
      type="button"
      aria-current={current ? "true" : undefined}
      className={cn("group flex items-start gap-2 rounded-md px-2 py-1.5 text-left hover:bg-zinc-800", focusRow, current && "bg-zinc-800")}
      onClick={onOpen}
      onContextMenu={onContextMenu}
    >
      <span
        id={`${id}-dot`}
        aria-hidden
        className={cn("mt-1.5 size-2 shrink-0 rounded-full", statusDot(status), running && "animate-pulse", flow.paused && !running && "bg-zinc-700")}
      />
      <span id={`${id}-text`} className="flex min-w-0 flex-1 flex-col">
        <span id={`${id}-name`} className={cn("truncate text-sm text-zinc-200", flow.paused && "text-zinc-400")}>
          {flow.name || "Untitled flow"}
        </span>
        <span id={`${id}-meta`} className="flex items-center gap-1.5 truncate text-xs text-zinc-500">
          {flow.paused && !running ? <Pause id={`${id}-paused-icon`} className="size-3 shrink-0" aria-hidden /> : null}
          <span id={`${id}-status`} className={cn("truncate", running && "text-sky-400", !running && flow.lastStatus === "failed" && !flow.paused && !nextRun && "text-red-400")}>
            {metaText(flow, running, nextRun, now)}
          </span>
          <span id={`${id}-steps`} className="ml-auto shrink-0 text-zinc-600">
            {flow.steps.length}
          </span>
        </span>
      </span>
    </button>
  );
}
