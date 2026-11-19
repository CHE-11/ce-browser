import { Fragment, useEffect, useState, type DragEvent } from "react";
import { ChevronsDownUp, ChevronsUpDown, Plus } from "lucide-react";
import type { FlowStep, FlowStepKind, TabInfo } from "@type/browser";
import { Button } from "@/components/Button";
import { cn } from "@/lib/cn";
import { AddStepMenu } from "./AddStepMenu";
import { blankStep, copyStep } from "./flow-model";
import { PageTabSelect } from "./PageTabSelect";
import { StepCard, type StepRunState } from "./StepCard";
import type { StepAction } from "./StepMenu";

/** Flows longer than this open with their steps folded. */
const FOLD_OVER = 6;

export type RunStepMode = "only" | "from";

type StepListProps = {
  steps: FlowStep[];
  issues: Record<number, string[]>;
  tabs: TabInfo[];
  tabId: number | null;
  onTabChange: (tabId: number) => void;
  runStates: Record<number, StepRunState>;
  currentIndex: number | null;
  busy: boolean;
  /** Step to scroll to and outline, from the address. */
  focusStep: number | null;
  onChange: (steps: FlowStep[]) => void;
  onRunStep: (index: number, mode: RunStepMode) => void;
  onShowShot: (shot: { runId: number; index: number }) => void;
};

type InsertGapProps = {
  id: string;
  active: boolean;
  onAdd: (x: number, y: number) => void;
  onDragOver: (event: DragEvent<HTMLLIElement>) => void;
  onDrop: (event: DragEvent<HTMLLIElement>) => void;
};

function InsertGap({ id, active, onAdd, onDragOver, onDrop }: InsertGapProps) {
  return (
    <li id={id} className="group relative -my-1 flex h-3 items-center justify-center" onDragOver={onDragOver} onDrop={onDrop}>
      <span id={`${id}-line`} className={cn("absolute inset-x-2 h-0.5 rounded-full transition-colors", active ? "bg-sky-500" : "bg-transparent group-hover:bg-zinc-700")} />
      <button
        id={`${id}-add`}
        type="button"
        title="Add a step here"
        aria-label="Add a step here"
        className="relative z-[1] inline-flex size-5 items-center justify-center rounded-full border border-zinc-700 bg-zinc-900 text-zinc-400 opacity-0 transition-opacity group-hover:opacity-100 hover:text-zinc-100 focus-visible:opacity-100"
        onClick={(event) => {
          const rect = event.currentTarget.getBoundingClientRect();
          onAdd(rect.left, rect.bottom + 4);
        }}
      >
        <Plus id={`${id}-add-icon`} className="size-3" aria-hidden />
      </button>
    </li>
  );
}

export function StepList({ steps, issues, tabs, tabId, onTabChange, runStates, currentIndex, busy, focusStep, onChange, onRunStep, onShowShot }: StepListProps) {
  const [folded, setFolded] = useState<Set<string>>(() => new Set(steps.length > FOLD_OVER ? steps.map((step) => step.id) : []));
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const [dropAt, setDropAt] = useState<number | null>(null);
  const [addAt, setAddAt] = useState<{ index: number; x: number; y: number } | null>(null);
  const [highlighted, setHighlighted] = useState<number | null>(null);

  useEffect(() => {
    if (focusStep === null) {
      return;
    }
    const id = steps[focusStep]?.id;
    if (id) {
      setFolded((current) => {
        const next = new Set(current);
        next.delete(id);
        return next;
      });
    }
    setHighlighted(focusStep);
    const frame = requestAnimationFrame(() => document.getElementById(`flow-step-${focusStep}`)?.scrollIntoView({ block: "center", behavior: "smooth" }));
    const timer = window.setTimeout(() => setHighlighted(null), 2500);
    return () => {
      cancelAnimationFrame(frame);
      window.clearTimeout(timer);
    };
    // Only a new address should scroll, not edits to the steps.
  }, [focusStep]);

  useEffect(() => {
    if (currentIndex !== null) {
      document.getElementById(`flow-step-${currentIndex}`)?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }
  }, [currentIndex]);

  const toggleFold = (id: string) =>
    setFolded((current) => {
      const next = new Set(current);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });

  const allFolded = steps.length > 0 && steps.every((step) => folded.has(step.id));

  const insert = (index: number, kind: FlowStepKind) => {
    const next = [...steps];
    next.splice(index, 0, blankStep(kind));
    onChange(next);
  };

  const move = (from: number, to: number) => {
    const target = to > from ? to - 1 : to;
    if (target === from) {
      return;
    }
    const next = [...steps];
    const [item] = next.splice(from, 1);
    next.splice(target, 0, item);
    onChange(next);
  };

  const act = (index: number, action: StepAction) => {
    const step = steps[index];
    switch (action) {
      case "runOnly":
        onRunStep(index, "only");
        break;
      case "runFrom":
        onRunStep(index, "from");
        break;
      case "duplicate": {
        const next = [...steps];
        next.splice(index + 1, 0, copyStep(step));
        onChange(next);
        break;
      }
      case "toggle":
        onChange(steps.map((item, at) => (at === index ? { ...item, disabled: !item.disabled } : item)));
        break;
      case "remove":
        onChange(steps.filter((_, at) => at !== index));
        break;
    }
  };

  const endDrag = () => {
    setDragFrom(null);
    setDropAt(null);
  };

  const dragOver = (index: number) => (event: DragEvent<HTMLLIElement>) => {
    if (dragFrom === null) {
      return;
    }
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    const rect = event.currentTarget.getBoundingClientRect();
    setDropAt(event.clientY > rect.top + rect.height / 2 ? index + 1 : index);
  };

  const gapOver = (index: number) => (event: DragEvent<HTMLLIElement>) => {
    if (dragFrom === null) {
      return;
    }
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    setDropAt(index);
  };

  const drop = (event: DragEvent<HTMLLIElement>) => {
    event.preventDefault();
    if (dragFrom !== null && dropAt !== null) {
      move(dragFrom, dropAt);
    }
    endDrag();
  };

  const showDrop = (index: number) => dragFrom !== null && dropAt === index && dropAt !== dragFrom && dropAt !== dragFrom + 1;

  return (
    <section id="flow-editor-steps" className="flex flex-col gap-3">
      <div id="flow-editor-steps-toolbar" className="flex flex-wrap items-center gap-2">
        <label id="flow-editor-pick-tab" className="flex min-w-0 flex-1 items-center gap-2 text-xs text-zinc-400">
          <span id="flow-editor-pick-tab-label" className="shrink-0">
            Pick and test on
          </span>
          <PageTabSelect id="flow-editor-pick-tab-input" tabs={tabs} value={tabId} onChange={onTabChange} label="Page to pick and test elements on" className="max-w-80" />
        </label>
        {steps.length > 1 ? (
          <Button
            id="flow-editor-fold-all"
            size="xs"
            variant="ghost"
            onClick={() => setFolded(allFolded ? new Set() : new Set(steps.map((step) => step.id)))}
          >
            {allFolded ? <ChevronsUpDown id="flow-editor-fold-all-icon" className="size-3.5" aria-hidden /> : <ChevronsDownUp id="flow-editor-fold-all-icon" className="size-3.5" aria-hidden />}
            {allFolded ? "Expand all" : "Collapse all"}
          </Button>
        ) : null}
      </div>

      {steps.length === 0 ? (
        <p id="flow-editor-steps-empty" className="rounded-lg border border-dashed border-zinc-800 px-4 py-6 text-center text-xs text-zinc-500">
          No steps yet. Record them with Record more steps above, or add them here.
        </p>
      ) : (
        <ol id="flow-editor-step-list" className="flex flex-col gap-2" onDragLeave={(event) => event.currentTarget === event.target && setDropAt(null)}>
          {steps.map((step, index) => (
            <Fragment key={step.id || index}>
              <InsertGap
                id={`flow-step-gap-${index}`}
                active={showDrop(index)}
                onAdd={(x, y) => setAddAt({ index, x, y })}
                onDragOver={gapOver(index)}
                onDrop={drop}
              />
              <StepCard
                id={`flow-step-${index}`}
                step={step}
                index={index}
                count={steps.length}
                tabId={tabId}
                collapsed={folded.has(step.id)}
                current={currentIndex === index}
                runState={runStates[index] ?? null}
                busy={busy}
                issues={issues[index] ?? []}
                highlighted={highlighted === index}
                dragging={dragFrom === index}
                onToggleCollapsed={() => toggleFold(step.id)}
                onChange={(next) => onChange(steps.map((item, at) => (at === index ? next : item)))}
                onMove={(delta) => move(index, delta > 0 ? index + 2 : index - 1)}
                onAction={(action) => act(index, action)}
                onShowShot={onShowShot}
                onDragStart={(event) => {
                  event.dataTransfer.effectAllowed = "move";
                  event.dataTransfer.setData("text/plain", String(index));
                  setDragFrom(index);
                }}
                onDragEnd={endDrag}
                onDragOver={dragOver(index)}
                onDrop={drop}
              />
            </Fragment>
          ))}
          <InsertGap
            id={`flow-step-gap-${steps.length}`}
            active={showDrop(steps.length)}
            onAdd={(x, y) => setAddAt({ index: steps.length, x, y })}
            onDragOver={gapOver(steps.length)}
            onDrop={drop}
          />
        </ol>
      )}

      <div id="flow-editor-add-step" className="flex items-center gap-2">
        <Button
          id="flow-editor-add"
          size="sm"
          aria-haspopup="menu"
          onClick={(event) => {
            const rect = event.currentTarget.getBoundingClientRect();
            setAddAt({ index: steps.length, x: rect.left, y: rect.bottom + 4 });
          }}
        >
          <Plus id="flow-editor-add-icon" className="size-3.5" aria-hidden />
          Add step
        </Button>
        <span id="flow-editor-add-hint" className="text-xs text-zinc-500">
          Drag a step by its handle to move it. Hover between steps to add one there.
        </span>
      </div>

      {addAt ? <AddStepMenu id="flow-editor-add-menu" x={addAt.x} y={addAt.y} onPick={(kind) => insert(addAt.index, kind)} onClose={() => setAddAt(null)} /> : null}
    </section>
  );
}