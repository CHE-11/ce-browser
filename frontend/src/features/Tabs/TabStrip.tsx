import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronDown, LayoutGrid, Plus } from "lucide-react";
import { createEphemeralTab, createTab, openPopup } from "@/lib/browser-client";
import { beginNewTabTyping, cancelNewTabTyping } from "@/lib/new-tab-typing";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { useTabDrag } from "@/lib/tab-drag";
import { useTabMotion } from "@/lib/tab-motion";
import { buildTabRows } from "@/lib/tab-rows";
import { useSettings, type WindowTabsState } from "@/lib/use-browser";
import { IconButton } from "@/components/IconButton";
import { openTabOverview } from "./TabOverview";
import { TabRowsView } from "./TabRowsView";
import { WindowControls } from "@/components/WindowControls";
import { WorkspaceSwitcher } from "../Workspace/WorkspaceSwitcher";

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
  const drag = useTabDrag(snapshot.tabs, "x", "tab-strip");
  const shown = useTabMotion(snapshot, drag.dragging !== null);
  const [scroller, setScroller] = useState<HTMLDivElement | null>(null);
  const overflowing = useOverflow(scroller);
  const lastActive = useRef<number | null>(null);
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
        return;
      }
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
    <div id="tab-strip" className="flex min-h-9 shrink-0 items-stretch bg-zinc-950">
      <div id="tab-strip-overview" className="flex shrink-0 items-end pb-1 pl-2">
        <IconButton
          id="tab-strip-overview-button"
          label="All tabs (Ctrl + `)"
          aria-haspopup="dialog"
          className="size-7 rounded-md"
          onClick={() => void openTabOverview()}
        >
          <LayoutGrid id="tab-strip-overview-icon" aria-hidden />
        </IconButton>
      </div>
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
            beginNewTabTyping();
            void createTab().catch(() => cancelNewTabTyping());
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
          onClick={() => {
            beginNewTabTyping();
            void createTab().catch(() => cancelNewTabTyping());
          }}
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
              void openPopup("tab-search", event.currentTarget, { params: "scope=workspace" })
            }
          >
            <ChevronDown id="tab-strip-overflow-icon" aria-hidden />
          </IconButton>
        </div>
      ) : null}
      {import.meta.env.DEV ? (
        <div id="tab-strip-dev-mode" className="flex shrink-0 items-center pr-2">
          <span
            id="tab-strip-dev-mode-chip"
            className="rounded-full bg-amber-500/20 px-2 py-0.5 text-[10px] font-semibold tracking-wide text-amber-300"
          >
            DEV MODE
          </span>
        </div>
      ) : null}
      <WindowControls />
    </div>
  );
}
