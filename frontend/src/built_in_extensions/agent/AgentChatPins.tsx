import { useEffect, useState } from "react";
import { BarChart3, ChevronsDownUp, ChevronsUpDown, ExternalLink, Link2, ListChecks } from "lucide-react";
import type { AgentPlanItem, AgentRun } from "@type/browser";
import { createTab } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { isActiveRun } from "@/lib/use-agent-run";
import { AGENT_TIMELINE_FILTERS, type AgentTimelineFilter } from "./agent-timeline-filter";
import { AgentPlan } from "./AgentPlan";
import { AgentUsage } from "./AgentUsage";
import { STATUS_TEXT, STATUS_TONE } from "./agent-styles";

type Pin = "stats" | "plan" | "pages";

type TrailItem = {
  url: string;
  role: "start" | "visited";
};

function trailOf(run: AgentRun): TrailItem[] {
  const items: TrailItem[] = [];
  const seen = new Set<string>();
  const add = (url: string, role: TrailItem["role"]) => {
    const value = url.trim();
    if (!value) {
      return;
    }
    const key = value.toLowerCase();
    if (seen.has(key)) {
      return;
    }
    seen.add(key);
    items.push({ url: value, role });
  };
  add(run.startUrl, "start");
  for (const url of run.sources ?? []) {
    add(url, items.length === 0 ? "start" : "visited");
  }
  return items;
}

function hostOf(url: string) {
  try {
    return new URL(url).hostname.replace(/^www\./, "") || url;
  } catch {
    return url;
  }
}

function pathOf(url: string) {
  try {
    const parsed = new URL(url);
    const path = `${parsed.pathname}${parsed.search}${parsed.hash}`;
    return path === "/" ? "" : path;
  } catch {
    return "";
  }
}

type Props = {
  id: string;
  run: AgentRun;
  filter: AgentTimelineFilter;
  onFilter: (filter: AgentTimelineFilter) => void;
  onExpandAll: () => void;
  onCollapseAll: () => void;
  onPlanChange?: (plan: AgentPlanItem[]) => void;
  /** Match the page body horizontal padding so the sticky bar covers edge-to-edge. */
  page?: boolean;
};

/** Sticky Stats / Plan / Pages controls above the chat transcript. */
export function AgentChatPins({ id, run, filter, onFilter, onExpandAll, onCollapseAll, onPlanChange, page = false }: Props) {
  const [open, setOpen] = useState<Pin | null>(null);
  const trail = trailOf(run);
  const planDone = run.plan.filter((item) => item.status === "done" || item.status === "skipped").length;
  const active = isActiveRun(run);

  useEffect(() => {
    setOpen(null);
  }, [run.id]);

  const toggle = (pin: Pin) => {
    setOpen((current) => (current === pin ? null : pin));
  };

  const pinButton = (pin: Pin, label: string, hint: string, Icon: typeof BarChart3) => (
    <button
      id={`${id}-${pin}`}
      type="button"
      aria-expanded={open === pin}
      aria-controls={`${id}-detail`}
      className={cn(
        "flex min-w-0 flex-1 flex-col items-start gap-0.5 rounded-lg px-2.5 py-2 text-left ring-1 transition-colors",
        open === pin ? "bg-zinc-800 ring-zinc-600" : "bg-zinc-900/80 ring-zinc-800 hover:bg-zinc-800/80",
      )}
      onClick={() => toggle(pin)}
    >
      <span id={`${id}-${pin}-label`} className="flex items-center gap-1 text-[11px] font-medium text-zinc-200">
        <Icon id={`${id}-${pin}-icon`} className="size-3.5 shrink-0 text-zinc-400" aria-hidden />
        {label}
      </span>
      <span id={`${id}-${pin}-hint`} className="w-full truncate text-[10px] text-zinc-500">
        {hint}
      </span>
    </button>
  );

  return (
    <div id={id} className={cn("sticky top-0 z-20 bg-zinc-950 pb-2 pt-5", page ? "-mx-4 px-4" : "-mx-3 px-3")}>
      <div id={`${id}-row`} className="flex gap-1.5">
        {pinButton("stats", "Stats", STATUS_TEXT[run.status], BarChart3)}
        {pinButton(
          "plan",
          "Plan",
          run.plan.length > 0 ? `${planDone} / ${run.plan.length}` : "No plan yet",
          ListChecks,
        )}
        {pinButton(
          "pages",
          "Pages",
          trail.length > 0 ? `${trail.length} · ${hostOf(trail[trail.length - 1].url)}` : "No pages yet",
          Link2,
        )}
      </div>

      {open ? (
        <div id={`${id}-detail`} className="mt-1.5 max-h-56 overflow-y-auto rounded-lg bg-zinc-900 p-3 ring-1 ring-zinc-800">
          {open === "stats" ? (
            <div id={`${id}-stats`} className="flex flex-col gap-2">
              <div id={`${id}-stats-status`} className="flex items-center gap-2 text-xs text-zinc-300">
                <span id={`${id}-stats-dot`} className={cn("size-2 rounded-full", STATUS_TONE[run.status], active && run.status === "running" && "animate-pulse")} />
                {STATUS_TEXT[run.status]}
              </div>
              <AgentUsage id={`${id}-stats-usage`} run={run} />
            </div>
          ) : null}

          {open === "plan" ? (
            run.plan.length > 0 ? (
              <AgentPlan
                id={`${id}-plan`}
                plan={run.plan}
                embedded
                editable={run.source === "agent" && !!onPlanChange}
                onChange={onPlanChange}
              />
            ) : (
              <p id={`${id}-plan-empty`} className="text-xs text-zinc-500">
                No plan yet. The agent adds one when the task has several parts.
              </p>
            )
          ) : null}

          {open === "pages" ? (
            trail.length > 0 ? (
              <ol id={`${id}-pages-list`} className="flex flex-col gap-0.5">
                {trail.map((item, index) => {
                  const openable = /^https?:\/\//i.test(item.url);
                  const path = pathOf(item.url);
                  return (
                    <li id={`${id}-pages-item-${index}`} key={item.url}>
                      <button
                        id={`${id}-pages-item-${index}-open`}
                        type="button"
                        disabled={!openable}
                        title={item.url}
                        className="flex w-full items-center gap-1.5 rounded px-1 py-1 text-left text-[11px] enabled:hover:bg-zinc-800/80 disabled:cursor-default"
                        onClick={() => {
                          if (openable) {
                            void createTab(item.url);
                          }
                        }}
                      >
                        <span id={`${id}-pages-item-${index}-index`} className="w-4 shrink-0 text-center tabular-nums text-zinc-600">
                          {index + 1}
                        </span>
                        <span id={`${id}-pages-item-${index}-body`} className="min-w-0 flex-1">
                          <span id={`${id}-pages-item-${index}-host`} className={openable ? "text-sky-300" : "text-zinc-300"}>
                            {hostOf(item.url)}
                          </span>
                          {path ? (
                            <span id={`${id}-pages-item-${index}-path`} className="text-zinc-500">
                              {path.length > 48 ? `${path.slice(0, 48)}…` : path}
                            </span>
                          ) : null}
                          {item.role === "start" ? (
                            <span id={`${id}-pages-item-${index}-start`} className="ml-1.5 text-[10px] text-zinc-600">
                              started
                            </span>
                          ) : null}
                        </span>
                        {openable ? <ExternalLink id={`${id}-pages-item-${index}-icon`} className="size-3 shrink-0 text-zinc-600" aria-hidden /> : null}
                      </button>
                    </li>
                  );
                })}
              </ol>
            ) : (
              <p id={`${id}-pages-empty`} className="text-xs text-zinc-500">
                No pages recorded yet.
              </p>
            )
          ) : null}
        </div>
      ) : null}

      <div id={`${id}-filters`} className="mt-1.5 flex items-center gap-1 px-0.5">
        <div id={`${id}-filters-tabs`} role="tablist" aria-label="Show" className="flex min-w-0 flex-1 gap-1">
          {AGENT_TIMELINE_FILTERS.map((item) => (
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
              onClick={() => onFilter(item.value)}
            >
              {item.label}
            </button>
          ))}
        </div>
        <div id={`${id}-filters-fold`} className="flex shrink-0 gap-1">
          <button
            id={`${id}-expand-all`}
            type="button"
            className="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] text-zinc-400 ring-1 ring-zinc-800 hover:text-zinc-100"
            title="Expand all messages"
            onClick={onExpandAll}
          >
            <ChevronsUpDown id={`${id}-expand-all-icon`} className="size-3" aria-hidden />
            Expand all
          </button>
          <button
            id={`${id}-collapse-all`}
            type="button"
            className="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] text-zinc-400 ring-1 ring-zinc-800 hover:text-zinc-100"
            title="Collapse all messages"
            onClick={onCollapseAll}
          >
            <ChevronsDownUp id={`${id}-collapse-all-icon`} className="size-3" aria-hidden />
            Collapse all
          </button>
        </div>
      </div>
    </div>
  );
}
