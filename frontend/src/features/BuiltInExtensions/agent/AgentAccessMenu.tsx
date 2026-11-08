import { useCallback, useRef, useState } from "react";
import { AppWindow, Bell, Brain, ChevronDown, ClipboardCopy, Code, Globe, HardDrive, History, Layers, Lock, Monitor, Users } from "lucide-react";
import type { AgentAccess } from "@type/browser";
import { Toggle } from "@/components/Toggle";
import { ACCESS_TOGGLES, accessSummary, type AccessToggle } from "@/lib/agent-access";
import { cn } from "@/lib/cn";
import { useDismiss } from "@/lib/use-dismiss";

const ICONS: Record<AccessToggle, typeof Lock> = {
  tabs: Layers,
  browserData: History,
  files: HardDrive,
  clipboard: ClipboardCopy,
  memory: Brain,
  helpers: Users,
  scripts: Code,
  notifications: Bell,
  desktop: Monitor,
};

type Props = {
  id: string;
  access: AgentAccess;
  onChange: (access: AgentAccess) => void;
  /** Whether the menu changes the run on show or the next task. */
  scope: "run" | "next";
  /** Site of the selected tab, shown for "This site" before a run fills its own in. */
  currentSite: string;
  /** Settings that switch some access off for every run. */
  blockedBySettings: Partial<Record<AccessToggle, string>>;
};

/** The lock menu: which sites and which parts of the browser the agent may reach. */
export function AgentAccessMenu({ id, access, onChange, scope, currentSite, blockedBySettings }: Props) {
  const root = useRef<HTMLDivElement | null>(null);
  const [open, setOpen] = useState(false);
  const close = useCallback(() => setOpen(false), []);
  useDismiss(root, open, close);
  const site = access.site || currentSite;
  const narrowed = access.sites === "site" || ACCESS_TOGGLES.some((item) => item.key !== "scripts" && item.key !== "desktop" && !access[item.key]);

  const siteOption = (value: AgentAccess["sites"], Icon: typeof Lock, label: string, hint: string) => {
    const selected = access.sites === value;
    return (
      <button
        id={`${id}-sites-${value}`}
        type="button"
        role="radio"
        aria-checked={selected}
        className={cn(
          "flex w-full items-start gap-2 rounded-md px-2 py-1.5 text-left ring-1 ring-transparent hover:bg-zinc-800/70",
          selected && "bg-zinc-800 ring-sky-800",
        )}
        onClick={() => onChange({ ...access, sites: value, site: value === "site" ? access.site : "" })}
      >
        <Icon id={`${id}-sites-${value}-icon`} className={cn("mt-0.5 size-3.5 shrink-0", selected ? "text-sky-400" : "text-zinc-500")} aria-hidden />
        <span id={`${id}-sites-${value}-body`} className="min-w-0">
          <span id={`${id}-sites-${value}-label`} className="block text-xs text-zinc-100">
            {label}
          </span>
          <span id={`${id}-sites-${value}-hint`} className="block text-[11px] text-zinc-500">
            {hint}
          </span>
        </span>
      </button>
    );
  };

  return (
    <div id={id} ref={root} className="relative shrink-0">
      <button
        id={`${id}-button`}
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        title={`Access: ${accessSummary(access)}`}
        className={cn(
          "flex h-7 max-w-36 items-center gap-1 rounded-md px-1.5 text-[11px] ring-1 ring-zinc-800 hover:bg-zinc-800",
          narrowed ? "bg-amber-950/40 text-amber-200" : "bg-zinc-900 text-zinc-300",
        )}
        onClick={() => setOpen((value) => !value)}
      >
        <Lock id={`${id}-button-icon`} className="size-3 shrink-0" aria-hidden />
        <span id={`${id}-button-label`} className="truncate">
          {access.sites === "site" ? site || "This site" : "Browser"}
        </span>
        <ChevronDown id={`${id}-button-chevron`} className="size-3 shrink-0 opacity-60" aria-hidden />
      </button>
      {open ? (
        <div
          id={`${id}-popover`}
          role="dialog"
          aria-label="What the agent can reach"
          data-agent-popover
          className="absolute left-0 top-full z-40 mt-1 w-[min(18rem,calc(100vw-1.5rem))] rounded-lg bg-zinc-900 p-2 shadow-2xl ring-1 ring-zinc-700"
        >
          <div id={`${id}-title`} className="mb-1.5 flex items-baseline justify-between px-1">
            <span id={`${id}-title-text`} className="text-xs font-semibold text-zinc-100">
              What the agent can reach
            </span>
            <span id={`${id}-scope`} className="text-[10px] text-zinc-500">
              {scope === "run" ? "This run, now" : "Next task"}
            </span>
          </div>
          <div id={`${id}-sites`} role="radiogroup" aria-label="Websites" className="mb-2 flex flex-col gap-1">
            {siteOption("site", AppWindow, site ? `This site: ${site}` : "This site", "Only pages on this site. Leaving it is refused.")}
            {siteOption("browser", Globe, "Whole browser", "Any site automation may use. Blocked sites stay off.")}
          </div>
          <ul id={`${id}-toggles`} className="flex flex-col border-t border-zinc-800 pt-1.5">
            {ACCESS_TOGGLES.filter((item) => item.key !== "desktop" || !blockedBySettings.desktop).map((item) => {
              const Icon = ICONS[item.key];
              const blocked = blockedBySettings[item.key];
              const needsTabs = item.key === "helpers" && !access.tabs;
              return (
                <li id={`${id}-${item.key}`} key={item.key} className="flex items-center gap-2 rounded-md px-1 py-1" title={blocked ?? item.hint}>
                  <Icon id={`${id}-${item.key}-icon`} className="size-3.5 shrink-0 text-zinc-500" aria-hidden />
                  <span id={`${id}-${item.key}-body`} className="min-w-0 flex-1">
                    <span id={`${id}-${item.key}-label`} className={cn("block text-xs", blocked ? "text-zinc-500" : "text-zinc-100")}>
                      {item.label}
                    </span>
                    <span id={`${id}-${item.key}-hint`} className="block truncate text-[10px] text-zinc-500">
                      {blocked ?? (needsTabs ? "Turn on other tabs to use helpers." : item.hint)}
                    </span>
                  </span>
                  <Toggle
                    id={`${id}-${item.key}-toggle`}
                    size="sm"
                    checked={access[item.key] && !blocked}
                    disabled={!!blocked}
                    aria-label={item.label}
                    onChange={(checked) => onChange({ ...access, [item.key]: checked })}
                  />
                </li>
              );
            })}
          </ul>
          <p id={`${id}-footer`} className="mt-1.5 border-t border-zinc-800 px-1 pt-1.5 text-[10px] text-zinc-500">
            The browser&apos;s own pages and sites blocked in Settings are always off.
          </p>
        </div>
      ) : null}
    </div>
  );
}
