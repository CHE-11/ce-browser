import { FavIcon } from "@/components/FavIcon";
import { IconButton } from "@/components/IconButton";
import { PopupFrame } from "@/components/PopupFrame";
import { closePopup, createTab, navigate } from "@/lib/browser-client";
import { CHROME, historyIconClass, historyMetaClass, historyRowClass, historyTitleClass, metric } from "@/lib/chrome-metrics";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import { hostOf } from "@/lib/format";
import { absoluteTime, relativeTime } from "@/lib/time";
import { useHistoryState, useSettings, useTabs } from "@/lib/use-browser";
import { HistoryItem } from "@type/browser";
import { History, SquareArrowOutUpRight, X } from "lucide-react";

const PANEL_LIMIT = 10;

function openVisit(url: string, activeId: number | null) {
  if (activeId === null) {
    void createTab(url);
  } else {
    void navigate(activeId, url);
  }
  void closePopup();
}

function HistoryPopoverRow({ item, activeId }: { item: HistoryItem; activeId: number | null }) {
  const domId = `history-popover-${item.id}`;
  const when = new Date(item.lastVisited).toISOString();
  const title = item.title || hostOf(item.url);
  return (
    <button
      id={domId}
      type="button"
      title={item.url}
      className={cn(
        "flex w-full items-center gap-3 rounded-md text-left hover:bg-zinc-800",
        historyRowClass,
        focusRow,
      )}
      onClick={() => openVisit(item.url, activeId)}
    >
      <FavIcon id={`${domId}-icon`} url={item.url} className={historyIconClass} />
      <span id={`${domId}-body`} className="min-w-0 flex-1">
        <span id={`${domId}-title`} className={historyTitleClass}>
          {title}
        </span>
        <span id={`${domId}-host`} className={cn(historyMetaClass, "block")}>
          {hostOf(item.url)}
        </span>
      </span>
      <span id={`${domId}-time`} className={cn(historyMetaClass, "shrink-0")} title={absoluteTime(when)}>
        {relativeTime(when)}
      </span>
    </button>
  );
}

export function HistoryPopover() {
  const { history, loaded } = useHistoryState();
  const settings = useSettings();
  const { activeId } = useTabs();
  const recent = history.slice(0, PANEL_LIMIT);

  return (
    <PopupFrame id="history-popover" width={metric(settings?.historyPanelWidth, CHROME.historyPanelWidth)} maxHeight={520} ready={loaded}>
      <div id="history-popover-header" className="flex items-center justify-between gap-2 py-1 pr-1 pl-4">
        <h2 id="history-popover-title" className="text-sm font-medium">
          History
        </h2>
        <IconButton id="history-popover-close" label="Close" onClick={() => void closePopup()}>
          <X id="history-popover-close-icon" aria-hidden />
        </IconButton>
      </div>
      <div id="history-popover-list" className="px-1 pb-1">
        {recent.length === 0 ? (
          <div id="history-popover-empty" className="flex flex-col items-center gap-2 px-4 py-8 text-sm text-zinc-500">
            <History id="history-popover-empty-icon" className="size-6" aria-hidden />
            Pages you visit appear here.
          </div>
        ) : (
          recent.map((item) => <HistoryPopoverRow key={item.id} item={item} activeId={activeId} />)
        )}
      </div>
      <div id="history-popover-footer" className="border-t border-zinc-800">
        <button
          id="history-popover-open"
          type="button"
          className={cn(
            "flex w-full items-center justify-between gap-3 px-4 py-3 text-left text-sm hover:bg-zinc-800",
            focusRow,
          )}
          onClick={() => {
            void createTab("ce://history");
            void closePopup();
          }}
        >
          <span id="history-popover-open-label">Open History</span>
          <SquareArrowOutUpRight id="history-popover-open-icon" className="size-4 shrink-0 text-zinc-400" aria-hidden />
        </button>
      </div>
    </PopupFrame>
  );
}
