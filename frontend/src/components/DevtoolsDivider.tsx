import { useRef, useState, type PointerEvent } from "react";
import { setDevtoolsRatio } from "../lib/browser-client";

type DevtoolsDividerProps = {
  /** Share of the page area the pages keep. Matches the dock column in the native layout. */
  ratio: number;
};

const MIN_RATIO = 0.35;
const MAX_RATIO = 0.8;
const RESET_RATIO = 0.62;

/**
 * Drawn in the browser UI, which shows through the gap between the page and the docked tools.
 * Sits in the page area, so the percentage is the whole page area, not the pages column.
 */
export function DevtoolsDivider({ ratio: dockRatio }: DevtoolsDividerProps) {
  const [dragRatio, setDragRatio] = useState<number | null>(null);
  const frame = useRef<number | null>(null);
  const pending = useRef<number | null>(null);
  const dragging = useRef(false);
  const ratio = dragRatio ?? dockRatio;

  const send = (next: number) => {
    pending.current = next;
    if (frame.current !== null) {
      return;
    }
    frame.current = window.requestAnimationFrame(() => {
      frame.current = null;
      if (pending.current !== null) {
        void setDevtoolsRatio(pending.current);
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
      id="devtools-divider"
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize developer tools. Double-click to reset."
      title="Drag to resize. Double-click to reset."
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
        void setDevtoolsRatio(next).finally(() => setDragRatio(null));
      }}
      onPointerCancel={() => {
        dragging.current = false;
        setDragRatio(null);
      }}
      onDoubleClick={() => void setDevtoolsRatio(RESET_RATIO)}
    >
      <span
        id="devtools-divider-grip"
        className="h-full w-0.5 rounded-full bg-zinc-700 transition-colors group-hover:bg-sky-500"
      />
    </div>
  );
}
