import { forwardRef, useEffect, useMemo, useRef, useState, type PointerEvent } from "react";
import { PanelLeftClose, PanelLeftOpen, Plus } from "lucide-react";
import type { BrowserSettings } from "../../../../../types/browser";
import { createEphemeralTab, createTab, saveSettings, zoomTab } from "../../../lib/browser-client";
import { cn } from "../../../lib/cn";
import { focusRing } from "../../../lib/focus-ring";
import { useTabDrag } from "../../../lib/tab-drag";
import { useTabMotion } from "../../../lib/tab-motion";
import { buildTabRows } from "../../../lib/tab-rows";
import type { WindowTabsState } from "../../../lib/use-browser";
import { zoomWheelStep } from "../../../lib/zoom";
import { TabRowsView } from "./TabRowsView";
import { WorkspaceSwitcher } from "../Workspace/WorkspaceSwitcher";

type VerticalTabsProps = {
  snapshot: WindowTabsState;
  settings: BrowserSettings;
};

const COLLAPSED_WIDTH = 48;
const MIN_WIDTH = 160;
const MAX_WIDTH = 400;

/** Tabs in a sidebar left of the pages. Its width is reported to the backend as the page inset. */
export const VerticalTabs = forwardRef<HTMLElement, VerticalTabsProps>(function VerticalTabs({ snapshot, settings }, ref) {
  const collapsed = settings.verticalTabsCollapsed;
  const activeId = useRef(snapshot.activeId);
  const zoomBucket = useRef(0);
  activeId.current = snapshot.activeId;
  const [dragWidth, setDragWidth] = useState<number | null>(null);
  const resizeStart = useRef<{ x: number; width: number } | null>(null);
  const drag = useTabDrag(snapshot.tabs, "y");
  const shown = useTabMotion(snapshot, drag.dragging !== null);
  const rows = useMemo(
    () => buildTabRows(shown.tabs, shown.groups, shown.activeId, shown.split),
    [shown.tabs, shown.groups, shown.activeId, shown.split],
  );
  const pinnedRows = rows.filter((row) => row.kind === "tab" && row.tab.pinned);
  const otherRows = rows.filter((row) => row.kind !== "tab" || !row.tab.pinned);
  const width = collapsed ? COLLAPSED_WIDTH : (dragWidth ?? settings.verticalTabsWidth);

  useEffect(() => {
    setDragWidth(null);
  }, [settings.verticalTabsWidth]);

  useEffect(() => {
    const aside = document.getElementById("vertical-tabs");
    if (!aside) {
      return;
    }
    const onWheel = (event: WheelEvent) => {
      if (!event.ctrlKey) {
        zoomBucket.current = 0;
        return;
      }
      event.preventDefault();
      const stepped = zoomWheelStep(zoomBucket.current, event.deltaY, event.deltaMode);
      zoomBucket.current = stepped.bucket;
      const id = activeId.current;
      if (id !== null && stepped.change) {
        void zoomTab(id, stepped.change);
      }
    };
    aside.addEventListener("wheel", onWheel, { passive: false });
    return () => aside.removeEventListener("wheel", onWheel);
  }, []);

  const toggleCollapsed = () => void saveSettings({ ...settings, verticalTabsCollapsed: !collapsed });

  const onResizeDown = (event: PointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    resizeStart.current = { x: event.clientX, width };
  };

  const onResizeMove = (event: PointerEvent<HTMLDivElement>) => {
    const start = resizeStart.current;
    if (!start) {
      return;
    }
    setDragWidth(Math.round(Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, start.width + event.clientX - start.x))));
  };

  const onResizeUp = (event: PointerEvent<HTMLDivElement>) => {
    if (!resizeStart.current) {
      return;
    }
    resizeStart.current = null;
    event.currentTarget.releasePointerCapture(event.pointerId);
    if (dragWidth !== null && dragWidth !== settings.verticalTabsWidth) {
      void saveSettings({ ...settings, verticalTabsWidth: dragWidth });
    }
  };

  return (
    <aside
      id="vertical-tabs"
      ref={ref}
      aria-label="Tabs"
      className="relative flex h-full shrink-0 flex-col bg-zinc-950"
      style={{ width }}
    >
      <div id="vertical-tabs-top" className={cn("flex shrink-0 items-center gap-1 px-1.5 pb-1 pt-2", collapsed && "flex-col")}>
        {settings.workspacesEnabled ? (
          <WorkspaceSwitcher
            workspaces={snapshot.workspaces}
            current={snapshot.workspace}
            compact={collapsed}
            className={collapsed ? undefined : "min-w-0 flex-1"}
          />
        ) : null}
        <button
          id="vertical-tabs-new"
          type="button"
          aria-label="New tab"
          className={cn("inline-flex size-7 shrink-0 items-center justify-center rounded-md text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100", focusRing)}
          title="New tab (Ctrl+T). Right-click for an ephemeral tab."
          onClick={() => void createTab()}
          onContextMenu={(event) => {
            event.preventDefault();
            void createEphemeralTab();
          }}
        >
          <Plus id="vertical-tabs-new-icon" className="size-4" aria-hidden />
        </button>
      </div>
      {pinnedRows.length > 0 ? (
        <div
          id="vertical-tabs-pinned"
          className={cn("flex shrink-0 flex-wrap gap-1 border-b border-zinc-800 px-1.5 pb-2", collapsed && "flex-col items-center")}
        >
          <TabRowsView snapshot={shown} rows={pinnedRows} orientation="vertical" compact drag={drag} />
        </div>
      ) : null}
      <div
        id="vertical-tabs-list"
        role="tablist"
        aria-orientation="vertical"
        className={cn("no-scrollbar flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto px-1.5 py-2", collapsed && "items-center")}
        {...drag.containerProps}
        onDoubleClick={(event) => {
          if (event.target === event.currentTarget) {
            void createTab();
          }
        }}
      >
        <TabRowsView snapshot={shown} rows={otherRows} orientation="vertical" compact={collapsed} drag={drag} />
      </div>
      <div id="vertical-tabs-bottom" className={cn("flex shrink-0 items-center px-1.5 py-1.5", collapsed && "justify-center")}>
        <button
          id="vertical-tabs-collapse"
          type="button"
          title={collapsed ? "Expand tabs" : "Collapse tabs"}
          aria-label={collapsed ? "Expand tabs" : "Collapse tabs"}
          className={cn("inline-flex size-7 items-center justify-center rounded-md text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100", focusRing)}
          onClick={toggleCollapsed}
        >
          {collapsed ? (
            <PanelLeftOpen id="vertical-tabs-expand-icon" className="size-4" aria-hidden />
          ) : (
            <PanelLeftClose id="vertical-tabs-collapse-icon" className="size-4" aria-hidden />
          )}
        </button>
      </div>
      {collapsed ? null : (
        <div
          id="vertical-tabs-resize"
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize tabs"
          className="absolute inset-y-0 -right-0.5 w-1.5 cursor-col-resize hover:bg-sky-500/40"
          onPointerDown={onResizeDown}
          onPointerMove={onResizeMove}
          onPointerUp={onResizeUp}
          onPointerCancel={onResizeUp}
        />
      )}
    </aside>
  );
});
