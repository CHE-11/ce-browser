import { CheckCircle2, Circle, CircleDot, CircleSlash, ListChecks, SkipForward } from "lucide-react";
import type { AgentPlanItem } from "@type/browser";
import { IconButton } from "@/components/IconButton";
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

type Props = {
  id: string;
  plan: AgentPlanItem[];
  /** When set, the user can mark steps done/skipped/active. */
  editable?: boolean;
  onChange?: (plan: AgentPlanItem[]) => void;
  /** Drop the outer card chrome when shown inside another panel. */
  embedded?: boolean;
};

/** The checklist the agent keeps for a task with several parts. */
export function AgentPlan({ id, plan, editable = false, onChange, embedded = false }: Props) {
  if (plan.length === 0) {
    return null;
  }
  const done = plan.filter((item) => item.status === "done" || item.status === "skipped").length;

  const setStatus = (index: number, status: AgentPlanItem["status"]) => {
    if (!onChange) {
      return;
    }
    const next = plan.map((item, itemIndex) => {
      if (itemIndex !== index) {
        if (status === "active" && item.status === "active") {
          return { ...item, status: "pending" as const };
        }
        return item;
      }
      return { ...item, status };
    });
    onChange(next);
  };

  const body = (
    <>
      {embedded ? null : (
        <div id={`${id}-header`} className="mb-1.5 flex items-center gap-1.5 text-[11px] text-zinc-400">
          <ListChecks id={`${id}-icon`} className="size-3.5" aria-hidden />
          Plan
          {editable ? (
            <span id={`${id}-editable`} className="text-zinc-600">
              · editable
            </span>
          ) : null}
          <span id={`${id}-count`} className="ml-auto">
            {done} / {plan.length}
          </span>
        </div>
      )}
      <div id={`${id}-bar`} className="mb-2 h-1 overflow-hidden rounded-full bg-zinc-800">
        <div id={`${id}-bar-fill`} className="h-full rounded-full bg-emerald-500 transition-[width]" style={{ width: `${(done / plan.length) * 100}%` }} />
      </div>
      <ol id={`${id}-list`} className="flex flex-col gap-1">
        {plan.map((item, index) => {
          const Icon = ICONS[item.status] ?? Circle;
          return (
            <li id={`${id}-item-${index}`} key={`${index}-${item.text}`} className="group flex items-start gap-2 text-xs">
              <button
                id={`${id}-item-${index}-toggle`}
                type="button"
                disabled={!editable}
                className={cn("mt-px shrink-0 rounded", editable && "hover:bg-zinc-800")}
                title={editable ? "Cycle status" : undefined}
                onClick={() => {
                  if (!editable) {
                    return;
                  }
                  const order: AgentPlanItem["status"][] = ["pending", "active", "done", "skipped"];
                  const next = order[(order.indexOf(item.status) + 1) % order.length] ?? "pending";
                  setStatus(index, next);
                }}
              >
                <Icon id={`${id}-item-${index}-icon`} className={cn("size-3.5", TONES[item.status], item.status === "active" && "animate-pulse")} aria-hidden />
              </button>
              <span
                id={`${id}-item-${index}-text`}
                className={cn(
                  "min-w-0 flex-1 break-words",
                  item.status === "done" && "text-zinc-400",
                  item.status === "skipped" && "text-zinc-600 line-through",
                  item.status === "active" && "text-zinc-50",
                  item.status === "pending" && "text-zinc-300",
                )}
              >
                {item.text}
              </span>
              {editable ? (
                <span id={`${id}-item-${index}-actions`} className="flex shrink-0 opacity-0 group-hover:opacity-100 focus-within:opacity-100">
                  <IconButton id={`${id}-item-${index}-done`} label="Mark done" className="size-6" onClick={() => setStatus(index, "done")}>
                    <CheckCircle2 id={`${id}-item-${index}-done-icon`} className="size-3" aria-hidden />
                  </IconButton>
                  <IconButton id={`${id}-item-${index}-skip`} label="Skip" className="size-6" onClick={() => setStatus(index, "skipped")}>
                    <SkipForward id={`${id}-item-${index}-skip-icon`} className="size-3" aria-hidden />
                  </IconButton>
                </span>
              ) : null}
            </li>
          );
        })}
      </ol>
    </>
  );

  if (embedded) {
    return (
      <div id={id} aria-label="Plan">
        {body}
      </div>
    );
  }

  return (
    <section id={id} aria-label="Plan" className="rounded-lg bg-zinc-900/70 p-3 ring-1 ring-zinc-800">
      {body}
    </section>
  );
}
