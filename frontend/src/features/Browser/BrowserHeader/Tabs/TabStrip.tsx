import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronDown, Plus } from "lucide-react";
import { createEphemeralTab, createTab, openPopupBelow, zoomTab } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { useTabDrag } from "@/lib/tab-drag";
import { useTabMotion } from "@/lib/tab-motion";
import { buildTabRows } from "@/lib/tab-rows";
import { useSettings, type WindowTabsState } from "@/lib/use-browser";
import { zoomWheelStep } from "@/lib/zoom";
import { IconButton } from "@/components/IconButton";
import { TabRowsView } from "./TabRowsView";
import { WindowControls } from "@/components/WindowControls";
import { WorkspaceSwitcher } from "@/features/Browser/BrowserHeader/Workspace/WorkspaceSwitcher";

type TabStripProps = {
  snapshot: WindowTabsState;
};

/** True when the tabs are wider than the strip, so some of them are scrolled out of view. */
function useOverflow(element: HTMLElement | null) {
  const [overflowing, setOverflowing] = useState(false);
  useEffect(() => {
    if (!element) {
      return;
    }
    const measure = () => setOverflowing(element.scrollWidth > element.clientWidth + 1);
    measure();
    const resize = new ResizeObserver(measure);
    resize.observe(element);
    const mutate = new MutationObserver(measure);
    mutate.observe(element, { childList: true, subtree: true, attributes: true, attributeFilter: ["class"] });
    return () => {
      resize.disconnect();
      mutate.disconnect();
    };
  }, [element]);
  return overflowing;
}

export function TabStrip({ snapshot }: TabStripProps) {
  const settings = useSettings();
  const workspacesEnabled = settings?.workspacesEnabled !== false;
  const drag = useTabDrag(snapshot.tabs, "x");
  const shown = useTabMotion(snapshot, drag.dragging !== null);
  const [scroller, setScroller] = useState<HTMLDivElement | null>(null);
  const overflowing = useOverflow(scroller);
  const lastActive = useRef<number | null>(null);
  const activeId = useRef(snapshot.activeId);
  const zoomBucket = useRef(0);
  activeId.current = snapshot.activeId;
  const rows = useMemo(
    () => buildTabRows(shown.tabs, shown.groups, shown.activeId, shown.split),
    [shown.tabs, shown.groups, shown.activeId, shown.split],
  );

  useEffect(() => {
    if (!scroller) {
      return;
    }
    const onWheel = (event: WheelEvent) => {
      if (event.ctrlKey) {
        event.preventDefault();
        const stepped = zoomWheelStep(zoomBucket.current, event.deltaY, event.deltaMode);
        zoomBucket.current = stepped.bucket;
        const id = activeId.current;
        if (id !== null && stepped.change) {
          void zoomTab(id, stepped.change);
        }
        return;
      }
      zoomBucket.current = 0;
      if (scroller.scrollWidth <= scroller.clientWidth) {
        return;
      }
      const delta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
      if (delta === 0) {
        return;
      }
      event.preventDefault();
      scroller.scrollLeft += event.deltaMode === WheelEvent.DOM_DELTA_LINE ? delta * 40 : delta;
    };
    scroller.addEventListener("wheel", onWheel, { passive: false });
    return () => scroller.removeEventListener("wheel", onWheel);
  }, [scroller]);

  useEffect(() => {
    const id = snapshot.activeId;
    if (id === null || id === lastActive.current) {
      return;
    }
    lastActive.current = id;
    document.getElementById(`tab-${id}`)?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [snapshot.activeId, rows]);

  return (
    <div id="tab-strip" className="flex h-9 shrink-0 items-stretch bg-zinc-950">
      {workspacesEnabled ? (
        <div id="tab-strip-workspace" className="flex shrink-0 items-end pb-1 pl-2">
          <WorkspaceSwitcher
            workspaces={snapshot.workspaces}
            current={snapshot.workspace}
            compact={snapshot.workspaces.length <= 1}
          />
        </div>
      ) : null}
      <div
        id="tab-strip-tabs"
        ref={setScroller}
        role="tablist"
        aria-label="Tabs"
        className="no-scrollbar flex min-w-0 flex-1 items-end gap-px overflow-x-auto scroll-smooth pl-2 pt-1.5"
        {...drag.containerProps}
        onDoubleClick={(event) => {
          if (event.target === event.currentTarget) {
            void createTab();
          }
        }}
      >
        <TabRowsView snapshot={shown} rows={rows} orientation="horizontal" compact={false} drag={drag} />
        <button
          id="tab-strip-new"
          type="button"
          title="New tab (Ctrl+T). Right-click for an ephemeral tab."
          aria-label="New tab"
          className={cn(
            "mb-1 ml-1 inline-flex size-7 shrink-0 items-center justify-center rounded-md text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100",
            focusRing,
          )}
          onClick={() => void createTab()}
          onContextMenu={(event) => {
            event.preventDefault();
            void createEphemeralTab();
          }}
        >
          <Plus id="tab-strip-new-icon" className="size-4" aria-hidden />
        </button>
        <div id="tab-strip-drag-space" className="h-full min-w-12 flex-1" data-tauri-drag-region />
      </div>
      {overflowing ? (
        <div id="tab-strip-overflow" className="flex shrink-0 items-end pb-1 pl-1">
          <IconButton
            id="tab-strip-overflow-button"
            label={`All ${snapshot.tabs.length} tabs`}
            aria-haspopup="listbox"
            className="size-7 rounded-md"
            onClick={(event) =>
              void openPopupBelow(event.currentTarget, { kind: "tab-search", params: "scope=workspace", width: 420, height: 460 })
            }
          >
            <ChevronDown id="tab-strip-overflow-icon" aria-hidden />
          </IconButton>
        </div>
      ) : null}
      <WindowControls />
    </div>
  );
}
