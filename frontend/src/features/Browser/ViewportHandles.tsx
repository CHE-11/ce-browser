import { useEffect, useRef, useState, type CSSProperties, type PointerEvent } from "react";
import { setViewport } from "@/lib/browser-client";

type ViewportHandlesProps = {
  width: number;
  height: number;
  cutout: boolean;
  homeBar: boolean;
};

/** Matches the device bar and the gutter reserved in the native page layout. */
const BAR = 36;
const GUTTER = 12;
const MIN_SIZE = 200;
const MAX_SIZE = 3840;

type HandleId = "n" | "s" | "e" | "w" | "nw" | "ne" | "sw" | "se";

const HANDLES: { id: HandleId; label: string; cursor: string; grip: "row" | "col" | "corner" }[] = [
  { id: "n", label: "Resize from the top", cursor: "cursor-ns-resize", grip: "row" },
  { id: "s", label: "Resize from the bottom", cursor: "cursor-ns-resize", grip: "row" },
  { id: "e", label: "Resize from the right", cursor: "cursor-ew-resize", grip: "col" },
  { id: "w", label: "Resize from the left", cursor: "cursor-ew-resize", grip: "col" },
  { id: "nw", label: "Resize from the top left", cursor: "cursor-nwse-resize", grip: "corner" },
  { id: "ne", label: "Resize from the top right", cursor: "cursor-nesw-resize", grip: "corner" },
  { id: "sw", label: "Resize from the bottom left", cursor: "cursor-nesw-resize", grip: "corner" },
  { id: "se", label: "Resize from the bottom right", cursor: "cursor-nwse-resize", grip: "corner" },
];

type Frame = {
  x: number;
  y: number;
  width: number;
  height: number;
  maxWidth: number;
  maxHeight: number;
};

/** Same centering as the native page view: under the bar, with a gutter the webview does not cover. */
function deviceFrame(cssWidth: number, cssHeight: number, areaWidth: number, areaHeight: number): Frame {
  const maxWidth = Math.max(1, areaWidth - GUTTER * 2);
  const maxHeight = Math.max(1, areaHeight - BAR - GUTTER * 2);
  const width = Math.min(Math.max(cssWidth, 1), maxWidth);
  const height = Math.min(Math.max(cssHeight, 1), maxHeight);
  return {
    x: GUTTER + (maxWidth - width) / 2,
    y: BAR + GUTTER + (maxHeight - height) / 2,
    width,
    height,
    maxWidth,
    maxHeight,
  };
}

function clampDimension(value: number, available: number) {
  const upper = Math.min(MAX_SIZE, Math.max(MIN_SIZE, Math.floor(available)));
  return Math.min(upper, Math.max(MIN_SIZE, Math.round(value)));
}

function resized(
  id: HandleId,
  startWidth: number,
  startHeight: number,
  dx: number,
  dy: number,
  maxWidth: number,
  maxHeight: number,
) {
  let width = startWidth;
  let height = startHeight;
  if (id === "e" || id === "ne" || id === "se") {
    width = startWidth + dx * 2;
  }
  if (id === "w" || id === "nw" || id === "sw") {
    width = startWidth - dx * 2;
  }
  if (id === "s" || id === "se" || id === "sw") {
    height = startHeight + dy * 2;
  }
  if (id === "n" || id === "ne" || id === "nw") {
    height = startHeight - dy * 2;
  }
  return {
    width: clampDimension(width, maxWidth),
    height: clampDimension(height, maxHeight),
  };
}

function handleStyle(id: HandleId, frame: Frame): CSSProperties {
  const { x, y, width, height } = frame;
  switch (id) {
    case "n":
      return { left: x, top: y - GUTTER, width, height: GUTTER };
    case "s":
      return { left: x, top: y + height, width, height: GUTTER };
    case "w":
      return { left: x - GUTTER, top: y, width: GUTTER, height };
    case "e":
      return { left: x + width, top: y, width: GUTTER, height };
    case "nw":
      return { left: x - GUTTER, top: y - GUTTER, width: GUTTER, height: GUTTER };
    case "ne":
      return { left: x + width, top: y - GUTTER, width: GUTTER, height: GUTTER };
    case "sw":
      return { left: x - GUTTER, top: y + height, width: GUTTER, height: GUTTER };
    case "se":
      return { left: x + width, top: y + height, width: GUTTER, height: GUTTER };
  }
}

/**
 * Chrome-style grips around a sized page. They sit in the gutter outside the native webview,
 * which paints above this UI. Dragging an edge resizes both sides so the page stays centered.
 */
export function ViewportHandles({ width, height, cutout, homeBar }: ViewportHandlesProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const chrome = useRef({ cutout, homeBar });
  chrome.current = { cutout, homeBar };
  const areaRef = useRef({ width: 0, height: 0 });
  const drag = useRef<{ id: HandleId; x: number; y: number; width: number; height: number } | null>(null);
  const pending = useRef<{ width: number; height: number } | null>(null);
  const frame = useRef<number | null>(null);
  const [area, setArea] = useState({ width: 0, height: 0 });
  const [live, setLive] = useState<{ width: number; height: number } | null>(null);
  areaRef.current = area;

  useEffect(() => {
    const pages = rootRef.current?.parentElement;
    if (!pages) {
      return;
    }
    const measure = () => {
      const rect = pages.getBoundingClientRect();
      setArea({ width: rect.width, height: rect.height });
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(pages);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (live && !drag.current && live.width === width && live.height === height) {
      setLive(null);
    }
  }, [height, live, width]);

  useEffect(() => {
    return () => {
      if (frame.current !== null) {
        window.cancelAnimationFrame(frame.current);
      }
    };
  }, []);

  const send = (next: { width: number; height: number }) => {
    pending.current = next;
    if (frame.current !== null) {
      return;
    }
    frame.current = window.requestAnimationFrame(() => {
      frame.current = null;
      const size = pending.current;
      if (!size) {
        return;
      }
      void setViewport(size.width, size.height, chrome.current.cutout, chrome.current.homeBar);
    });
  };

  const finish = (event: PointerEvent<HTMLDivElement>) => {
    const start = drag.current;
    if (!start) {
      return;
    }
    drag.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    if (frame.current !== null) {
      window.cancelAnimationFrame(frame.current);
      frame.current = null;
    }
    pending.current = null;
    const box = deviceFrame(width, height, areaRef.current.width, areaRef.current.height);
    const next = resized(start.id, start.width, start.height, event.clientX - start.x, event.clientY - start.y, box.maxWidth, box.maxHeight);
    setLive(next);
    void setViewport(next.width, next.height, chrome.current.cutout, chrome.current.homeBar).catch(() => setLive(null));
  };

  const shown = live ?? { width, height };
  const box = deviceFrame(shown.width, shown.height, area.width, area.height);
  if (area.width <= 0 || area.height <= 0) {
    return <div id="viewport-handles" ref={rootRef} className="pointer-events-none absolute inset-0" />;
  }

  return (
    <div id="viewport-handles" ref={rootRef} className="pointer-events-none absolute inset-0 z-10">
      {HANDLES.map((handle) => (
        <div
          id={`viewport-handle-${handle.id}`}
          key={handle.id}
          role="separator"
          aria-label={handle.label}
          aria-orientation={handle.grip === "col" ? "vertical" : "horizontal"}
          title="Drag to resize"
          className={`group pointer-events-auto absolute flex touch-none items-center justify-center ${handle.cursor}`}
          style={handleStyle(handle.id, box)}
          onPointerDown={(event) => {
            event.preventDefault();
            event.stopPropagation();
            event.currentTarget.setPointerCapture(event.pointerId);
            const current = deviceFrame(width, height, areaRef.current.width, areaRef.current.height);
            drag.current = {
              id: handle.id,
              x: event.clientX,
              y: event.clientY,
              width: current.width,
              height: current.height,
            };
            setLive({ width: current.width, height: current.height });
          }}
          onPointerMove={(event) => {
            const start = drag.current;
            if (!start || start.id !== handle.id) {
              return;
            }
            const current = deviceFrame(width, height, areaRef.current.width, areaRef.current.height);
            const next = resized(
              start.id,
              start.width,
              start.height,
              event.clientX - start.x,
              event.clientY - start.y,
              current.maxWidth,
              current.maxHeight,
            );
            setLive(next);
            send(next);
          }}
          onPointerUp={finish}
          onPointerCancel={() => {
            drag.current = null;
            setLive(null);
          }}
        >
          <span
            id={`viewport-handle-${handle.id}-grip`}
            className={
              handle.grip === "row"
                ? "h-1 w-8 rounded-full bg-zinc-400 group-hover:bg-sky-500"
                : handle.grip === "col"
                  ? "h-8 w-1 rounded-full bg-zinc-400 group-hover:bg-sky-500"
                  : "size-2 rounded-sm bg-zinc-400 group-hover:bg-sky-500"
            }
          />
        </div>
      ))}
    </div>
  );
}
