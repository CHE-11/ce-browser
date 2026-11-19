import { useRef, type PointerEvent as ReactPointerEvent } from "react";
import { clock } from "./time";

type Handle = "start" | "end";

type TrimBarProps = {
  durationMs: number;
  startMs: number;
  endMs: number;
  currentMs: number;
  onChange: (startMs: number, endMs: number, handle: Handle) => void;
  onCommit: (startMs: number, endMs: number) => void;
};

const GAP_MS = 200;

export function TrimBar({ durationMs, startMs, endMs, currentMs, onChange, onCommit }: TrimBarProps) {
  const trackRef = useRef<HTMLDivElement>(null);
  const drag = useRef<{ handle: Handle; startMs: number; endMs: number } | null>(null);
  const span = Math.max(durationMs, 1);
  const start = Math.min(startMs, span);
  const end = Math.min(Math.max(endMs, start + GAP_MS), span);
  const startPct = (start / span) * 100;
  const endPct = (end / span) * 100;
  const playPct = (Math.min(Math.max(currentMs, 0), span) / span) * 100;

  const msAt = (clientX: number) => {
    const track = trackRef.current;
    if (!track) return 0;
    const box = track.getBoundingClientRect();
    const ratio = box.width <= 0 ? 0 : (clientX - box.left) / box.width;
    return Math.round(Math.min(1, Math.max(0, ratio)) * span);
  };

  const move = (clientX: number) => {
    const current = drag.current;
    if (!current) return;
    const at = msAt(clientX);
    if (current.handle === "start") {
      current.startMs = Math.max(0, Math.min(at, current.endMs - GAP_MS));
    } else {
      current.endMs = Math.min(span, Math.max(at, current.startMs + GAP_MS));
    }
    onChange(current.startMs, current.endMs, current.handle);
  };

  const finish = () => {
    const current = drag.current;
    drag.current = null;
    if (current) onCommit(current.startMs, current.endMs);
  };

  const begin = (handle: Handle, event: ReactPointerEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
    drag.current = { handle, startMs: start, endMs: end };
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  return (
    <div id="recordings-trim" className="px-4 pb-4">
      <div id="recordings-trim-times" className="mb-1.5 flex justify-between text-[11px] tabular-nums text-zinc-500">
        <span id="recordings-trim-start-label">{clock(start)}</span>
        <span id="recordings-trim-end-label">{clock(end)}</span>
      </div>
      <div id="recordings-trim-track" ref={trackRef} className="relative h-8">
        <div id="recordings-trim-rail" className="absolute top-1/2 right-0 left-0 h-1.5 -translate-y-1/2 rounded-full bg-zinc-800" />
        <div
          id="recordings-trim-range"
          className="absolute top-1/2 h-1.5 -translate-y-1/2 rounded-full bg-sky-500"
          style={{ left: `${startPct}%`, width: `${Math.max(endPct - startPct, 0)}%` }}
        />
        <div
          id="recordings-trim-playhead"
          className="pointer-events-none absolute top-1/2 h-3.5 w-px -translate-y-1/2 bg-white/80"
          style={{ left: `${playPct}%` }}
        />
        <button
          id="recordings-trim-start-handle"
          type="button"
          role="slider"
          aria-label="Trim start"
          aria-valuemin={0}
          aria-valuemax={span}
          aria-valuenow={start}
          className="absolute top-1/2 z-10 size-4 -translate-x-1/2 -translate-y-1/2 cursor-ew-resize rounded-full bg-white shadow ring-2 ring-sky-500"
          style={{ left: `${startPct}%` }}
          onPointerDown={(event) => begin("start", event)}
          onPointerMove={(event) => move(event.clientX)}
          onPointerUp={finish}
          onPointerCancel={finish}
        />
        <button
          id="recordings-trim-end-handle"
          type="button"
          role="slider"
          aria-label="Trim end"
          aria-valuemin={0}
          aria-valuemax={span}
          aria-valuenow={end}
          className="absolute top-1/2 z-10 size-4 -translate-x-1/2 -translate-y-1/2 cursor-ew-resize rounded-full bg-white shadow ring-2 ring-sky-500"
          style={{ left: `${endPct}%` }}
          onPointerDown={(event) => begin("end", event)}
          onPointerMove={(event) => move(event.clientX)}
          onPointerUp={finish}
          onPointerCancel={finish}
        />
      </div>
    </div>
  );
}
