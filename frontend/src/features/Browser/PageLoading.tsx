import { useEffect, useRef, useState, type CSSProperties } from "react";
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

/** Tabs that have already shown a document. Later loads keep that page up, so they get no full-page loader. */
export function useShownPages(tabs: TabInfo[]) {
  const shown = useRef(new Set<number>());
  const live = new Set<number>();
  for (const tab of tabs) {
    live.add(tab.id);
    if (!tab.loaded || tab.failure) {
      shown.current.delete(tab.id);
    } else if (!tab.loading) {
      shown.current.add(tab.id);
    }
  }
  for (const id of [...shown.current]) {
    if (!live.has(id)) {
      shown.current.delete(id);
    }
  }
  return shown.current;
}

export function pagePending(tab: TabInfo | undefined, shown: ReadonlySet<number>): tab is TabInfo {
  return !!tab && !tab.failure && !shown.has(tab.id) && (!tab.loaded || tab.loading);
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
