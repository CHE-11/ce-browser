import type { CSSProperties } from "react";

/** Defaults and limits in rem. They match `BrowserSettings::normalize` in settings.rs. */
export const BOOKMARK_SIZE = {
  barText: 0.75,
  barPadding: 0.5,
  folderText: 0.875,
  folderPadding: 0.75,
  menuText: 0.875,
  menuPadding: 0.625,
  text: { min: 0.5, max: 1.5 },
  padding: { min: 0, max: 1.5 },
  step: 0.025,
} as const;

/**
 * Font size and padding for one bookmark. `padding` is the side padding and the top and bottom
 * get half, so the default bar item stays 1.5rem tall. `height` above 0 sets a minimum.
 * `icon` is the row icon, read by menu rows as `--item-icon`.
 */
export function bookmarkItemStyle(
  textSize: number | undefined,
  padding: number | undefined,
  fallback: { text: number; padding: number },
  extra?: { height?: number; icon?: number },
): CSSProperties {
  const text = Number.isFinite(textSize) ? (textSize as number) : fallback.text;
  const side = Number.isFinite(padding) ? (padding as number) : fallback.padding;
  const style: CSSProperties & { "--item-icon"?: string } = {
    fontSize: `${text}rem`,
    lineHeight: 1.35,
    paddingBlock: `${side / 2}rem`,
    paddingInline: `${side}rem`,
  };
  if (extra?.height && extra.height > 0) {
    style.minHeight = `${extra.height}rem`;
  }
  if (extra?.icon && extra.icon > 0) {
    style["--item-icon"] = `${extra.icon}rem`;
  }
  return style;
}

/**
 * Put this on the label, not the row. `text-box-trim` does not inherit, and the row's line box
 * keeps the space under the baseline, so centering the row lines the icon up with that empty
 * space. Trimming the label to the cap height and the baseline makes its box match the letters.
 * Glyphs still paint past that box. Clip only the inline axis so a `truncate` ellipsis works
 * without cutting off descenders.
 */
export const bookmarkLabelStyle = {
  textBoxTrim: "trim-both",
  textBoxEdge: "cap alphabetic",
  overflowX: "clip",
  overflowY: "visible",
} as CSSProperties;

/** Padding and type size for the bookmark editor panel. The gap stays a step tighter than the padding. */
export function bookmarkEditorFrameStyle(textSize: number, padding: number): CSSProperties {
  return {
    fontSize: `${textSize}rem`,
    padding: `${padding}rem`,
    gap: `${Math.max(padding * 0.65, 0.3)}rem`,
  };
}
