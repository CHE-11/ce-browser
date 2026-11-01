import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { createTab, closePopup } from "@/lib/browser-client";
import { useDownloadsState } from "@/lib/use-browser";
import { Download } from "lucide-react";
import { DownloadRow } from "./DownloadRow";


const PANEL_LIMIT = 8;

export function DownloadsPanel() {
  const { downloads, loaded } = useDownloadsState();
  const recent = downloads.slice(0, PANEL_LIMIT);

  return (
    <PopupFrame id="downloads-panel" maxHeight={520} ready={loaded}>
      <div id="downloads-panel-header" className="flex items-center justify-between px-4 pb-1 pt-3">
        <h2 id="downloads-panel-title" className="text-sm font-semibold">
          Recent downloads
        </h2>
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
      <div id="downloads-panel-footer" className="border-t border-zinc-800 p-2">
        <Button
          id="downloads-panel-all"
          variant="ghost"
          className="w-full justify-start"
          onClick={() => {
            void createTab("ce://downloads");
            void closePopup();
          }}
        >
          Show all downloads
        </Button>
      </div>
    </PopupFrame>
  );
}
