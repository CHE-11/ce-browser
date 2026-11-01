import { useState } from "react";
import { CircleAlert, ClipboardCopy, FolderOpen, ImageDown, ImageUp, RotateCcw } from "lucide-react";
import { ActionTile, OptionRow, PopoverHeader, Segmented } from "../PopoverParts";
import { readPopoverTab } from "../shared";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { requestPageTool, currentWindowLabel, closePopup, openSavedFile, showSavedFile } from "@/lib/browser-client";

type PageCapturePopoverProps = {
  params: URLSearchParams;
};

type Area = "full" | "visible";

export function PageCapturePopover({ params }: PageCapturePopoverProps) {
  const tab = readPopoverTab(params);
  const status = params.get("status");
  const [area, setArea] = useState<Area>(params.get("fullPage") === "false" ? "visible" : "full");
  const [loadLazy, setLoadLazy] = useState(params.get("loadLazy") !== "false");
  const [showResult, setShowResult] = useState(status !== null);

  const send = (action: "save" | "copy") => {
    void requestPageTool({
      tool: "page-capture",
      action,
      window: currentWindowLabel(),
      tabId: tab.id,
      title: tab.title,
      url: tab.url,
      fullPage: area === "full",
      loadLazy: area === "full" && loadLazy,
    });
    void closePopup();
  };

  return (
    <PopupFrame id="page-capture-popover" maxHeight={420}>
      <PopoverHeader
        id="page-capture-header"
        icon={<ImageDown aria-hidden />}
        title="Page to PNG"
        subtitle={tab.title || tab.url}
      />
      {showResult && status ? (
        <CaptureResult params={params} onAgain={() => setShowResult(false)} />
      ) : (
        <div id="page-capture-form" className="flex flex-col gap-2 px-3 pb-3">
          <Segmented<Area>
            id="page-capture-area"
            label="Capture area"
            value={area}
            options={[
              { value: "full", label: "Entire page" },
              { value: "visible", label: "Visible area" },
            ]}
            onChange={setArea}
          />
          <OptionRow
            id="page-capture-lazy"
            label="Load lazy images first"
            hint="Scrolls through the page before capturing"
            checked={area === "full" && loadLazy}
            disabled={area !== "full"}
            onChange={setLoadLazy}
          />
          <ActionTile
            id="page-capture-save"
            primary
            icon={<ImageDown aria-hidden />}
            label="Save PNG"
            hint="Choose where to save the image"
            onClick={() => send("save")}
          />
          <ActionTile
            id="page-capture-copy"
            icon={<ClipboardCopy aria-hidden />}
            label="Copy image"
            hint="Paste it into any app"
            onClick={() => send("copy")}
          />
        </div>
      )}
    </PopupFrame>
  );
}

function CaptureResult({ params, onAgain }: { params: URLSearchParams; onAgain: () => void }) {
  const status = params.get("status");
  const path = params.get("path") ?? "";
  const size = params.get("size") ?? "";
  const truncated = params.get("truncated") === "true";
  const failed = status === "error";

  return (
    <div id="page-capture-result" className="flex flex-col gap-2 px-3 pb-3">
      <div
        id="page-capture-result-message"
        className={failed ? "rounded-lg bg-amber-500/10 p-3 text-amber-200" : "rounded-lg bg-sky-500/10 p-3 text-sky-100"}
      >
        <div id="page-capture-result-title" className="flex items-center gap-2 text-sm font-medium">
          {failed ? (
            <CircleAlert id="page-capture-result-icon" className="size-4 shrink-0" aria-hidden />
          ) : (
            <ImageUp id="page-capture-result-icon" className="size-4 shrink-0" aria-hidden />
          )}
          {failed ? "Couldn't capture this page" : status === "copied" ? "Image copied" : "Image saved"}
        </div>
        <p id="page-capture-result-detail" className="mt-1 break-all text-xs opacity-80">
          {failed ? params.get("message") : status === "copied" ? `${size} px, ready to paste` : `${size} px · ${path}`}
        </p>
        {truncated ? (
          <p id="page-capture-result-truncated" className="mt-1 text-xs text-amber-300">
            The page is very long, so the image stops at 60,000 px.
          </p>
        ) : null}
      </div>
      <div id="page-capture-result-actions" className="flex gap-2">
        {status === "saved" ? (
          <>
            <Button
              id="page-capture-result-open"
              variant="primary"
              className="flex-1"
              onClick={() => {
                void openSavedFile(path);
                void closePopup();
              }}
            >
              Open
            </Button>
            <Button
              id="page-capture-result-folder"
              className="flex-1"
              onClick={() => {
                void showSavedFile(path);
                void closePopup();
              }}
            >
              <FolderOpen id="page-capture-result-folder-icon" className="size-4" aria-hidden />
              Show in folder
            </Button>
          </>
        ) : null}
        <Button id="page-capture-result-again" className="flex-1" onClick={onAgain}>
          <RotateCcw id="page-capture-result-again-icon" className="size-4" aria-hidden />
          Capture again
        </Button>
      </div>
    </div>
  );
}
