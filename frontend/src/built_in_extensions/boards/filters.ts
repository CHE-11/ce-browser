import type { Pin, PinKind } from "@type/browser";
import { hostOf, pinAddress } from "./folder-tree";

export type SizeFilter = "any" | "small" | "medium" | "large" | "huge";
export type ShapeFilter = "any" | "landscape" | "portrait" | "square";
export type DateFilter = "any" | "today" | "week" | "month" | "year";

export type PinFilter = {
  kinds: PinKind[];
  tags: string[];
  /** Pins need every chosen tag, or any of them. */
  tagMode: "all" | "any";
  /** Minimum stars; 0 is any. */
  rating: number;
  /** `#rrggbb` the palette should contain. */
  color: string;
  exts: string[];
  size: SizeFilter;
  shape: ShapeFilter;
  date: DateFilter;
  /** Site the pin came from. */
  host: string;
  /** Only pins whose file is saved more than once. */
  duplicates: boolean;
};

export const EMPTY_FILTER: PinFilter = {
  kinds: [],
  tags: [],
  tagMode: "all",
  rating: 0,
  color: "",
  exts: [],
  size: "any",
  shape: "any",
  date: "any",
  host: "",
  duplicates: false,
};

export const SIZE_LABELS: Record<SizeFilter, string> = {
  any: "Any size",
  small: "Under 100 KB",
  medium: "100 KB – 1 MB",
  large: "1 MB – 10 MB",
  huge: "Over 10 MB",
};

export const SHAPE_LABELS: Record<ShapeFilter, string> = {
  any: "Any shape",
  landscape: "Landscape",
  portrait: "Portrait",
  square: "Square",
};

export const DATE_LABELS: Record<DateFilter, string> = {
  any: "Any time",
  today: "Today",
  week: "Past 7 days",
  month: "Past 30 days",
  year: "Past year",
};

/** Colors offered by the filter's color picker. */
export const FILTER_SWATCHES = [
  "#ef4444",
  "#f97316",
  "#facc15",
  "#22c55e",
  "#14b8a6",
  "#06b6d4",
  "#3b82f6",
  "#8b5cf6",
  "#ec4899",
  "#a16207",
  "#ffffff",
  "#9ca3af",
  "#000000",
];

const KB = 1024;
const MB = 1024 * 1024;
const DAY = 24 * 60 * 60 * 1000;

/** How many filter groups are set. */
export function activeFilterCount(filter: PinFilter) {
  return [
    filter.kinds.length > 0,
    filter.tags.length > 0,
    filter.rating > 0,
    Boolean(filter.color),
    filter.exts.length > 0,
    filter.size !== "any",
    filter.shape !== "any",
    filter.date !== "any",
    Boolean(filter.host),
    filter.duplicates,
  ].filter(Boolean).length;
}

function hexToRgb(hex: string): [number, number, number] | null {
  const match = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!match) {
    return null;
  }
  const value = Number.parseInt(match[1], 16);
  return [(value >> 16) & 255, (value >> 8) & 255, value & 255];
}

/** Perceptual-ish distance between two colors ("redmean"), 0 to about 765. */
export function colorDistance(left: string, right: string) {
  const a = hexToRgb(left);
  const b = hexToRgb(right);
  if (!a || !b) {
    return Number.POSITIVE_INFINITY;
  }
  const mean = (a[0] + b[0]) / 2;
  const red = a[0] - b[0];
  const green = a[1] - b[1];
  const blue = a[2] - b[2];
  return Math.sqrt((2 + mean / 256) * red * red + 4 * green * green + (2 + (255 - mean) / 256) * blue * blue);
}

/** Close enough for the color filter, and at least this much of the image. */
const COLOR_MATCH = 110;
const COLOR_SHARE = 0.04;

export function hasColor(pin: Pin, color: string) {
  return pin.palette.some((entry) => entry.ratio >= COLOR_SHARE && colorDistance(entry.color, color) <= COLOR_MATCH);
}

function sizeMatches(bytes: number, size: SizeFilter) {
  switch (size) {
    case "small":
      return bytes > 0 && bytes < 100 * KB;
    case "medium":
      return bytes >= 100 * KB && bytes < MB;
    case "large":
      return bytes >= MB && bytes < 10 * MB;
    case "huge":
      return bytes >= 10 * MB;
    default:
      return true;
  }
}

function shapeMatches(pin: Pin, shape: ShapeFilter) {
  if (shape === "any") {
    return true;
  }
  if (pin.width <= 0 || pin.height <= 0) {
    return false;
  }
  const ratio = pin.width / pin.height;
  if (shape === "square") {
    return ratio > 0.9 && ratio < 1.1;
  }
  return shape === "landscape" ? ratio >= 1.1 : ratio <= 0.9;
}

function dateMatches(createdAt: number, date: DateFilter, now: number) {
  switch (date) {
    case "today": {
      const start = new Date(now);
      start.setHours(0, 0, 0, 0);
      return createdAt >= start.getTime();
    }
    case "week":
      return createdAt >= now - 7 * DAY;
    case "month":
      return createdAt >= now - 30 * DAY;
    case "year":
      return createdAt >= now - 365 * DAY;
    default:
      return true;
  }
}

/** Hashes saved more than once among `pins`. */
export function duplicateHashes(pins: Pin[]) {
  const seen = new Map<string, number>();
  for (const pin of pins) {
    if (pin.hash && pin.deletedAt === null) {
      seen.set(pin.hash, (seen.get(pin.hash) ?? 0) + 1);
    }
  }
  return new Set([...seen.entries()].filter(([, count]) => count > 1).map(([hash]) => hash));
}

/** `library` is every pin, so duplicates are found across folders. */
export function applyFilter(pins: Pin[], filter: PinFilter, library: Pin[]): Pin[] {
  if (activeFilterCount(filter) === 0) {
    return pins;
  }
  const now = Date.now();
  const tags = filter.tags.map((tag) => tag.toLowerCase());
  const exts = new Set(filter.exts);
  const kinds = new Set(filter.kinds);
  const duplicates = filter.duplicates ? duplicateHashes(library) : null;
  return pins.filter((pin) => {
    if (kinds.size > 0 && !kinds.has(pin.kind)) {
      return false;
    }
    if (tags.length > 0) {
      const own = new Set(pin.tags.map((tag) => tag.toLowerCase()));
      const matched = filter.tagMode === "all" ? tags.every((tag) => own.has(tag)) : tags.some((tag) => own.has(tag));
      if (!matched) {
        return false;
      }
    }
    if (filter.rating > 0 && pin.rating < filter.rating) {
      return false;
    }
    if (filter.color && !hasColor(pin, filter.color)) {
      return false;
    }
    if (exts.size > 0 && !exts.has(pin.ext)) {
      return false;
    }
    if (!sizeMatches(pin.size, filter.size) || !shapeMatches(pin, filter.shape) || !dateMatches(pin.createdAt, filter.date, now)) {
      return false;
    }
    if (filter.host && hostOf(pinAddress(pin)) !== filter.host) {
      return false;
    }
    if (duplicates && !duplicates.has(pin.hash)) {
      return false;
    }
    return true;
  });
}

/** Values present in `pins`, most common first, for the filter menus. */
export function filterFacets(pins: Pin[]) {
  const count = (values: string[]) => {
    const counts = new Map<string, number>();
    for (const value of values) {
      if (value) {
        counts.set(value, (counts.get(value) ?? 0) + 1);
      }
    }
    return [...counts.entries()].map(([value, total]) => ({ value, count: total })).sort((left, right) => right.count - left.count || left.value.localeCompare(right.value));
  };
  return {
    exts: count(pins.map((pin) => pin.ext)),
    hosts: count(pins.map((pin) => hostOf(pinAddress(pin)))),
    tags: count(pins.flatMap((pin) => pin.tags)),
  };
}
