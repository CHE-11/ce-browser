import type { ToolbarPlacement } from "@type/browser";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";

type PinPlacementProps = {
  id: string;
  placement: ToolbarPlacement;
  disabled?: boolean;
  onChange: (placement: ToolbarPlacement) => void;
};

export function PinPlacement({ id, placement, disabled, onChange }: PinPlacementProps) {
  const unpinned = !placement.primary && !placement.secondary;
  const options = [
    { value: "primary" as const, label: "Primary", pressed: placement.primary, title: "Show next to the address bar" },
    { value: "secondary" as const, label: "Secondary", pressed: placement.secondary, title: "Show on the tools toolbar" },
    { value: "unpinned" as const, label: "Unpinned", pressed: unpinned, title: "Hide from both bars" },
  ];
  return (
    <div id={id} role="group" aria-label="Where to show this" className={cn("flex w-fit rounded-md border border-zinc-700 p-0.5", disabled && "opacity-40")}>
      {options.map((option) => (
        <button
          key={option.value}
          id={`${id}-${option.value}`}
          type="button"
          aria-pressed={option.pressed}
          disabled={disabled}
          title={option.title}
          className={cn(
            "rounded px-2 py-1 text-xs text-zinc-400 hover:text-zinc-100 disabled:hover:text-zinc-400",
            focusRing,
            option.pressed && "bg-zinc-700 text-zinc-100",
          )}
          onClick={() => {
            if (option.value === "unpinned") {
              if (!unpinned) {
                onChange({ primary: false, secondary: false });
              }
              return;
            }
            onChange({ ...placement, [option.value]: !placement[option.value] });
          }}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
