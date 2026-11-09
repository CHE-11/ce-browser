import { useLayoutEffect, useState, type PointerEvent as ReactPointerEvent } from "react";

type ExtensionsToolbarScrollbarProps = {
  scroller: HTMLDivElement | null;
  axis: "x" | "y";
  /** Changes when the icons do, so the thumb is measured again. */
  items: string;
};

type ScrollbarBox = {
  trackStart: number;
  trackSize: number;
  thumbStart: number;
  thumbSize: number;
};

/** A 2px thumb. The native bar cannot be drawn this thin, and it covered the captions. */
export function ExtensionsToolbarScrollbar({ scroller, axis, items }: ExtensionsToolbarScrollbarProps) {
  const [box, setBox] = useState<ScrollbarBox | null>(null);

  useLayoutEffect(() => {
    if (!scroller) {
      return;
    }
    const update = () => {
      const horizontal = axis === "x";
      const viewport = horizontal ? scroller.clientWidth : scroller.clientHeight;
      const content = horizontal ? scroller.scrollWidth : scroller.scrollHeight;
      const scroll = horizontal ? scroller.scrollLeft : scroller.scrollTop;
      if (viewport <= 0 || content - viewport <= 1) {
        setBox((current) => (current === null ? current : null));
        return;
      }
      const thumbSize = Math.round(Math.max(16, (viewport / content) * viewport));
      const maxScroll = content - viewport;
      const travel = Math.max(viewport - thumbSize, 0);
      const next: ScrollbarBox = {
        trackStart: horizontal ? scroller.offsetLeft : scroller.offsetTop,
        trackSize: viewport,
        thumbSize,
        thumbStart: maxScroll > 0 ? Math.round((scroll / maxScroll) * travel) : 0,
      };
      setBox((current) =>
        current &&
        current.trackStart === next.trackStart &&
        current.trackSize === next.trackSize &&
        current.thumbStart === next.thumbStart &&
        current.thumbSize === next.thumbSize
          ? current
          : next,
      );
    };
    update();
    scroller.addEventListener("scroll", update, { passive: true });
    const observer = new ResizeObserver(update);
    observer.observe(scroller);
    for (const child of scroller.children) {
      observer.observe(child);
    }
    return () => {
      scroller.removeEventListener("scroll", update);
      observer.disconnect();
    };
  }, [scroller, axis, items]);

  if (!box || !scroller) {
    return null;
  }

  const horizontal = axis === "x";

  const drag = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.stopPropagation();
    const pointerStart = horizontal ? event.clientX : event.clientY;
    const scrollStart = horizontal ? scroller.scrollLeft : scroller.scrollTop;
    const viewport = horizontal ? scroller.clientWidth : scroller.clientHeight;
    const content = horizontal ? scroller.scrollWidth : scroller.scrollHeight;
    const travel = Math.max(viewport - box.thumbSize, 1);
    const maxScroll = content - viewport;
    const target = event.currentTarget;
    target.setPointerCapture(event.pointerId);
    const move = (next: PointerEvent) => {
      const delta = (horizontal ? next.clientX : next.clientY) - pointerStart;
      const position = scrollStart + (delta / travel) * maxScroll;
      if (horizontal) {
        scroller.scrollLeft = position;
      } else {
        scroller.scrollTop = position;
      }
    };
    const end = () => {
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", end);
      target.removeEventListener("pointercancel", end);
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", end);
    target.addEventListener("pointercancel", end);
  };

  return (
    <div
      id="tools-toolbar-scrollbar"
      className="pointer-events-none absolute z-20"
      style={
        horizontal
          ? { left: box.trackStart, width: box.trackSize, bottom: 0, height: 1 }
          : { top: box.trackStart, height: box.trackSize, right: 0, width: 1 }
      }
    >
      <div
        id="tools-toolbar-scrollbar-thumb"
        className="pointer-events-auto absolute cursor-default rounded-full bg-zinc-400 hover:bg-zinc-200"
        style={
          horizontal
            ? { left: box.thumbStart, width: box.thumbSize, top: 0, height: 1 }
            : { top: box.thumbStart, height: box.thumbSize, left: 0, width: 1 }
        }
        onPointerDown={drag}
      />
    </div>
  );
}
