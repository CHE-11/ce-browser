import { useEffect, useRef, useState } from "react";
import { AlertTriangle, Brain, CheckCircle2, ChevronRight, MousePointerClick, ShieldAlert, ShieldCheck, User, Wrench } from "lucide-react";
import type { AgentStep, AgentStepKind } from "@type/browser";
import { Markdown } from "@/components/Markdown";
import { cn } from "@/lib/cn";
import { formatClock } from "./agent-styles";
import { LongMessage } from "./AgentError";

const ICONS: Record<AgentStepKind, typeof Brain> = {
  think: Brain,
  action: MousePointerClick,
  tool: Wrench,
  approved: ShieldCheck,
  blocked: ShieldAlert,
  error: AlertTriangle,
  user: User,
  answer: CheckCircle2,
};

const TONES: Record<AgentStepKind, string> = {
  think: "text-zinc-400",
  action: "text-sky-400",
  tool: "text-teal-400",
  approved: "text-fuchsia-300",
  blocked: "text-amber-400",
  error: "text-red-400",
  user: "text-violet-300",
  answer: "text-emerald-400",
};

type Filter = "all" | "actions" | "problems";

const FILTERS: { value: Filter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "actions", label: "Actions" },
  { value: "problems", label: "Problems" },
];

/** Groups open by default, counted from the newest. */
const OPEN_GROUPS = 4;

type Indexed = { step: AgentStep; index: number };
type Group = { head: Indexed | null; items: Indexed[] };

function groupSteps(steps: AgentStep[]) {
  const groups: Group[] = [];
  steps.forEach((step, index) => {
    const entry = { step, index };
    if (step.kind === "think" || step.kind === "user" || step.kind === "answer" || groups.length === 0) {
      const head = step.kind === "think" || step.kind === "user" || step.kind === "answer" ? entry : null;
      groups.push({ head, items: head ? [] : [entry] });
      return;
    }
    groups[groups.length - 1].items.push(entry);
  });
  return groups;
}

function matches(filter: Filter, kind: AgentStepKind) {
  if (filter === "actions") {
    return kind === "action" || kind === "tool" || kind === "approved";
  }
  if (filter === "problems") {
    return kind === "error" || kind === "blocked";
  }
  return true;
}

type Props = {
  id: string;
  steps: AgentStep[];
  /** What the model is writing now, while the run works. */
  streaming?: { text: string; thinking: string } | null;
};

/** The run's steps: each thought with the actions that follow it, newest at the bottom. */
export function AgentTimeline({ id, steps, streaming }: Props) {
  const end = useRef<HTMLDivElement | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [toggled, setToggled] = useState<Record<number, boolean>>({});
  const [zoomed, setZoomed] = useState<number | null>(null);
  const groups = groupSteps(steps);
  const live = streaming && (streaming.text || streaming.thinking) ? streaming : null;

  useEffect(() => {
    const scroller = end.current?.closest("#agent-panel-body");
    if (!scroller) {
      return;
    }
    const nearBottom = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 200;
    if (nearBottom) {
      end.current?.scrollIntoView({ block: "end" });
    }
  }, [steps.length, live?.text, live?.thinking]);

  const row = ({ step, index }: Indexed) => {
    const Icon = ICONS[step.kind] ?? Wrench;
    const errorText = step.kind === "error" && step.detail ? `${step.text}\n${step.detail}` : step.text;
    return (
      <li id={`${id}-${index}`} key={`${step.at}-${index}`} className="flex gap-2 rounded-md bg-zinc-900/70 p-2 ring-1 ring-zinc-800">
        <Icon id={`${id}-${index}-icon`} className={cn("mt-0.5 size-3.5 shrink-0", TONES[step.kind])} aria-hidden />
        <div id={`${id}-${index}-body`} className="min-w-0 flex-1">
          {step.kind === "error" ? (
            <LongMessage id={`${id}-${index}-text`} text={errorText} className="text-xs text-zinc-100" />
          ) : (
            <>
              <div id={`${id}-${index}-text`} className="whitespace-pre-wrap break-words text-xs text-zinc-100">
                {step.text}
              </div>
              {step.detail ? (
                <div id={`${id}-${index}-detail`} className="mt-0.5 line-clamp-3 whitespace-pre-wrap break-words text-[11px] text-zinc-500">
                  {step.detail}
                </div>
              ) : null}
            </>
          )}
          {step.thumb ? (
            <button
              id={`${id}-${index}-thumb`}
              type="button"
              className="mt-1.5 block overflow-hidden rounded ring-1 ring-zinc-700"
              title={zoomed === index ? "Shrink" : "Enlarge"}
              onClick={() => setZoomed(zoomed === index ? null : index)}
            >
              <img id={`${id}-${index}-thumb-image`} src={step.thumb} alt="Page after this step" className={cn("block", zoomed === index ? "w-full" : "w-36")} />
            </button>
          ) : null}
        </div>
        <time id={`${id}-${index}-time`} className="shrink-0 text-[10px] text-zinc-600">
          {formatClock(step.at)}
        </time>
      </li>
    );
  };

  const head = ({ step, index }: Indexed, open: boolean, count: number, onToggle: () => void) => {
    if (step.kind === "user") {
      return (
        <div id={`${id}-${index}`} className="ml-6 rounded-lg rounded-br-sm bg-violet-950/50 px-2.5 py-1.5 text-xs text-violet-100 ring-1 ring-violet-900/60">
          <div id={`${id}-${index}-text`} className="whitespace-pre-wrap break-words">
            {step.text}
          </div>
        </div>
      );
    }
    if (step.kind === "answer") {
      return (
        <div id={`${id}-${index}`} className="flex items-center gap-1.5 text-[11px] text-emerald-400">
          <CheckCircle2 id={`${id}-${index}-icon`} className="size-3.5" aria-hidden />
          Finished
          <time id={`${id}-${index}-time`} className="ml-auto text-[10px] text-zinc-600">
            {formatClock(step.at)}
          </time>
        </div>
      );
    }
    return (
      <div id={`${id}-${index}`} className="flex gap-1.5">
        <button
          id={`${id}-${index}-toggle`}
          type="button"
          className="mt-0.5 flex size-4 shrink-0 items-center justify-center rounded text-zinc-500 hover:text-zinc-200 disabled:invisible"
          aria-expanded={open}
          aria-label={open ? "Hide actions" : `Show ${count} actions`}
          disabled={count === 0}
          onClick={onToggle}
        >
          <ChevronRight id={`${id}-${index}-toggle-icon`} className={cn("size-3.5 transition-transform", open && "rotate-90")} aria-hidden />
        </button>
        <div id={`${id}-${index}-body`} className="min-w-0 flex-1">
          <Markdown id={`${id}-${index}-text`} text={step.text} className={cn("text-xs text-zinc-300", !open && "line-clamp-3")} />
          {!open && count > 0 ? (
            <div id={`${id}-${index}-folded`} className="mt-0.5 text-[10px] text-zinc-500">
              {count} {count === 1 ? "action" : "actions"}
            </div>
          ) : null}
        </div>
      </div>
    );
  };

  return (
    <div id={id} className="flex flex-col gap-2">
      {steps.length > 6 ? (
        <div id={`${id}-filters`} role="tablist" aria-label="Show" className="flex gap-1">
          {FILTERS.map((item) => (
            <button
              id={`${id}-filter-${item.value}`}
              key={item.value}
              type="button"
              role="tab"
              aria-selected={filter === item.value}
              className={cn(
                "rounded-full px-2 py-0.5 text-[11px] text-zinc-400 ring-1 ring-zinc-800 hover:text-zinc-100",
                filter === item.value && "bg-zinc-800 text-zinc-100",
              )}
              onClick={() => setFilter(item.value)}
            >
              {item.label}
            </button>
          ))}
        </div>
      ) : null}
      {filter === "all" ? (
        <ol id={`${id}-groups`} className="flex flex-col gap-3">
          {groups.map((group, groupIndex) => {
            const key = group.head?.index ?? group.items[0]?.index ?? groupIndex;
            const fallbackOpen = groupIndex >= groups.length - OPEN_GROUPS;
            const open = toggled[key] ?? fallbackOpen;
            return (
              <li id={`${id}-group-${key}`} key={key} className="flex flex-col gap-1.5">
                {group.head ? head(group.head, open, group.items.length, () => setToggled((previous) => ({ ...previous, [key]: !open }))) : null}
                {group.items.length > 0 && (open || !group.head) ? (
                  <ol id={`${id}-group-${key}-items`} className={cn("flex flex-col gap-1.5", group.head?.step.kind === "think" && "ml-5")}>
                    {group.items.map(row)}
                  </ol>
                ) : null}
              </li>
            );
          })}
        </ol>
      ) : (
        <ol id={`${id}-filtered`} className="flex flex-col gap-1.5">
          {steps
            .map((step, index) => ({ step, index }))
            .filter(({ step }) => matches(filter, step.kind))
            .map(row)}
        </ol>
      )}
      {live ? (
        <div id={`${id}-live`} aria-live="polite" className="flex gap-1.5 rounded-md border border-dashed border-zinc-800 p-2">
          <Brain id={`${id}-live-icon`} className="mt-0.5 size-3.5 shrink-0 animate-pulse text-sky-400" aria-hidden />
          <div id={`${id}-live-body`} className="min-w-0 flex-1">
            {live.thinking ? (
              <p id={`${id}-live-thinking`} className="line-clamp-4 whitespace-pre-wrap break-words text-[11px] italic text-zinc-500">
                {live.thinking.slice(-600)}
              </p>
            ) : null}
            {live.text ? <Markdown id={`${id}-live-text`} text={live.text} className="text-xs text-zinc-200" /> : null}
          </div>
        </div>
      ) : null}
      <div id={`${id}-end`} ref={end} aria-hidden className="h-px" />
    </div>
  );
}
