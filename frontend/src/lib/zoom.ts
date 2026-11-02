export const ZOOM_KEY = "ce-browser.ui-zoom";
export const DEFAULT_ZOOM = 100;
export const MIN_ZOOM = 80;
export const MAX_ZOOM = 150;
export const ZOOM_STEP = 5;
export const PAGE_ZOOM_LEVELS = [25, 33, 50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200, 250, 300];

const BASE_FONT_PX = 16;

/**
 * Turns wheel movement into one zoom step. Negative `deltaY` zooms in.
 * `bucket` keeps leftover movement so a trackpad does not jump several steps at once.
 */
export function zoomWheelStep(bucket: number, deltaY: number, deltaMode: number): { bucket: number; change: "in" | "out" | null } {
  if (deltaY === 0) {
    return { bucket, change: null };
  }
  if (deltaMode === WheelEvent.DOM_DELTA_PAGE) {
    return { bucket: 0, change: deltaY < 0 ? "in" : "out" };
  }
  const next = bucket + deltaY;
  const notch = deltaMode === WheelEvent.DOM_DELTA_LINE ? 1 : 100;
  if (Math.abs(next) < notch) {
    return { bucket: next, change: null };
  }
  return { bucket: 0, change: next < 0 ? "in" : "out" };
}

export function stepPageZoom(current: number, direction: -1 | 1) {
  const index = PAGE_ZOOM_LEVELS.findIndex((level) => level >= current);
  const at = index < 0 ? PAGE_ZOOM_LEVELS.length - 1 : index;
  const next = at + direction;
  return PAGE_ZOOM_LEVELS[Math.min(PAGE_ZOOM_LEVELS.length - 1, Math.max(0, next))];
}

export function clampZoom(value: number) {
  const stepped = Math.round(value / ZOOM_STEP) * ZOOM_STEP;
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, stepped));
}

export function readZoom() {
  try {
    const raw = localStorage.getItem(ZOOM_KEY);
    if (raw === null) {
      return DEFAULT_ZOOM;
    }
    const value = Number(raw);
    if (!Number.isFinite(value)) {
      return DEFAULT_ZOOM;
    }
    return clampZoom(value);
  } catch {
    return DEFAULT_ZOOM;
  }
}

export function applyZoom(percent: number) {
  document.documentElement.style.fontSize = `${(BASE_FONT_PX * percent) / 100}px`;
}

export function publishZoom(percent: number) {
  const next = clampZoom(percent);
  applyZoom(next);
  try {
    localStorage.setItem(ZOOM_KEY, String(next));
  } catch {
    // Ignore quota and private-mode failures; the size still applies for this session.
  }
  return next;
}
