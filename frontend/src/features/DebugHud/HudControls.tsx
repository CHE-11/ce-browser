import type { ReactNode } from "react";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";

type HudButtonProps = {
  id: string;
  title?: string;
  onClick: () => void;
  children: ReactNode;
  className?: string;
  disabled?: boolean;
};

export function HudButton({ id, title, onClick, children, className, disabled }: HudButtonProps) {
  return (
    <button
      id={id}
      type="button"
      title={title}
      disabled={disabled}
      className={cn(
        "inline-flex h-6 shrink-0 items-center gap-1 rounded px-1.5 text-[11px] text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 disabled:opacity-40",
        focusRing,
        className,
      )}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

type HudToggleProps = {
  id: string;
  pressed: boolean;
  onChange: (pressed: boolean) => void;
  children: ReactNode;
  title?: string;
};

export function HudToggle({ id, pressed, onChange, children, title }: HudToggleProps) {
  return (
    <button
      id={id}
      type="button"
      title={title}
      aria-pressed={pressed}
      className={cn(
        "inline-flex h-6 shrink-0 items-center gap-1 rounded px-1.5 text-[11px]",
        pressed ? "bg-sky-500/15 text-sky-200" : "text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100",
        focusRing,
      )}
      onClick={() => onChange(!pressed)}
    >
      {children}
    </button>
  );
}

type HudLiveToggleProps = {
  id: string;
  live: boolean;
  onChange: (live: boolean) => void;
};

/** Pauses a list in place. New items keep arriving and show when it goes live again. */
export function HudLiveToggle({ id, live, onChange }: HudLiveToggleProps) {
  return (
    <button
      id={id}
      type="button"
      aria-pressed={live}
      className={cn(
        "inline-flex h-6 shrink-0 items-center gap-1.5 rounded px-1.5 text-[11px]",
        live ? "text-emerald-300 hover:bg-zinc-800" : "text-amber-300 hover:bg-zinc-800",
        focusRing,
      )}
      onClick={() => onChange(!live)}
    >
      <span id={`${id}-dot`} className={cn("size-1.5 rounded-full", live ? "bg-emerald-400" : "bg-amber-400")} />
      {live ? "Live" : "Paused"}
    </button>
  );
}

type HudSearchProps = {
  id: string;
  value: string;
  placeholder: string;
  onChange: (value: string) => void;
};

export function HudSearch({ id, value, placeholder, onChange }: HudSearchProps) {
  return (
    <label id={id} className="flex h-6 min-w-28 flex-1 items-center rounded border border-zinc-800 bg-zinc-950 px-1.5 focus-within:border-sky-500">
      <input
        id={`${id}-input`}
        type="search"
        value={value}
        placeholder={placeholder}
        spellCheck={false}
        className="min-w-0 flex-1 bg-transparent text-[11px] text-zinc-100 outline-none placeholder:text-zinc-500"
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}

type HudSelectProps = {
  id: string;
  value: string;
  options: { value: string; label: string }[];
  onChange: (value: string) => void;
  title?: string;
};

export function HudSelect({ id, value, options, onChange, title }: HudSelectProps) {
  return (
    <select
      id={id}
      value={value}
      title={title}
      className={cn("h-6 max-w-36 shrink-0 rounded border border-zinc-800 bg-zinc-950 px-1 text-[11px] text-zinc-200 outline-none focus:border-sky-500")}
      onChange={(event) => onChange(event.target.value)}
    >
      {options.map((option) => (
        <option id={`${id}-${option.value || "all"}`} key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  );
}

export function HudToolbar({ id, children }: { id: string; children: ReactNode }) {
  return (
    <div id={id} className="flex flex-wrap items-center gap-1 border-b border-zinc-800 px-2 py-1.5">
      {children}
    </div>
  );
}

export function HudEmpty({ id, children }: { id: string; children: ReactNode }) {
  return (
    <p id={id} className="px-3 py-4 text-xs text-zinc-500">
      {children}
    </p>
  );
}
