import type { Board, BoardFolder, BoardsSnapshot, Pin, PinKind } from "@type/browser";

export const EMPTY_BOARDS: BoardsSnapshot = { folders: [], boards: [], pins: [], lastBoardId: null };

/** Drag data types on `ce://boards`. Pins carry a JSON array of ids, boards and folders one id. */
export const DRAG_PINS = "application/x-ce-pins";
export const DRAG_BOARD = "application/x-ce-board";
export const DRAG_FOLDER = "application/x-ce-board-folder";

/** What the main area of `ce://boards` shows. */
export type BoardsScope = { kind: "all" } | { kind: "board"; id: string } | { kind: "folder"; id: string };

/** Pin counts by board id. */
export function pinCounts(boards: BoardsSnapshot) {
  const counts = new Map<string, number>();
  for (const pin of boards.pins) {
    counts.set(pin.boardId, (counts.get(pin.boardId) ?? 0) + 1);
  }
  return counts;
}

/** Pins on every board inside `folderId`, at any depth. */
export function pinsInFolder(boards: BoardsSnapshot, folderId: string): Pin[] {
  const folders = folderTree(boards, folderId);
  const boardIds = new Set(boards.boards.filter((board) => board.folderId && folders.has(board.folderId)).map((board) => board.id));
  return boards.pins.filter((pin) => boardIds.has(pin.boardId));
}

export const PIN_KIND_LABELS: Record<PinKind, string> = {
  page: "Page",
  region: "Region",
  element: "Section",
  link: "Link",
  image: "Image",
};

const byName = (left: { name: string }, right: { name: string }) =>
  left.name.localeCompare(right.name, undefined, { sensitivity: "base", numeric: true });

export function childFolders(boards: BoardsSnapshot, parentId: string | null): BoardFolder[] {
  return boards.folders.filter((folder) => folder.parentId === parentId).sort(byName);
}

export function boardsIn(boards: BoardsSnapshot, folderId: string | null): Board[] {
  return boards.boards.filter((board) => board.folderId === folderId).sort(byName);
}

/** Folder names from the top level down to `folderId`. */
export function folderPath(boards: BoardsSnapshot, folderId: string | null): BoardFolder[] {
  const path: BoardFolder[] = [];
  const seen = new Set<string>();
  let current = folderId;
  while (current && !seen.has(current)) {
    seen.add(current);
    const folder = boards.folders.find((item) => item.id === current);
    if (!folder) {
      break;
    }
    path.unshift(folder);
    current = folder.parentId;
  }
  return path;
}

/** "Folder / Sub folder / Board". */
export function boardLabel(boards: BoardsSnapshot, board: Board) {
  return [...folderPath(boards, board.folderId).map((folder) => folder.name), board.name].join(" / ");
}

/** `folderId` and every folder inside it. */
export function folderTree(boards: BoardsSnapshot, folderId: string): Set<string> {
  const found = new Set([folderId]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const folder of boards.folders) {
      if (folder.parentId && found.has(folder.parentId) && !found.has(folder.id)) {
        found.add(folder.id);
        grew = true;
      }
    }
  }
  return found;
}

/** Boards and pins a folder delete would remove. */
export function folderContents(boards: BoardsSnapshot, folderId: string) {
  const folders = folderTree(boards, folderId);
  const boardIds = new Set(boards.boards.filter((board) => board.folderId && folders.has(board.folderId)).map((board) => board.id));
  return {
    folders: folders.size - 1,
    boards: boardIds.size,
    pins: boards.pins.filter((pin) => boardIds.has(pin.boardId)).length,
  };
}

export function pinsOnBoard(boards: BoardsSnapshot, boardId: string): Pin[] {
  return boards.pins.filter((pin) => pin.boardId === boardId);
}

/** The cover pin, otherwise the newest pin with an image. */
export function coverPin(boards: BoardsSnapshot, board: Board): Pin | undefined {
  const pins = pinsOnBoard(boards, board.id);
  return pins.find((pin) => pin.id === board.coverPinId) ?? pins.find((pin) => pin.hasImage);
}

/** Boards sorted by folder path, for pickers. */
export function boardChoices(boards: BoardsSnapshot) {
  return boards.boards
    .map((board) => ({ board, label: boardLabel(boards, board) }))
    .sort((left, right) => left.label.localeCompare(right.label, undefined, { sensitivity: "base", numeric: true }));
}

export function hostOf(url: string) {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return "";
  }
}

export function pinMatches(pin: Pin, query: string) {
  const needle = query.trim().toLowerCase();
  if (!needle) {
    return true;
  }
  return [pin.title, pin.text, pin.note, pin.pageUrl, pin.linkUrl].some((field) => field.toLowerCase().includes(needle));
}
