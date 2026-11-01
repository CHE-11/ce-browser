import type { ButtonHTMLAttributes } from "react";
import { cn } from "../lib/cn";

type ToggleProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "id" | "onClick" | "onChange"> & {
  id: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  size?: "sm" | "md";
};

export function Toggle({ id, checked, onChange, className, disabled, size = "md", ...props }: ToggleProps) {
  const small = size === "sm";
  return (
    <button
      id={id}
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      className={cn(
        "inline-flex shrink-0 items-center rounded-full transition-colors disabled:cursor-not-allowed disabled:opacity-40",
        small ? "h-3 w-5 p-px" : "h-5 w-9 p-0.5",
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
          "pointer-events-none rounded-full bg-white transition-transform",
          small ? "size-2" : "size-4",
          checked ? (small ? "translate-x-2.5" : "translate-x-4") : "translate-x-0",
        )}
      />
    </button>
  );
}
