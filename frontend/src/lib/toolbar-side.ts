import type { BrowserSettings, ToolbarSide } from "@type/browser";

const SIDES: ToolbarSide[] = ["top", "right", "bottom", "left"];

/** Right is the old default. A horizontal bar from an older file stays on top. */
export function toolbarSide(settings: BrowserSettings | null): ToolbarSide {
  const side = settings?.toolbarSide;
  if (side === "top" || side === "right" || side === "bottom" || side === "left") {
    return side;
  }
  return settings?.verticalToolbar === false ? "top" : "right";
}

/** `verticalToolbar` stays in step so an older build still picks a vertical or horizontal bar. */
export function toolbarSidePatch(side: ToolbarSide) {
  return {
    toolbarSide: side,
    verticalToolbar: side === "left" || side === "right",
  };
}

/** Right, then top, then left, then bottom. The first step still matches the old right-to-top toggle. */
export function nextToolbarSide(side: ToolbarSide): ToolbarSide {
  const index = SIDES.indexOf(side);
  return SIDES[(index + 1) % SIDES.length] ?? "right";
}
