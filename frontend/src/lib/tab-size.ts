import type { CSSProperties } from "react";

/** Defaults and limits. They match `BrowserSettings::normalize` in settings.rs. */
export const TAB_SIZE = {
  width: 220,
  widthLimits: { min: 88, max: 480 },
  /** 11px, the size the tab title used before this setting existed. */
  itemText: 0.6875,
  /** Side padding of a horizontal tab. Top and bottom get half. */
  itemPadding: 0.625,
  menuText: 0.875,
  menuPadding: 0.625,
  text: { min: 0.5, max: 1.5 },
  padding: { min: 0, max: 1.5 },
  step: 0.025,
} as const;

const TITLE_LINE = 1.35;

function tabText(textSize: number | undefined) {
  return Number.isFinite(textSize) ? (textSize as number) : TAB_SIZE.itemText;
}

function tabSide(padding: number | undefined) {
  return Number.isFinite(padding) ? (padding as number) : TAB_SIZE.itemPadding;
}

/** Font size and padding for a tab title. `padding` is the side padding and the top and bottom get half. */
export function tabItemStyle(textSize: number | undefined, padding: number | undefined, height?: number): CSSProperties {
  const text = tabText(textSize);
  const side = tabSide(padding);
  const minHeight = Number.isFinite(height) && (height as number) > 0 ? `${height}rem` : undefined;
  return {
    fontSize: `${text}rem`,
    lineHeight: TITLE_LINE,
    paddingBlock: `${side / 2}rem`,
    paddingInline: `${side}rem`,
    ...(minHeight ? { minHeight } : null),
  };
}

/**
 * Height a horizontal tab occupies. The title line, icon, and close control sit inside the vertical
 * padding, and `height` is the floor. Pinned tabs use this on their own, with no other tab open.
 */
export function horizontalTabHeight(
  textSize: number | undefined,
  padding: number | undefined,
  height: number | undefined,
  row: number,
): number {
  const content = Number.isFinite(row) && row > 0 ? row : 0;
  const floor = Number.isFinite(height) && (height as number) > 0 ? (height as number) : 0;
  return Math.max(floor, tabSide(padding) + Math.max(tabText(textSize) * TITLE_LINE, content));
}
