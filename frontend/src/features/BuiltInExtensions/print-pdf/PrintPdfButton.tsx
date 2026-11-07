import { useRef } from "react";
import { errorText, usePageToolRequests, useToolStatus } from "../use-page-tool";
import { IconButton } from "@/components/IconButton";
import { openPopup, defaultDownloadDir, chooseSavePath, printTabToPdf, extractPage, printHtmlToPdf, openSavedFile } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { TabInfo, BrowserSettings, PageToolRequest } from "@type/browser";
import { LoaderCircle, Check, Printer } from "lucide-react";
import { tabParams, joinPath, fileNameFor, capturable } from "../shared";
import { contentStateFrom, buildDocument, parsePage, renderOffscreen } from "./print-document";
import { DEFAULT_PRINT_SETTINGS, applyPrintPreset, pdfOptions, contentBox } from "./print-options";

type PrintPdfButtonProps = {
  activeTab: TabInfo | undefined;
  settings: BrowserSettings | null;
};

export function PrintPdfButton({ activeTab, settings }: PrintPdfButtonProps) {
  const buttonRef = useRef<HTMLButtonElement>(null);
  const [status, setStatus] = useToolStatus();
  const settingsRef = useRef(settings);
  settingsRef.current = settings;

  const showPopover = (params: URLSearchParams) => {
    const anchor = buttonRef.current;
    if (anchor) {
      void openPopup("print-pdf", anchor, { params: params.toString() });
    }
  };

  const open = () => {
    if (activeTab) {
      showPopover(new URLSearchParams(tabParams(activeTab)));
    }
  };

  const run = async (request: Extract<PageToolRequest, { tool: "print-pdf" }>) => {
    const stored = settingsRef.current?.printEditor ?? DEFAULT_PRINT_SETTINGS;
    const format = request.action === "quick" && request.presetId ? applyPrintPreset(stored, request.presetId) : stored;
    const result = new URLSearchParams({ tab: String(request.tabId), title: request.title, url: request.url });
    try {
      const folder = settingsRef.current?.downloadDir || (await defaultDownloadDir());
      const path = await chooseSavePath(joinPath(folder, fileNameFor(request.title, request.url, "pdf")), "PDF document", [
        "pdf",
      ]);
      if (!path) {
        return;
      }
      setStatus("busy");
      if (request.action === "as-is") {
        await printTabToPdf(
          request.tabId,
          { ...pdfOptions(format, request.title, request.url), backgrounds: true },
          path,
        );
      } else {
        const page = await extractPage(request.tabId);
        const state = contentStateFrom(format);
        const built = buildDocument(parsePage(page), page, state, format);
        const html = await renderOffscreen(built.html, state, contentBox(format).width);
        await printHtmlToPdf(html, pdfOptions(format, page.title || request.title, page.url || request.url), path);
      }
      result.set("status", "saved");
      result.set("path", path);
      setStatus("done");
      if (format.openAfterSave) {
        void openSavedFile(path).catch(() => undefined);
        return;
      }
    } catch (error) {
      result.set("status", "error");
      result.set("message", errorText(error));
      setStatus("failed");
    }
    showPopover(result);
  };

  usePageToolRequests("print-pdf", (request) => void run(request));

  const busy = status === "busy";
  const label = busy
    ? "Saving PDF…"
    : status === "done"
      ? "PDF saved"
      : status === "failed"
        ? "Couldn't save PDF"
        : "Print to PDF";

  return (
    <IconButton
      id="address-print-pdf"
      ref={buttonRef}
      label={label}
      disabled={busy || !capturable(activeTab?.url)}
      className={cn(status === "done" && "text-sky-300", status === "failed" && "text-amber-300")}
      onClick={open}
    >
      {busy ? (
        <LoaderCircle id="address-print-pdf-icon" className="animate-spin" aria-hidden />
      ) : status === "done" ? (
        <Check id="address-print-pdf-icon" aria-hidden />
      ) : (
        <Printer id="address-print-pdf-icon" aria-hidden />
      )}
    </IconButton>
  );
}
