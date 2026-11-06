import { IconButton } from "@/components/IconButton";
import { PopupFrame } from "@/components/PopupFrame";
import { createTab, closePopup } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import { CHROME, metric } from "@/lib/chrome-metrics";
import { useDownloadsState, useSettings } from "@/lib/use-browser";
import { Download, SquareArrowOutUpRight, X } from "lucide-react";
import { DownloadRow } from "./DownloadRow";


const PANEL_LIMIT = 8;

export function DownloadsPopover() {
  const { downloads, loaded } = useDownloadsState();
  const settings = useSettings();
  const recent = downloads.slice(0, PANEL_LIMIT);

  return (
    <PopupFrame id="downloads-panel" width={metric(settings?.downloadsPanelWidth, CHROME.downloadsPanelWidth)} maxHeight={520} ready={loaded}>
      <div id="downloads-panel-header" className="flex items-center justify-between gap-2 py-1 pr-1 pl-4">
        <h2 id="downloads-panel-title" className="text-sm font-medium">
          Recent download history
        </h2>
        <IconButton id="downloads-panel-close" label="Close" onClick={() => void closePopup()}>
          <X id="downloads-panel-close-icon" aria-hidden />
        </IconButton>
      </div>
      <div id="downloads-panel-list" className="px-1 pb-1">
        {recent.length === 0 ? (
          <div id="downloads-panel-empty" className="flex flex-col items-center gap-2 px-4 py-8 text-sm text-zinc-500">
            <Download id="downloads-panel-empty-icon" className="size-6" aria-hidden />
            Files you download appear here.
          </div>
        ) : (
          recent.map((item) => <DownloadRow key={item.id} item={item} compact />)
        )}
      </div>
      <div id="downloads-panel-footer" className="border-t border-zinc-800">
        <button
          id="downloads-panel-all"
          type="button"
          className={cn(
            "flex w-full items-center justify-between gap-3 px-4 py-3 text-left text-sm hover:bg-zinc-800",
            focusRow,
          )}
          onClick={() => {
            void createTab("ce://downloads");
            void closePopup();
          }}
        >
          <span id="downloads-panel-all-label">Full download history</span>
          <SquareArrowOutUpRight id="downloads-panel-all-icon" className="size-4 shrink-0 text-zinc-400" aria-hidden />
        </button>
      </div>
    </PopupFrame>
  );
}
