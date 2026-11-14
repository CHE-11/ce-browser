import type { CSSProperties } from "react";
import type { BookmarkDividerLine, BookmarkDividerShape, BrowserSettings } from "@type/browser";
import { CHROME, metric } from "./chrome-metrics";

export const BOOKMARK_DIVIDER_LINE_OPTIONS: { value: BookmarkDividerLine; label: string }[] = [
  { value: "solid", label: "Solid" },
  { value: "dashed", label: "Dashed" },
  { value: "dotted", label: "Dotted" },
  { value: "double", label: "Double" },
];

export const BOOKMARK_DIVIDER_SHAPE_OPTIONS: { value: BookmarkDividerShape; label: string }[] = [
  { value: "square", label: "Square" },
  { value: "round", label: "Round" },
  { value: "pill", label: "Pill" },
];

/** Zinc-600. Dividers on the bar and in menus. */
export const BOOKMARK_DIVIDER_COLOR = "#52525b";

/** Zinc-700. Dividers on the bookmarks page, which uses the quieter rule. */
export const BOOKMARK_PAGE_DIVIDER_COLOR = "#3f3f46";

export type BookmarkDividerMetrics = {
  /** Space on each side of a vertical line, in rem. */
  padding: number;
  /** Space above a plain horizontal line, in rem. */
  paddingTop: number;
  /** Space below a plain horizontal line, in rem. */
  paddingBottom: number;
  /** Space above a horizontal line that has a label, in rem. */
  labelPaddingTop: number;
  /** Space below a horizontal line that has a label, in rem. */
  labelPaddingBottom: number;
  /** Space at each end of a horizontal line, in rem. Vertical lines ignore this. */
  inset: number;
  /** Stroke width in CSS pixels. */
  thickness: number;
  /** Share of the cross axis the line fills, as a percent. */
  length: number;
  line: BookmarkDividerLine;
  shape: BookmarkDividerShape;
};

/** An older file omits this. Anything else stays solid. */
export function bookmarkDividerLine(value: string | null | undefined): BookmarkDividerLine {
  if (value === "dashed" || value === "dotted" || value === "double") {
    return value;
  }
  return "solid";
}

/** An older file omits this. Anything else stays square. */
export function bookmarkDividerShape(value: string | null | undefined): BookmarkDividerShape {
  if (value === "round" || value === "pill") {
    return value;
  }
  return "square";
}

export function verticalDividerMetrics(settings: Partial<BrowserSettings> | null | undefined): BookmarkDividerMetrics {
  return {
    padding: metric(settings?.bookmarksBarDividerPadding, CHROME.bookmarksBarDividerPadding),
    paddingTop: 0,
    paddingBottom: 0,
    labelPaddingTop: 0,
    labelPaddingBottom: 0,
    inset: 0,
    thickness: metric(settings?.bookmarksBarDividerThickness, CHROME.bookmarksBarDividerThickness),
    length: metric(settings?.bookmarksBarDividerLength, CHROME.bookmarksBarDividerLength),
    line: bookmarkDividerLine(settings?.bookmarksBarDividerLine),
    shape: bookmarkDividerShape(settings?.bookmarksBarDividerShape),
  };
}

export function horizontalDividerMetrics(settings: Partial<BrowserSettings> | null | undefined): BookmarkDividerMetrics {
  const shared = settings?.bookmarkDividerPadding;
  return {
    padding: 0,
    paddingTop: metric(settings?.bookmarkDividerPaddingTop ?? shared, CHROME.bookmarkDividerPaddingTop),
    paddingBottom: metric(settings?.bookmarkDividerPaddingBottom ?? shared, CHROME.bookmarkDividerPaddingBottom),
    labelPaddingTop: metric(settings?.bookmarkDividerLabelPaddingTop, CHROME.bookmarkDividerLabelPaddingTop),
    labelPaddingBottom: metric(settings?.bookmarkDividerLabelPaddingBottom, CHROME.bookmarkDividerLabelPaddingBottom),
    inset: metric(settings?.bookmarkDividerInset, CHROME.bookmarkDividerInset),
    thickness: metric(settings?.bookmarkDividerThickness, CHROME.bookmarkDividerThickness),
    length: metric(settings?.bookmarkDividerLength, CHROME.bookmarkDividerLength),
    line: bookmarkDividerLine(settings?.bookmarkDividerLine),
    shape: bookmarkDividerShape(settings?.bookmarkDividerShape),
  };
}

/** Changes the bar's fitted width when the vertical divider's box changes. */
export function bookmarkDividerKey(metrics: BookmarkDividerMetrics) {
  return `${metrics.padding}|${metrics.paddingTop}|${metrics.paddingBottom}|${metrics.labelPaddingTop}|${metrics.labelPaddingBottom}|${metrics.inset}|${metrics.thickness}|${metrics.length}|${metrics.line}|${metrics.shape}`;
}

export function bookmarkDividerFrameStyle(
  orientation: "vertical" | "horizontal",
  metrics: BookmarkDividerMetrics,
  labeled = false,
): CSSProperties {
  if (orientation === "vertical") {
    return {
      paddingLeft: `${metrics.padding}rem`,
      paddingRight: `${metrics.padding}rem`,
    };
  }
  const top = labeled ? metrics.labelPaddingTop : metrics.paddingTop;
  const bottom = labeled ? metrics.labelPaddingBottom : metrics.paddingBottom;
  return {
    paddingTop: `${top}rem`,
    paddingBottom: `${bottom}rem`,
    paddingLeft: `${metrics.inset}rem`,
    paddingRight: `${metrics.inset}rem`,
    boxSizing: "border-box",
  };
}

function endRadius(shape: BookmarkDividerShape, thickness: number) {
  if (shape === "pill") {
    return 999;
  }
  if (shape === "round") {
    return Math.max(1, thickness / 2);
  }
  return 0;
}

/** The rule itself. A double line needs three pixels before the gap shows. */
export function bookmarkDividerMarkStyle(
  orientation: "vertical" | "horizontal",
  metrics: BookmarkDividerMetrics,
  color = BOOKMARK_DIVIDER_COLOR,
): CSSProperties {
  const vertical = orientation === "vertical";
  const drawn = metrics.line === "double" ? Math.max(metrics.thickness, 3) : metrics.thickness;
  const radius = endRadius(metrics.shape, drawn);
  const along = vertical ? "height" : "width";
  const across = vertical ? "width" : "height";
  const style: CSSProperties = {
    [along]: `${metrics.length}%`,
    flexShrink: 0,
    borderRadius: radius,
  };
  if (metrics.line === "solid") {
    style[across] = drawn;
    style.backgroundColor = color;
    return style;
  }
  style[across] = 0;
  const border = `${drawn}px ${metrics.line} ${color}`;
  if (vertical) {
    style.borderLeft = border;
  } else {
    style.borderTop = border;
  }
  return style;
}
