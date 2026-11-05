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

/** Font size and padding for a tab title. `padding` is the side padding and the top and bottom get half. */
export function tabItemStyle(textSize: number | undefined, padding: number | undefined, height?: number): CSSProperties {
  const text = Number.isFinite(textSize) ? (textSize as number) : TAB_SIZE.itemText;
  const side = Number.isFinite(padding) ? (padding as number) : TAB_SIZE.itemPadding;
  const minHeight = Number.isFinite(height) && (height as number) > 0 ? `${height}rem` : undefined;
  return {
    fontSize: `${text}rem`,
    lineHeight: 1.35,
    paddingBlock: `${side / 2}rem`,
    paddingInline: `${side}rem`,
    ...(minHeight ? { minHeight } : null),
  };
}
