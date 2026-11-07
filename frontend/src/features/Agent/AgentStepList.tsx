import { useEffect, useRef, useState } from "react";
import { AlertTriangle, Brain, CheckCircle2, MousePointerClick, ShieldAlert, User } from "lucide-react";
import type { AgentStep, AgentStepKind } from "@type/browser";
import { cn } from "@/lib/cn";

const ICONS: Record<AgentStepKind, typeof Brain> = {
  think: Brain,
  action: MousePointerClick,
  blocked: ShieldAlert,
  error: AlertTriangle,
  user: User,
  answer: CheckCircle2,
};

const TONES: Record<AgentStepKind, string> = {
  think: "text-zinc-400",
  action: "text-sky-400",
  blocked: "text-amber-400",
  error: "text-red-400",
  user: "text-violet-300",
  answer: "text-emerald-400",
};

function time(at: number) {
  return new Date(at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

export function AgentStepList({ id, steps }: { id: string; steps: AgentStep[] }) {
  const end = useRef<HTMLLIElement | null>(null);
  const [zoomed, setZoomed] = useState<number | null>(null);

  useEffect(() => {
    end.current?.scrollIntoView({ block: "end" });
  }, [steps.length]);

  return (
    <ol id={id} className="flex flex-col gap-2">
      {steps.map((step, index) => {
        const Icon = ICONS[step.kind] ?? Brain;
        return (
          <li id={`${id}-${index}`} key={`${step.at}-${index}`} className="flex gap-2 rounded-md bg-zinc-900/70 p-2 ring-1 ring-zinc-800">
            <Icon id={`${id}-${index}-icon`} className={cn("mt-0.5 size-4 shrink-0", TONES[step.kind])} aria-hidden />
            <div id={`${id}-${index}-body`} className="min-w-0 flex-1">
              <div id={`${id}-${index}-text`} className={cn("whitespace-pre-wrap break-words text-xs", step.kind === "think" ? "text-zinc-400" : "text-zinc-100")}>
                {step.text}
              </div>
              {step.detail ? (
                <div id={`${id}-${index}-detail`} className="mt-1 whitespace-pre-wrap break-words text-[11px] text-zinc-500">
                  {step.detail}
                </div>
              ) : null}
              {step.thumb ? (
                <button
                  id={`${id}-${index}-thumb`}
                  type="button"
                  className="mt-1.5 block overflow-hidden rounded ring-1 ring-zinc-700"
                  title={zoomed === index ? "Shrink" : "Enlarge"}
                  onClick={() => setZoomed(zoomed === index ? null : index)}
                >
                  <img id={`${id}-${index}-thumb-image`} src={step.thumb} alt="Page after this step" className={cn("block", zoomed === index ? "w-full" : "w-40")} />
                </button>
              ) : null}
            </div>
            <time id={`${id}-${index}-time`} className="shrink-0 text-[10px] text-zinc-600">
              {time(step.at)}
            </time>
          </li>
        );
      })}
      <li id={`${id}-end`} ref={end} aria-hidden className="h-px" />
    </ol>
  );
}
