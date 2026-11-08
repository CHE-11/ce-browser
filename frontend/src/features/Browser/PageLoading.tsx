import { useEffect, useState, type CSSProperties } from "react";
import { LoaderCircle } from "lucide-react";
import type { TabInfo } from "@type/browser";
import { hostOf } from "@/lib/format";
import { cn } from "@/lib/cn";

type PageLoadingProps = {
  tab: TabInfo;
  className?: string;
  style?: CSSProperties;
};

/** The page view is still off screen. A short wait hides the spinner for loads that finish immediately. */
const SPINNER_DELAY_MS = 200;

export function pagePending(tab: TabInfo | undefined): tab is TabInfo {
  return !!tab && !tab.failure && (!tab.loaded || tab.loading);
}

function loadingText(url: string) {
  if (url === "ce://newtab") {
    return "New tab";
  }
  if (url.startsWith("ce://")) {
    const page = url.slice("ce://".length).split(/[/?#]/)[0];
    return page ? page.charAt(0).toUpperCase() + page.slice(1) : "Loading";
  }
  return hostOf(url) || "Loading";
}

export function PageLoading({ tab, className, style }: PageLoadingProps) {
  const [visible, setVisible] = useState(false);
  useEffect(() => {
    const timer = window.setTimeout(() => setVisible(true), SPINNER_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [tab.id, tab.url]);
  if (!visible) {
    return null;
  }
  const rootId = `browser-page-loading-${tab.id}`;
  return (
    <div id={rootId} className={cn("flex items-center justify-center", className)} style={style} role="status">
      <div id={`${rootId}-status`} className="flex max-w-[70%] items-center gap-2 text-sm text-zinc-500">
        <LoaderCircle id={`${rootId}-icon`} className="size-4 shrink-0 animate-spin text-sky-400" aria-hidden />
        <span id={`${rootId}-label`} className="truncate">
          {loadingText(tab.url)}
        </span>
      </div>
    </div>
  );
}
