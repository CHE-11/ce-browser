import { useMemo, useState } from "react";
import { History, Search, Trash2 } from "lucide-react";
import type { Flow, FlowRun } from "@type/browser";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { clearFlowRuns } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import { useNow } from "@/lib/use-now";
import { inputClassName } from "@features/Agent/agent-styles";
import { dayLabel } from "./flow-model";
import { RunItem } from "./RunItem";
import type { RunShot } from "./RunShotDialog";

type StatusFilter = "all" | FlowRun["status"];

const STATUS_FILTERS: { value: StatusFilter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "running", label: "Running" },
  { value: "failed", label: "Failed" },
  { value: "done", label: "Done" },
  { value: "stopped", label: "Stopped" },
];

type RunHistoryProps = {
  runs: FlowRun[];
  flows: Flow[];
  /** Shows only this flow's runs, inside its editor. */
  flowId?: string;
  onOpenFlow: (id: string, step?: number) => void;
  onOpenDataset: (id: string) => void;
  onRerun: (run: FlowRun) => void;
  onShowShot: (shot: RunShot) => void;
};

export function RunHistory({ runs, flows, flowId, onOpenFlow, onOpenDataset, onRerun, onShowShot }: RunHistoryProps) {
  const now = useNow(60_000);
  const [status, setStatus] = useState<StatusFilter>("all");
  const [flowFilter, setFlowFilter] = useState("");
  const [query, setQuery] = useState("");
  const scoped = flowId !== undefined;
  const flowIds = useMemo(() => new Set(flows.map((flow) => flow.id)), [flows]);

  const own = useMemo(() => (scoped ? runs.filter((run) => run.flowId === flowId) : runs), [runs, scoped, flowId]);

  const shown = useMemo(() => {
    const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
    return own
      .filter((run) => status === "all" || run.status === status)
      .filter((run) => !flowFilter || run.flowId === flowFilter)
      .filter((run) => {
        if (!words.length) {
          return true;
        }
        const haystack = [run.flowName, run.trigger, run.output, ...run.steps.map((step) => `${step.label} ${step.detail}`)].join(" ").toLowerCase();
        return words.every((word) => haystack.includes(word));
      })
      .slice()
      .sort((a, b) => b.started - a.started);
  }, [own, status, flowFilter, query]);

  const groups = useMemo(() => {
    const out: { label: string; runs: FlowRun[] }[] = [];
    for (const run of shown) {
      const label = dayLabel(run.started, now);
      const last = out[out.length - 1];
      if (last && last.label === label) {
        last.runs.push(run);
      } else {
        out.push({ label, runs: [run] });
      }
    }
    return out;
  }, [shown, now]);

  const flowOptions = useMemo(() => {
    const seen = new Map<string, string>();
    for (const run of runs) {
      if (!seen.has(run.flowId)) {
        seen.set(run.flowId, run.flowName || "Flow");
      }
    }
    return [...seen.entries()].sort((a, b) => a[1].localeCompare(b[1]));
  }, [runs]);

  const toolbar = (
    <div id="run-history-filters" className="flex flex-wrap items-center gap-2">
      <div id="run-history-status" role="group" aria-label="Show runs" className="flex gap-1">
        {STATUS_FILTERS.map((item) => (
          <button
            id={`run-history-status-${item.value}`}
            key={item.value}
            type="button"
            aria-pressed={status === item.value}
            className={cn(
              "h-6 rounded-full px-2 text-[11px]",
              focusRow,
              status === item.value ? "bg-sky-900/70 text-sky-100" : "bg-zinc-900 text-zinc-400 hover:bg-zinc-800",
            )}
            onClick={() => setStatus(item.value)}
          >
            {item.label}
          </button>
        ))}
      </div>
      {!scoped && flowOptions.length > 1 ? (
        <select id="run-history-flow" className={cn(inputClassName, "h-7 w-48 py-0 text-xs")} value={flowFilter} onChange={(event) => setFlowFilter(event.target.value)} aria-label="Flow">
          <option id="run-history-flow-all" value="">
            All flows
          </option>
          {flowOptions.map(([id, name]) => (
            <option id={`run-history-flow-${id}`} key={id} value={id}>
              {name}
            </option>
          ))}
        </select>
      ) : null}
      <label id="run-history-search" className="flex h-7 min-w-40 flex-1 items-center gap-2 rounded-md border border-zinc-800 bg-zinc-900 px-2 text-xs focus-within:border-sky-500">
        <Search id="run-history-search-icon" className="size-3.5 shrink-0 text-zinc-500" aria-hidden />
        <input
          id="run-history-search-input"
          className="min-w-0 flex-1 bg-transparent text-zinc-100 outline-none placeholder:text-zinc-600"
          value={query}
          placeholder="Search steps and output"
          aria-label="Search runs"
          onChange={(event) => setQuery(event.target.value)}
        />
      </label>
    </div>
  );

  const list =
    own.length === 0 ? (
      <EmptyState
        id="run-history-empty"
        icon={History}
        title="No runs yet"
        hint={scoped ? "Runs of this flow show up here, with each step's result." : "Runs from the editor, schedules, monitors and MCP clients show up here."}
      />
    ) : shown.length === 0 ? (
      <p id="run-history-none" className="px-1 py-4 text-sm text-zinc-500">
        No runs match.
      </p>
    ) : (
      <div id="run-history-groups" className="flex flex-col gap-4">
        {groups.map((group, at) => (
          <section id={`run-history-group-${at}`} key={group.label} className="flex flex-col gap-2">
            <h2 id={`run-history-group-${at}-title`} className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
              {group.label}
            </h2>
            <ul id={`run-history-group-${at}-list`} className="flex flex-col gap-2">
              {group.runs.map((run) => (
                <RunItem
                  key={run.id}
                  run={run}
                  showName={!scoped}
                  flowExists={flowIds.has(run.flowId)}
                  onOpenFlow={onOpenFlow}
                  onOpenDataset={onOpenDataset}
                  onRerun={onRerun}
                  onShowShot={onShowShot}
                />
              ))}
            </ul>
          </section>
        ))}
      </div>
    );

  if (scoped) {
    return (
      <section id="run-history" className="flex flex-col gap-3">
        {own.length ? toolbar : null}
        {list}
      </section>
    );
  }

  return (
    <section id="run-history" className="flex min-w-0 flex-1 flex-col overflow-y-auto">
      <header id="run-history-header" className="sticky top-0 z-10 flex flex-col gap-3 border-b border-zinc-800 bg-zinc-950/95 px-6 py-3 backdrop-blur">
        <div id="run-history-heading" className="flex items-center gap-2">
          <h1 id="run-history-title" className="flex-1 text-lg font-semibold text-zinc-100">
            History
          </h1>
          <Button id="run-history-clear" size="sm" variant="ghost" disabled={!runs.some((run) => run.status !== "running")} onClick={() => void clearFlowRuns()}>
            <Trash2 id="run-history-clear-icon" className="size-3.5" aria-hidden />
            Clear finished
          </Button>
        </div>
        {runs.length ? toolbar : null}
      </header>
      <div id="run-history-body" className="px-6 py-5">
        {list}
      </div>
    </section>
  );
}
