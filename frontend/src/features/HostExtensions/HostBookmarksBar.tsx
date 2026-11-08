import { useLayoutEffect, useRef } from "react";
import { setHostBookmarksBounds } from "@/lib/browser-client";
import { CHROME, metric } from "@/lib/chrome-metrics";

type HostBookmarksBarProps = {
  extensionId: string;
  height?: number;
};

/** Reserves the bookmarks bar and asks the host to draw the extension there. */
export function HostBookmarksBar({ extensionId, height }: HostBookmarksBarProps) {
  const ref = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const node = ref.current;
    if (!node) {
      return;
    }
    const report = () => {
      const header = node.closest("#browser-header");
      const hidden = header instanceof HTMLElement && header.classList.contains("hidden");
      const rect = node.getBoundingClientRect();
      if (hidden || rect.width < 1 || rect.height < 1) {
        void setHostBookmarksBounds(0, 0, 0, 0);
        return;
      }
      void setHostBookmarksBounds(rect.x, rect.y, rect.width, rect.height);
    };
    report();
    const observer = new ResizeObserver(report);
    observer.observe(node);
    const header = node.closest("#browser-header");
    if (header instanceof HTMLElement) {
      observer.observe(header);
    }
    window.addEventListener("resize", report);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", report);
      void setHostBookmarksBounds(0, 0, 0, 0);
    };
  }, [extensionId]);

  return (
    <div
      id="host-bookmarks-bar"
      ref={ref}
      style={{ height: `${metric(height, CHROME.bookmarksBarHeight)}rem` }}
    />
  );
}
