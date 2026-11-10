import type { PresentCursor, PresentRect, PresentTool } from "@type/browser";

export type PresentPoint = { x: number; y: number };

export type PresentMark = {
  id: string;
  tool: "highlight" | "pen" | "arrow" | "text" | "lock";
  color: string;
  text: string;
  points: PresentPoint[];
};

export const PRESENT_COLORS = ["#f59e0b", "#38bdf8", "#fb7185", "#84cc16"] as const;

/** Logical px. The native lens uses the same size. */
export const LENS_WIDTH = 280;
export const LENS_HEIGHT = 168;

export const PRESENT_TOOLS: { id: PresentTool; label: string }[] = [
  { id: "spotlight", label: "Spotlight. Click again or Alt-drag to lock a rectangle. Click again to follow." },
  { id: "zoom", label: "Zoom. Click again or Alt-drag to lock a rectangle. Click again to follow." },
  { id: "highlight", label: "Highlight a rectangle" },
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
  return tool === "highlight" || tool === "pen" || tool === "arrow" || tool === "text" || tool === "eraser";
}

export function presentBox(points: PresentPoint[]): PresentRect | null {
  const first = points[0];
  const last = points[points.length - 1];
  if (!first || !last) return null;
  const x = Math.min(first.x, last.x);
  const y = Math.min(first.y, last.y);
  const width = Math.abs(last.x - first.x);
  const height = Math.abs(last.y - first.y);
  if (width < 0.01 || height < 0.01) return null;
  return { x, y, width, height };
}
