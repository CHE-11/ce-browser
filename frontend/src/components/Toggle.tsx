import type { ButtonHTMLAttributes } from "react";
import { cn } from "../lib/cn";

type ToggleProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "id" | "onClick" | "onChange"> & {
  id: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
};

export function Toggle({ id, checked, onChange, className, disabled, ...props }: ToggleProps) {
  return (
    <button
      id={id}
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      className={cn(
        "inline-flex h-5 w-9 shrink-0 items-center rounded-full p-0.5 transition-colors disabled:cursor-not-allowed disabled:opacity-40",
        checked ? "bg-sky-600" : "bg-zinc-700",
        className,
      )}
      {...props}
      onClick={() => onChange(!checked)}
    >
      <span
        id={`${id}-thumb`}
        aria-hidden
        className={cn(
          "pointer-events-none size-4 rounded-full bg-white transition-transform",
          checked ? "translate-x-4" : "translate-x-0",
        )}
      />
    </button>
  );
}
