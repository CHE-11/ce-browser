import type { HostPanelSide } from "@type/browser";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";

type PanelSideProps = {
  id: string;
  side: HostPanelSide;
  disabled?: boolean;
  onChange: (side: HostPanelSide) => void;
};

/** Left or right edge for a docked panel. */
export function PanelSide({ id, side, disabled, onChange }: PanelSideProps) {
  const options = [
    { value: "left" as const, label: "L", name: "Left" },
    { value: "right" as const, label: "R", name: "Right" },
  ];
  return (
    <div
      id={id}
      role="group"
      aria-label="Panel side"
      title="Left or right edge of the page"
      className={cn("flex w-fit rounded-md border border-zinc-700 p-0.5", disabled && "opacity-40")}
    >
      {options.map((option) => (
        <button
          key={option.value}
          id={`${id}-${option.value}`}
          type="button"
          aria-pressed={side === option.value}
          aria-label={option.name}
          title={option.name}
          disabled={disabled}
          className={cn(
            "grid size-6 place-items-center rounded text-xs text-zinc-400 hover:text-zinc-100 disabled:hover:text-zinc-400",
            focusRing,
            side === option.value && "bg-zinc-700 text-zinc-100",
          )}
          onClick={() => {
            if (side !== option.value) {
              onChange(option.value);
            }
          }}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
