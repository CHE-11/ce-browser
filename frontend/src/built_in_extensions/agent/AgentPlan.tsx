import { CheckCircle2, Circle, CircleDot, CircleSlash, ListChecks } from "lucide-react";
import type { AgentPlanItem } from "@type/browser";
import { cn } from "@/lib/cn";

const ICONS: Record<AgentPlanItem["status"], typeof Circle> = {
  pending: Circle,
  active: CircleDot,
  done: CheckCircle2,
  skipped: CircleSlash,
};

const TONES: Record<AgentPlanItem["status"], string> = {
  pending: "text-zinc-600",
  active: "text-sky-400",
  done: "text-emerald-400",
  skipped: "text-zinc-600",
};

/** The checklist the agent keeps for a task with several parts. */
export function AgentPlan({ id, plan }: { id: string; plan: AgentPlanItem[] }) {
  if (plan.length === 0) {
    return null;
  }
  const done = plan.filter((item) => item.status === "done" || item.status === "skipped").length;

  return (
    <section id={id} aria-label="Plan" className="rounded-lg bg-zinc-900/70 p-3 ring-1 ring-zinc-800">
      <div id={`${id}-header`} className="mb-1.5 flex items-center gap-1.5 text-[11px] text-zinc-400">
        <ListChecks id={`${id}-icon`} className="size-3.5" aria-hidden />
        Plan
        <span id={`${id}-count`} className="ml-auto">
          {done} / {plan.length}
        </span>
      </div>
      <div id={`${id}-bar`} className="mb-2 h-1 overflow-hidden rounded-full bg-zinc-800">
        <div id={`${id}-bar-fill`} className="h-full rounded-full bg-emerald-500 transition-[width]" style={{ width: `${(done / plan.length) * 100}%` }} />
      </div>
      <ol id={`${id}-list`} className="flex flex-col gap-1">
        {plan.map((item, index) => {
          const Icon = ICONS[item.status] ?? Circle;
          return (
            <li id={`${id}-item-${index}`} key={`${index}-${item.text}`} className="flex items-start gap-2 text-xs">
              <Icon id={`${id}-item-${index}-icon`} className={cn("mt-px size-3.5 shrink-0", TONES[item.status], item.status === "active" && "animate-pulse")} aria-hidden />
              <span
                id={`${id}-item-${index}-text`}
                className={cn(
                  "min-w-0 break-words",
                  item.status === "done" && "text-zinc-400",
                  item.status === "skipped" && "text-zinc-600 line-through",
                  item.status === "active" && "text-zinc-50",
                  item.status === "pending" && "text-zinc-300",
                )}
              >
                {item.text}
              </span>
            </li>
          );
        })}
      </ol>
    </section>
  );
}
