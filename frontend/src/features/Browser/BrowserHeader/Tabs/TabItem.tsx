import type { DragEvent, MouseEvent } from "react";
import { Glasses, Moon, TriangleAlert, Volume2, VolumeX, X } from "lucide-react";
import { FavIcon } from "@/components/FavIcon";
import { setTabsMuted, selectTabRange, toggleTabSelection, activateTab } from "@/lib/browser-client";
import { accentTint } from "@/lib/accent";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { tabProblemTitle, type HeavyTab } from "@/lib/tab-alert";
import { TAB_SIZE } from "@/lib/tab-size";
import { useSettings } from "@/lib/use-browser";
import { hoverTab, leaveTab } from "@/lib/tab-hover";
import { groupColorClasses } from "@/lib/tab-rows";
import { TabInfo } from "@type/browser";
import { TabAlertDot } from "./TabAlertDot";

/** These palettes already separate the strip (zinc-950) from the active tab. The rest keep those two steps too close. */
const QUIET_TAB_THEMES = new Set(["nord", "fog", "solarized"]);
const ACTIVE_TAB_FILL = "color-mix(in oklab, var(--app-zinc-950) 65%, var(--app-zinc-50))";

type TabItemProps = {
  tab: TabInfo;
  active: boolean;
  selected: boolean;
  /** Part of a multi-tab selection. */
  grouped: boolean;
  orientation: "horizontal" | "vertical";
  /** Icon only: pinned tabs, or the collapsed vertical sidebar. */
  compact: boolean;
  groupColor: string | null;
  /** This tab's renderer is over the memory mark. */
  heavy: HeavyTab | null;
  split: "left" | "right" | null;
  drop: "before" | "after" | null;
  onContextMenu: (event: MouseEvent) => void;
  onClose: () => void;
  onDragStart: () => void;
  onDragEnd: (event: DragEvent) => void;
  onDragOver: (event: DragEvent) => void;
};

export function TabItem({
  tab,
  active,
  selected,
  grouped,
  orientation,
  compact,
  groupColor,
  heavy,
  split,
  drop,
  onContextMenu,
  onClose,
  onDragStart,
  onDragEnd,
  onDragOver,
}: TabItemProps) {
  const settings = useSettings();
  const quietActive = QUIET_TAB_THEMES.has(settings?.theme ?? document.documentElement.dataset.theme ?? "");
  const hoverPreview = settings?.tabHoverPreview !== false;
  const tabWidth = settings?.tabWidth ?? TAB_SIZE.width;
  const accent = settings?.siteAccent && tab.accent && !tab.pinned ? tab.accent : null;
  const pinBorder = (settings?.pinTabBorder ?? true) && tab.pinned ? tab.accent : null;
  const domId = `tab-${tab.id}`;
  const vertical = orientation === "vertical";
  const showAudio = tab.audible || tab.muted;
  const alert = tabProblemTitle(tab, heavy);
  const accentFill = accent && active ? accentTint(accent, 0.28) : undefined;
  const chromeTab = !vertical && active;
  const backgroundColor =
    accentFill ?? (active && !quietActive ? ACTIVE_TAB_FILL : chromeTab && quietActive ? "var(--color-zinc-900)" : undefined);

  const toggleMute = (event: MouseEvent) => {
    event.stopPropagation();
    void setTabsMuted([tab.id], !tab.muted);
  };

  const audioIcon = tab.muted ? (
    <VolumeX id={`${domId}-audio-muted`} className={compact ? "size-2" : "size-3.5"} aria-hidden />
  ) : (
    <Volume2 id={`${domId}-audio-playing`} className={cn(compact ? "size-2" : "size-3.5", "text-sky-400")} aria-hidden />
  );
  const audioButton = showAudio ? (
    compact ? (
      <button
        id={`${domId}-audio`}
        type="button"
        title={tab.muted ? "Unmute tab" : "Mute tab"}
        aria-label={tab.muted ? `Unmute ${tab.title}` : `Mute ${tab.title}`}
        className={cn(
          "absolute -right-1.5 -top-1.5 z-10 inline-flex size-3.5 items-center justify-center rounded-full bg-zinc-950 text-zinc-300 ring-1 ring-zinc-700 hover:bg-zinc-800 hover:text-zinc-100",
          focusRing,
        )}
        onMouseDown={(event) => event.stopPropagation()}
        onClick={toggleMute}
      >
        {audioIcon}
      </button>
    ) : (
      <button
        id={`${domId}-audio`}
        type="button"
        title={tab.muted ? "Unmute tab" : "Mute tab"}
        aria-label={tab.muted ? `Unmute ${tab.title}` : `Mute ${tab.title}`}
        className={cn(
          "inline-flex size-5 shrink-0 items-center justify-center rounded text-zinc-400 hover:bg-zinc-700 hover:text-zinc-100",
          focusRing,
        )}
        onMouseDown={(event) => event.stopPropagation()}
        onClick={toggleMute}
      >
        {audioIcon}
      </button>
    )
  ) : null;

  return (
    <div
      id={domId}
      role="tab"
      aria-selected={selected}
      aria-label={`${tab.title || tab.url}${tab.sleeping ? ", asleep" : ""}${tab.audible ? ", playing sound" : ""}${alert ? `, ${alert}` : ""}`}
      title={!hoverPreview && alert ? alert : undefined}
      draggable
      tabIndex={0}
      onMouseEnter={(event) => {
        if (hoverPreview) {
          hoverTab(tab.id, event.currentTarget, vertical);
        }
      }}
      onMouseLeave={() => leaveTab()}
      className={cn(
        "group relative flex shrink-0 cursor-default select-none items-center gap-2 text-[11px] transition-colors",
        focusRing,
        vertical ? "h-8 w-full rounded-lg px-2" : "h-7.5 rounded-t-lg px-2.5",
        chromeTab && "tab-curve z-10",
        !vertical && (compact ? "w-9 justify-center px-0" : "min-w-22 shrink"),
        vertical && compact && "w-9 justify-center px-0",
        active
          ? quietActive
            ? "bg-zinc-800 text-zinc-100"
            : "text-zinc-50"
          : "text-zinc-400 hover:bg-zinc-900/60 hover:text-zinc-200",
        quietActive && !vertical && active && "bg-zinc-900",
        grouped && !active && "bg-zinc-800 text-zinc-100",
        !tab.loaded && !active && "opacity-80",
        tab.sleeping && !active && "opacity-55",
        split && "ring-1 ring-inset ring-sky-500/50",
        split === "left" && (vertical ? "rounded-b-none" : "rounded-tr-none"),
        split === "right" && (vertical ? "rounded-t-none" : "rounded-tl-none"),
      )}
      style={{
        viewTransitionName: `tab-${tab.id}`,
        ...(!vertical && !compact ? { width: tabWidth } : null),
        ...(backgroundColor ? { backgroundColor, "--tab-fill": backgroundColor } : null),
        ...(accent && vertical ? { boxShadow: `inset 3px 0 0 ${accent}` } : null),
        ...(pinBorder ? { boxShadow: `inset 0 2px 0 ${pinBorder}` } : null),
      }}
      onMouseDown={(event) => {
        leaveTab(true);
        if (event.button !== 0) {
          return;
        }
        if (event.shiftKey && !event.ctrlKey && !event.metaKey) {
          event.preventDefault();
          void selectTabRange(tab.id);
          return;
        }
        if (event.ctrlKey || event.metaKey) {
          event.preventDefault();
          void toggleTabSelection(tab.id);
          return;
        }
        void activateTab(tab.id);
      }}
      onAuxClick={(event) => {
        if (event.button === 1) {
          event.preventDefault();
          onClose();
        }
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          void activateTab(tab.id);
        }
      }}
      onContextMenu={onContextMenu}
      onDragStart={(event) => {
        event.dataTransfer.effectAllowed = "move";
        event.dataTransfer.setData("text/plain", tab.url);
        onDragStart();
      }}
      onDragEnd={onDragEnd}
      onDragOver={onDragOver}
    >
      {drop === "before" ? (
        <span
          id={`${domId}-drop-before`}
          className={cn("absolute rounded bg-sky-500", vertical ? "inset-x-1 -top-px h-0.5" : "inset-y-1 -left-px w-0.5")}
        />
      ) : null}
      {drop === "after" ? (
        <span
          id={`${domId}-drop-after`}
          className={cn("absolute rounded bg-sky-500", vertical ? "inset-x-1 -bottom-px h-0.5" : "inset-y-1 -right-px w-0.5")}
        />
      ) : null}
      {grouped ? (
        <span
          id={`${domId}-selected`}
          className={cn("absolute rounded-full bg-sky-400", vertical ? "inset-y-1.5 right-0 w-0.5" : "inset-x-1.5 top-0 h-0.5")}
        />
      ) : null}
      {groupColor ? (
        <span
          id={`${domId}-group-line`}
          className={cn(
            "absolute rounded-full",
            groupColorClasses(groupColor).line,
            vertical ? "inset-y-1 left-0 w-0.5" : "inset-x-1 bottom-0 h-0.5",
          )}
        />
      ) : null}
      <span id={`${domId}-icon-wrap`} className="relative inline-flex shrink-0">
        {tab.failure ? (
          <TriangleAlert id={`${domId}-failed`} className="size-4 shrink-0 text-amber-400" aria-label="Page failed to load" />
        ) : (
          <FavIcon id={`${domId}-icon`} url={tab.url} src={tab.favicon} className={cn("transition-transform", tab.loading && "scale-75")} />
        )}
        {tab.loading ? (
          <span
            id={`${domId}-loading`}
            aria-hidden
            className="pointer-events-none absolute -inset-0.5 animate-spin rounded-full border-2 border-sky-400/25 border-t-sky-400"
          />
        ) : null}
        {compact ? audioButton : null}
        {alert ? (
          <TabAlertDot
            id={`${domId}-alert`}
            title={alert}
            className={compact && showAudio ? "-left-0.5 -top-0.5" : "-right-0.5 -top-0.5"}
          />
        ) : null}
      </span>
      {compact ? null : (
        <>
          <span id={`${domId}-title`} className="min-w-0 flex-1 overflow-hidden whitespace-nowrap text-left">
            {tab.title || tab.url}
          </span>
          {tab.ephemeral ? (
            <Glasses id={`${domId}-ephemeral`} className="size-3 shrink-0 text-violet-300" aria-label="Ephemeral tab" />
          ) : null}
          {tab.changed ? (
            <span id={`${domId}-changed`} className="size-2 shrink-0 rounded-full bg-sky-400" title="This page changed" />
          ) : null}
          {tab.sleeping ? <Moon id={`${domId}-sleeping`} className="size-3 shrink-0 text-zinc-500" aria-label="Sleeping" /> : null}
          {audioButton}
          <button
            id={`${domId}-close`}
            type="button"
            title={grouped ? "Close selected tabs (Ctrl+W)" : "Close tab (Ctrl+W)"}
            aria-label={grouped ? "Close selected tabs" : `Close ${tab.title}`}
            className={cn(
              "pointer-events-none -ml-2 inline-flex h-5 w-0 min-w-0 shrink-0 items-center justify-center overflow-hidden rounded p-0 text-zinc-400 opacity-0 transition-[width,opacity,margin] duration-100 hover:bg-zinc-700 hover:text-zinc-100",
              "group-hover:pointer-events-auto group-hover:ml-0 group-hover:w-5 group-hover:overflow-visible group-hover:opacity-100",
              "focus-visible:pointer-events-auto focus-visible:ml-0 focus-visible:w-5 focus-visible:overflow-visible focus-visible:opacity-100",
              focusRing,
            )}
            onMouseDown={(event) => event.stopPropagation()}
            onClick={onClose}
          >
            <X id={`${domId}-close-icon`} className="size-3.5 shrink-0" aria-hidden />
          </button>
        </>
      )}
    </div>
  );
}
