import { useEffect, useRef } from "react";
import { addRecordingStroke, eraseRecordingStrokes, onScreenRecording, screenRecordingStatus, setRecordingDraft, setRecordingDrawing } from "@/lib/browser-client";
import type { RecordingStroke } from "@type/browser";

type Point = { x: number; y: number };

const WIDTH: Record<string, number> = {
  pen: 0.004,
  arrow: 0.004,
  rectangle: 0.004,
  highlighter: 0.02,
  eraser: 0.02,
};

const ERASE = 0.02;
const DRAW_CHOICE = "ce-recording-draw";

function chosenStyle(): { tool: string; color: string; width: number } | null {
  try {
    const parsed = JSON.parse(localStorage.getItem(DRAW_CHOICE) || "") as { tool?: string; color?: string; width?: number };
    if (!parsed.tool) return null;
    const width = parsed.width === 0 || parsed.width === 1 || parsed.width === 2 || parsed.width === 3 ? parsed.width : 2;
    return { tool: parsed.tool, color: parsed.color || "#ef4444", width };
  } catch {
    return null;
  }
}

const WIDTH_SCALE: Record<number, number> = { 0: 0.32, 1: 0.55, 2: 1, 3: 2.4 };
const DRAW_UNDO = "ce-recording-undo";
const DRAW_CLEAR = "ce-recording-clear";

function drawStroke(context: CanvasRenderingContext2D, canvas: HTMLCanvasElement, local: RecordingStroke) {
  if (local.points.length === 0 || local.tool === "eraser") return;
  const xOf = (point: Point) => point.x * canvas.width;
  const yOf = (point: Point) => point.y * canvas.height;
  context.strokeStyle = local.color;
  context.fillStyle = local.color;
  context.lineWidth = Math.max(2, local.width * canvas.width);
  context.lineCap = "round";
  context.lineJoin = "round";
  context.globalAlpha = local.tool === "highlighter" ? 0.45 : 1;
  const first = local.points[0];
  const last = local.points[local.points.length - 1];
  if (local.tool === "rectangle") {
    context.strokeRect(xOf(first), yOf(first), xOf(last) - xOf(first), yOf(last) - yOf(first));
  } else if (local.tool === "arrow") {
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
  } else {
    context.beginPath();
    local.points.forEach((point, index) => {
      const x = xOf(point);
      const y = yOf(point);
      if (index === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    });
    context.stroke();
  }
  context.globalAlpha = 1;
}

function nearSegment(point: Point, start: Point, end: Point, radius: number) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const len2 = dx * dx + dy * dy;
  const t = len2 <= 1e-8 ? 0 : Math.min(1, Math.max(0, ((point.x - start.x) * dx + (point.y - start.y) * dy) / len2));
  const px = start.x + dx * t - point.x;
  const py = start.y + dy * t - point.y;
  return px * px + py * py <= radius * radius;
}

function hits(stroke: RecordingStroke, point: Point) {
  const radius = Math.max(ERASE, (stroke.width || 0.004) * 0.5);
  const edge = (start: Point, end: Point) => nearSegment(point, start, end, radius);
  const first = stroke.points[0];
  const last = stroke.points[stroke.points.length - 1];
  if (!first || !last) return false;
  if (stroke.tool === "rectangle") {
    return edge(first, { x: last.x, y: first.y }) || edge({ x: last.x, y: first.y }, last) || edge(last, { x: first.x, y: last.y }) || edge({ x: first.x, y: last.y }, first);
  }
  if (stroke.tool === "arrow") return edge(first, last);
  return stroke.points.some((sample, index) => edge(stroke.points[index - 1] ?? sample, sample));
}

function clearPage() {
  document.documentElement.dataset.recordingClear = "true";
  return () => {
    delete document.documentElement.dataset.recordingClear;
  };
}

export function RecordingDraw() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const style = useRef({ tool: "pen", color: "#ef4444" });
  const points = useRef<Point[]>([]);
  const finished = useRef<RecordingStroke[]>([]);
  const drawing = useRef(false);
  const seenUndo = useRef(Number(localStorage.getItem(DRAW_UNDO) || 0));
  const seenClear = useRef(localStorage.getItem(DRAW_CLEAR) || "");

  useEffect(() => {
    document.title = "Recording draw";
    const restore = clearPage();
    const canvas = canvasRef.current;
    const paintStyle = (tool: string, color: string) => {
      style.current = { tool, color };
      if (canvas) canvas.style.cursor = tool === "eraser" ? "cell" : "crosshair";
    };
    const apply = (status: { tool: string; color: string; strokes: number }) => {
      paintStyle(status.tool, status.color);
    };
    void screenRecordingStatus().then(apply).catch(() => undefined);
    const listen = onScreenRecording(apply);
    const resize = () => {
      if (!canvas) return;
      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;
    };
    resize();
    window.addEventListener("resize", resize);
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.ctrlKey || event.altKey || event.metaKey) return;
      event.preventDefault();
      void setRecordingDrawing(false);
    };
    window.addEventListener("keydown", onKey);
    const takeUndo = () => {
      const next = Number(localStorage.getItem(DRAW_UNDO) || 0);
      const drop = next - seenUndo.current;
      if (drop <= 0) return;
      seenUndo.current = next;
      finished.current.splice(Math.max(0, finished.current.length - drop));
      points.current = [];
      redraw();
    };
    const takeClear = () => {
      const next = localStorage.getItem(DRAW_CLEAR) || "";
      if (!next || next === seenClear.current) return;
      seenClear.current = next;
      finished.current = [];
      points.current = [];
      redraw();
    };
    const onStore = (event: StorageEvent) => {
      if (event.key === DRAW_UNDO) takeUndo();
      if (event.key === DRAW_CLEAR) takeClear();
    };
    window.addEventListener("storage", onStore);
    const poll = window.setInterval(() => {
      takeUndo();
      takeClear();
      void screenRecordingStatus()
        .then((status) => {
          if (!chosenStyle()) paintStyle(status.tool, status.color);
        })
        .catch(() => undefined);
    }, 150);
    return () => {
      restore();
      window.clearInterval(poll);
      window.removeEventListener("resize", resize);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("storage", onStore);
      void listen.then((stop) => stop());
    };
  }, []);

  const stroke = (draft: boolean): RecordingStroke => {
    const chosen = chosenStyle();
    if (chosen) style.current = chosen;
    const tool = style.current.tool;
    const raw = points.current;
    const shape = tool === "rectangle" || tool === "arrow";
    const used = shape && raw.length > 1 ? [raw[0], raw[raw.length - 1]] : raw;
    const scale = WIDTH_SCALE[chosen?.width ?? 2] ?? 1;
    return {
      tool,
      color: style.current.color,
      width: (WIDTH[tool] ?? 0.004) * scale,
      points: used.map((point) => ({ x: point.x, y: point.y })),
      draft,
    };
  };

  const redraw = () => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context) return;
    context.clearRect(0, 0, canvas.width, canvas.height);
    for (const done of finished.current) drawStroke(context, canvas, done);
    if (style.current.tool === "eraser") return;
    const local = stroke(false);
    if (local.points.length === 0) return;
    drawStroke(context, canvas, local);
  };

  const pointOf = (event: { nativeEvent: { offsetX: number; offsetY: number } }, canvas: HTMLCanvasElement): Point => ({
    x: event.nativeEvent.offsetX / canvas.width,
    y: event.nativeEvent.offsetY / canvas.height,
  });

  const erase = (point: Point) => {
    const before = finished.current.length;
    finished.current = finished.current.filter((done) => !hits(done, point));
    if (finished.current.length !== before) redraw();
    void eraseRecordingStrokes([point]).catch(() => undefined);
  };

  return (
    <canvas
      id="recording-draw-canvas"
      ref={canvasRef}
      className="h-full w-full touch-none"
      onPointerDown={(event) => {
        const chosen = chosenStyle();
        if (chosen) style.current = chosen;
        const canvas = canvasRef.current;
        if (!canvas) return;
        drawing.current = true;
        canvas.setPointerCapture(event.pointerId);
        const point = pointOf(event, canvas);
        if (style.current.tool === "eraser") {
          erase(point);
          return;
        }
        points.current = [point];
        redraw();
      }}
      onPointerMove={(event) => {
        const chosen = chosenStyle();
        if (chosen) style.current = chosen;
        const canvas = canvasRef.current;
        if (!drawing.current || !canvas) return;
        const point = pointOf(event, canvas);
        if (style.current.tool === "eraser") {
          erase(point);
          return;
        }
        points.current.push(point);
        redraw();
        void setRecordingDraft(stroke(true)).catch(() => undefined);
      }}
      onPointerUp={() => {
        if (!drawing.current) return;
        drawing.current = false;
        if (style.current.tool === "eraser") {
          points.current = [];
          return;
        }
        const done = stroke(false);
        points.current = [];
        if (done.points.length > 0) {
          finished.current.push(done);
          void addRecordingStroke(done).catch(() => undefined);
        }
        redraw();
      }}
    />
  );
}
