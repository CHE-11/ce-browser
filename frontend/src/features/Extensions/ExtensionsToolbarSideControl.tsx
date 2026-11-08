import { PanelBottom, PanelLeft, PanelRight, PanelTop, type LucideIcon } from "lucide-react";
import type { ToolbarSide } from "@type/browser";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";

const SIDES: { side: ToolbarSide; label: string; icon: LucideIcon; cell: string }[] = [
  { side: "top", label: "Top", icon: PanelTop, cell: "col-start-2 row-start-1" },
  { side: "left", label: "Left", icon: PanelLeft, cell: "col-start-1 row-start-2" },
  { side: "right", label: "Right", icon: PanelRight, cell: "col-start-3 row-start-2" },
  { side: "bottom", label: "Bottom", icon: PanelBottom, cell: "col-start-2 row-start-3" },
];

type ExtensionsToolbarSideControlProps = {
  id: string;
  value: ToolbarSide;
  disabled?: boolean;
  /** Fits the tools strip. The extensions page keeps the larger control. */
  compact?: boolean;
  onChange: (side: ToolbarSide) => void;
};

/** Four edges around a page. The selected edge is where the tools toolbar sits. */
export function ExtensionsToolbarSideControl({ id, value, disabled, compact = false, onChange }: ExtensionsToolbarSideControlProps) {
  return (
    <div
      id={id}
      role="radiogroup"
      aria-label="Toolbar side"
      className={cn(
        "grid w-fit shrink-0 place-items-center",
        compact
          ? "grid-cols-[0.75rem_0.75rem_0.75rem] grid-rows-[0.75rem_0.75rem_0.75rem] gap-px"
          : "grid-cols-[1.75rem_1.75rem_1.75rem] grid-rows-[1.75rem_1.75rem_1.75rem] gap-0.5",
        disabled && "opacity-40",
      )}
    >
      <div
        id={`${id}-page`}
        aria-hidden
        className={cn(
          "col-start-2 row-start-2 rounded-[2px] border border-zinc-600 bg-zinc-800",
          compact ? "h-[7px] w-[9px]" : "h-3 w-4",
        )}
      />
      {SIDES.map((option) => {
        const selected = value === option.side;
        const Icon = option.icon;
        return (
          <button
            key={option.side}
            id={`${id}-${option.side}`}
            type="button"
            role="radio"
            aria-checked={selected}
            aria-label={option.label}
            title={`Show on the ${option.label.toLowerCase()}`}
            disabled={disabled}
            className={cn(
              "inline-flex items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 disabled:hover:bg-transparent disabled:hover:text-zinc-400",
              compact ? "size-3" : "size-7",
              option.cell,
              focusRing,
              selected && "bg-zinc-700 text-zinc-100",
            )}
            onClick={() => onChange(option.side)}
          >
            <Icon id={`${id}-${option.side}-icon`} className={compact ? "size-2" : "size-3.5"} aria-hidden />
          </button>
        );
      })}
    </div>
  );
}
