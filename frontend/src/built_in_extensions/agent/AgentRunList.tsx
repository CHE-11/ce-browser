import { useState } from "react";
import { Search, Trash2, X } from "lucide-react";
import type { AgentRun } from "@type/browser";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { cn } from "@/lib/cn";
import { isActiveRun } from "@/lib/use-agent-run";
import { inputClassName, STATUS_TEXT, STATUS_TONE } from "./agent-styles";

type Props = {
  runs: AgentRun[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  onDelete: (id: number) => void;
  onClear: () => void;
  onClose: () => void;
};

function startOfToday() {
  const now = new Date();
  return new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
}

function when(at: number, today: number) {
  const date = new Date(at);
  return at >= today
    ? date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
    : date.toLocaleDateString([], { month: "short", day: "numeric" });
}

/** Every task in this window, searchable, with helpers under the task that started them. */
export function AgentRunList({ runs, selectedId, onSelect, onDelete, onClear, onClose }: Props) {
  const [query, setQuery] = useState("");
  const needle = query.trim().toLowerCase();
  const matches = (run: AgentRun) => !needle || run.goal.toLowerCase().includes(needle) || run.result.toLowerCase().includes(needle);
  const ids = new Set(runs.map((run) => run.id));
  const roots = runs.filter((run) => run.parent === null || !ids.has(run.parent));
  const visible = roots.filter((run) => matches(run) || runs.some((child) => child.parent === run.id && matches(child)));
  const today = startOfToday();
  const groups = [
    { id: "today", label: "Today", runs: visible.filter((run) => run.started >= today) },
    { id: "earlier", label: "Earlier", runs: visible.filter((run) => run.started < today) },
  ].filter((group) => group.runs.length > 0);
  const finished = runs.some((run) => !isActiveRun(run));

  const row = (run: AgentRun, child: boolean) => (
    <li id={`agent-runs-item-${run.id}`} key={run.id} className={cn("group flex items-start gap-1", child && "ml-4")}>
      <button
        id={`agent-runs-item-${run.id}-open`}
        type="button"
        className={cn(
          "flex min-w-0 flex-1 items-start gap-2 rounded-md px-2 py-1.5 text-left hover:bg-zinc-800/80",
          selectedId === run.id && "bg-zinc-800 ring-1 ring-zinc-700",
        )}
        onClick={() => onSelect(run.id)}
      >
        <span id={`agent-runs-item-${run.id}-dot`} className={cn("mt-1.5 size-2 shrink-0 rounded-full", STATUS_TONE[run.status], isActiveRun(run) && "animate-pulse")} />
        <span id={`agent-runs-item-${run.id}-body`} className="min-w-0 flex-1">
          <span id={`agent-runs-item-${run.id}-goal`} className="line-clamp-2 text-xs text-zinc-100">
            {child ? `Helper · ${run.goal}` : run.goal}
          </span>
          <span id={`agent-runs-item-${run.id}-meta`} className="mt-0.5 block text-[10px] text-zinc-500">
            {[run.source === "mcp" ? "MCP" : null, STATUS_TEXT[run.status], `${run.actions} actions`, run.background ? "background" : null, when(run.started, today)]
              .filter(Boolean)
              .join(" · ")}
          </span>
        </span>
      </button>
      <IconButton
        id={`agent-runs-item-${run.id}-delete`}
        label="Delete this task"
        className="mt-1 size-6 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
        onClick={() => onDelete(run.id)}
      >
        <Trash2 id={`agent-runs-item-${run.id}-delete-icon`} aria-hidden />
      </IconButton>
    </li>
  );

  return (
    <section id="agent-runs" aria-label="Tasks" className="flex min-h-0 flex-1 flex-col">
      <div id="agent-runs-toolbar" className="flex shrink-0 items-center gap-2 px-3 py-2">
        <div id="agent-runs-search" className="relative min-w-0 flex-1">
          <Search id="agent-runs-search-icon" className="pointer-events-none absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-zinc-500" aria-hidden />
          <input
            id="agent-runs-search-input"
            className={cn(inputClassName, "h-7 py-1 pl-7 text-xs")}
            placeholder="Search tasks"
            value={query}
            autoFocus
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.stopPropagation();
                onClose();
              }
            }}
          />
        </div>
        <IconButton id="agent-runs-close" label="Back to the task" className="size-7" onClick={onClose}>
          <X id="agent-runs-close-icon" aria-hidden />
        </IconButton>
      </div>
      <div id="agent-runs-scroll" className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
        {groups.length === 0 ? (
          <p id="agent-runs-empty" className="px-2 py-6 text-center text-xs text-zinc-500">
            {needle ? "No tasks match." : "No tasks yet."}
          </p>
        ) : (
          groups.map((group) => (
            <div id={`agent-runs-group-${group.id}`} key={group.id} className="mb-2">
              <h2 id={`agent-runs-group-${group.id}-title`} className="px-2 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-wide text-zinc-500">
                {group.label}
              </h2>
              <ul id={`agent-runs-group-${group.id}-list`} className="flex flex-col gap-0.5">
                {group.runs.flatMap((run) => [row(run, false), ...runs.filter((child) => child.parent === run.id).map((child) => row(child, true))])}
              </ul>
            </div>
          ))
        )}
      </div>
      {finished ? (
        <div id="agent-runs-footer" className="shrink-0 border-t border-zinc-800 px-3 py-2">
          <Button id="agent-runs-clear" size="xs" variant="ghost" className="w-full" onClick={onClear}>
            <Trash2 id="agent-runs-clear-icon" className="size-3" aria-hidden />
            Clear finished tasks
          </Button>
        </div>
      ) : null}
    </section>
  );
}
