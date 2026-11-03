import type { ReactNode } from "react";
import { Toggle } from "../../components/Toggle";
import { cn } from "../../lib/cn";

type PopoverHeaderProps = {
  id: string;
  icon: ReactNode;
  title: string;
  subtitle: string;
  action?: ReactNode;
};

export function PopoverHeader({ id, icon, title, subtitle, action }: PopoverHeaderProps) {
  return (
    <div id={id} className="flex items-center gap-3 px-4 pb-2 pt-3.5">
      <span
        id={`${id}-icon`}
        className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg bg-sky-500/15 text-sky-300 [&_svg]:size-4"
      >
        {icon}
      </span>
      <div id={`${id}-text`} className="min-w-0 flex-1">
        <h2 id={`${id}-title`} className="truncate text-sm font-semibold">
          {title}
        </h2>
        <p id={`${id}-subtitle`} className="truncate text-xs text-zinc-500">
          {subtitle}
        </p>
      </div>
      {action ? <div id={`${id}-action`}>{action}</div> : null}
    </div>
  );
}

type OptionRowProps = {
  id: string;
  label: string;
  hint?: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
};

export function OptionRow({ id, label, hint, checked, disabled, onChange }: OptionRowProps) {
  return (
    <label
      id={id}
      className={cn("flex h-11 items-center gap-3 rounded-lg px-2.5 hover:bg-zinc-800/60", disabled && "opacity-40")}
    >
      <span id={`${id}-text`} className="min-w-0 flex-1">
        <span id={`${id}-label`} className="block truncate text-sm">
          {label}
        </span>
        {hint ? (
          <span id={`${id}-hint`} className="block truncate text-[11px] text-zinc-500">
            {hint}
          </span>
        ) : null}
      </span>
      <Toggle id={`${id}-toggle`} checked={checked} disabled={disabled} onChange={onChange} aria-label={label} />
    </label>
  );
}

type SegmentedProps<T extends string> = {
  id: string;
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
};

export function Segmented<T extends string>({ id, label, value, options, onChange }: SegmentedProps<T>) {
  return (
    <div id={id} role="radiogroup" aria-label={label} className="flex rounded-lg bg-zinc-800 p-0.5">
      {options.map((option) => (
        <button
          id={`${id}-${option.value}`}
          key={option.value}
          type="button"
          role="radio"
          aria-checked={option.value === value}
          className={cn(
            "h-7 flex-1 rounded-md px-2 text-xs font-medium text-zinc-400 hover:text-zinc-100",
            option.value === value && "bg-zinc-700 text-zinc-100 shadow-sm",
          )}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

type ActionTileProps = {
  id: string;
  icon: ReactNode;
  label: string;
  hint: string;
  primary?: boolean;
  disabled?: boolean;
  onClick: () => void;
};

/** A large two-line action button used in the built-in extension popovers. */
export function ActionTile({ id, icon, label, hint, primary, disabled, onClick }: ActionTileProps) {
  return (
    <button
      id={id}
      type="button"
      disabled={disabled}
      className={cn(
        "group flex h-14 w-full items-center gap-3 rounded-lg px-3 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-40",
        primary ? "bg-sky-600 text-white hover:bg-sky-500" : "bg-zinc-800 text-zinc-100 hover:bg-zinc-700",
      )}
      onClick={onClick}
    >
      <span
        id={`${id}-icon`}
        className={cn("inline-flex size-5 shrink-0 items-center justify-center [&_svg]:size-4", primary ? "text-white" : "text-zinc-400")}
      >
        {icon}
      </span>
      <span id={`${id}-text`} className="min-w-0 flex-1">
        <span id={`${id}-label`} className="block truncate text-sm font-medium">
          {label}
        </span>
        <span id={`${id}-hint`} className={cn("block truncate text-[11px]", primary ? "text-sky-100/80" : "text-zinc-500")}>
          {hint}
        </span>
      </span>
    </button>
  );
}
