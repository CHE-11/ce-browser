import { importBoardItems, pinImages } from "@/lib/browser-client";
import { BOOKMARK_DRAG_TYPE } from "@/lib/use-browser";
import type { Pin } from "@type/browser";
import { DRAG_BOARD, DRAG_FOLDER, DRAG_PINS } from "./board-tree";
import { MAX_IMPORT, filesToImports, linksInText, linksToImports, pathsToImports } from "./import-items";

const INTERNAL_DRAG = new Set([BOOKMARK_DRAG_TYPE, DRAG_PINS, DRAG_BOARD, DRAG_FOLDER]);
const IMAGE_EXT = /\.(apng|avif|bmp|gif|jpe?g|png|webp)$/i;

export type BoardDropPayload = {
  files: File[];
  paths: string[];
  imageUrls: string[];
  links: string[];
};

export type SavedDrop = {
  pins: Pin[];
  skipped: number;
};

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

/** Reads a drop synchronously. `getData` is empty once the drop event has finished. */
export function readBoardDrop(data: DataTransfer, pageUrl = ""): BoardDropPayload {
  const files = [...data.files].filter((file) => file.size > 0);
  if (files.length > 0) {
    return { files, paths: [], imageUrls: [], links: [] };
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
  return { files: [], paths: unique(paths), imageUrls, links };
}

export function savedDropMessage(count: number, boardName: string, skipped: number) {
  const items = count === 1 ? "1 item" : `${count} items`;
  const extra = skipped > 0 ? ` ${skipped === 1 ? "1 was" : `${skipped} were`} left out.` : "";
  return `Saved ${items} to ${boardName}.${extra}`;
}

/** Saves a drop onto `boardId`. An empty id uses the last board, or creates Unsorted. */
export async function saveBoardDrop(drop: BoardDropPayload, boardId: string, tabId: number): Promise<SavedDrop> {
  if (drop.files.length === 0 && drop.paths.length === 0 && drop.imageUrls.length === 0 && drop.links.length === 0) {
    throw new Error("Drop an image, file, or link.");
  }

  let board = boardId;
  let skipped = 0;
  let firstError: unknown;
  const pins: Pin[] = [];

  const take = (saved: Pin[]) => {
    const last = saved[saved.length - 1];
    if (last) {
      board = last.boardId;
    }
    pins.push(...saved);
  };

  const run = async (work: () => Promise<Pin[]>) => {
    try {
      take(await work());
    } catch (error) {
      firstError ??= error;
    }
  };

  if (drop.files.length > 0) {
    const imported = await filesToImports(drop.files);
    skipped += imported.skipped;
    await run(() => importBoardItems(board, imported.items));
  }
  if (drop.paths.length > 0) {
    skipped += Math.max(0, drop.paths.length - MAX_IMPORT);
    await run(() => importBoardItems(board, pathsToImports(drop.paths)));
  }

  let images = drop.imageUrls;
  if (images.length > 0 && tabId > 0) {
    const batch = images.slice(0, MAX_IMPORT);
    try {
      take(await pinImages(tabId, board, batch));
      skipped += Math.max(0, images.length - MAX_IMPORT);
      images = [];
    } catch (error) {
      firstError ??= error;
    }
  }
  const links = images.length > 0 ? [...drop.links, ...images] : drop.links;
  if (links.length > 0) {
    skipped += Math.max(0, links.length - MAX_IMPORT);
    await run(() => importBoardItems(board, linksToImports(links)));
  }
  if (pins.length === 0) {
    if (firstError instanceof Error || typeof firstError === "string") {
      throw firstError;
    }
    throw new Error("Nothing could be saved.");
  }
  return { pins, skipped };
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
