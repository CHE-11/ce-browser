import { useRef } from "react";
import { Check, ImageDown, LoaderCircle } from "lucide-react";
import type { BrowserSettings, PageToolRequest, TabInfo } from "../../../../types/browser";
import { IconButton } from "../../components/IconButton";
import {
  capturePagePng,
  chooseSavePath,
  copyPagePng,
  defaultDownloadDir,
  openPopupBelow,
  saveSettings,
} from "../../lib/browser-client";
import { cn } from "../../lib/cn";
import { capturable, fileNameFor, joinPath, tabParams } from "../shared";
import { errorText, usePageToolRequests, useToolStatus } from "../use-page-tool";

type PageCaptureButtonProps = {
  activeTab: TabInfo | undefined;
  settings: BrowserSettings | null;
};

const POPOVER_WIDTH = 320;
/** Placeholder only. The popover stays hidden until it reports its real height. */
const POPOVER_HEIGHT = 300;

export function PageCaptureButton({ activeTab, settings }: PageCaptureButtonProps) {
  const buttonRef = useRef<HTMLButtonElement>(null);
  const [status, setStatus] = useToolStatus();
  const settingsRef = useRef(settings);
  settingsRef.current = settings;

  const showPopover = (params: URLSearchParams) => {
    const anchor = buttonRef.current;
    if (anchor) {
      void openPopupBelow(anchor, {
        kind: "page-capture",
        params: params.toString(),
        width: POPOVER_WIDTH,
        height: POPOVER_HEIGHT,
      });
    }
  };

  const open = () => {
    if (!activeTab) {
      return;
    }
    const capture = settings?.pageCapture ?? { fullPage: true, loadLazy: true };
    const params = new URLSearchParams(tabParams(activeTab));
    params.set("fullPage", String(capture.fullPage));
    params.set("loadLazy", String(capture.loadLazy));
    showPopover(params);
  };

  const remember = (fullPage: boolean, loadLazy: boolean) => {
    const current = settingsRef.current;
    if (!current || (current.pageCapture.fullPage === fullPage && current.pageCapture.loadLazy === loadLazy)) {
      return;
    }
    void saveSettings({ ...current, pageCapture: { fullPage, loadLazy } }).catch(() => undefined);
  };

  const run = async (request: Extract<PageToolRequest, { tool: "page-capture" }>) => {
    remember(request.fullPage, request.loadLazy);
    const result = new URLSearchParams({ tab: String(request.tabId), title: request.title, url: request.url });
    result.set("fullPage", String(request.fullPage));
    result.set("loadLazy", String(request.loadLazy));
    try {
      if (request.action === "save") {
        const folder = settingsRef.current?.downloadDir || (await defaultDownloadDir());
        const path = await chooseSavePath(joinPath(folder, fileNameFor(request.title, request.url, "png")), "PNG image", [
          "png",
        ]);
        if (!path) {
          return;
        }
        setStatus("busy");
        const saved = await capturePagePng(request.tabId, request.fullPage, request.loadLazy, path);
        result.set("status", "saved");
        result.set("path", saved.path ?? path);
        result.set("size", `${saved.width} × ${saved.height}`);
        result.set("truncated", String(saved.truncated));
      } else {
        setStatus("busy");
        const copied = await copyPagePng(request.tabId, request.fullPage, request.loadLazy);
        result.set("status", "copied");
        result.set("size", `${copied.width} × ${copied.height}`);
        result.set("truncated", String(copied.truncated));
      }
      setStatus("done");
    } catch (error) {
      result.set("status", "error");
      result.set("message", errorText(error));
      setStatus("failed");
    }
    showPopover(result);
  };

  usePageToolRequests("page-capture", (request) => void run(request));

  const busy = status === "busy";
  const label = busy
    ? "Capturing page…"
    : status === "done"
      ? "Page captured"
      : status === "failed"
        ? "Couldn't capture page"
        : "Save page as PNG";

  return (
    <IconButton
      id="address-page-capture"
      ref={buttonRef}
      label={label}
      disabled={busy || !capturable(activeTab?.url)}
      className={cn(status === "done" && "text-sky-300", status === "failed" && "text-amber-300")}
      onClick={open}
    >
      {busy ? (
        <LoaderCircle id="address-page-capture-icon" className="animate-spin" aria-hidden />
      ) : status === "done" ? (
        <Check id="address-page-capture-icon" aria-hidden />
      ) : (
        <ImageDown id="address-page-capture-icon" aria-hidden />
      )}
    </IconButton>
  );
}
