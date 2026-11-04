import { useState } from "react";
import { CircleAlert, ClipboardCopy, FileCheck, FileDown, FileText, FolderOpen } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { closePopup, currentWindowLabel, openSavedFile, requestPageTool, showSavedFile } from "@/lib/browser-client";
import type { MarkdownScope } from "@type/browser";
import { ActionTile, OptionRow, PopoverHeader, Segmented } from "../PopoverParts";
import { readPopoverTab } from "../shared";

type WebpageMarkdownPopoverProps = {
  params: URLSearchParams;
};

export function WebpageMarkdownPopover({ params }: WebpageMarkdownPopoverProps) {
  const tab = readPopoverTab(params);
  const status = params.get("status");
  const [scope, setScope] = useState<MarkdownScope>(params.get("scope") === "page" ? "page" : "article");
  const [images, setImages] = useState(params.get("images") !== "false");
  const [links, setLinks] = useState(params.get("links") !== "false");
  const [header, setHeader] = useState(params.get("header") !== "false");
  const [showResult, setShowResult] = useState(status !== null);

  const send = (action: "download" | "copy") => {
    void requestPageTool({
      tool: "webpage-markdown",
      action,
      window: currentWindowLabel(),
      tabId: tab.id,
      title: tab.title,
      url: tab.url,
      scope,
      images,
      links,
      header,
    });
    void closePopup();
  };

  return (
    <PopupFrame id="webpage-markdown-popover" width={320} maxHeight={520}>
      <PopoverHeader
        id="webpage-markdown-header"
        icon={<FileText aria-hidden />}
        title="Webpage to Markdown"
        subtitle={tab.title || tab.url}
      />
      {showResult && status ? (
        <MarkdownResult params={params} onAgain={() => setShowResult(false)} />
      ) : (
        <div id="webpage-markdown-form" className="flex flex-col gap-2 px-3 pb-3">
          <Segmented<MarkdownScope>
            id="webpage-markdown-scope"
            label="What to convert"
            value={scope}
            options={[
              { value: "article", label: "Article" },
              { value: "page", label: "Entire page" },
            ]}
            onChange={setScope}
          />
          <OptionRow
            id="webpage-markdown-images"
            label="Include images"
            hint="Image addresses become Markdown images"
            checked={images}
            onChange={setImages}
          />
          <OptionRow
            id="webpage-markdown-links"
            label="Keep links"
            hint="Off leaves the link text only"
            checked={links}
            onChange={setLinks}
          />
          <OptionRow
            id="webpage-markdown-header"
            label="Title and source URL"
            hint="Puts the page title and address at the top"
            checked={header}
            onChange={setHeader}
          />
          <ActionTile
            id="webpage-markdown-download"
            primary
            icon={<FileDown aria-hidden />}
            label="Download"
            hint="Save a .md file"
            onClick={() => send("download")}
          />
          <ActionTile
            id="webpage-markdown-copy"
            icon={<ClipboardCopy aria-hidden />}
            label="Copy to clipboard"
            hint="Paste it into any app"
            onClick={() => send("copy")}
          />
        </div>
      )}
    </PopupFrame>
  );
}

function MarkdownResult({ params, onAgain }: { params: URLSearchParams; onAgain: () => void }) {
  const status = params.get("status");
  const path = params.get("path") ?? "";
  const failed = status === "error";
  const copied = status === "copied";

  return (
    <div id="webpage-markdown-result" className="flex flex-col gap-2 px-3 pb-3">
      <div
        id="webpage-markdown-result-message"
        className={failed ? "rounded-lg bg-amber-500/10 p-3 text-amber-200" : "rounded-lg bg-sky-500/10 p-3 text-sky-100"}
      >
        <div id="webpage-markdown-result-title" className="flex items-center gap-2 text-sm font-medium">
          {failed ? (
            <CircleAlert id="webpage-markdown-result-icon" className="size-4 shrink-0" aria-hidden />
          ) : (
            <FileCheck id="webpage-markdown-result-icon" className="size-4 shrink-0" aria-hidden />
          )}
          {failed ? "Couldn't convert this page" : copied ? "Markdown copied" : "Markdown saved"}
        </div>
        <p id="webpage-markdown-result-detail" className="mt-1 break-all text-xs opacity-80">
          {failed ? params.get("message") : copied ? "Ready to paste" : path}
        </p>
      </div>
      <div id="webpage-markdown-result-actions" className="flex gap-2">
        {status === "saved" ? (
          <>
            <Button
              id="webpage-markdown-result-open"
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
              id="webpage-markdown-result-folder"
              className="flex-1"
              onClick={() => {
                void showSavedFile(path);
                void closePopup();
              }}
            >
              <FolderOpen id="webpage-markdown-result-folder-icon" className="size-4" aria-hidden />
              Show in folder
            </Button>
          </>
        ) : null}
        <Button id="webpage-markdown-result-again" className="flex-1" onClick={onAgain}>
          Back
        </Button>
      </div>
    </div>
  );
}
