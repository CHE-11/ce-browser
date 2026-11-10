import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { finishPresentPlace, onPresentDelete, onPresentEscape, onPresentPointer, setPresentHoldEscape, undoPresentMark } from "@/lib/browser-client";
import type { PresentCursor, PresentPointer, PresentRect } from "@type/browser";
import { lassoHits, presentBox, presentDraws, presentSpan, type PresentMark, type PresentPoint } from "./present-model";

type Editor = { x: number; y: number; generation: number };

type PresentStageProps = {
  tool: string;
  cursor: PresentCursor | string;
  color: string;
  width: number;
  dim: number;
  radius: number;
  spread: number;
  circle: boolean;
  placing: boolean;
  clearToken: number;
  undoToken: number;
  spotlight: PresentRect | null;
};

const AWAY: PresentPointer = { x: 0, y: 0, inside: false, lens: null };
const SPOT_SIZE = 240;

function nearSegment(point: PresentPoint, start: PresentPoint, end: PresentPoint, radius: number) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const len2 = dx * dx + dy * dy;
  const t = len2 <= 1e-8 ? 0 : Math.min(1, Math.max(0, ((point.x - start.x) * dx + (point.y - start.y) * dy) / len2));
  const px = start.x + dx * t - point.x;
  const py = start.y + dy * t - point.y;
  return px * px + py * py <= radius * radius;
}

function textSize(cssWidth: number, scale: number) {
  return Math.max(16, cssWidth * 0.018 * scale);
}

function strokeWidth(mark: PresentMark, cssWidth: number) {
  const base = mark.tool === "highlight" || mark.tool === "lock" ? 0.003 : 0.004;
  return Math.max(2, base * mark.width * cssWidth);
}

function hitRadius(mark: PresentMark) {
  const base = mark.tool === "highlight" ? 0.003 : 0.004;
  return Math.max(0.014, base * mark.width);
}

function hits(mark: PresentMark, point: PresentPoint, textBox: { w: number; h: number } | null) {
  const first = mark.points[0];
  const last = mark.points[mark.points.length - 1];
  if (!first || !last) return false;
  if (mark.tool === "highlight") {
    return point.x >= Math.min(first.x, last.x) && point.x <= Math.max(first.x, last.x) && point.y >= Math.min(first.y, last.y) && point.y <= Math.max(first.y, last.y);
  }
  if (mark.tool === "text") {
    if (!textBox) return false;
    return point.x >= first.x && point.x <= first.x + textBox.w && point.y >= first.y && point.y <= first.y + textBox.h;
  }
  const radius = hitRadius(mark);
  if (mark.tool === "arrow" || mark.tool === "lock") return nearSegment(point, first, last, radius);
  return mark.points.some((sample, index) => nearSegment(point, mark.points[index - 1] ?? sample, sample, radius));
}

function drawMark(context: CanvasRenderingContext2D, cssWidth: number, cssHeight: number, mark: PresentMark) {
  const first = mark.points[0];
  const last = mark.points[mark.points.length - 1];
  if (!first || !last) return;
  const xOf = (point: PresentPoint) => point.x * cssWidth;
  const yOf = (point: PresentPoint) => point.y * cssHeight;
  context.save();
  context.strokeStyle = mark.color;
  context.fillStyle = mark.color;
  context.lineWidth = strokeWidth(mark, cssWidth);
  context.lineCap = "round";
  context.lineJoin = "round";
  if (mark.tool === "highlight" || mark.tool === "lock") {
    const x = xOf(first);
    const y = yOf(first);
    const width = xOf(last) - x;
    const height = yOf(last) - y;
    const left = Math.min(x, x + width);
    const top = Math.min(y, y + height);
    const boxWidth = Math.abs(width);
    const boxHeight = Math.abs(height);
    if (mark.tool === "highlight") {
      const radius = Math.min(Math.max(0, mark.radius), boxWidth / 2, boxHeight / 2);
      context.beginPath();
      context.roundRect(left, top, boxWidth, boxHeight, radius);
      context.stroke();
    } else {
      context.setLineDash([8, 6]);
      context.strokeRect(left, top, boxWidth, boxHeight);
    }
  } else if (mark.tool === "arrow") {
    const x0 = xOf(first);
    const y0 = yOf(first);
    const x1 = xOf(last);
    const y1 = yOf(last);
    const head = Math.max(14, context.lineWidth * 4);
    const angle = Math.atan2(y1 - y0, x1 - x0);
    const baseX = x1 - head * 0.55 * Math.cos(angle);
    const baseY = y1 - head * 0.55 * Math.sin(angle);
    context.beginPath();
    context.moveTo(x0, y0);
    context.lineTo(baseX, baseY);
    context.stroke();
    context.beginPath();
    context.moveTo(x1, y1);
    context.lineTo(x1 - head * Math.cos(angle - 0.42), y1 - head * Math.sin(angle - 0.42));
    context.lineTo(x1 - head * Math.cos(angle + 0.42), y1 - head * Math.sin(angle + 0.42));
    context.closePath();
    context.fill();
  } else if (mark.tool === "text") {
    context.font = `${textSize(cssWidth, mark.width)}px Segoe UI, sans-serif`;
    context.textBaseline = "top";
    context.fillText(mark.text, xOf(first), yOf(first));
  } else {
    tracePen(context, mark.points, xOf, yOf);
  }
  context.restore();
}

function drawLasso(context: CanvasRenderingContext2D, cssWidth: number, cssHeight: number, points: PresentPoint[]) {
  const first = points[0];
  if (!first) return;
  context.save();
  context.strokeStyle = "#7dd3fc";
  context.lineWidth = 1.5;
  context.setLineDash([4, 4]);
  context.beginPath();
  points.forEach((point, index) => {
    const x = point.x * cssWidth;
    const y = point.y * cssHeight;
    if (index === 0) context.moveTo(x, y);
    else context.lineTo(x, y);
  });
  if (points.length > 2) context.lineTo(first.x * cssWidth, first.y * cssHeight);
  context.stroke();
  context.restore();
}

function drawSelected(context: CanvasRenderingContext2D, cssWidth: number, cssHeight: number, mark: PresentMark, box: { w: number; h: number } | null) {
  const first = mark.points[0];
  const last = mark.points[mark.points.length - 1];
  if (!first) return;
  context.save();
  context.strokeStyle = "#7dd3fc";
  context.lineWidth = 2;
  context.setLineDash([5, 4]);
  const xOf = (point: PresentPoint) => point.x * cssWidth;
  const yOf = (point: PresentPoint) => point.y * cssHeight;
  if (mark.tool === "highlight" && last) {
    const left = Math.min(xOf(first), xOf(last)) - 4;
    const top = Math.min(yOf(first), yOf(last)) - 4;
    context.strokeRect(left, top, Math.abs(xOf(last) - xOf(first)) + 8, Math.abs(yOf(last) - yOf(first)) + 8);
  } else if (mark.tool === "text" && box) {
    context.strokeRect(xOf(first) - 4, yOf(first) - 4, box.w * cssWidth + 8, box.h * cssHeight + 8);
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

function tracePen(context: CanvasRenderingContext2D, points: PresentPoint[], xOf: (point: PresentPoint) => number, yOf: (point: PresentPoint) => number) {
  const first = points[0];
  if (!first) return;
  if (points.length === 1) {
    context.beginPath();
    context.arc(xOf(first), yOf(first), context.lineWidth / 2, 0, Math.PI * 2);
    context.fill();
    return;
  }
  context.beginPath();
  context.moveTo(xOf(first), yOf(first));
  if (points.length === 2) {
    const last = points[1];
    context.lineTo(xOf(last), yOf(last));
    context.stroke();
    return;
  }
  for (let index = 1; index < points.length - 1; index += 1) {
    const current = points[index];
    const next = points[index + 1];
    context.quadraticCurveTo(xOf(current), yOf(current), (xOf(current) + xOf(next)) / 2, (yOf(current) + yOf(next)) / 2);
  }
  const last = points[points.length - 1];
  context.lineTo(xOf(last), yOf(last));
  context.stroke();
}

function samplesOf(event: React.PointerEvent<HTMLCanvasElement>): PresentPoint[] {
  const box = event.currentTarget.getBoundingClientRect();
  const native = event.nativeEvent;
  const coalesced = typeof native.getCoalescedEvents === "function" ? native.getCoalescedEvents() : [];
  const events = coalesced.length > 0 ? coalesced : [native];
  return events.map((sample) => ({
    x: box.width ? (sample.clientX - box.left) / box.width : 0,
    y: box.height ? (sample.clientY - box.top) / box.height : 0,
  }));
}

function showLaser(tool: string, cursor: string, placing: boolean) {
  if (tool === "zoom" || tool === "none") return false;
  return cursor !== "off" && !placing && !presentDraws(tool);
}

export function PresentStage({ tool, cursor, color, width, dim, radius, spread, circle, placing, clearToken, undoToken, spotlight }: PresentStageProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const cursorRef = useRef<HTMLSpanElement>(null);
  const spotlightRef = useRef<HTMLDivElement>(null);
  const lensRef = useRef<HTMLDivElement>(null);
  const seenClear = useRef(clearToken);
  const seenUndo = useRef(undoToken);
  const serial = useRef(0);
  const editorSerial = useRef(0);
  const committed = useRef(new Set<number>());
  const toolRef = useRef(tool);
  const colorRef = useRef(color);
  const widthRef = useRef(width);
  const dimRef = useRef(dim);
  const radiusRef = useRef(radius);
  const spreadRef = useRef(spread || SPOT_SIZE);
  const circleRef = useRef(circle);
  const placingRef = useRef(placing);
  const cursorMode = useRef(cursor);
  const spotlightLock = useRef(spotlight);
  const pointerRef = useRef<PresentPointer>(AWAY);
  const editorRef = useRef<Editor | null>(null);
  const marksRef = useRef<PresentMark[]>([]);
  const draftRef = useRef<PresentMark | null>(null);
  const points = useRef<PresentPoint[]>([]);
  const drawing = useRef(false);
  const dragText = useRef<{ index: number; origin: PresentPoint; start: PresentPoint } | null>(null);
  const lassoRef = useRef<PresentPoint[] | null>(null);
  const selectedRef = useRef<Set<string>>(new Set());
  const [picked, setPicked] = useState(0);
  const kind = useRef<PresentMark["tool"]>("pen");
  const paintRef = useRef<() => void>(() => undefined);
  const placeRef = useRef<() => void>(() => undefined);
  const cancelRef = useRef<() => boolean>(() => false);
  toolRef.current = tool;
  colorRef.current = color;
  widthRef.current = width;
  dimRef.current = dim;
  radiusRef.current = radius;
  spreadRef.current = spread || SPOT_SIZE;
  circleRef.current = circle;
  placingRef.current = placing;
  cursorMode.current = cursor;
  spotlightLock.current = spotlight;
  const [editor, setEditor] = useState<Editor | null>(null);

  const hold = (on: boolean) => {
    void setPresentHoldEscape(on);
  };

  const paint = () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    const cssWidth = canvas.clientWidth || window.innerWidth;
    const cssHeight = canvas.clientHeight || window.innerHeight;
    const nextW = Math.max(1, Math.round(cssWidth * dpr));
    const nextH = Math.max(1, Math.round(cssHeight * dpr));
    if (canvas.width !== nextW || canvas.height !== nextH) {
      canvas.width = nextW;
      canvas.height = nextH;
    }
    const context = canvas.getContext("2d");
    if (!context) return;
    context.setTransform(dpr, 0, 0, dpr, 0, 0);
    context.clearRect(0, 0, cssWidth, cssHeight);
    for (const mark of marksRef.current) {
      drawMark(context, cssWidth, cssHeight, mark);
      if (selectedRef.current.has(mark.id)) drawSelected(context, cssWidth, cssHeight, mark, textBox(mark));
    }
    if (draftRef.current) drawMark(context, cssWidth, cssHeight, draftRef.current);
    if (lassoRef.current) drawLasso(context, cssWidth, cssHeight, lassoRef.current);
  };
  paintRef.current = paint;

  const textBox = (mark: PresentMark) => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    const cssWidth = canvas?.clientWidth || window.innerWidth;
    const cssHeight = canvas?.clientHeight || window.innerHeight;
    if (!context || cssWidth < 1 || cssHeight < 1) return null;
    const size = textSize(cssWidth, mark.width);
    context.save();
    context.setTransform(1, 0, 0, 1, 0, 0);
    context.font = `${size}px Segoe UI, sans-serif`;
    const measured = context.measureText(mark.text).width;
    context.restore();
    return { w: measured / cssWidth, h: size / cssHeight };
  };

  const placeCursor = () => {
    const node = cursorRef.current;
    if (!node) return;
    const pointer = pointerRef.current;
    const show = showLaser(toolRef.current, cursorMode.current, placingRef.current) && pointer.inside;
    node.style.display = show ? "block" : "none";
    if (!show) return;
    node.style.left = `${pointer.x}px`;
    node.style.top = `${pointer.y}px`;
  };

  const placeSpotlight = () => {
    const node = spotlightRef.current;
    if (!node) return;
    const spotlightOn = toolRef.current === "spotlight";
    const circleOn = toolRef.current === "cursor" && circleRef.current;
    node.style.boxShadow = spotlightOn ? `0 0 0 100vmax rgba(0,0,0,${dimRef.current})` : "none";
    if (!spotlightOn && !circleOn) {
      node.style.display = "none";
      return;
    }
    const pointer = pointerRef.current;
    const lock = spotlightOn ? spotlightLock.current : null;
    node.style.border = `2px solid ${colorRef.current}`;
    if (lock) {
      node.style.display = "block";
      node.style.left = `${lock.x * 100}%`;
      node.style.top = `${lock.y * 100}%`;
      node.style.width = `${lock.width * 100}%`;
      node.style.height = `${lock.height * 100}%`;
      node.style.borderRadius = "8px";
      return;
    }
    if (!pointer.inside) {
      node.style.display = "none";
      return;
    }
    const size = spreadRef.current;
    node.style.display = "block";
    node.style.left = `${pointer.x - size / 2}px`;
    node.style.top = `${pointer.y - size / 2}px`;
    node.style.width = `${size}px`;
    node.style.height = `${size}px`;
    node.style.borderRadius = "9999px";
  };

  const placeLens = () => {
    const node = lensRef.current;
    if (!node) return;
    const lens = toolRef.current === "zoom" ? pointerRef.current.lens : null;
    if (!lens) {
      node.style.display = "none";
      return;
    }
    node.style.display = "block";
    node.style.left = `${lens.x * 100}%`;
    node.style.top = `${lens.y * 100}%`;
    node.style.width = `${lens.width * 100}%`;
    node.style.height = `${lens.height * 100}%`;
    node.style.borderColor = colorRef.current;
  };

  placeRef.current = () => {
    placeCursor();
    placeSpotlight();
    placeLens();
  };

  const cancelGesture = () => {
    const current = editorRef.current;
    if (current) {
      committed.current.add(current.generation);
      editorRef.current = null;
      setEditor(null);
      hold(drawing.current);
      return true;
    }
    if (!drawing.current && !dragText.current) return false;
    drawing.current = false;
    dragText.current = null;
    points.current = [];
    draftRef.current = null;
    paint();
    hold(false);
    return true;
  };
  cancelRef.current = cancelGesture;

  useEffect(() => {
    const onResize = () => paintRef.current();
    window.addEventListener("resize", onResize);
    paintRef.current();
    const listenPointer = onPresentPointer((next) => {
      pointerRef.current = next;
      placeRef.current();
    });
    const listenEscape = onPresentEscape(() => {
      cancelRef.current();
    });
    const listenDelete = onPresentDelete(() => {
      removeRef.current();
    });
    const onKey = (event: KeyboardEvent) => {
      if ((event.key === "Delete" || event.key === "Backspace") && toolRef.current === "lasso") {
        event.preventDefault();
        removeRef.current();
        return;
      }
      if (event.key === "Escape" && cancelRef.current()) event.preventDefault();
      const undo = (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "z" && !event.shiftKey;
      if (!undo) return;
      if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return;
      event.preventDefault();
      void undoPresentMark();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("resize", onResize);
      window.removeEventListener("keydown", onKey);
      void setPresentHoldEscape(false);
      void listenPointer.then((unlisten) => unlisten());
      void listenEscape.then((unlisten) => unlisten());
      void listenDelete.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    placeRef.current();
  }, [tool, cursor, color, dim, radius, spread, circle, placing, spotlight]);

  useEffect(() => {
    if (seenClear.current === clearToken) return;
    seenClear.current = clearToken;
    points.current = [];
    drawing.current = false;
    draftRef.current = null;
    marksRef.current = [];
    paintRef.current();
    hold(editorRef.current !== null);
  }, [clearToken]);

  useEffect(() => {
    if (seenUndo.current === undoToken) return;
    let drop = undoToken - seenUndo.current;
    if (drop < 0) drop = 1;
    seenUndo.current = undoToken;
    marksRef.current.splice(Math.max(0, marksRef.current.length - drop));
    paintRef.current();
  }, [undoToken]);

  useEffect(() => {
    hold(editor !== null || drawing.current);
    if (!editor) return;
    let stop = false;
    void getCurrentWindow()
      .setFocus()
      .catch(() => undefined)
      .finally(() => {
        if (!stop) document.getElementById("present-text-input")?.focus();
      });
    return () => {
      stop = true;
    };
  }, [editor]);

  const textIndexAt = (point: PresentPoint) => {
    for (let index = marksRef.current.length - 1; index >= 0; index -= 1) {
      const mark = marksRef.current[index];
      if (mark?.tool === "text" && hits(mark, point, textBox(mark))) return index;
    }
    return -1;
  };

  const erase = (point: PresentPoint) => {
    const before = marksRef.current.length;
    marksRef.current = marksRef.current.filter((mark) => !hits(mark, point, mark.tool === "text" ? textBox(mark) : null));
    if (marksRef.current.length !== before) paint();
  };

  const removeRef = useRef<() => void>(() => undefined);
  const removeSelection = () => {
    if (selectedRef.current.size === 0) return;
    marksRef.current = marksRef.current.filter((mark) => !selectedRef.current.has(mark.id));
    selectedRef.current = new Set();
    setPicked(0);
    paint();
  };
  removeRef.current = removeSelection;

  const textCorners = (mark: PresentMark) => {
    const box = textBox(mark);
    const anchor = mark.points[0];
    if (!box || !anchor) return [];
    return [
      anchor,
      { x: anchor.x + box.w, y: anchor.y },
      { x: anchor.x + box.w, y: anchor.y + box.h },
      { x: anchor.x, y: anchor.y + box.h },
    ];
  };

  const draftOf = (next: PresentPoint[], shape: PresentMark["tool"]): PresentMark => ({
    id: "present-draft",
    tool: shape,
    color: colorRef.current,
    width: widthRef.current,
    radius: radiusRef.current,
    text: "",
    points: next,
  });

  const begin = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const current = toolRef.current;
    const locking = (current === "spotlight" || current === "cursor" || current === "zoom") && (placingRef.current || event.altKey);
    if (!locking && !presentDraws(current)) return;
    try {
      event.currentTarget.setPointerCapture(event.pointerId);
    } catch {
      // A synthetic event cannot capture the pointer. The drag still lands on this canvas.
    }
    const point = samplesOf(event)[0];
    if (!point) return;
    if (current === "lasso" && !locking) {
      drawing.current = true;
      lassoRef.current = [point];
      selectedRef.current = new Set();
      setPicked(0);
      hold(true);
      paint();
      return;
    }
    if (current === "eraser" && !locking) {
      drawing.current = true;
      kind.current = "pen";
      points.current = [];
      hold(true);
      erase(point);
      return;
    }
    if (current === "text" && !locking) {
      const index = textIndexAt(point);
      if (index >= 0) {
        const origin = marksRef.current[index]?.points[0];
        if (origin) {
          dragText.current = { index, origin: { ...origin }, start: point };
          drawing.current = true;
          hold(true);
        }
        return;
      }
      const generation = editorSerial.current + 1;
      editorSerial.current = generation;
      const next = { x: point.x, y: point.y, generation };
      editorRef.current = next;
      setEditor(next);
      return;
    }
    drawing.current = true;
    points.current = [point];
    kind.current = locking ? "lock" : current === "highlight" ? "highlight" : current === "arrow" ? "arrow" : "pen";
    draftRef.current = draftOf([point], kind.current);
    hold(true);
    paint();
  };

  const move = (event: React.PointerEvent<HTMLCanvasElement>) => {
    if (!drawing.current) return;
    const incoming = samplesOf(event);
    if (incoming.length === 0) return;
    if (lassoRef.current) {
      lassoRef.current = [...lassoRef.current, ...incoming];
      paint();
      return;
    }
    const dragging = dragText.current;
    if (dragging) {
      const point = incoming[incoming.length - 1];
      const mark = marksRef.current[dragging.index];
      if (mark && point) {
        mark.points = [{ x: dragging.origin.x + point.x - dragging.start.x, y: dragging.origin.y + point.y - dragging.start.y }];
        paint();
      }
      return;
    }
    if (toolRef.current === "eraser") {
      for (const point of incoming) erase(point);
      return;
    }
    const shaped = kind.current === "arrow" || kind.current === "highlight" || kind.current === "lock";
    const next = shaped ? [points.current[0] ?? incoming[0], incoming[incoming.length - 1]] : [...points.current, ...incoming];
    points.current = next;
    draftRef.current = draftOf(next, kind.current);
    paint();
  };

  const end = () => {
    if (!drawing.current) return;
    drawing.current = false;
    if (lassoRef.current) {
      const loop = lassoRef.current;
      lassoRef.current = null;
      const next = new Set<string>();
      if (loop.length >= 3) {
        for (const mark of marksRef.current) {
          if (lassoHits(mark, loop, mark.tool === "text" ? textCorners(mark) : [])) next.add(mark.id);
        }
      }
      selectedRef.current = next;
      setPicked(next.size);
      hold(false);
      paint();
      return;
    }
    if (dragText.current) {
      dragText.current = null;
      hold(editorRef.current !== null);
      return;
    }
    hold(editorRef.current !== null);
    if (toolRef.current === "eraser") {
      points.current = [];
      draftRef.current = null;
      return;
    }
    const used = points.current;
    const shape = kind.current;
    points.current = [];
    draftRef.current = null;
    if (shape === "lock") {
      const lock = toolRef.current === "zoom" ? "zoom" : "spotlight";
      void finishPresentPlace(lock, presentBox(used));
      paint();
      return;
    }
    if (shape === "highlight" && !presentBox(used)) {
      paint();
      return;
    }
    if (shape === "arrow" && presentSpan(used) < 0.01) {
      paint();
      return;
    }
    serial.current += 1;
    marksRef.current.push({
      id: `present-mark-${serial.current}`,
      tool: shape,
      color: colorRef.current,
      width: widthRef.current,
      radius: radiusRef.current,
      text: "",
      points: used,
    });
    paint();
  };

  const commitText = (value: string, at: Editor) => {
    if (committed.current.has(at.generation)) return;
    committed.current.add(at.generation);
    const text = value.trim();
    if (editorRef.current?.generation === at.generation) {
      editorRef.current = null;
      setEditor(null);
    }
    if (!text) return;
    serial.current += 1;
    marksRef.current.push({
      id: `present-mark-${serial.current}`,
      tool: "text",
      color: colorRef.current,
      width: widthRef.current,
      radius: radiusRef.current,
      text,
      points: [{ x: at.x, y: at.y }],
    });
    paint();
  };

  const cursorClass =
    tool === "text" ? "cursor-move" : tool === "zoom" && !placing ? "cursor-none" : !placing && tool === "eraser" ? "cursor-cell" : placing || presentDraws(tool) ? "cursor-crosshair" : "cursor-default";
  const laser = showLaser(tool, cursor, placing);

  return (
    <div id="present-overlay" className="fixed inset-0 h-screen w-screen overflow-hidden bg-transparent">
      <div id="present-spotlight" ref={spotlightRef} className="pointer-events-none absolute" style={{ display: "none" }} />
      <div
        id="present-zoom-frame"
        ref={lensRef}
        className="pointer-events-none absolute box-border rounded-md border-[3px] border-solid shadow-[0_0_0_1px_rgba(0,0,0,0.45)]"
        style={{ display: "none" }}
      />
      <canvas
        id="present-stage"
        ref={canvasRef}
        className={`absolute inset-0 h-full w-full touch-none ${cursorClass}`}
        onPointerDown={begin}
        onPointerMove={move}
        onPointerUp={end}
        onPointerCancel={end}
      />
      {laser ? (
        <span id="present-cursor" ref={cursorRef} className="pointer-events-none absolute" style={{ display: "none" }}>
          <CursorShape cursor={cursor} color={color} />
        </span>
      ) : null}
      {picked > 0 && tool === "lasso" ? (
        <p id="present-lasso-hint" className="pointer-events-none absolute top-4 left-1/2 z-20 -translate-x-1/2 rounded-full bg-zinc-950/90 px-3 py-1 text-xs text-zinc-100">
          {picked === 1 ? "1 selected. Delete to remove." : `${picked} selected. Delete to remove.`}
        </p>
      ) : null}
      {placing ? (
        <p id="present-place-hint" className="pointer-events-none absolute top-4 left-1/2 z-20 -translate-x-1/2 rounded-full bg-zinc-950/90 px-3 py-1 text-xs text-zinc-100">
          Drag a rectangle. Click the tool again to follow.
        </p>
      ) : null}
      {editor ? (
        <input
          id="present-text-input"
          key={editor.generation}
          className="absolute z-20 min-w-24 rounded bg-zinc-950/90 px-1.5 py-0.5 outline-none ring-1 ring-white/30"
          style={{ left: `${editor.x * 100}%`, top: `${editor.y * 100}%`, color, fontFamily: "Segoe UI, sans-serif", fontSize: textSize(window.innerWidth, width), lineHeight: 1 }}
          aria-label="Label"
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              commitText(event.currentTarget.value, editor);
            } else if (event.key === "Escape") {
              event.preventDefault();
              cancelGesture();
            }
          }}
          onBlur={(event) => {
            const value = event.currentTarget.value;
            const at = editor;
            window.setTimeout(() => {
              const input = document.getElementById("present-text-input");
              if (input && document.activeElement === input) return;
              commitText(value, at);
            }, 0);
          }}
        />
      ) : null}
    </div>
  );
}

function CursorShape({ cursor, color }: { cursor: string; color: string }) {
  if (cursor === "ring") {
    return (
      <span id="present-cursor-ring" className="absolute top-0 left-0 size-9 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 shadow-[0_0_0_1px_rgba(0,0,0,0.7)]" style={{ borderColor: color }}>
        <span id="present-cursor-center" className="absolute top-1/2 left-1/2 size-1.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-zinc-950" />
      </span>
    );
  }
  if (cursor === "arrow") {
    return (
      <svg id="present-cursor-arrow" className="absolute top-0 left-0 drop-shadow" width="22" height="22" viewBox="0 0 22 22" aria-hidden>
        <path id="present-cursor-arrow-shape" d="M2 2 L2 18 L7 13 L11 20 L14 18 L10 12 L17 12 Z" fill={color} stroke="white" strokeWidth="1.2" />
      </svg>
    );
  }
  if (cursor === "crosshair") {
    return (
      <span id="present-cursor-crosshair" className="absolute top-0 left-0 size-7 -translate-x-1/2 -translate-y-1/2">
        <span id="present-cursor-x" className="absolute top-1/2 right-0 left-0 h-0.5 -translate-y-1/2 shadow-[0_0_0_1px_rgba(0,0,0,0.7)]" style={{ background: color }} />
        <span id="present-cursor-y" className="absolute top-0 bottom-0 left-1/2 w-0.5 -translate-x-1/2 shadow-[0_0_0_1px_rgba(0,0,0,0.7)]" style={{ background: color }} />
      </span>
    );
  }
  return <span id="present-cursor-dot" className="absolute top-0 left-0 size-4 -translate-x-1/2 -translate-y-1/2 rounded-full ring-2 ring-white" style={{ background: color }} />;
}
