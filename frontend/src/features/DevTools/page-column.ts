/** Half the gap between the page column and the docked inspector. Matches the native layout. */
const DOCK_GAP = 3;

export type DockedPanels = {
  left: number;
  right: number;
};

/** The side column, split onto the edge it occupies. Zero on both sides while it is closed. */
export function dockedPanels(width: number, edge: string): DockedPanels {
  if (!Number.isFinite(width) || width <= 0) {
    return { left: 0, right: 0 };
  }
  const size = Math.round(width);
  return edge === "left" ? { left: size, right: 0 } : { left: 0, right: size };
}

/** Pages column in the page area: panel widths come off first, then the inspector takes its share. */
export function pagesColumnStyle(panels: DockedPanels, devtoolsRatio: number | null) {
  const inset = panels.left + panels.right;
  if (devtoolsRatio == null) {
    return { left: panels.left, width: `calc(100% - ${inset}px)` };
  }
  return { left: panels.left, width: `calc((100% - ${inset}px) * ${devtoolsRatio} - ${DOCK_GAP}px)` };
}

export type PageBox = {
  /** Offset from the left of the pages element to the real page. */
  left: number;
  width: number;
  height: number;
};

/**
 * The page the tools should sit over. `pages` may still include docked panels; their whole
 * width comes off the page area before the inspector's share is applied.
 */
export function pageBox(
  area: { left: number; width: number; height: number },
  pages: { left: number; width: number; height: number },
  panels: DockedPanels,
  devtoolsRatio: number | null,
): PageBox {
  const span = Math.max(0, area.width - panels.left - panels.right);
  const width = devtoolsRatio == null ? span : Math.max(0, span * devtoolsRatio - DOCK_GAP);
  return {
    left: area.left + panels.left - pages.left,
    width,
    height: pages.height,
  };
}
