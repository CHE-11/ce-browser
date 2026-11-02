import { useEffect, useLayoutEffect, useRef, type ReactNode } from "react";
import { closePopup, fitPopup } from "../lib/browser-client";
import { cn } from "../lib/cn";

/** Background, corners and border every popup surface shares. */
export const popupSurfaceClass = "overflow-hidden rounded-xl bg-zinc-900 text-zinc-100 ring-1 ring-inset ring-zinc-700";

type PopupFrameProps = {
  id: string;
  children: ReactNode;
  className?: string;
  /** Report the content height once so the native popup can match it. */
  fit?: boolean;
  maxHeight?: number;
  /** The content shows real data, not a loading placeholder. */
  ready?: boolean;
};

export function PopupFrame({ id, children, className, fit = true, maxHeight = 640, ready = true }: PopupFrameProps) {
  const contentRef = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const content = contentRef.current;
    if (!fit || !ready || !content) {
      return;
    }
    const height = Math.min(maxHeight, Math.ceil(content.offsetHeight));
    if (height < 1) {
      return;
    }
    void fitPopup(height).catch(() => undefined);
  }, [fit, ready, maxHeight]);

  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented) {
        void closePopup();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div
      id={id}
      className={cn(popupSurfaceClass, "flex h-full flex-col", className)}
      onContextMenu={(event) => event.preventDefault()}
    >
      <div id={`${id}-scroller`} className="min-h-0 flex-1 overflow-y-auto">
        <div id={`${id}-content`} ref={contentRef}>
          {children}
        </div>
      </div>
    </div>
  );
}

export { Menu as PopupMenu, MenuItem as PopupMenuItem, MenuSeparator as PopupMenuSeparator } from "./Menu";
