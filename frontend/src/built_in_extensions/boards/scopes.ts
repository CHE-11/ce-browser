import type { BoardsSnapshot, Pin } from "@type/browser";
import { UNCATEGORIZED, folderLabel, livePins, pinsInFolder } from "./folder-tree";

/** What the main area of `ce://boards` shows. */
export type BoardsScope =
  | { kind: "all" }
  | { kind: "uncategorized" }
  | { kind: "untagged" }
  | { kind: "recent" }
  | { kind: "random"; seed: number }
  | { kind: "tags" }
  | { kind: "tag"; name: string }
  | { kind: "trash" }
  | { kind: "folder"; id: string };

export type LibraryScopeKind = "all" | "uncategorized" | "untagged" | "recent" | "random" | "tags" | "trash";

export const LIBRARY_LABELS: Record<LibraryScopeKind, string> = {
  all: "All",
  uncategorized: "Uncategorized",
  untagged: "Untagged",
  recent: "Recently Used",
  random: "Random",
  tags: "All Tags",
  trash: "Trash",
};

/** How many pins Recently Used and Random show. */
export const RECENT_LIMIT = 200;
export const RANDOM_LIMIT = 200;

export function sameScope(left: BoardsScope, right: BoardsScope) {
  return scopeKey(left) === scopeKey(right);
}

export function scopeKey(scope: BoardsScope) {
  switch (scope.kind) {
    case "folder":
      return `folder:${scope.id}`;
    case "tag":
      return `tag:${scope.name}`;
    case "random":
      return `random:${scope.seed}`;
    default:
      return scope.kind;
  }
}

export function scopeTitle(boards: BoardsSnapshot, scope: BoardsScope) {
  switch (scope.kind) {
    case "folder":
      return boards.folders.find((folder) => folder.id === scope.id)?.name ?? "Folder";
    case "tag":
      return `#${scope.name}`;
    default:
      return LIBRARY_LABELS[scope.kind];
  }
}

/** Longer description for tooltips and confirm messages. */
export function scopeLabel(boards: BoardsSnapshot, scope: BoardsScope) {
  return scope.kind === "folder" ? folderLabel(boards, scope.id) : scopeTitle(boards, scope);
}

/** A small deterministic generator so one Random seed keeps its order until reshuffled. */
function seeded(seed: number) {
  let state = seed >>> 0 || 1;
  return () => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return (state >>> 0) / 4294967296;
  };
}

function shuffled<T>(items: T[], seed: number) {
  const next = [...items];
  const random = seeded(seed);
  for (let index = next.length - 1; index > 0; index -= 1) {
    const other = Math.floor(random() * (index + 1));
    [next[index], next[other]] = [next[other], next[index]];
  }
  return next;
}

/** Pins a scope lists before search, filters and sorting. */
export function scopePins(boards: BoardsSnapshot, scope: BoardsScope): Pin[] {
  switch (scope.kind) {
    case "all":
      return livePins(boards);
    case "uncategorized":
      return livePins(boards).filter((pin) => pin.folderIds.length === 0);
    case "untagged":
      return livePins(boards).filter((pin) => pin.tags.length === 0);
    case "recent":
      return livePins(boards)
        .filter((pin) => pin.lastOpenedAt > 0)
        .sort((left, right) => right.lastOpenedAt - left.lastOpenedAt)
        .slice(0, RECENT_LIMIT);
    case "random":
      return shuffled(livePins(boards), scope.seed).slice(0, RANDOM_LIMIT);
    case "tags":
      return [];
    case "tag": {
      const name = scope.name.toLowerCase();
      return livePins(boards).filter((pin) => pin.tags.some((tag) => tag.toLowerCase() === name));
    }
    case "trash":
      return boards.pins.filter((pin) => pin.deletedAt !== null).sort((left, right) => (right.deletedAt ?? 0) - (left.deletedAt ?? 0));
    case "folder": {
      const folder = boards.folders.find((item) => item.id === scope.id);
      return folder ? pinsInFolder(boards, folder.id, folder.showSubfolders) : [];
    }
  }
}

/** The scope keeps its own order (Recently Used, Random, Trash), so the sort menu does not apply. */
export function scopeHasOwnOrder(scope: BoardsScope) {
  return scope.kind === "recent" || scope.kind === "random" || scope.kind === "trash";
}

/** Folder that files added in this scope go to. Other scopes save to Uncategorized. */
export function scopeSaveTarget(scope: BoardsScope) {
  return scope.kind === "folder" ? scope.id : UNCATEGORIZED;
}

/** The scope still exists in `boards`. */
export function scopeExists(boards: BoardsSnapshot, scope: BoardsScope) {
  if (scope.kind === "folder") {
    return boards.folders.some((folder) => folder.id === scope.id);
  }
  return true;
}

/** `ce://boards?folder=<id>` (or the older `board=<id>`) opens that folder. */
export function scopeFromParams(params: URLSearchParams): BoardsScope {
  const folder = params.get("folder") ?? params.get("board");
  return folder ? { kind: "folder", id: folder } : { kind: "all" };
}
