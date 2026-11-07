import { useState, type DragEvent } from "react";
import { AlertTriangle, ArrowDown, ArrowUp, ChevronDown, ChevronRight, GripVertical, ImageIcon, LoaderCircle, MoreHorizontal } from "lucide-react";
import type { FlowStep } from "@type/browser";
import { IconButton } from "@/components/IconButton";
import { Toggle } from "@/components/Toggle";
import { cn } from "@/lib/cn";
import { inputClassName } from "@features/Agent/agent-styles";
import { STEP_KINDS, stepKindLabel, stepSummary, statusTone } from "./flow-model";
import { StepFields } from "./StepFields";
import { StepMenu, type StepAction } from "./StepMenu";

/** How a step did in the run being shown. */
export type StepRunState = {
  status: string;
  detail: string;
  /** Run id and step index of a failure screenshot. */
  shot: { runId: number; index: number } | null;
};

type StepCardProps = {
  id: string;
  step: FlowStep;
  index: number;
  count: number;
  /** Tab to pick elements and lists in. */
  tabId: number | null;
  collapsed: boolean;
  /** The run of this flow that is going now is on this step. */
  current: boolean;
  runState: StepRunState | null;
  /** A run of this flow is going. */
  busy: boolean;
  issues: string[];
  highlighted: boolean;
  dragging: boolean;
  onToggleCollapsed: () => void;
  onChange: (step: FlowStep) => void;
  onMove: (delta: number) => void;
  onAction: (action: StepAction) => void;
  onShowShot: (shot: { runId: number; index: number }) => void;
  onDragStart: (event: DragEvent<HTMLLIElement>) => void;
  onDragEnd: () => void;
  onDragOver: (event: DragEvent<HTMLLIElement>) => void;
  onDrop: (event: DragEvent<HTMLLIElement>) => void;
};

function StatusPill({ id, current, state }: { id: string; current: boolean; state: StepRunState | null }) {
  if (current) {
    return (
      <span id={id} className="inline-flex shrink-0 items-center gap-1 rounded-full bg-sky-950 px-2 py-0.5 text-[11px] text-sky-300">
        <LoaderCircle id={`${id}-icon`} className="size-3 animate-spin" aria-hidden />
        Running
      </span>
    );
  }
  if (!state) {
    return null;
  }
  return (
    <span id={id} title={state.detail || undefined} className={cn("shrink-0 rounded-full bg-zinc-800 px-2 py-0.5 text-[11px]", statusTone(state.status))}>
      {state.status}
    </span>
  );
}

export function StepCard({
  id,
  step,
  index,
  count,
  tabId,
  collapsed,
  current,
  runState,
  busy,
  issues,
  highlighted,
  dragging,
  onToggleCollapsed,
  onChange,
  onMove,
  onAction,
  onShowShot,
  onDragStart,
  onDragEnd,
  onDragOver,
  onDrop,
}: StepCardProps) {
  const [armed, setArmed] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const set = (change: Partial<FlowStep>) => onChange({ ...step, ...change });
  const known = STEP_KINDS.some((item) => item.kind === step.kind);
  const hint = STEP_KINDS.find((item) => item.kind === step.kind)?.hint ?? "This step was made by a newer version of ce-browser and is skipped.";
  const failed = runState?.status === "failed";

  return (
    <li
      id={id}
      draggable={armed}
      onDragStart={onDragStart}
      onDragEnd={() => {
        setArmed(false);
        onDragEnd();
      }}
      onDragOver={onDragOver}
      onDrop={onDrop}
      onContextMenu={(event) => {
        if (event.target instanceof HTMLElement && event.target.closest("input, textarea, select")) {
          return;
        }
        event.preventDefault();
        setMenu({ x: event.clientX, y: event.clientY });
      }}
      className={cn(
        "flex scroll-mt-28 flex-col gap-3 rounded-lg border bg-zinc-900/60 p-3 transition-[box-shadow,opacity,border-color]",
        current ? "border-sky-700 shadow-[0_0_0_1px_rgba(14,165,233,0.4)]" : failed ? "border-red-900/80" : "border-zinc-800",
        highlighted && "ring-2 ring-amber-400/70",
        step.disabled && "opacity-55",
        dragging && "opacity-40",
      )}
    >
      <div id={`${id}-header`} className="flex items-center gap-2">
        <span
          id={`${id}-grip`}
          title="Drag to reorder"
          className="-ml-1 inline-flex h-6 w-4 shrink-0 cursor-grab items-center justify-center text-zinc-600 hover:text-zinc-300 active:cursor-grabbing"
          onPointerDown={() => setArmed(true)}
          onPointerUp={() => setArmed(false)}
        >
          <GripVertical id={`${id}-grip-icon`} className="size-4" aria-hidden />
        </span>
        <button
          id={`${id}-collapse`}
          type="button"
          aria-expanded={!collapsed}
          aria-label={collapsed ? "Show this step's settings" : "Hide this step's settings"}
          className="inline-flex size-6 shrink-0 items-center justify-center rounded-full bg-zinc-800 text-xs text-zinc-300 hover:bg-zinc-700"
          onClick={onToggleCollapsed}
        >
          {index + 1}
        </button>
        {collapsed ? (
          <button id={`${id}-summary`} type="button" className="flex min-w-0 flex-1 items-center gap-2 text-left" onClick={onToggleCollapsed}>
            <ChevronRight id={`${id}-summary-icon`} className="size-3.5 shrink-0 text-zinc-500" aria-hidden />
            <span id={`${id}-summary-text`} className={cn("truncate text-sm text-zinc-200", step.disabled && "line-through")}>
              {stepSummary(step)}
            </span>
            {issues.length ? <AlertTriangle id={`${id}-summary-issue`} className="size-3.5 shrink-0 text-amber-400" aria-label={issues.join(" ")} /> : null}
          </button>
        ) : (
          <>
            <ChevronDown id={`${id}-open-icon`} className="size-3.5 shrink-0 text-zinc-500" aria-hidden />
            {known ? (
              <select id={`${id}-kind`} className={cn(inputClassName, "w-44")} value={step.kind} onChange={(event) => set({ kind: event.target.value })} aria-label="Step kind">
                {STEP_KINDS.map((item) => (
                  <option id={`${id}-kind-${item.kind}`} key={item.kind} value={item.kind}>
                    {item.label}
                  </option>
                ))}
              </select>
            ) : (
              <span id={`${id}-kind-unknown`} className="text-sm text-amber-300">
                {stepKindLabel(step.kind)}
              </span>
            )}
            <span id={`${id}-hint`} className="min-w-0 flex-1 truncate text-xs text-zinc-500">
              {step.disabled ? "Turned off. Runs skip it." : hint}
            </span>
          </>
        )}
        <StatusPill id={`${id}-status`} current={current} state={runState} />
        {runState?.shot ? (
          <IconButton id={`${id}-shot`} label="See the page when it failed" className="size-7 text-red-300" onClick={() => runState.shot && onShowShot(runState.shot)}>
            <ImageIcon id={`${id}-shot-icon`} aria-hidden />
          </IconButton>
        ) : null}
        {!collapsed ? (
          <label id={`${id}-optional`} className="flex items-center gap-1.5 text-xs text-zinc-400">
            <Toggle id={`${id}-optional-toggle`} size="sm" checked={step.optional} onChange={(optional) => set({ optional })} aria-label="Keep going if this fails" />
            Optional
          </label>
        ) : null}
        <IconButton id={`${id}-up`} label="Move up" className="size-7" disabled={index === 0} onClick={() => onMove(-1)}>
          <ArrowUp id={`${id}-up-icon`} aria-hidden />
        </IconButton>
        <IconButton id={`${id}-down`} label="Move down" className="size-7" disabled={index === count - 1} onClick={() => onMove(1)}>
          <ArrowDown id={`${id}-down-icon`} aria-hidden />
        </IconButton>
        <IconButton
          id={`${id}-more`}
          label="More step actions"
          className="size-7"
          aria-haspopup="menu"
          aria-expanded={menu !== null}
          onClick={(event) => {
            const rect = event.currentTarget.getBoundingClientRect();
            setMenu({ x: rect.right - 192, y: rect.bottom + 4 });
          }}
        >
          <MoreHorizontal id={`${id}-more-icon`} aria-hidden />
        </IconButton>
      </div>

      {!collapsed ? <StepFields id={id} step={step} tabId={tabId} set={set} /> : null}

      {!collapsed && runState && runState.detail && runState.status !== "ok" ? (
        <p id={`${id}-run-detail`} className={cn("rounded bg-zinc-950/70 px-2 py-1 text-xs", statusTone(runState.status))}>
          Last run: {runState.detail}
        </p>
      ) : null}

      {!collapsed && issues.length ? (
        <ul id={`${id}-issues`} className="flex flex-col gap-0.5">
          {issues.map((issue, at) => (
            <li id={`${id}-issue-${at}`} key={issue} className="flex items-start gap-1.5 text-xs text-amber-300">
              <AlertTriangle id={`${id}-issue-${at}-icon`} className="mt-0.5 size-3 shrink-0" aria-hidden />
              {issue}
            </li>
          ))}
        </ul>
      ) : null}

      {!collapsed && step.note ? (
        <p id={`${id}-note`} className="text-xs text-amber-300/90">
          {step.note}
        </p>
      ) : null}

      {menu ? <StepMenu id={`${id}-menu`} disabled={step.disabled} busy={busy} x={menu.x} y={menu.y} onAction={onAction} onClose={() => setMenu(null)} /> : null}
    </li>
  );
}
