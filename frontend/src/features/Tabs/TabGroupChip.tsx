import { useEffect, useRef, type MouseEvent } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { TabGroup, TabInfo } from "../../../../types/browser";
import { closePopup, openPopup, updateGroup } from "../../lib/browser-client";
import { cn } from "../../lib/cn";
import { focusRing } from "../../lib/focus-ring";
import { groupColorClasses, groupLabel } from "../../lib/tab-rows";
import { TabAlertDot } from "./TabAlertDot";

type TabGroupChipProps = {
  group: TabGroup;
  tabs: TabInfo[];
  orientation: "horizontal" | "vertical";
  /** Collapsed vertical sidebar: show only a colored dot. */
  compact: boolean;
  /** Problems on tabs this collapsed group is hiding. */
  alert: string | null;
};

const HOVER_DELAY = 300;

export function TabGroupChip({ group, tabs, orientation, compact, alert }: TabGroupChipProps) {
  const domId = `tab-group-${group.id}`;
  const colors = groupColorClasses(group.color);
  const label = groupLabel(group, tabs.length);
  const hoverTimer = useRef<number | null>(null);
  const flyoutOpen = useRef(false);
  const vertical = orientation === "vertical";

  const clearHover = () => {
    if (hoverTimer.current !== null) {
      window.clearTimeout(hoverTimer.current);
      hoverTimer.current = null;
    }
  };

  useEffect(() => clearHover, []);

  const openFlyout = (element: HTMLElement) => {
    flyoutOpen.current = true;
    void openPopup("tab-group", element, {
      params: `id=${group.id}`,
      placement: vertical ? "right-start" : "bottom-start",
      focus: false,
    });
  };

  const openMenu = (event: MouseEvent) => {
    event.preventDefault();
    clearHover();
    void openPopup("tab-group-menu", { x: event.clientX, y: event.clientY }, { params: `id=${group.id}` });
  };

  return (
    <button
      id={domId}
      type="button"
      title={`${label}\nClick to ${group.collapsed ? "expand" : "collapse"}. Right-click for options.${alert ? `\n${alert}` : ""}`}
      aria-expanded={!group.collapsed}
      aria-label={`${label}, ${group.collapsed ? "collapsed" : "expanded"} tab group${alert ? `. ${alert.replaceAll("\n", ". ")}` : ""}`}
      className={cn(
        "relative flex shrink-0 select-none items-center gap-1.5 rounded-md text-xs font-medium transition-[color,background-color,max-width] duration-200",
        focusRing,
        colors.chip,
        vertical ? (compact ? "h-6 w-9 justify-center" : "h-7 w-full px-2") : "mb-1 h-6 max-w-40 px-2",
      )}
      style={{ viewTransitionName: `tab-group-${group.id}` }}
      onClick={() => {
        clearHover();
        if (flyoutOpen.current) {
          flyoutOpen.current = false;
          void closePopup();
        }
        void updateGroup(group.id, { collapsed: !group.collapsed });
      }}
      onContextMenu={openMenu}
      onMouseEnter={(event) => {
        if (!group.collapsed) {
          return;
        }
        const element = event.currentTarget;
        clearHover();
        hoverTimer.current = window.setTimeout(() => openFlyout(element), HOVER_DELAY);
      }}
      onMouseLeave={clearHover}
    >
      <span id={`${domId}-swatch`} className={cn("size-2 shrink-0 rounded-full", colors.swatch)} />
      {alert ? <TabAlertDot id={`${domId}-alert`} title={alert} className="-right-1 -top-1" /> : null}
      {compact ? null : (
        <>
          <span id={`${domId}-label`} className="min-w-0 flex-1 truncate text-left">
            {label}
          </span>
          {vertical ? (
            group.collapsed ? (
              <ChevronRight id={`${domId}-expand`} className="size-3.5 shrink-0" aria-hidden />
            ) : (
              <ChevronDown id={`${domId}-collapse`} className="size-3.5 shrink-0" aria-hidden />
            )
          ) : group.collapsed ? (
            <span id={`${domId}-count`} className="shrink-0 rounded bg-black/20 px-1 text-[10px]">
              {tabs.length}
            </span>
          ) : null}
        </>
      )}
    </button>
  );
}
