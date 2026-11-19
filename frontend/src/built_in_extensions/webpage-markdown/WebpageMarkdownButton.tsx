import { useRef } from "react";
import { Check, FileText, LoaderCircle } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import {
  chooseSavePath,
  copyText,
  defaultDownloadDir,
  extractPage,
  openPopup,
  saveSettings,
  writeTextFile,
} from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import type { BrowserSettings, PageMarkdownSettings, PageToolRequest, TabInfo } from "@type/browser";
import { capturable, fileNameFor, joinPath, tabParams } from "../shared";
import { errorText, usePageToolRequests, useToolStatus } from "../use-page-tool";
import { htmlToMarkdown } from "./markdown";

const DEFAULT_MARKDOWN: PageMarkdownSettings = {
  scope: "article",
  images: true,
  links: true,
  header: true,
};

type WebpageMarkdownButtonProps = {
  activeTab: TabInfo | undefined;
  settings: BrowserSettings | null;
};

export function WebpageMarkdownButton({ activeTab, settings }: WebpageMarkdownButtonProps) {
  const buttonRef = useRef<HTMLButtonElement>(null);
  const [status, setStatus] = useToolStatus();
  const settingsRef = useRef(settings);
  settingsRef.current = settings;

  const showPopover = (params: URLSearchParams) => {
    const anchor = buttonRef.current;
    if (anchor) {
      void openPopup("webpage-markdown", anchor, { params: params.toString() });
    }
  };

  const open = () => {
    if (!activeTab) {
      return;
    }
    const saved = settings?.pageMarkdown ?? DEFAULT_MARKDOWN;
    const params = new URLSearchParams(tabParams(activeTab));
    writeOptions(params, saved);
    showPopover(params);
  };

  const remember = (options: PageMarkdownSettings) => {
    const current = settingsRef.current;
    const previous = current?.pageMarkdown;
    if (
      !current ||
      (previous?.scope === options.scope &&
        previous.images === options.images &&
        previous.links === options.links &&
        previous.header === options.header)
    ) {
      return;
    }
    void saveSettings({ ...current, pageMarkdown: options }).catch(() => undefined);
  };

  const run = async (request: Extract<PageToolRequest, { tool: "webpage-markdown" }>) => {
    const options: PageMarkdownSettings = {
      scope: request.scope === "page" ? "page" : "article",
      images: request.images,
      links: request.links,
      header: request.header,
    };
    remember(options);
    const result = new URLSearchParams({
      tab: String(request.tabId),
      title: request.title,
      url: request.url,
    });
    writeOptions(result, options);
    try {
      let path = "";
      if (request.action === "download") {
        const folder = settingsRef.current?.downloadDir || (await defaultDownloadDir());
        const chosen = await chooseSavePath(joinPath(folder, fileNameFor(request.title, request.url, "md")), "Markdown", ["md"]);
        if (!chosen) {
          return;
        }
        path = chosen;
      }
      setStatus("busy");
      const page = await extractPage(request.tabId);
      const markdown = htmlToMarkdown(page.html, page.title || request.title, page.url || request.url, options);
      if (!markdown.trim()) {
        throw new Error("This page has no text to convert.");
      }
      if (request.action === "download") {
        await writeTextFile(path, markdown);
        result.set("status", "saved");
        result.set("path", path);
      } else {
        await copyText(markdown);
        result.set("status", "copied");
      }
      setStatus("done");
    } catch (error) {
      result.set("status", "error");
      result.set("message", errorText(error));
      setStatus("failed");
    }
    showPopover(result);
  };

  usePageToolRequests("webpage-markdown", (request) => void run(request));

  const busy = status === "busy";
  const label = busy
    ? "Converting page…"
    : status === "done"
      ? "Markdown ready"
      : status === "failed"
        ? "Couldn't convert page"
        : "To .md";

  return (
    <IconButton
      id="address-webpage-markdown"
      ref={buttonRef}
      label={label}
      disabled={busy || !capturable(activeTab?.url)}
      className={cn(status === "done" && "text-sky-300", status === "failed" && "text-amber-300")}
      onClick={open}
    >
      {busy ? (
        <LoaderCircle id="address-webpage-markdown-icon" className="animate-spin" aria-hidden />
      ) : status === "done" ? (
        <Check id="address-webpage-markdown-icon" aria-hidden />
      ) : (
        <FileText id="address-webpage-markdown-icon" aria-hidden />
      )}
    </IconButton>
  );
}

function writeOptions(params: URLSearchParams, options: PageMarkdownSettings) {
  params.set("scope", options.scope);
  params.set("images", String(options.images));
  params.set("links", String(options.links));
  params.set("header", String(options.header));
}
