import { createContext, useContext, useEffect, type ReactNode } from "react";
import type { BrowserSettings } from "@type/browser";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";

/** A row of mutually exclusive buttons. */
export function Segmented<T extends string>({
  id,
  label,
  value,
  options,
  disabled,
  onChange,
}: {
  id: string;
  label: string;
  value: T;
  options: { value: T; label: string }[];
  disabled?: boolean;
  onChange: (value: T) => void;
}) {
  return (
    <div id={id} role="radiogroup" aria-label={label} className={cn("flex rounded-md border border-zinc-700 p-0.5", disabled && "opacity-40")}>
      {options.map((option) => (
        <button
          key={option.value}
          id={`${id}-${option.value}`}
          type="button"
          role="radio"
          aria-checked={value === option.value}
          disabled={disabled}
          className={cn(
            "rounded px-3 py-1 text-xs text-zinc-400 transition-colors hover:text-zinc-100 disabled:hover:text-zinc-400",
            focusRing,
            value === option.value && "bg-zinc-700 text-zinc-100",
          )}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

export const inputClass =
  "h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500";

const SettingsQueryContext = createContext("");
const SectionMatchContext = createContext(false);

export type SettingsSectionProps = {
  settings: BrowserSettings;
  query: string;
  onUpdate: (patch: Partial<BrowserSettings>) => void;
  onError: (message: string | null) => void;
  onMatch: (id: string, matches: boolean) => void;
};

export function SettingsQueryProvider({ query, children }: { query: string; children: ReactNode }) {
  return <SettingsQueryContext.Provider value={query}>{children}</SettingsQueryContext.Provider>;
}

export function matchesQuery(query: string, ...parts: Array<string | number | undefined | null | false>) {
  const needle = query.trim().toLowerCase();
  if (!needle) {
    return true;
  }
  return parts.some((part) => part !== undefined && part !== null && part !== false && String(part).toLowerCase().includes(needle));
}

export function useSettingsQuery() {
  return useContext(SettingsQueryContext);
}

export function useSectionMatches() {
  return useContext(SectionMatchContext);
}

export function useSettingVisible(...parts: Array<string | number | undefined | null | false>) {
  const query = useSettingsQuery();
  const sectionMatches = useSectionMatches();
  return sectionMatches || matchesQuery(query, ...parts);
}

export function useReportMatch(id: string, matches: boolean, onMatch: (id: string, matches: boolean) => void) {
  useEffect(() => {
    onMatch(id, matches);
  }, [id, matches, onMatch]);
}

export function errorText(reason: unknown) {
  if (typeof reason === "string" && reason.trim()) {
    return reason;
  }
  if (reason instanceof Error && reason.message.trim()) {
    return reason.message;
  }
  if (reason && typeof reason === "object" && "message" in reason) {
    const message = reason.message;
    if (typeof message === "string" && message.trim()) {
      return message;
    }
  }
  try {
    const text = JSON.stringify(reason);
    if (text && text !== "null" && text !== "{}") {
      return text;
    }
  } catch {
    // A circular value still needs a readable fallback below.
  }
  return "Unknown error";
}

export function Section({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  const query = useSettingsQuery();
  const showAll = matchesQuery(query, title);
  return (
    <SectionMatchContext.Provider value={showAll}>
      <section id={id} className="flex scroll-mt-4 flex-col gap-1 rounded-lg border border-zinc-800 bg-zinc-900/60">
        <h2 id={`${id}-title`} className="px-5 pt-4 text-sm font-semibold text-zinc-200">
          {title}
        </h2>
        <div id={`${id}-rows`} className="flex flex-col divide-y divide-zinc-800">
          {children}
        </div>
      </section>
    </SectionMatchContext.Provider>
  );
}

export function Row({
  id,
  label,
  hint,
  search,
  children,
}: {
  id: string;
  label: string;
  hint?: string;
  search?: Array<string | undefined | null | false>;
  children: ReactNode;
}) {
  const visible = useSettingVisible(label, hint, ...(search ?? []));
  if (!visible) {
    return null;
  }
  return (
    <div id={id} className="flex items-center justify-between gap-6 px-5 py-3">
      <div id={`${id}-text`} className="min-w-0">
        <div id={`${id}-label`} className="text-sm text-zinc-100">
          {label}
        </div>
        {hint ? (
          <div id={`${id}-hint`} className="text-xs text-zinc-500">
            {hint}
          </div>
        ) : null}
      </div>
      <div id={`${id}-control`} className="flex w-80 shrink-0 items-center justify-end gap-2">
        {children}
      </div>
    </div>
  );
}
