/** Item placement for the Justified, Waterfall and Grid layouts. Pure, so it can run on every
 * resize and size change without touching the DOM. */

export type LayoutItem = {
  id: string;
  /** Width divided by height of the preview. */
  aspect: number;
};

export type Tile = {
  id: string;
  index: number;
  x: number;
  y: number;
  width: number;
  /** Height of the preview, without the caption. */
  height: number;
};

export type GridLayout = {
  tiles: Tile[];
  height: number;
};

export type LayoutKind = "justified" | "masonry" | "grid";

export const TILE_GAP = 12;

/** Tall previews are cropped at this height for each unit of width, matching the stored thumbnails. */
const MIN_ASPECT = 0.4;
const MAX_ASPECT = 5;

function clampAspect(aspect: number) {
  if (!Number.isFinite(aspect) || aspect <= 0) {
    return 1;
  }
  return Math.min(MAX_ASPECT, Math.max(MIN_ASPECT, aspect));
}

/** Rows that fill the width, each scaled so its items share one height near `rowHeight`. */
export function justifiedLayout(items: LayoutItem[], width: number, rowHeight: number, caption: number): GridLayout {
  const tiles: Tile[] = [];
  let y = 0;
  let row: { item: LayoutItem; index: number; aspect: number }[] = [];
  let rowAspect = 0;

  const place = (height: number) => {
    let x = 0;
    for (const entry of row) {
      const itemWidth = entry.aspect * height;
      tiles.push({ id: entry.item.id, index: entry.index, x, y, width: itemWidth, height });
      x += itemWidth + TILE_GAP;
    }
    y += height + caption + TILE_GAP;
    row = [];
    rowAspect = 0;
  };

  items.forEach((item, index) => {
    const aspect = clampAspect(item.aspect);
    row.push({ item, index, aspect });
    rowAspect += aspect;
    const gaps = (row.length - 1) * TILE_GAP;
    const height = (width - gaps) / rowAspect;
    if (height <= rowHeight) {
      place(height);
    }
  });
  if (row.length > 0) {
    place(rowHeight);
  }
  return { tiles, height: Math.max(0, y - TILE_GAP) };
}

function columns(width: number, columnWidth: number) {
  const count = Math.max(1, Math.floor((width + TILE_GAP) / (columnWidth + TILE_GAP)));
  return { count, width: (width - (count - 1) * TILE_GAP) / count };
}

/** Columns of equal width; each item goes under the shortest column. */
export function masonryLayout(items: LayoutItem[], width: number, columnWidth: number, caption: number): GridLayout {
  const { count, width: itemWidth } = columns(width, columnWidth);
  const heights = new Array<number>(count).fill(0);
  const tiles = items.map((item, index) => {
    let column = 0;
    for (let next = 1; next < count; next += 1) {
      if (heights[next] < heights[column]) {
        column = next;
      }
    }
    const height = itemWidth / clampAspect(item.aspect);
    const tile = { id: item.id, index, x: column * (itemWidth + TILE_GAP), y: heights[column], width: itemWidth, height };
    heights[column] += height + caption + TILE_GAP;
    return tile;
  });
  return { tiles, height: Math.max(0, Math.max(...heights) - TILE_GAP) };
}

/** Square cells; previews are fitted inside. */
export function gridLayout(items: LayoutItem[], width: number, cellSize: number, caption: number): GridLayout {
  const { count, width: cell } = columns(width, cellSize);
  const tiles = items.map((item, index) => ({
    id: item.id,
    index,
    x: (index % count) * (cell + TILE_GAP),
    y: Math.floor(index / count) * (cell + caption + TILE_GAP),
    width: cell,
    height: cell,
  }));
  const rows = Math.ceil(items.length / count);
  return { tiles, height: Math.max(0, rows * (cell + caption + TILE_GAP) - TILE_GAP) };
}

export function computeLayout(kind: LayoutKind, items: LayoutItem[], width: number, size: number, caption: number): GridLayout {
  if (width <= 0) {
    return { tiles: [], height: 0 };
  }
  if (kind === "masonry") {
    return masonryLayout(items, width, size, caption);
  }
  if (kind === "grid") {
    return gridLayout(items, width, size, caption);
  }
  return justifiedLayout(items, width, size, caption);
}

/** Tiles that overlap the band from `top` to `bottom`. */
export function tilesInBand(layout: GridLayout, top: number, bottom: number, caption: number) {
  return layout.tiles.filter((tile) => tile.y < bottom && tile.y + tile.height + caption > top);
}

/** The tile to move to from `current` with an arrow key. */
export function neighbor(layout: GridLayout, current: number, key: "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown"): number {
  const tiles = layout.tiles;
  if (tiles.length === 0) {
    return -1;
  }
  const from = tiles.find((tile) => tile.index === current);
  if (!from) {
    return tiles[0].index;
  }
  if (key === "ArrowLeft") {
    return Math.max(0, current - 1);
  }
  if (key === "ArrowRight") {
    return Math.min(tiles.length - 1, current + 1);
  }
  const center = from.x + from.width / 2;
  const below = key === "ArrowDown";
  let best: Tile | null = null;
  let bestScore = Number.POSITIVE_INFINITY;
  for (const tile of tiles) {
    const vertical = below ? tile.y - (from.y + from.height) : from.y - (tile.y + tile.height);
    if (vertical < 0) {
      continue;
    }
    const horizontal = Math.abs(tile.x + tile.width / 2 - center);
    const score = vertical * 4 + horizontal;
    if (score < bestScore) {
      bestScore = score;
      best = tile;
    }
  }
  return best ? best.index : current;
}

export function intersects(tile: Tile, caption: number, rect: { left: number; top: number; right: number; bottom: number }) {
  return tile.x < rect.right && tile.x + tile.width > rect.left && tile.y < rect.bottom && tile.y + tile.height + caption > rect.top;
}
