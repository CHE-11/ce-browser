import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { finishPresentPlace } from "@/lib/browser-client";
import type { PresentCursor, PresentPointer, PresentRect, PresentTool } from "@type/browser";
import { LENS_HEIGHT, LENS_WIDTH, presentBox, presentDraws, type PresentMark, type PresentPoint } from "./present-model";

type PresentStageProps = {
  tool: PresentTool | string;
  cursor: PresentCursor | string;
  color: string;
  placing: boolean;
  clearToken: number;
  spotlight: PresentRect | null;
  zoom: PresentRect | null;
  pointer: PresentPointer | null;
};

function rgba(color: string, alpha: number) {
  const value = Number.parseInt(color.slice(1), 16);
  if (!Number.isFinite(value)) return color;
  return `rgba(${(value >> 16) & 255}, ${(value >> 8) & 255}, ${value & 255}, ${alpha})`;
}

function nearSegment(point: PresentPoint, start: PresentPoint, end: PresentPoint, radius: number) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const len2 = dx * dx + dy * dy;
  const t = len2 <= 1e-8 ? 0 : Math.min(1, Math.max(0, ((point.x - start.x) * dx + (point.y - start.y) * dy) / len2));
  const px = start.x + dx * t - point.x;
  const py = start.y + dy * t - point.y;
  return px * px + py * py <= radius * radius;
}

function hits(mark: PresentMark, point: PresentPoint) {
  const first = mark.points[0];
  const last = mark.points[mark.points.length - 1];
  if (!first || !last) return false;
  if (mark.tool === "highlight") {
    return point.x >= Math.min(first.x, last.x) && point.x <= Math.max(first.x, last.x) && point.y >= Math.min(first.y, last.y) && point.y <= Math.max(first.y, last.y);
  }
  if (mark.tool === "text") {
    const width = Math.max(0.05, mark.text.length * 0.012);
    return point.x >= first.x && point.x <= first.x + width && point.y >= first.y - 0.04 && point.y <= first.y + 0.012;
  }
  const radius = 0.014;
  if (mark.tool === "arrow" || mark.tool === "lock") return nearSegment(point, first, last, radius);
  return mark.points.some((sample, index) => nearSegment(point, mark.points[index - 1] ?? sample, sample, radius));
}

function drawMark(context: CanvasRenderingContext2D, canvas: HTMLCanvasElement, mark: PresentMark) {
  const first = mark.points[0];
  const last = mark.points[mark.points.length - 1];
  if (!first || !last) return;
  const xOf = (point: PresentPoint) => point.x * canvas.width;
  const yOf = (point: PresentPoint) => point.y * canvas.height;
  context.save();
  context.strokeStyle = mark.color;
  context.fillStyle = mark.color;
  context.lineWidth = Math.max(2, (mark.tool === "highlight" ? 0.003 : 0.004) * canvas.width);
  context.lineCap = "round";
  context.lineJoin = "round";
  if (mark.tool === "highlight" || mark.tool === "lock") {
    const x = xOf(first);
    const y = yOf(first);
    const width = xOf(last) - x;
    const height = yOf(last) - y;
    if (mark.tool === "highlight") {
      context.fillStyle = rgba(mark.color, 0.28);
      context.fillRect(x, y, width, height);
    } else {
      context.setLineDash([8, 6]);
    }
    context.strokeRect(x, y, width, height);
  } else if (mark.tool === "arrow") {
    const x0 = xOf(first);
    const y0 = yOf(first);
    const x1 = xOf(last);
    const y1 = yOf(last);
    context.beginPath();
    context.moveTo(x0, y0);
    context.lineTo(x1, y1);
    context.stroke();
    const angle = Math.atan2(y1 - y0, x1 - x0);
    const length = Math.max(14, context.lineWidth * 4);
    context.beginPath();
    context.moveTo(x1, y1);
    context.lineTo(x1 - length * Math.cos(angle - 0.45), y1 - length * Math.sin(angle - 0.45));
    context.moveTo(x1, y1);
    context.lineTo(x1 - length * Math.cos(angle + 0.45), y1 - length * Math.sin(angle + 0.45));
    context.stroke();
  } else if (mark.tool === "text") {
    context.font = `${Math.max(16, canvas.width * 0.018)}px Segoe UI, sans-serif`;
    context.fillText(mark.text, xOf(first), yOf(first));
  } else if (mark.points.length === 1) {
    context.beginPath();
    context.arc(xOf(first), yOf(first), context.lineWidth / 2, 0, Math.PI * 2);
    context.fill();
  } else {
    context.beginPath();
    mark.points.forEach((point, index) => {
      const x = xOf(point);
      const y = yOf(point);
      if (index === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    });
    context.stroke();
  }
  context.restore();
}

function pointOf(event: React.PointerEvent<HTMLCanvasElement>): PresentPoint {
  const box = event.currentTarget.getBoundingClientRect();
  return {
    x: box.width ? (event.clientX - box.left) / box.width : 0,
    y: box.height ? (event.clientY - box.top) / box.height : 0,
  };
}

export function PresentStage({ tool, cursor, color, placing, clearToken, spotlight, zoom, pointer }: PresentStageProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const seenClear = useRef(clearToken);
  const serial = useRef(0);
  const toolRef = useRef(tool);
  const colorRef = useRef(color);
  const placingRef = useRef(placing);
  toolRef.current = tool;
  colorRef.current = color;
  placingRef.current = placing;
  const [marks, setMarks] = useState<PresentMark[]>([]);
  const [draft, setDraft] = useState<PresentMark | null>(null);
  const [editor, setEditor] = useState<PresentPoint | null>(null);
  const points = useRef<PresentPoint[]>([]);
  const drawing = useRef(false);
  const kind = useRef<PresentMark["tool"]>("pen");
  const committing = useRef(false);

  useEffect(() => {
    if (seenClear.current === clearToken) return;
    seenClear.current = clearToken;
    points.current = [];
    drawing.current = false;
    setMarks([]);
    setDraft(null);
  }, [clearToken]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const paint = () => {
      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;
      const context = canvas.getContext("2d");
      if (!context) return;
      for (const mark of marks) drawMark(context, canvas, mark);
      if (draft) drawMark(context, canvas, draft);
    };
    paint();
    window.addEventListener("resize", paint);
    return () => window.removeEventListener("resize", paint);
  }, [marks, draft]);

  useEffect(() => {
    if (!editor) return;
    void getCurrentWindow().setFocus();
  }, [editor]);

  const erase = (point: PresentPoint) => {
    setMarks((current) => current.filter((mark) => !hits(mark, point)));
  };

  const begin = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const current = toolRef.current;
    const locking = (current === "spotlight" || current === "zoom") && (placingRef.current || event.altKey);
    if (!locking && !presentDraws(current)) return;
    try {
      event.currentTarget.setPointerCapture(event.pointerId);
    } catch {
      // A synthetic event cannot capture the pointer. The drag still lands on this canvas.
    }
    const point = pointOf(event);
    if (current === "eraser" && !locking) {
      drawing.current = true;
      kind.current = "pen";
      points.current = [];
      erase(point);
      return;
    }
    if (current === "text" && !locking) {
      committing.current = false;
      setEditor(point);
      return;
    }
    drawing.current = true;
    points.current = [point];
    kind.current = locking ? "lock" : current === "highlight" ? "highlight" : current === "arrow" ? "arrow" : "pen";
    setDraft({ id: "present-draft", tool: kind.current, color: colorRef.current, text: "", points: [point] });
  };

  const move = (event: React.PointerEvent<HTMLCanvasElement>) => {
    if (!drawing.current) return;
    const point = pointOf(event);
    if (toolRef.current === "eraser") {
      erase(point);
      return;
    }
    const shaped = kind.current === "arrow" || kind.current === "highlight" || kind.current === "lock";
    const next = shaped ? [points.current[0] ?? point, point] : [...points.current, point];
    points.current = next;
    setDraft({ id: "present-draft", tool: kind.current, color: colorRef.current, text: "", points: next });
  };

  const end = () => {
    if (!drawing.current) return;
    drawing.current = false;
    if (toolRef.current === "eraser") {
      points.current = [];
      setDraft(null);
      return;
    }
    const used = points.current;
    const shape = kind.current;
    points.current = [];
    setDraft(null);
    if (shape === "lock") {
      const lock = toolRef.current === "zoom" ? "zoom" : "spotlight";
      void finishPresentPlace(lock, presentBox(used));
      return;
    }
    if ((shape === "highlight" || shape === "arrow") && !presentBox(used)) return;
    serial.current += 1;
    setMarks((current) => [...current, { id: `present-mark-${serial.current}`, tool: shape, color: colorRef.current, text: "", points: used }]);
  };

  const commitText = (value: string) => {
    if (committing.current) return;
    committing.current = true;
    const at = editor;
    setEditor(null);
    const text = value.trim();
    if (!at || !text) return;
    serial.current += 1;
    setMarks((current) => [...current, { id: `present-mark-${serial.current}`, tool: "text", color: colorRef.current, text, points: [at] }]);
  };

  const lens = zoomFrame(zoom, pointer);
  const cursorClass = placing || presentDraws(tool) ? (tool === "eraser" ? "cursor-cell" : "cursor-crosshair") : "cursor-default";

  return (
    <div id="present-overlay" className="fixed inset-0 h-screen w-screen overflow-hidden bg-transparent">
      {tool === "spotlight" ? <SpotlightHole lock={spotlight} pointer={pointer} /> : null}
      {lens ? (
        <div id="present-zoom-frame" className="pointer-events-none absolute rounded-md border-[3px] border-amber-300 shadow-[0_0_0_1px_rgba(0,0,0,0.45)]" style={lens} />
      ) : null}
      <canvas id="present-stage" ref={canvasRef} className={`absolute inset-0 h-full w-full touch-none ${cursorClass}`} onPointerDown={begin} onPointerMove={move} onPointerUp={end} onPointerCancel={end} />
      <PresentCursorMark cursor={cursor} pointer={pointer} />
      {placing ? (
        <p id="present-place-hint" className="pointer-events-none absolute top-4 left-1/2 z-20 -translate-x-1/2 rounded-full bg-zinc-950/90 px-3 py-1 text-xs text-zinc-100">
          Drag a rectangle. Click the tool again to follow.
        </p>
      ) : null}
      {editor ? (
        <input
          id="present-text-input"
          className="absolute z-20 min-w-24 rounded bg-zinc-950/90 px-1.5 py-0.5 text-sm outline-none ring-1 ring-white/30"
          style={{ left: `${editor.x * 100}%`, top: `${editor.y * 100}%`, color }}
          autoFocus
          aria-label="Label"
          onKeyDown={(event) => {
            if (event.key === "Enter") commitText(event.currentTarget.value);
          }}
          onBlur={(event) => commitText(event.currentTarget.value)}
        />
      ) : null}
    </div>
  );
}

function SpotlightHole({ lock, pointer }: { lock: PresentRect | null; pointer: PresentPointer | null }) {
  if (lock) {
    return (
      <div
        id="present-spotlight"
        className="pointer-events-none absolute rounded-lg"
        style={{
          left: `${lock.x * 100}%`,
          top: `${lock.y * 100}%`,
          width: `${lock.width * 100}%`,
          height: `${lock.height * 100}%`,
          boxShadow: "0 0 0 100vmax rgba(0,0,0,0.62)",
        }}
      />
    );
  }
  if (!pointer?.inside) return null;
  const size = 240;
  return (
    <div
      id="present-spotlight"
      className="pointer-events-none absolute rounded-full"
      style={{ left: pointer.x - size / 2, top: pointer.y - size / 2, width: size, height: size, boxShadow: "0 0 0 100vmax rgba(0,0,0,0.62)" }}
    />
  );
}

function zoomFrame(lock: PresentRect | null, pointer: PresentPointer | null): { left: number | string; top: number | string; width: number | string; height: number | string } | null {
  if (lock) {
    return { left: `${lock.x * 100}%`, top: `${lock.y * 100}%`, width: `${lock.width * 100}%`, height: `${lock.height * 100}%` };
  }
  if (!pointer?.inside) return null;
  const left = Math.min(Math.max(0, pointer.x - LENS_WIDTH / 2), Math.max(0, window.innerWidth - LENS_WIDTH));
  const top = Math.min(Math.max(0, pointer.y - LENS_HEIGHT / 2), Math.max(0, window.innerHeight - LENS_HEIGHT));
  return { left, top, width: LENS_WIDTH, height: LENS_HEIGHT };
}

function PresentCursorMark({ cursor, pointer }: { cursor: string; pointer: PresentPointer | null }) {
  if (cursor === "off" || !pointer?.inside) return null;
  const style = { left: pointer.x, top: pointer.y };
  if (cursor === "ring") {
    return (
      <span id="present-cursor" className="pointer-events-none absolute size-9 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_0_0_1px_rgba(0,0,0,0.7)]" style={style}>
        <span id="present-cursor-center" className="absolute top-1/2 left-1/2 size-1.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-zinc-950" />
      </span>
    );
  }
  if (cursor === "arrow") {
    return (
      <svg id="present-cursor" className="pointer-events-none absolute drop-shadow" style={style} width="22" height="22" viewBox="0 0 22 22" aria-hidden>
        <path id="present-cursor-arrow" d="M2 2 L2 18 L7 13 L11 20 L14 18 L10 12 L17 12 Z" fill="#fb7185" stroke="white" strokeWidth="1.2" />
      </svg>
    );
  }
  if (cursor === "crosshair") {
    return (
      <span id="present-cursor" className="pointer-events-none absolute size-7 -translate-x-1/2 -translate-y-1/2" style={style}>
        <span id="present-cursor-x" className="absolute top-1/2 right-0 left-0 h-0.5 -translate-y-1/2 bg-white shadow-[0_0_0_1px_rgba(0,0,0,0.7)]" />
        <span id="present-cursor-y" className="absolute top-0 bottom-0 left-1/2 w-0.5 -translate-x-1/2 bg-white shadow-[0_0_0_1px_rgba(0,0,0,0.7)]" />
      </span>
    );
  }
  return <span id="present-cursor" className="pointer-events-none absolute size-4 -translate-x-1/2 -translate-y-1/2 rounded-full bg-rose-500 ring-2 ring-white" style={style} />;
}
