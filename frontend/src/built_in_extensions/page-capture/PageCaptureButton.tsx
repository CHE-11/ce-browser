import { IconButton } from "@/components/IconButton";
import {
  openPopup,
  saveSettings,
  defaultDownloadDir,
  chooseSavePath,
  capturePagePng,
  copyPagePng,
  captureRegion,
  storeVisiblePng,
  lastCapturePng,
  createTab,
  startRecording,
  stopRecording,
  onRecordingStatus,
  savePageHtml,
  savePageMarkdown,
  ocrVisible,
  ocrRegion,
} from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { TabInfo, BrowserSettings, PageToolRequest, RecordingStatus } from "@type/browser";
import { LoaderCircle, Check, ImageDown, Square } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { tabParams, joinPath, fileNameFor, capturable } from "../shared";
import { useToolStatus, errorText, usePageToolRequests } from "../use-page-tool";


type PageCaptureButtonProps = {
  activeTab: TabInfo | undefined;
  settings: BrowserSettings | null;
};

export function PageCaptureButton({ activeTab, settings }: PageCaptureButtonProps) {
  const buttonRef = useRef<HTMLButtonElement>(null);
  const [status, setStatus] = useToolStatus();
  const [recording, setRecording] = useState<RecordingStatus | null>(null);
  const settingsRef = useRef(settings);
  settingsRef.current = settings;

  const showPopover = (params: URLSearchParams) => {
    const anchor = buttonRef.current;
    if (anchor) {
      void openPopup("page-capture", anchor, { params: params.toString() });
    }
  };

  useEffect(() => {
    const unlisten = onRecordingStatus((next) => {
      setRecording(next.running ? next : null);
      if (!next.running && next.error) {
        setStatus("failed");
      }
      if (!next.running && next.path) {
        const result = new URLSearchParams();
        result.set("status", "saved");
        result.set("path", next.path);
        result.set("size", "GIF");
        showPopover(result);
      }
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  const open = () => {
    if (!activeTab) {
      return;
    }
    const capture = settings?.pageCapture ?? { fullPage: true, loadLazy: true };
    const params = new URLSearchParams(tabParams(activeTab));
    params.set("fullPage", String(capture.fullPage));
    params.set("loadLazy", String(capture.loadLazy));
    if (recording?.running) {
      params.set("recording", "true");
      params.set("seconds", String(recording.secondsLeft));
    }
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
      const folder = settingsRef.current?.downloadDir || (await defaultDownloadDir());
      if (request.action === "save") {
        const path = await chooseSavePath(joinPath(folder, fileNameFor(request.title, request.url, "png")), "PNG image", ["png"]);
        if (!path) {
          return;
        }
        setStatus("busy");
        const saved = await capturePagePng(request.tabId, request.fullPage, request.loadLazy, path);
        result.set("status", "saved");
        result.set("path", saved.path ?? path);
        result.set("size", `${saved.width} × ${saved.height}`);
        result.set("truncated", String(saved.truncated));
      } else if (request.action === "copy") {
        setStatus("busy");
        const copied = await copyPagePng(request.tabId, request.fullPage, request.loadLazy);
        result.set("status", "copied");
        result.set("size", `${copied.width} × ${copied.height}`);
        result.set("truncated", String(copied.truncated));
      } else if (request.action === "region") {
        setStatus("busy");
        await captureRegion(request.tabId);
        await createTab("ce://capture-editor");
        setStatus("done");
        return;
      } else if (request.action === "edit") {
        setStatus("busy");
        const existing = await lastCapturePng().catch(() => "");
        if (!existing) {
          await storeVisiblePng(request.tabId);
        }
        await createTab("ce://capture-editor");
        setStatus("done");
        return;
      } else if (request.action === "record-tab" || request.action === "record-area" || request.action === "record-window") {
        const path = await chooseSavePath(joinPath(folder, fileNameFor(request.title, request.url, "gif")), "GIF", ["gif"]);
        if (!path) {
          return;
        }
        const kind = request.action === "record-window" ? "window" : request.action === "record-area" ? "area" : "tab";
        setStatus("busy");
        await startRecording(request.tabId, kind, path);
        setStatus("done");
        return;
      } else if (request.action === "stop-record") {
        await stopRecording();
        return;
      } else if (request.action === "media") {
        const anchor = buttonRef.current;
        if (anchor) {
          void openPopup("media-grid", anchor, { params: `id=${request.tabId}` });
        }
        return;
      } else if (request.action === "save-html" || request.action === "save-markdown") {
        const extension = request.action === "save-html" ? "html" : "md";
        const label = request.action === "save-html" ? "HTML" : "Markdown";
        const path = await chooseSavePath(joinPath(folder, fileNameFor(request.title, request.url, extension)), label, [extension]);
        if (!path) {
          return;
        }
        setStatus("busy");
        if (request.action === "save-html") {
          await savePageHtml(request.tabId, path);
        } else {
          await savePageMarkdown(request.tabId, path);
        }
        result.set("status", "saved");
        result.set("path", path);
        result.set("size", label);
      } else if (request.action === "ocr" || request.action === "ocr-region") {
        setStatus("busy");
        const text = request.action === "ocr" ? await ocrVisible(request.tabId) : await ocrRegion(request.tabId);
        result.set("status", "ocr");
        result.set("message", text.slice(0, 400));
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

  const busy = status === "busy" || recording?.running === true;
  const label = recording?.running
    ? `Recording · ${recording.secondsLeft}s`
    : busy
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
      disabled={(status === "busy" && !recording?.running) || !capturable(activeTab?.url)}
      className={cn(status === "done" && "text-sky-300", status === "failed" && "text-amber-300")}
      onClick={open}
    >
      {recording?.running ? (
        <Square id="address-page-capture-icon" aria-hidden />
      ) : busy ? (
        <LoaderCircle id="address-page-capture-icon" className="animate-spin" aria-hidden />
      ) : status === "done" ? (
        <Check id="address-page-capture-icon" aria-hidden />
      ) : (
        <ImageDown id="address-page-capture-icon" aria-hidden />
      )}
    </IconButton>
  );
}
