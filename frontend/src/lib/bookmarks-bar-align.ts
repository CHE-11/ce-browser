import type { BookmarksBarAlign } from "@type/browser";

export const BOOKMARKS_BAR_ALIGN_OPTIONS: { value: BookmarksBarAlign; label: string }[] = [
  { value: "left", label: "Left" },
  { value: "center", label: "Center" },
  { value: "right", label: "Right" },
];

/** An older settings file omits this. Anything else stays on the left. */
export function bookmarksBarAlign(value: string | null | undefined): BookmarksBarAlign {
  if (value === "center" || value === "right") {
    return value;
  }
  return "left";
}

/** Main-axis placement of the folders and bookmarks in the bar. */
export function bookmarksBarAlignClass(align: BookmarksBarAlign) {
  if (align === "center") {
    return "justify-center";
  }
  if (align === "right") {
    return "justify-end";
  }
  return "justify-start";
}
