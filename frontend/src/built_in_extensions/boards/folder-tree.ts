import { boardFileUrl, boardThumbUrl } from "@/lib/browser-client";
import type { BoardFolder, BoardsSnapshot, FolderColor, Pin, PinKind, PinSort } from "@type/browser";

export const EMPTY_BOARDS: BoardsSnapshot = { folders: [], pins: [], lastFolderId: null };

/** Save target for pins that belong to no folder. An empty target means the last folder used. */
export const UNCATEGORIZED = "uncategorized";

/** Drag data types on `ce://boards`. Pins carry a JSON array of ids, folders one id. */
export const DRAG_PINS = "application/x-ce-pins";
export const DRAG_FOLDER = "application/x-ce-board-folder";

export const FOLDER_COLORS: FolderColor[] = ["gray", "red", "orange", "yellow", "green", "aqua", "blue", "purple", "pink"];

/** Text and swatch classes for each folder color. */
export const FOLDER_COLOR_CLASS: Record<FolderColor, { text: string; swatch: string }> = {
  gray: { text: "text-zinc-400", swatch: "bg-zinc-400" },
  red: { text: "text-red-400", swatch: "bg-red-500" },
  orange: { text: "text-orange-400", swatch: "bg-orange-500" },
  yellow: { text: "text-yellow-300", swatch: "bg-yellow-400" },
  green: { text: "text-emerald-400", swatch: "bg-emerald-500" },
  aqua: { text: "text-cyan-300", swatch: "bg-cyan-400" },
  blue: { text: "text-sky-400", swatch: "bg-sky-500" },
  purple: { text: "text-violet-400", swatch: "bg-violet-500" },
  pink: { text: "text-pink-400", swatch: "bg-pink-500" },
};

export function folderColorText(folder: Pick<BoardFolder, "color"> | undefined) {
  return folder?.color ? FOLDER_COLOR_CLASS[folder.color].text : "";
}

export const PIN_KIND_LABELS: Record<PinKind, string> = {
  page: "Page",
  region: "Region",
  element: "Section",
  link: "Link",
  image: "Image",
  file: "File",
};

export const PIN_KINDS: PinKind[] = ["page", "region", "element", "link", "image", "file"];

export const PIN_SORT_LABELS: Record<PinSort, string> = {
  newest: "Date added (newest)",
  oldest: "Date added (oldest)",
  title: "Name",
  kind: "Type",
  size: "File size",
  rating: "Rating",
  dimensions: "Dimensions",
  extension: "Extension",
};

export const PIN_SORTS = Object.keys(PIN_SORT_LABELS) as PinSort[];

export function isPinSort(value: unknown): value is PinSort {
  return typeof value === "string" && value in PIN_SORT_LABELS;
}

const compareText = (left: string, right: string) => left.localeCompare(right, undefined, { sensitivity: "base", numeric: true });

const byName = (left: { name: string }, right: { name: string }) => compareText(left.name, right.name);

/** `pins` stay in newest-first order until a sort asks for something else. */
export function sortPins(pins: Pin[], sort: PinSort): Pin[] {
  const next = [...pins];
  const title = (left: Pin, right: Pin) => compareText(pinName(left), pinName(right));
  switch (sort) {
    case "oldest":
      next.reverse();
      break;
    case "title":
      next.sort(title);
      break;
    case "kind":
      next.sort((left, right) => PIN_KINDS.indexOf(left.kind) - PIN_KINDS.indexOf(right.kind) || title(left, right));
      break;
    case "size":
      next.sort((left, right) => right.size - left.size || title(left, right));
      break;
    case "rating":
      next.sort((left, right) => right.rating - left.rating || right.createdAt - left.createdAt);
      break;
    case "dimensions":
      next.sort((left, right) => right.width * right.height - left.width * left.height || title(left, right));
      break;
    case "extension":
      next.sort((left, right) => compareText(left.ext, right.ext) || title(left, right));
      break;
    default:
      break;
  }
  return next;
}

export function childFolders(boards: BoardsSnapshot, parentId: string | null): BoardFolder[] {
  return boards.folders.filter((folder) => folder.parentId === parentId).sort(byName);
}

/** Folders from the top level down to `folderId`. */
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

/** "Folder / Sub folder". */
export function folderLabel(boards: BoardsSnapshot, folderId: string) {
  return folderPath(boards, folderId)
    .map((folder) => folder.name)
    .join(" / ");
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

/** Every folder in tree order with its depth, for pickers. */
export function folderChoices(boards: BoardsSnapshot): { folder: BoardFolder; depth: number; label: string }[] {
  const out: { folder: BoardFolder; depth: number; label: string }[] = [];
  const walk = (parentId: string | null, depth: number, prefix: string) => {
    for (const folder of childFolders(boards, parentId)) {
      const label = prefix ? `${prefix} / ${folder.name}` : folder.name;
      out.push({ folder, depth, label });
      walk(folder.id, depth + 1, label);
    }
  };
  walk(null, 0, "");
  return out;
}

/** Pins that are not in the trash. */
export function livePins(boards: BoardsSnapshot) {
  return boards.pins.filter((pin) => pin.deletedAt === null);
}

/** Pins directly in `folderId`, or in it and its subfolders. */
export function pinsInFolder(boards: BoardsSnapshot, folderId: string, withSubfolders: boolean): Pin[] {
  const folders = withSubfolders ? folderTree(boards, folderId) : new Set([folderId]);
  return boards.pins.filter((pin) => pin.deletedAt === null && pin.folderIds.some((id) => folders.has(id)));
}

/** Live pin counts by folder id, counting each folder's own pins only. */
export function folderCounts(boards: BoardsSnapshot) {
  const counts = new Map<string, number>();
  for (const pin of boards.pins) {
    if (pin.deletedAt !== null) {
      continue;
    }
    for (const id of pin.folderIds) {
      counts.set(id, (counts.get(id) ?? 0) + 1);
    }
  }
  return counts;
}

/** What deleting a folder affects: subfolders, pins inside, and pins that would go to the trash. */
export function folderContents(boards: BoardsSnapshot, folderId: string) {
  const folders = folderTree(boards, folderId);
  const inside = pinsInFolder(boards, folderId, true);
  const trashed = inside.filter((pin) => pin.folderIds.every((id) => folders.has(id))).length;
  return { folders: folders.size - 1, pins: inside.length, trashed };
}

/** Up to `count` pins for a folder's mosaic: its cover first, then the newest with a preview. */
export function folderPreviewPins(boards: BoardsSnapshot, folder: BoardFolder, count = 4): Pin[] {
  const pins = pinsInFolder(boards, folder.id, true).filter(hasPreview);
  const cover = folder.coverPinId ? pins.find((pin) => pin.id === folder.coverPinId) : undefined;
  return (cover ? [cover, ...pins.filter((pin) => pin.id !== cover.id)] : pins).slice(0, count);
}

/** Every tag in use with its live pin count, by name. */
export function tagCounts(boards: BoardsSnapshot) {
  const counts = new Map<string, number>();
  for (const pin of boards.pins) {
    if (pin.deletedAt !== null) {
      continue;
    }
    for (const tag of pin.tags) {
      counts.set(tag, (counts.get(tag) ?? 0) + 1);
    }
  }
  return [...counts.entries()].map(([name, count]) => ({ name, count })).sort((left, right) => compareText(left.name, right.name));
}

/** The pin shows an image: a stored image, a Windows thumbnail, or an SVG. */
export function hasPreview(pin: Pin) {
  return pin.hasImage || pin.hasThumb || pin.ext === "svg";
}

/** Address of the pin's preview image, or empty when it has none. */
export function previewUrl(pin: Pin) {
  if (pin.ext === "svg") {
    return boardFileUrl(pin);
  }
  return pin.hasImage || pin.hasThumb ? boardThumbUrl(pin) : "";
}

const VIDEO_EXT = new Set(["mp4", "m4v", "webm", "mov", "mkv", "ogv"]);
const AUDIO_EXT = new Set(["mp3", "wav", "ogg", "m4a", "flac", "aac"]);
const TEXT_EXT = new Set(["txt", "md", "log", "csv", "json", "html", "htm", "css", "js", "ts", "tsx", "jsx", "xml", "yml", "yaml", "toml", "ini", "rs", "py"]);
/** Files the viewer shows with `<img>` rather than a thumbnail. */
const IMAGE_EXT = new Set(["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "svg", "avif"]);

export type ViewerKind = "image" | "video" | "audio" | "pdf" | "text" | "link" | "file";

/** How the viewer shows the pin. */
export function viewerKind(pin: Pin): ViewerKind {
  const ext = pin.ext.toLowerCase();
  if (pin.hasImage || (pin.kind === "image" && IMAGE_EXT.has(ext))) {
    return "image";
  }
  if (pin.kind === "file" || pin.kind === "image") {
    if (IMAGE_EXT.has(ext)) {
      return "image";
    }
    if (VIDEO_EXT.has(ext)) {
      return "video";
    }
    if (AUDIO_EXT.has(ext)) {
      return "audio";
    }
    if (ext === "pdf") {
      return "pdf";
    }
    if (TEXT_EXT.has(ext)) {
      return "text";
    }
    return "file";
  }
  return "link";
}

/** Page or link address. File pins have neither. */
export function pinAddress(pin: Pin) {
  return pin.kind === "link" && pin.linkUrl ? pin.linkUrl : pin.pageUrl || pin.linkUrl;
}

/** The pin's address when it is a web page that can be previewed, otherwise empty. */
export function previewableAddress(pin: Pin) {
  const address = pinAddress(pin);
  return /^https?:\/\//i.test(address) ? address : "";
}

export function hostOf(url: string) {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return "";
  }
}

/** The name shown under a pin. */
export function pinName(pin: Pin) {
  return pin.title || pin.fileName || hostOf(pinAddress(pin)) || PIN_KIND_LABELS[pin.kind];
}

export function pinMatches(pin: Pin, query: string) {
  const needle = query.trim().toLowerCase();
  if (!needle) {
    return true;
  }
  return [pin.title, pin.text, pin.note, pin.pageUrl, pin.linkUrl, pin.fileName, pin.ext, ...pin.tags].some((field) =>
    field.toLowerCase().includes(needle),
  );
}

export function formatBytes(bytes: number) {
  if (bytes <= 0) {
    return "";
  }
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value >= 100 || unit === 0 ? Math.round(value) : value.toFixed(1)} ${units[unit]}`;
}

export function formatDimensions(pin: Pin) {
  return pin.width > 0 && pin.height > 0 ? `${pin.width} × ${pin.height}` : "";
}
