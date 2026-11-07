import { useMemo, useState } from "react";
import { Search } from "lucide-react";
import type { Flow, FlowRun } from "@type/browser";
import { SkeletonRows } from "@/components/Skeleton";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import { useNow } from "@/lib/use-now";
import { FlowContextMenu, type FlowAction } from "./FlowContextMenu";
import { FlowListItem } from "./FlowListItem";

type Filter = "all" | "scheduled" | "running" | "failed" | "paused";

const FILTERS: { value: Filter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "scheduled", label: "Scheduled" },
  { value: "running", label: "Running" },
  { value: "failed", label: "Failed" },
  { value: "paused", label: "Paused" },
];

type FlowListProps = {
  flows: Flow[];
  runs: FlowRun[];
  nextRuns: Record<string, number>;
  loaded: boolean;
  currentId: string | null;
  /** Name of the unsaved flow being written, shown at the top. */
  draftName: string | null;
  onOpen: (id: string) => void;
  onAction: (flow: Flow, action: FlowAction) => void;
};

function isScheduled(flow: Flow, nextRuns: Record<string, number>) {
  return nextRuns[flow.id] !== undefined || (!flow.paused && flow.triggers.some((trigger) => trigger.enabled));
}

function matches(flow: Flow, filter: Filter, running: boolean, scheduled: boolean) {
  switch (filter) {
    case "scheduled":
      return scheduled;
    case "running":
      return running;
    case "failed":
      return flow.lastStatus === "failed";
    case "paused":
      return flow.paused;
    default:
      return true;
  }
}

export function FlowList({ flows, runs, nextRuns, loaded, currentId, draftName, onOpen, onAction }: FlowListProps) {
  const now = useNow(30_000);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [menu, setMenu] = useState<{ flow: Flow; x: number; y: number } | null>(null);

  const runningByFlow = useMemo(() => {
    const out = new Map<string, FlowRun>();
    for (const run of runs) {
      if (run.status === "running") {
        out.set(run.flowId, run);
      }
    }
    return out;
  }, [runs]);

  const shown = useMemo(() => {
    const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
    return flows.filter((flow) => {
      if (!matches(flow, filter, runningByFlow.has(flow.id), isScheduled(flow, nextRuns))) {
        return false;
      }
      const haystack = `${flow.name} ${flow.description} ${flow.startUrl}`.toLowerCase();
      return words.every((word) => haystack.includes(word));
    });
  }, [flows, filter, query, nextRuns, runningByFlow]);

  const counts = useMemo(() => {
    const out: Record<Filter, number> = { all: flows.length, scheduled: 0, running: runningByFlow.size, failed: 0, paused: 0 };
    for (const flow of flows) {
      if (isScheduled(flow, nextRuns)) {
        out.scheduled += 1;
      }
      if (flow.lastStatus === "failed") {
        out.failed += 1;
      }
      if (flow.paused) {
        out.paused += 1;
      }
    }
    return out;
  }, [flows, nextRuns, runningByFlow]);

  return (
    <div id="automations-flows" className="flex min-h-0 flex-1 flex-col gap-2">
      {flows.length > 0 ? (
        <>
          <label id="automations-search" className="flex h-8 items-center gap-2 rounded-md border border-zinc-800 bg-zinc-900 px-2 text-sm focus-within:border-sky-500">
            <Search id="automations-search-icon" className="size-3.5 shrink-0 text-zinc-500" aria-hidden />
            <input
              id="automations-search-input"
              className="min-w-0 flex-1 bg-transparent text-zinc-100 outline-none placeholder:text-zinc-600"
              value={query}
              placeholder="Search flows"
              aria-label="Search flows"
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Escape" && query) {
                  event.stopPropagation();
                  setQuery("");
                }
              }}
            />
          </label>
          <div id="automations-filters" role="group" aria-label="Show flows" className="flex flex-wrap gap-1">
            {FILTERS.filter((item) => item.value === "all" || counts[item.value] > 0 || filter === item.value).map((item) => (
              <button
                id={`automations-filter-${item.value}`}
                key={item.value}
                type="button"
                aria-pressed={filter === item.value}
                className={cn(
                  "inline-flex h-6 items-center gap-1 rounded-full px-2 text-[11px]",
                  focusRow,
                  filter === item.value ? "bg-sky-900/70 text-sky-100" : "bg-zinc-900 text-zinc-400 hover:bg-zinc-800",
                )}
                onClick={() => setFilter(item.value)}
              >
                {item.label}
                <span id={`automations-filter-${item.value}-count`} className="text-zinc-500">
                  {counts[item.value]}
                </span>
              </button>
            ))}
          </div>
        </>
      ) : null}
      <div id="automations-flow-list" className="-mx-1 flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto px-1">
        {!loaded ? <SkeletonRows id="automations-loading" rows={3} className="px-2 py-1" /> : null}
        {loaded && flows.length === 0 && draftName === null ? (
          <p id="automations-flows-empty" className="px-2 py-2 text-xs leading-relaxed text-zinc-500">
            No flows yet. Record one on a page, or build it step by step.
          </p>
        ) : null}
        {loaded && flows.length > 0 && shown.length === 0 ? (
          <p id="automations-flows-none" className="px-2 py-2 text-xs text-zinc-500">
            No flows match.
          </p>
        ) : null}
        {draftName !== null ? (
          <div id="automations-flow-draft" aria-current="true" className="flex h-9 items-center gap-2 rounded-md bg-zinc-800 px-2 text-sm text-zinc-300 italic">
            <span id="automations-flow-draft-dot" aria-hidden className="size-2 shrink-0 rounded-full border border-dashed border-zinc-500" />
            <span id="automations-flow-draft-name" className="truncate">
              {draftName || "New flow"}
            </span>
          </div>
        ) : null}
        {shown.map((flow) => (
          <FlowListItem
            key={flow.id}
            flow={flow}
            current={flow.id === currentId}
            running={runningByFlow.get(flow.id)}
            nextRun={nextRuns[flow.id]}
            now={now}
            onOpen={() => onOpen(flow.id)}
            onContextMenu={(event) => {
              event.preventDefault();
              setMenu({ flow, x: event.clientX, y: event.clientY });
            }}
          />
        ))}
      </div>
      {menu ? (
        <FlowContextMenu
          flow={menu.flow}
          running={runningByFlow.has(menu.flow.id)}
          x={menu.x}
          y={menu.y}
          onAction={(action) => onAction(menu.flow, action)}
          onClose={() => setMenu(null)}
        />
      ) : null}
    </div>
  );
}
