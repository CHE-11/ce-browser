import { useState } from "react";
import { CircleAlert, FileCheck, FileDown, FolderOpen, PencilRuler, Printer, Settings, Sparkles } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { activateTab, closePopup, createTab, currentWindowLabel, navigate, openSavedFile, requestPageTool, showSavedFile } from "@/lib/browser-client";
import { useSettings, useTabs } from "@/lib/use-browser";
import { PopoverHeader, ActionTile } from "../PopoverParts";
import { readPopoverTab } from "../shared";
import { presetHint } from "./print-options";

const PRINT_SETTINGS_URL = "ce://settings?focus=settings-print";

type PrintPdfPopoverProps = {
  params: URLSearchParams;
};

export function PrintPdfPopover({ params }: PrintPdfPopoverProps) {
  const tab = readPopoverTab(params);
  const status = params.get("status");
  const [showResult, setShowResult] = useState(status !== null);
  const settings = useSettings();
  const { windowTabs } = useTabs();
  const presets = settings?.printEditor.presets ?? [];

  const send = (action: "quick" | "as-is", presetId = "") => {
    void requestPageTool({
      tool: "print-pdf",
      action,
      presetId,
      window: currentWindowLabel(),
      tabId: tab.id,
      title: tab.title,
      url: tab.url,
    });
    void closePopup();
  };

  const openEditor = () => {
    void createTab(`ce://print-editor?tab=${tab.id}`);
    void closePopup();
  };

  const openSettings = () => {
    const open = windowTabs.find((item) => item.url.startsWith("ce://settings"));
    const going = open ? activateTab(open.id).then(() => navigate(open.id, PRINT_SETTINGS_URL)) : createTab(PRINT_SETTINGS_URL);
    void going.finally(() => closePopup());
  };

  return (
    <PopupFrame id="print-pdf-popover" width={320} maxHeight={560}>
      <PopoverHeader id="print-pdf-header" icon={<Printer aria-hidden />} title="Print to PDF" subtitle={tab.title || tab.url} />
      <div id="print-pdf-body" className="flex flex-col gap-2 px-3 pb-3">
        {showResult && status ? (
          <PrintResult params={params} onAgain={() => setShowResult(false)} />
        ) : (
          <div id="print-pdf-actions" className="flex flex-col gap-2">
            <ActionTile
              id="print-pdf-editor"
              primary
              icon={<PencilRuler aria-hidden />}
              label="Open editor"
              hint="Remove ads and elements, then save"
              onClick={openEditor}
            />
            {presets.length > 0 ? (
              <div id="print-pdf-presets" className="flex flex-col gap-2">
                <p id="print-pdf-presets-label" className="px-1 text-[11px] font-semibold uppercase tracking-wider text-zinc-500">
                  Quick clean
                </p>
                {presets.map((preset) => (
                  <ActionTile
                    key={preset.id}
                    id={`print-pdf-preset-${preset.id}`}
                    icon={<Sparkles aria-hidden />}
                    label={preset.name}
                    hint={presetHint(preset)}
                    onClick={() => send("quick", preset.id)}
                  />
                ))}
              </div>
            ) : (
              <ActionTile
                id="print-pdf-quick"
                icon={<Sparkles aria-hidden />}
                label="Quick clean PDF"
                hint="Your saved cleanup, no editing"
                onClick={() => send("quick")}
              />
            )}
            <ActionTile
              id="print-pdf-as-is"
              icon={<FileDown aria-hidden />}
              label="Save page as is"
              hint="Exactly what the tab shows"
              onClick={() => send("as-is")}
            />
          </div>
        )}
        <ActionTile
          id="print-pdf-settings"
          icon={<Settings aria-hidden />}
          label="Settings"
          hint="Presets, page sizes, and defaults"
          onClick={openSettings}
        />
      </div>
    </PopupFrame>
  );
}

function fileNameOf(path: string) {
  const cut = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return cut >= 0 ? path.slice(cut + 1) : path;
}

function PrintResult({ params, onAgain }: { params: URLSearchParams; onAgain: () => void }) {
  const failed = params.get("status") === "error";
  const path = params.get("path") ?? "";
  const fileName = fileNameOf(path);

  return (
    <div id="print-pdf-result" className="flex flex-col gap-2">
      <div
        id="print-pdf-result-message"
        className={failed ? "rounded-lg bg-amber-500/10 p-3 text-amber-200" : "rounded-lg bg-sky-500/10 p-3 text-sky-100"}
      >
        <div id="print-pdf-result-title" className="flex items-center gap-2 text-sm font-medium">
          {failed ? (
            <CircleAlert id="print-pdf-result-icon" className="size-4 shrink-0" aria-hidden />
          ) : (
            <FileCheck id="print-pdf-result-icon" className="size-4 shrink-0" aria-hidden />
          )}
          {failed ? "Couldn't save the PDF" : "PDF saved"}
        </div>
        <p id="print-pdf-result-detail" className="mt-1 truncate text-xs opacity-80" title={failed ? undefined : path}>
          {failed ? params.get("message") : fileName}
        </p>
      </div>
      <div id="print-pdf-result-actions" className="flex items-center gap-2">
        {failed ? null : (
          <>
            <Button
              id="print-pdf-result-open"
              variant="primary"
              className="min-w-0 flex-1 whitespace-nowrap"
              onClick={() => {
                void openSavedFile(path);
                void closePopup();
              }}
            >
              Open
            </Button>
            <Button
              id="print-pdf-result-folder"
              className="shrink-0 whitespace-nowrap px-2"
              aria-label="Show in folder"
              title="Show in folder"
              onClick={() => {
                void showSavedFile(path);
                void closePopup();
              }}
            >
              <FolderOpen id="print-pdf-result-folder-icon" className="size-4" aria-hidden />
            </Button>
          </>
        )}
        <Button id="print-pdf-result-again" className="shrink-0 whitespace-nowrap" onClick={onAgain}>
          Back
        </Button>
      </div>
    </div>
  );
}
