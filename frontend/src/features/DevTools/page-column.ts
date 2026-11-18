/** Half the gap between the page column and the docked inspector. Matches the native layout. */
const DOCK_GAP = 3;

export type DockedPanels = {
  left: number;
  right: number;
};

/** Collapsed rail width. Matches the native `PANEL_RAIL`. */
export const PANEL_RAIL = 36;

/** Header above an expanded panel webview. Matches the native `PANEL_HEADER`. */
export const PANEL_HEADER = 32;

type DockColumn = {
  edge: string;
  width: number;
  collapsed: boolean;
};

/** Every docked column, split onto the edge it occupies. */
export function dockedPanels(panels: DockColumn[] | undefined): DockedPanels {
  let left = 0;
  let right = 0;
  for (const panel of panels ?? []) {
    const width = panel.collapsed ? PANEL_RAIL : panel.width;
    if (!Number.isFinite(width) || width <= 0) {
      continue;
    }
    const size = Math.round(width);
    if (panel.edge === "left") {
      left += size;
    } else {
      right += size;
    }
  }
  return { left, right };
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
