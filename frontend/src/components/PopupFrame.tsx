import { useCallback, useContext, useEffect, useLayoutEffect, useRef, type ReactNode } from "react";
import { closePopup, resizePopup } from "../lib/browser-client";
import { cn } from "../lib/cn";
import { PopupGenerationContext, usePopupPainted } from "../lib/popup-ready";

/** Background, corners and border every popup surface shares. */
export const popupSurfaceClass = "overflow-hidden rounded-xl bg-zinc-900 text-zinc-100 ring-1 ring-inset ring-zinc-700";

type PopupFrameProps = {
  id: string;
  children: ReactNode;
  className?: string;
  /** Grow or shrink the native popup to the content height. */
  fit?: boolean;
  maxHeight?: number;
  /** The content shows real data, not a loading placeholder. The popup stays offscreen until then. */
  ready?: boolean;
};

export function PopupFrame({ id, children, className, fit = true, maxHeight = 640, ready = true }: PopupFrameProps) {
  const ref = useRef<HTMLDivElement>(null);
  const lastResize = useRef<Promise<unknown>>(Promise.resolve());
  const generation = useContext(PopupGenerationContext);
  const settle = useCallback(() => lastResize.current, []);

  usePopupPainted(ready, settle);

  useLayoutEffect(() => {
    const element = ref.current;
    if (!fit || !element) {
      return;
    }
    let last = 0;
    const report = () => {
      // The border box, not scrollHeight. scrollHeight can report the previous popup's
      // viewport while this one is still laying out, which leaves an empty slab under the content.
      const measured = Math.ceil(Math.max(element.offsetHeight, element.getBoundingClientRect().height));
      const height = Math.min(maxHeight, measured);
      if (height < 1 || Math.abs(height - last) < 1) {
        return;
      }
      last = height;
      lastResize.current = resizePopup(height, undefined, generation > 0 ? generation : undefined).catch(() => undefined);
    };
    report();
    const observer = new ResizeObserver(report);
    observer.observe(element);
    return () => observer.disconnect();
  }, [fit, maxHeight, generation]);

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
      ref={ref}
      style={fit ? { maxHeight, overflowX: "hidden", overflowY: "auto" } : undefined}
      className={cn(popupSurfaceClass, !fit && "flex h-full flex-col", className)}
      onContextMenu={(event) => event.preventDefault()}
    >
      {children}
    </div>
  );
}

export { Menu as PopupMenu, MenuItem as PopupMenuItem, MenuSeparator as PopupMenuSeparator } from "./Menu";
