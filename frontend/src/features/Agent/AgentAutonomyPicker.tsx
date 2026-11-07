import { Hand, Rocket, Zap } from "lucide-react";
import type { AgentAutonomy } from "@type/browser";
import { AUTONOMY_LEVELS } from "@/lib/agent-autonomy";
import { cn } from "@/lib/cn";

const ICONS: Record<AgentAutonomy, typeof Hand> = {
  ask: Hand,
  auto: Zap,
  full: Rocket,
};

const ACTIVE: Record<AgentAutonomy, string> = {
  ask: "bg-zinc-700 text-zinc-50",
  auto: "bg-sky-700 text-white",
  full: "bg-fuchsia-700 text-white",
};

type Props = {
  id: string;
  value: AgentAutonomy;
  onChange: (value: AgentAutonomy) => void;
  disabled?: boolean;
};

/** Ask, Auto, or Full auto: how much the agent does before it asks. */
export function AgentAutonomyPicker({ id, value, onChange, disabled }: Props) {
  return (
    <div id={id} role="radiogroup" aria-label="Autonomy" className="flex shrink-0 rounded-md bg-zinc-900 p-0.5 ring-1 ring-zinc-800">
      {AUTONOMY_LEVELS.map((level) => {
        const Icon = ICONS[level.value];
        const selected = level.value === value;
        return (
          <button
            id={`${id}-${level.value}`}
            key={level.value}
            type="button"
            role="radio"
            aria-checked={selected}
            disabled={disabled}
            title={`${level.label}: ${level.hint}`}
            className={cn(
              "flex h-6 items-center gap-1 rounded px-1.5 text-[11px] font-medium text-zinc-400 transition-colors hover:text-zinc-100 disabled:opacity-40",
              selected && ACTIVE[level.value],
            )}
            onClick={() => onChange(level.value)}
          >
            <Icon id={`${id}-${level.value}-icon`} className="size-3" aria-hidden />
            {level.value === "full" ? "Full" : level.label}
          </button>
        );
      })}
    </div>
  );
}
