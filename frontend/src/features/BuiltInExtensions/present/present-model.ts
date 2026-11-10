import type { PresentCursor, PresentRect, PresentTool } from "@type/browser";

export type PresentPoint = { x: number; y: number };

export type PresentMark = {
  id: string;
  tool: "highlight" | "pen" | "arrow" | "text" | "lock";
  color: string;
  /** Stroke scale. 1 is the medium pen. */
  width: number;
  /** Highlight corner radius in CSS pixels. */
  radius: number;
  text: string;
  points: PresentPoint[];
};

export const PRESENT_COLORS = ["#f59e0b", "#38bdf8", "#fb7185", "#84cc16", "#f5f5f5", "#171717"] as const;

/** Thin, medium, thick. Medium is 1. */
export const PRESENT_WIDTHS = [0.32, 1, 2.4] as const;

const MIN_SPAN = 0.01;

/** Logical px. The native lens uses the same size. */
export const LENS_WIDTH = 280;
export const LENS_HEIGHT = 168;

export const PRESENT_TOOLS: { id: PresentTool; label: string }[] = [
  { id: "lasso", label: "Lasso" },
  { id: "spotlight", label: "Spotlight" },
  { id: "cursor", label: "Cursor" },
  { id: "zoom", label: "Zoom" },
  { id: "highlight", label: "Rectangle" },
  { id: "pen", label: "Pen" },
  { id: "arrow", label: "Arrow" },
  { id: "text", label: "Text" },
  { id: "eraser", label: "Eraser" },
];

export const PRESENT_CURSORS: { id: PresentCursor; label: string }[] = [
  { id: "dot", label: "Laser dot" },
  { id: "ring", label: "Ring" },
  { id: "arrow", label: "Arrow" },
  { id: "crosshair", label: "Crosshair" },
  { id: "off", label: "System cursor" },
];

export function presentDraws(tool: string) {
  return tool === "lasso" || tool === "highlight" || tool === "pen" || tool === "arrow" || tool === "text" || tool === "eraser";
}

function cross(ax: number, ay: number, bx: number, by: number, cx: number, cy: number) {
  return (cy - ay) * (bx - ax) - (cx - ax) * (by - ay);
}

function segmentsCross(a: PresentPoint, b: PresentPoint, c: PresentPoint, d: PresentPoint) {
  const ab = cross(a.x, a.y, b.x, b.y, c.x, c.y);
  const ab2 = cross(a.x, a.y, b.x, b.y, d.x, d.y);
  const cd = cross(c.x, c.y, d.x, d.y, a.x, a.y);
  const cd2 = cross(c.x, c.y, d.x, d.y, b.x, b.y);
  return ((ab > 0 && ab2 < 0) || (ab < 0 && ab2 > 0)) && ((cd > 0 && cd2 < 0) || (cd < 0 && cd2 > 0));
}

/** Ray cast. The polygon does not need to repeat its first point at the end. */
export function pointInPolygon(point: PresentPoint, polygon: PresentPoint[]) {
  if (polygon.length < 3) return false;
  let inside = false;
  for (let index = 0, previous = polygon.length - 1; index < polygon.length; previous = index++) {
    const current = polygon[index];
    const prior = polygon[previous];
    const crosses = current.y > point.y !== prior.y > point.y;
    if (!crosses || prior.y === current.y) continue;
    const x = ((prior.x - current.x) * (point.y - current.y)) / (prior.y - current.y) + current.x;
    if (point.x < x) inside = !inside;
  }
  return inside;
}

function markOutline(mark: PresentMark): PresentPoint[] {
  const first = mark.points[0];
  const last = mark.points[mark.points.length - 1];
  if (!first) return [];
  if (mark.tool === "highlight" && last) {
    const left = Math.min(first.x, last.x);
    const right = Math.max(first.x, last.x);
    const top = Math.min(first.y, last.y);
    const bottom = Math.max(first.y, last.y);
    return [
      { x: left, y: top },
      { x: right, y: top },
      { x: right, y: bottom },
      { x: left, y: bottom },
      { x: left, y: top },
    ];
  }
  if (mark.points.length === 1) return [first, first];
  return mark.points;
}

/** A mark is caught when the loop contains it or crosses it. */
export function lassoHits(mark: PresentMark, lasso: PresentPoint[], extra: PresentPoint[] = []) {
  if (lasso.length < 3) return false;
  const samples = [...markOutline(mark), ...extra];
  if (samples.some((point) => pointInPolygon(point, lasso))) return true;
  const loop = [...lasso, lasso[0]];
  for (let index = 1; index < samples.length; index += 1) {
    for (let edge = 1; edge < loop.length; edge += 1) {
      if (segmentsCross(samples[index - 1], samples[index], loop[edge - 1], loop[edge])) return true;
    }
  }
  return false;
}

/** A box whose longer side is at least 1% of the window. A thin bar still counts. */
export function presentBox(points: PresentPoint[]): PresentRect | null {
  const first = points[0];
  const last = points[points.length - 1];
  if (!first || !last) return null;
  const x = Math.min(first.x, last.x);
  const y = Math.min(first.y, last.y);
  const width = Math.abs(last.x - first.x);
  const height = Math.abs(last.y - first.y);
  if (Math.max(width, height) < MIN_SPAN) return null;
  return { x, y, width, height };
}

/** Distance from the first point to the last, as a fraction of the window. */
export function presentSpan(points: PresentPoint[]): number {
  const first = points[0];
  const last = points[points.length - 1];
  if (!first || !last) return 0;
  return Math.hypot(last.x - first.x, last.y - first.y);
}
