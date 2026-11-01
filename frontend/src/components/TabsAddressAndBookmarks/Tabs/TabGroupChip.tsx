import { useEffect, useRef, type MouseEvent } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { TabGroup, TabInfo } from "../../../../../types/browser";
import { closePopup, openPopup, updateGroup, windowPoint } from "../../../lib/browser-client";
import { cn } from "../../../lib/cn";
import { focusRing } from "../../../lib/focus-ring";
import { groupColorClasses, groupLabel } from "../../../lib/tab-rows";

type TabGroupChipProps = {
  group: TabGroup;
  tabs: TabInfo[];
  orientation: "horizontal" | "vertical";
  /** Collapsed vertical sidebar: show only a colored dot. */
  compact: boolean;
};

const HOVER_DELAY = 300;

export function TabGroupChip({ group, tabs, orientation, compact }: TabGroupChipProps) {
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
    const rect = element.getBoundingClientRect();
    flyoutOpen.current = true;
    void openPopup({
      kind: "tab-group",
      params: `id=${group.id}`,
      focus: false,
      x: vertical ? rect.right + 4 : rect.left,
      y: vertical ? rect.top : rect.bottom + 2,
      width: 280,
      height: Math.min(48 + tabs.length * 36, 420),
    });
  };

  const openMenu = (event: MouseEvent) => {
    event.preventDefault();
    clearHover();
    void windowPoint(event.clientX, event.clientY).then((point) =>
      openPopup({
        kind: "tab-group-menu",
        params: `id=${group.id}`,
        x: point.x,
        y: point.y,
        width: 280,
        height: 300,
      }),
    );
  };

  return (
    <button
      id={domId}
      type="button"
      title={`${label}\nClick to ${group.collapsed ? "expand" : "collapse"}. Right-click for options.`}
      aria-expanded={!group.collapsed}
      aria-label={`${label}, ${group.collapsed ? "collapsed" : "expanded"} tab group`}
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
