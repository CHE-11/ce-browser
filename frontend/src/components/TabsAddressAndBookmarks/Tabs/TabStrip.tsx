import { useMemo } from "react";
import { Plus } from "lucide-react";
import { createTab } from "@/lib/browser-client";
import { useTabDrag } from "@/lib/tab-drag";
import { buildTabRows } from "@/lib/tab-rows";
import type { WindowTabsState } from "@/lib/use-browser";
import { TabRowsView } from "./TabRowsView";
import { WindowControls } from "@/components/WindowControls";
import { WorkspaceSwitcher } from "@/components/WorkspaceSwitcher";

type TabStripProps = {
  snapshot: WindowTabsState;
};

export function TabStrip({ snapshot }: TabStripProps) {
  const drag = useTabDrag(snapshot.tabs, "x");
  const rows = useMemo(
    () => buildTabRows(snapshot.tabs, snapshot.groups, snapshot.activeId, snapshot.split),
    [snapshot.tabs, snapshot.groups, snapshot.activeId, snapshot.split],
  );

  return (
    <div id="tab-strip" className="flex h-9 shrink-0 items-stretch bg-zinc-950">
      <div id="tab-strip-workspace" className="flex shrink-0 items-end pb-1 pl-2">
        <WorkspaceSwitcher
          workspaces={snapshot.workspaces}
          current={snapshot.workspace}
          compact={snapshot.workspaces.length <= 1}
        />
      </div>
      <div
        id="tab-strip-tabs"
        role="tablist"
        aria-label="Tabs"
        className="no-scrollbar flex min-w-0 flex-1 items-end gap-px overflow-x-auto pl-2 pt-1.5"
        {...drag.containerProps}
        onDoubleClick={(event) => {
          if (event.target === event.currentTarget) {
            void createTab();
          }
        }}
      >
        <TabRowsView snapshot={snapshot} rows={rows} orientation="horizontal" compact={false} drag={drag} />
        <button
          id="tab-strip-new"
          type="button"
          title="New tab (Ctrl+T)"
          aria-label="New tab"
          className="mb-1 ml-1 inline-flex size-7 shrink-0 items-center justify-center rounded-md text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
          onClick={() => void createTab()}
        >
          <Plus id="tab-strip-new-icon" className="size-4" aria-hidden />
        </button>
        <div id="tab-strip-drag-space" className="h-full min-w-12 flex-1" data-tauri-drag-region />
      </div>
      <WindowControls />
    </div>
  );
}
