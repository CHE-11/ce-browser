import { useRef, useState, type PointerEvent } from "react";
import type { SplitView } from "../../../types/browser";
import { setSplitRatio } from "../lib/browser-client";
import { cn } from "../lib/cn";

type SplitDividerProps = {
  split: SplitView;
  activeId: number | null;
};

const MIN_RATIO = 0.2;
const MAX_RATIO = 0.8;

/**
 * Drawn in the browser UI, which shows through the gap the backend leaves between the two
 * page views. Sits inside the page area element, so percentages follow the page area.
 */
export function SplitDivider({ split, activeId }: SplitDividerProps) {
  const [dragRatio, setDragRatio] = useState<number | null>(null);
  const frame = useRef<number | null>(null);
  const pending = useRef<number | null>(null);
  const dragging = useRef(false);
  const ratio = dragRatio ?? split.ratio;
  const leftActive = activeId === split.left;

  const send = (next: number) => {
    pending.current = next;
    if (frame.current !== null) {
      return;
    }
    frame.current = window.requestAnimationFrame(() => {
      frame.current = null;
      if (pending.current !== null) {
        void setSplitRatio(pending.current);
      }
    });
  };

  const ratioAt = (event: PointerEvent<HTMLDivElement>) => {
    const area = event.currentTarget.parentElement?.getBoundingClientRect();
    if (!area || area.width <= 0) {
      return ratio;
    }
    return Math.min(MAX_RATIO, Math.max(MIN_RATIO, (event.clientX - area.left) / area.width));
  };

  return (
    <div
      id="split-divider"
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize split view. Double-click to reset."
      title="Drag to resize. Double-click to make both sides equal."
      className="group absolute inset-y-0 z-10 flex w-1.5 cursor-col-resize justify-center"
      style={{ left: `calc(${ratio * 100}% - 3px)` }}
      onPointerDown={(event) => {
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
        dragging.current = true;
      }}
      onPointerMove={(event) => {
        if (!dragging.current) {
          return;
        }
        const next = ratioAt(event);
        setDragRatio(next);
        send(next);
      }}
      onPointerUp={(event) => {
        if (!dragging.current) {
          return;
        }
        dragging.current = false;
        event.currentTarget.releasePointerCapture(event.pointerId);
        const next = ratioAt(event);
        void setSplitRatio(next).finally(() => setDragRatio(null));
      }}
      onPointerCancel={() => {
        dragging.current = false;
        setDragRatio(null);
      }}
      onDoubleClick={() => void setSplitRatio(0.5)}
    >
      <span
        id="split-divider-left-edge"
        className={cn("h-full w-px", leftActive ? "bg-sky-500/70" : "bg-transparent")}
      />
      <span
        id="split-divider-grip"
        className="mx-px h-full w-0.5 rounded-full bg-zinc-700 transition-colors group-hover:bg-sky-500"
      />
      <span
        id="split-divider-right-edge"
        className={cn("h-full w-px", !leftActive ? "bg-sky-500/70" : "bg-transparent")}
      />
    </div>
  );
}
