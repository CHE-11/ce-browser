import type { DragEvent, MouseEvent } from "react";
import { Glasses, LoaderCircle, Moon, Volume2, VolumeX, X } from "lucide-react";
import { FavIcon } from "@/components/FavIcon";
import { setTabsMuted, selectTabRange, toggleTabSelection, activateTab } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { groupColorClasses } from "@/lib/tab-rows";
import { TabInfo } from "@type/browser";


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
  split,
  drop,
  onContextMenu,
  onClose,
  onDragStart,
  onDragEnd,
  onDragOver,
}: TabItemProps) {
  const domId = `tab-${tab.id}`;
  const vertical = orientation === "vertical";
  const showAudio = tab.audible || tab.muted;

  const toggleMute = (event: MouseEvent) => {
    event.stopPropagation();
    void setTabsMuted([tab.id], !tab.muted);
  };

  const audioButton = showAudio ? (
    <button
      id={`${domId}-audio`}
      type="button"
      title={tab.muted ? "Unmute tab" : "Mute tab"}
      aria-label={tab.muted ? `Unmute ${tab.title}` : `Mute ${tab.title}`}
      className={cn(
        "inline-flex size-5 shrink-0 items-center justify-center rounded text-zinc-400 hover:bg-zinc-700 hover:text-zinc-100",
        compact && "absolute -right-0.5 -top-0.5 size-4 rounded-full bg-zinc-900",
      )}
      onMouseDown={(event) => event.stopPropagation()}
      onClick={toggleMute}
    >
      {tab.muted ? (
        <VolumeX id={`${domId}-audio-muted`} className={compact ? "size-2.5" : "size-3.5"} aria-hidden />
      ) : (
        <Volume2 id={`${domId}-audio-playing`} className={cn(compact ? "size-2.5" : "size-3.5", "text-sky-400")} aria-hidden />
      )}
    </button>
  ) : null;

  return (
    <div
      id={domId}
      role="tab"
      aria-selected={selected}
      title={`${tab.title}\n${tab.url}${tab.sleeping ? "\nSleeping" : ""}`}
      draggable
      tabIndex={0}
      className={cn(
        "group relative flex shrink-0 cursor-default select-none items-center gap-2 text-xs transition-colors",
        vertical ? "h-8 w-full rounded-lg px-2" : "h-7.5 rounded-t-lg px-2.5",
        !vertical && (compact ? "w-10 justify-center px-0" : "w-56 min-w-14 shrink"),
        vertical && compact && "w-9 justify-center px-0",
        active ? "bg-zinc-800 text-zinc-100" : "text-zinc-400 hover:bg-zinc-900/60 hover:text-zinc-200",
        !vertical && active && "bg-zinc-900",
        grouped && !active && "bg-zinc-800 text-zinc-100",
        !tab.loaded && !active && "opacity-80",
        tab.sleeping && !active && "opacity-55",
        split && "ring-1 ring-inset ring-sky-500/50",
        split === "left" && (vertical ? "rounded-b-none" : "rounded-tr-none"),
        split === "right" && (vertical ? "rounded-t-none" : "rounded-tl-none"),
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
        {tab.loading ? (
          <LoaderCircle id={`${domId}-loading`} className="size-4 shrink-0 animate-spin text-sky-400" aria-hidden />
        ) : (
          <FavIcon id={`${domId}-icon`} url={tab.url} src={tab.favicon} />
        )}
        {compact ? audioButton : null}
      </span>
      {compact ? null : (
        <>
          <span id={`${domId}-title`} className="min-w-0 flex-1 truncate">
            {tab.title || tab.url}
          </span>
          {tab.ephemeral ? (
            <Glasses id={`${domId}-ephemeral`} className="size-3 shrink-0 text-violet-300" aria-label="Ephemeral tab" />
          ) : null}
          {tab.changed ? (
            <span id={`${domId}-changed`} className="size-2 shrink-0 rounded-full bg-sky-400" title="This page changed" />
          ) : null}
          {tab.errors > 0 ? (
            <span id={`${domId}-errors`} className="shrink-0 rounded-full bg-rose-500 px-1.5 text-[10px] font-semibold text-white" title="Page errors">
              {tab.errors > 99 ? "99+" : tab.errors}
            </span>
          ) : null}
          {tab.sleeping ? <Moon id={`${domId}-sleeping`} className="size-3 shrink-0 text-zinc-500" aria-label="Sleeping" /> : null}
          {audioButton}
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
