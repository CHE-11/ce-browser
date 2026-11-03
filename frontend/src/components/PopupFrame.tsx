import { useEffect, useLayoutEffect, useRef, type CSSProperties, type ReactNode } from "react";
import { closeThisPopup, popupMeasured } from "../lib/browser-client";
import { cn } from "../lib/cn";
import { usePopupHost } from "../lib/popup-host";

/** Background and text every popup surface shares. The window draws the corners and border. */
export const popupSurfaceClass = "bg-zinc-900 text-zinc-100";

/** Matches `::-webkit-scrollbar` in index.css. */
const SCROLLBAR_WIDTH = 8;

type PopupFrameProps = {
  id: string;
  children: ReactNode;
  className?: string;
  /** Px at 100% Browser UI size. `menu` fits the longest row, between 180 and 360 px. `fill`
   * takes the `minWidth` the opener asked for. Omitted: the content's own width, up to 480 px. */
  width?: number | "menu" | "fill";
  /** Px at 100% Browser UI size. Taller content scrolls. */
  maxHeight?: number;
  /** The content shows real data, not a loading placeholder. The popup appears once it is. */
  ready?: boolean;
  role?: "dialog";
  labelledBy?: string;
};

function widthLayout(width: PopupFrameProps["width"]): { className: string; style?: CSSProperties } {
  if (typeof width === "number") {
    return { className: "", style: { width: `${width / 16}rem` } };
  }
  if (width === "menu") {
    return { className: "w-max min-w-45 max-w-90" };
  }
  if (width === "fill") {
    return { className: "w-full" };
  }
  return { className: "w-max max-w-120" };
}

/**
 * Popup surface. Its content lays out at its natural size and reports it, so the popup window
 * opens at exactly that size and follows the content as it grows or shrinks.
 */
export function PopupFrame({ id, children, className, width, maxHeight = 640, ready = true, role, labelledBy }: PopupFrameProps) {
  const host = usePopupHost();
  const scrollerRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const token = host?.token;
  const live = ready && (host?.appearanceReady ?? false);

  useLayoutEffect(() => {
    const scroller = scrollerRef.current;
    const content = contentRef.current;
    if (token === undefined || !scroller || !content) {
      return;
    }
    let reported = "";
    const measure = () => {
      const rem = parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
      const limit = (maxHeight * rem) / 16;
      const box = content.getBoundingClientRect();
      // Judged from the content alone: a reused window still has the last popup's size here.
      // The native side widens a popup the screen makes shorter than this.
      const scrolls = box.height > limit;
      // A `fill` popup is as wide as the opener's `minWidth`, which always wins over 1.
      const reportWidth = width === "fill" ? 1 : Math.ceil(box.width + (scrolls ? SCROLLBAR_WIDTH : 0));
      const reportHeight = Math.ceil(Math.min(box.height, limit));
      if (reportWidth < 1 || reportHeight < 1) {
        return;
      }
      const key = `${reportWidth}x${reportHeight}:${live}`;
      if (key === reported) {
        return;
      }
      reported = key;
      void popupMeasured(token, reportWidth, reportHeight, live).catch(() => undefined);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(content);
    observer.observe(scroller);
    return () => observer.disconnect();
  }, [token, live, maxHeight, width]);

  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented) {
        void closeThisPopup();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const layout = widthLayout(width);
  return (
    <div
      id={id}
      role={role}
      aria-labelledby={labelledBy}
      className={cn(popupSurfaceClass, "flex h-screen w-screen flex-col overflow-hidden")}
      onContextMenu={(event) => event.preventDefault()}
    >
      <div id={`${id}-scroller`} ref={scrollerRef} className="min-h-0 flex-1 overflow-anchor-none overflow-x-hidden overflow-y-auto">
        <div id={`${id}-content`} ref={contentRef} className={cn(layout.className, className)} style={layout.style}>
          {children}
        </div>
      </div>
    </div>
  );
}

export { Menu as PopupMenu, MenuItem as PopupMenuItem, MenuSeparator as PopupMenuSeparator } from "./Menu";
