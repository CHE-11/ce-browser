import type { TabInfo } from "@type/browser";
import { cn } from "@/lib/cn";
import { inputClassName } from "@features/Agent/agent-styles";

type PageTabSelectProps = {
  id: string;
  tabs: TabInfo[];
  value: number | null;
  onChange: (tabId: number) => void;
  label: string;
  /** Shown when no web page is open. */
  emptyText?: string;
  className?: string;
};

/** Chooses one of the web pages open in this window. */
export function PageTabSelect({ id, tabs, value, onChange, label, emptyText = "Open the page in a tab first", className }: PageTabSelectProps) {
  return (
    <select
      id={id}
      className={cn(inputClassName, "min-w-0", className)}
      value={value ?? ""}
      disabled={tabs.length === 0}
      onChange={(event) => onChange(Number(event.target.value))}
      aria-label={label}
    >
      {tabs.length === 0 ? (
        <option id={`${id}-none`} value="">
          {emptyText}
        </option>
      ) : null}
      {tabs.map((tab) => (
        <option id={`${id}-${tab.id}`} key={tab.id} value={tab.id}>
          {tab.title || tab.url}
        </option>
      ))}
    </select>
  );
}

/** The chosen tab when it is still open, otherwise the most recently used one. */
export function pickedTab(tabs: TabInfo[], chosen: number | null) {
  return tabs.some((tab) => tab.id === chosen) ? chosen : (tabs[0]?.id ?? null);
}
