import type { PinSort } from "@type/browser";
import { isPinSort } from "./folder-tree";

export type PinLayout = "justified" | "masonry" | "grid" | "list";

export const PIN_LAYOUT_LABELS: Record<PinLayout, string> = {
  justified: "Justified",
  masonry: "Waterfall",
  grid: "Grid",
  list: "List",
};

export type ViewPrefs = {
  layout: PinLayout;
  /** Row height for Justified, column width for Waterfall and Grid, in pixels. */
  size: number;
  sort: PinSort;
  /** Show the name and dimensions under each item. */
  captions: boolean;
  inspector: boolean;
};

export const MIN_TILE_SIZE = 80;
export const MAX_TILE_SIZE = 480;

const PREFS_KEY = "ce-boards-view-prefs";

const DEFAULT_PREFS: ViewPrefs = { layout: "justified", size: 200, sort: "newest", captions: true, inspector: true };

export function readViewPrefs(): ViewPrefs {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}");
    if (!stored || typeof stored !== "object") {
      return DEFAULT_PREFS;
    }
    const value = stored as Partial<Record<keyof ViewPrefs, unknown>>;
    const layout = typeof value.layout === "string" && value.layout in PIN_LAYOUT_LABELS ? (value.layout as PinLayout) : DEFAULT_PREFS.layout;
    const size = typeof value.size === "number" && Number.isFinite(value.size) ? Math.min(MAX_TILE_SIZE, Math.max(MIN_TILE_SIZE, value.size)) : DEFAULT_PREFS.size;
    return {
      layout,
      size,
      sort: isPinSort(value.sort) ? value.sort : DEFAULT_PREFS.sort,
      captions: typeof value.captions === "boolean" ? value.captions : DEFAULT_PREFS.captions,
      inspector: typeof value.inspector === "boolean" ? value.inspector : DEFAULT_PREFS.inspector,
    };
  } catch {
    return DEFAULT_PREFS;
  }
}

export function writeViewPrefs(prefs: ViewPrefs) {
  localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
}
