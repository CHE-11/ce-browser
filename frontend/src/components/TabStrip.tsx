import { useState, type DragEvent, type MouseEvent } from "react";
import { LoaderCircle, Plus, X } from "lucide-react";
import type { TabInfo, TabsSnapshot } from "../../../types/browser";
import { activateTab, closeSelectedTabs, closeTab, createTab, moveTab, openPopup, selectTabRange, toggleTabSelection } from "../lib/browser-client";
import { cn } from "../lib/cn";
import { FavIcon } from "./FavIcon";
import { WindowControls } from "./WindowControls";

type TabStripProps = {
  snapshot: TabsSnapshot;
};

export function TabStrip({ snapshot }: TabStripProps) {
  const [dragging, setDragging] = useState<number | null>(null);
  const [dropIndex, setDropIndex] = useState<number | null>(null);

  const grouped = (id: number) => snapshot.selectedIds.length > 1 && snapshot.selectedIds.includes(id);

  const closeTarget = (id: number) => {
    void (grouped(id) ? closeSelectedTabs() : closeTab(id));
  };

  const openTabMenu = (event: MouseEvent, tab: TabInfo) => {
    event.preventDefault();
    if (!grouped(tab.id)) {
      void activateTab(tab.id);
    }
    void openPopup({
      kind: "tab-menu",
      params: `id=${tab.id}`,
      x: event.clientX,
      y: event.clientY,
      width: 240,
      height: 300,
    });
  };

  const onDragOver = (event: DragEvent, index: number) => {
    if (dragging === null) {
      return;
    }
    event.preventDefault();
    const rect = event.currentTarget.getBoundingClientRect();
    const after = event.clientX > rect.left + rect.width / 2;
    setDropIndex(after ? index + 1 : index);
  };

  const onDrop = (event: DragEvent) => {
    event.preventDefault();
    if (dragging !== null && dropIndex !== null) {
      const from = snapshot.tabs.findIndex((tab) => tab.id === dragging);
      const target = from >= 0 && from < dropIndex ? dropIndex - 1 : dropIndex;
      void moveTab(dragging, target);
    }
    setDragging(null);
    setDropIndex(null);
  };

  return (
    <div id="tab-strip" className="flex h-9 shrink-0 items-stretch bg-zinc-950" data-tauri-drag-region>
      <div
        id="tab-strip-tabs"
        role="tablist"
        aria-label="Tabs"
        className="no-scrollbar flex min-w-0 flex-1 items-end gap-px overflow-x-auto pl-2 pt-1.5"
        data-tauri-drag-region
        onDragOver={(event) => dragging !== null && event.preventDefault()}
        onDrop={onDrop}
        onDoubleClick={(event) => {
          if (event.target === event.currentTarget) {
            void createTab();
          }
        }}
      >
        {snapshot.tabs.map((tab, index) => (
          <TabButton
            key={tab.id}
            tab={tab}
            active={tab.id === snapshot.activeId}
            selected={snapshot.selectedIds.includes(tab.id)}
            grouped={grouped(tab.id)}
            dropBefore={dropIndex === index && dragging !== tab.id}
            dropAfter={dropIndex === index + 1 && index === snapshot.tabs.length - 1}
            onContextMenu={(event) => openTabMenu(event, tab)}
            onClose={() => closeTarget(tab.id)}
            onDragStart={() => setDragging(tab.id)}
            onDragEnd={() => {
              setDragging(null);
              setDropIndex(null);
            }}
            onDragOver={(event) => onDragOver(event, index)}
          />
        ))}
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

type TabButtonProps = {
  tab: TabInfo;
  active: boolean;
  selected: boolean;
  grouped: boolean;
  dropBefore: boolean;
  dropAfter: boolean;
  onContextMenu: (event: MouseEvent) => void;
  onClose: () => void;
  onDragStart: () => void;
  onDragEnd: () => void;
  onDragOver: (event: DragEvent) => void;
};

function TabButton({
  tab,
  active,
  selected,
  grouped,
  dropBefore,
  dropAfter,
  onContextMenu,
  onClose,
  onDragStart,
  onDragEnd,
  onDragOver,
}: TabButtonProps) {
  const domId = `tab-${tab.id}`;
  return (
    <div
      id={domId}
      role="tab"
      aria-selected={selected}
      title={`${tab.title}\n${tab.url}`}
      draggable
      tabIndex={0}
      className={cn(
        "group relative flex h-7.5 shrink-0 cursor-default select-none items-center gap-2 rounded-t-lg px-2.5 text-xs transition-colors",
        tab.pinned ? "w-10 justify-center px-0" : "w-56 min-w-14 shrink",
        active ? "bg-zinc-900 text-zinc-100" : "text-zinc-400 hover:bg-zinc-900/60 hover:text-zinc-200",
        grouped && !active && "bg-zinc-800 text-zinc-100",
        !tab.loaded && !active && "opacity-80",
      )}
      onMouseDown={(event) => {
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
      {dropBefore ? <span id={`${domId}-drop-before`} className="absolute inset-y-1 -left-px w-0.5 rounded bg-sky-500" /> : null}
      {dropAfter ? <span id={`${domId}-drop-after`} className="absolute inset-y-1 -right-px w-0.5 rounded bg-sky-500" /> : null}
      {grouped ? <span id={`${domId}-selected`} className="absolute inset-x-1.5 top-0 h-0.5 rounded-full bg-sky-400" /> : null}
      {tab.loading ? (
        <LoaderCircle id={`${domId}-loading`} className="size-4 shrink-0 animate-spin text-sky-400" aria-hidden />
      ) : (
        <FavIcon id={`${domId}-icon`} url={tab.url} src={tab.favicon} />
      )}
      {tab.pinned ? null : (
        <>
          <span id={`${domId}-title`} className="min-w-0 flex-1 truncate">
            {tab.title || tab.url}
          </span>
          {tab.zoom !== 1 ? (
            <span id={`${domId}-zoom`} className="shrink-0 text-[10px] text-zinc-500">
              {Math.round(tab.zoom * 100)}%
            </span>
          ) : null}
          <button
            id={`${domId}-close`}
            type="button"
            title={grouped ? "Close selected tabs (Ctrl+W)" : "Close tab (Ctrl+W)"}
            aria-label={grouped ? "Close selected tabs" : `Close ${tab.title}`}
            className={cn(
              "inline-flex size-5 shrink-0 items-center justify-center rounded text-zinc-400 hover:bg-zinc-700 hover:text-zinc-100",
              !active && !grouped && "opacity-0 group-hover:opacity-100",
            )}
            onMouseDown={(event) => event.stopPropagation()}
            onClick={onClose}
          >
            <X id={`${domId}-close-icon`} className="size-3.5" aria-hidden />
          </button>
        </>
      )}
    </div>
  );
}
