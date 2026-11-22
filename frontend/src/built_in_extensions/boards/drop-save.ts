import { importBoardItems, pinImages } from "@/lib/browser-client";
import { BOOKMARK_DRAG_TYPE } from "@/lib/use-browser";
import { DRAG_FOLDER, DRAG_PINS } from "./folder-tree";
import { MAX_LINKS, importFiles, linksInText, linksToImports, pathsToImports } from "./import-items";
import { dropEntries, walkEntries } from "./import/walk-drop";

const INTERNAL_DRAG = new Set([BOOKMARK_DRAG_TYPE, DRAG_PINS, DRAG_FOLDER]);
const IMAGE_EXT = /\.(apng|avif|bmp|gif|jpe?g|png|webp)$/i;

export type BoardDropPayload = {
  /** Dropped files and folders. Folders are walked so their structure is kept. */
  entries: FileSystemEntry[];
  /** Files when the drop gave no entries. */
  files: File[];
  paths: string[];
  imageUrls: string[];
  links: string[];
};

export type SavedDrop = {
  saved: number;
  duplicates: number;
  failed: number;
};

export function isEmptyDrop(drop: BoardDropPayload) {
  return drop.entries.length === 0 && drop.files.length === 0 && drop.paths.length === 0 && drop.imageUrls.length === 0 && drop.links.length === 0;
}

/** A drag the boards button can accept. Toolbar reordering is a move of plain text, so it is left alone. */
export function isBoardFileDrag(data: DataTransfer) {
  const types = [...data.types];
  if (types.some((type) => INTERNAL_DRAG.has(type))) {
    return false;
  }
  if (types.includes("Files")) {
    return true;
  }
  if (data.effectAllowed === "move") {
    return false;
  }
  return types.includes("text/uri-list") || types.includes("URL") || types.includes("text/html") || types.includes("text/plain");
}

/** Reads a drop synchronously. `getData` and the entries are gone once the drop event has finished. */
export function readBoardDrop(data: DataTransfer, pageUrl = ""): BoardDropPayload {
  const entries = dropEntries(data);
  if (entries.length > 0) {
    return { entries, files: [], paths: [], imageUrls: [], links: [] };
  }
  const files = [...data.files].filter((file) => file.size > 0);
  if (files.length > 0) {
    return { entries: [], files, paths: [], imageUrls: [], links: [] };
  }

  const listed = uriList(data);
  const paths: string[] = [];
  const urls: string[] = [];
  for (const line of listed) {
    const path = fileUrlToPath(line);
    if (path) {
      paths.push(path);
    } else {
      urls.push(...linksInText(line));
    }
  }

  const imagesInHtml = imageSources(data.getData("text/html"), pageUrl);
  const imageUrls: string[] = [];
  const links: string[] = [];
  for (const url of unique(urls)) {
    if (isImageAddress(url) || imagesInHtml.some((image) => sameUrl(image, url))) {
      imageUrls.push(url);
    } else {
      links.push(url);
    }
  }
  if (links.length === 0) {
    for (const image of imagesInHtml) {
      if (!imageUrls.some((url) => sameUrl(url, image))) {
        imageUrls.push(image);
      }
    }
  }
  return { entries: [], files: [], paths: unique(paths), imageUrls, links };
}

export function savedDropMessage(result: SavedDrop, folderName: string) {
  const parts = [result.saved > 0 ? `Saved ${result.saved === 1 ? "1 item" : `${result.saved} items`} to ${folderName}.` : "Nothing new was saved."];
  if (result.duplicates > 0) {
    parts.push(`${result.duplicates === 1 ? "1 was" : `${result.duplicates} were`} already there.`);
  }
  if (result.failed > 0) {
    parts.push(`${result.failed === 1 ? "1 was" : `${result.failed} were`} left out.`);
  }
  return parts.join(" ");
}

/** Saves a drop to `folderId`: a folder id, `UNCATEGORIZED`, or empty for the folder used last. */
export async function saveBoardDrop(drop: BoardDropPayload, folderId: string, tabId: number): Promise<SavedDrop> {
  if (isEmptyDrop(drop)) {
    throw new Error("Drop an image, file, folder or link.");
  }

  const result: SavedDrop = { saved: 0, duplicates: 0, failed: 0 };
  let firstError: unknown;

  const run = async (work: () => Promise<{ pins: unknown[]; duplicates: number; failed: number }>) => {
    try {
      const done = await work();
      result.saved += done.pins.length;
      result.duplicates += done.duplicates;
      result.failed += done.failed;
    } catch (error) {
      firstError ??= error;
    }
  };

  const files = drop.entries.length > 0 ? await walkEntries(drop.entries) : drop.files.map((file) => ({ file, relativeDir: "" }));
  if (files.length > 0) {
    const progress = await importFiles(files, folderId, `drop-${Date.now()}`, () => undefined, () => false);
    result.saved += progress.imported;
    result.duplicates += progress.duplicates;
    result.failed += progress.failed;
    if (progress.error) {
      firstError ??= new Error(progress.error);
    }
  }
  if (drop.paths.length > 0) {
    await run(() => importBoardItems(folderId, pathsToImports(drop.paths)));
  }

  let images = drop.imageUrls;
  if (images.length > 0 && tabId > 0) {
    const batch = images.slice(0, MAX_LINKS);
    try {
      const pins = await pinImages(tabId, folderId, batch);
      result.saved += pins.length;
      result.failed += Math.max(0, images.length - MAX_LINKS);
      images = [];
    } catch (error) {
      firstError ??= error;
    }
  }
  const links = images.length > 0 ? [...drop.links, ...images] : drop.links;
  if (links.length > 0) {
    result.failed += Math.max(0, links.length - MAX_LINKS);
    await run(() => importBoardItems(folderId, linksToImports(links)));
  }
  if (result.saved === 0 && result.duplicates === 0) {
    if (firstError instanceof Error || typeof firstError === "string") {
      throw firstError;
    }
    throw new Error("Nothing could be saved.");
  }
  return result;
}

function uriList(data: DataTransfer) {
  const raw = [data.getData("text/uri-list"), data.getData("URL"), data.getData("text/plain")].filter(Boolean).join("\n");
  return raw
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith("#"));
}

function imageSources(html: string, pageUrl: string) {
  const found: string[] = [];
  const pattern = /<img\b[^>]*\bsrc\s*=\s*(?:"([^"]+)"|'([^']+)'|([^\s>]+))/gi;
  for (const match of html.matchAll(pattern)) {
    const url = resolveUrl(match[1] ?? match[2] ?? match[3] ?? "", pageUrl);
    if (url.startsWith("data:image/svg")) {
      continue;
    }
    if (url.startsWith("data:image/") || url.startsWith("http://") || url.startsWith("https://")) {
      found.push(url);
    }
  }
  return found;
}

function resolveUrl(raw: string, base: string) {
  const value = raw.trim();
  if (!value || value.startsWith("blob:") || value.startsWith("javascript:")) {
    return "";
  }
  try {
    return new URL(value, base || undefined).href;
  } catch {
    return "";
  }
}

function fileUrlToPath(value: string) {
  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    return "";
  }
  if (parsed.protocol !== "file:") {
    return "";
  }
  let path = decodeURIComponent(parsed.pathname);
  if (/^\/[a-zA-Z]:/.test(path)) {
    path = path.slice(1);
  }
  return path.replaceAll("/", "\\");
}

function isImageAddress(url: string) {
  if (url.startsWith("data:image/")) {
    return !url.startsWith("data:image/svg");
  }
  try {
    return IMAGE_EXT.test(decodeURIComponent(new URL(url).pathname));
  } catch {
    return IMAGE_EXT.test(url);
  }
}

function sameUrl(left: string, right: string) {
  return left === right || left.replace(/\/$/, "") === right.replace(/\/$/, "");
}

function unique(values: string[]) {
  return [...new Set(values)];
}
