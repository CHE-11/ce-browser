import type { ExtensionOpenAs } from "@type/browser";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";

type OpenAsProps = {
  id: string;
  value: ExtensionOpenAs;
  disabled?: boolean;
  onChange: (value: ExtensionOpenAs) => void;
};

/** Popover opens under the icon. Panel docks beside the page. */
export function OpenAs({ id, value, disabled, onChange }: OpenAsProps) {
  const options = [
    { value: "popover" as const, label: "Popover" },
    { value: "panel" as const, label: "Panel" },
  ];
  return (
    <div
      id={id}
      role="group"
      aria-label="Open as"
      title="Popover opens under the icon. Panel docks beside the page."
      className={cn("flex w-fit rounded-md border border-zinc-700 p-0.5", disabled && "opacity-40")}
    >
      {options.map((option) => (
        <button
          key={option.value}
          id={`${id}-${option.value}`}
          type="button"
          aria-pressed={value === option.value}
          disabled={disabled}
          className={cn(
            "rounded px-2 py-1 text-xs text-zinc-400 hover:text-zinc-100 disabled:hover:text-zinc-400",
            focusRing,
            value === option.value && "bg-zinc-700 text-zinc-100",
          )}
          onClick={() => {
            if (value !== option.value) {
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
