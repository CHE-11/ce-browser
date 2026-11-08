import { PinOff } from "lucide-react";
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
    { value: "primary" as const, label: "P", name: "Primary", pressed: placement.primary, title: "Primary: show next to the address bar" },
    { value: "secondary" as const, label: "S", name: "Secondary", pressed: placement.secondary, title: "Secondary: show on the tools toolbar" },
    { value: "unpinned" as const, label: "", name: "Unpin", pressed: unpinned, title: "Unpin" },
  ];
  return (
    <div id={id} role="group" aria-label="Where to show this" className={cn("flex w-fit rounded-md border border-zinc-700 p-0.5", disabled && "opacity-40")}>
      {options.map((option) => (
        <button
          key={option.value}
          id={`${id}-${option.value}`}
          type="button"
          aria-pressed={option.pressed}
          aria-label={option.name}
          disabled={disabled}
          title={option.title}
          className={cn(
            "grid size-6 place-items-center rounded text-xs text-zinc-400 hover:text-zinc-100 disabled:hover:text-zinc-400",
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
          {option.value === "unpinned" ? (
            <PinOff id={`${id}-unpinned-icon`} className="size-3.5" aria-hidden />
          ) : (
            option.label
          )}
        </button>
      ))}
    </div>
  );
}
