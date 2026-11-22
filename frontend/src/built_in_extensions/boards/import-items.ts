import { importBoardItems } from "@/lib/browser-client";
import type { BoardImportItem, BoardImportProgress } from "@type/browser";
import type { DroppedFile } from "./import/walk-drop";

/** Links saved at once from a paste or drop. Files have no count limit. */
export const MAX_LINKS = 200;
/** Largest file read from a drop or paste. Matches the backend limit. */
export const MAX_IMPORT_BYTES = 200 * 1024 * 1024;
/** Each call to the backend carries about this much file data. */
const BATCH_BYTES = 48 * 1024 * 1024;
const BATCH_FILES = 40;

const URL_PATTERN = /https?:\/\/[^\s<>"']+/gi;

export function linksInText(text: string) {
  const found = text.match(URL_PATTERN) ?? [];
  const urls = found.map((url) => url.replace(/[),.;]+$/, ""));
  return [...new Set(urls)];
}

export function pathsToImports(paths: string[]): BoardImportItem[] {
  return paths.map((path) => ({
    name: path.split(/[\\/]/).pop() ?? "",
    mime: "",
    url: "",
    data: "",
    path,
  }));
}

export function linksToImports(urls: string[]): BoardImportItem[] {
  return urls.slice(0, MAX_LINKS).map((url) => ({
    name: "",
    mime: "",
    url,
    data: "",
    path: "",
  }));
}

function fileToBase64(file: File) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = typeof reader.result === "string" ? reader.result : "";
      const comma = result.indexOf(",");
      resolve(comma >= 0 ? result.slice(comma + 1) : result);
    };
    reader.onerror = () => reject(reader.error ?? new Error("Couldn't read that file."));
    reader.readAsDataURL(file);
  });
}

async function toItem({ file, relativeDir }: DroppedFile): Promise<BoardImportItem> {
  return { name: file.name, mime: file.type, url: "", data: await fileToBase64(file), path: "", relativeDir };
}

/** Groups files so each backend call stays a manageable size. */
function batches(files: DroppedFile[]) {
  const out: DroppedFile[][] = [];
  let current: DroppedFile[] = [];
  let bytes = 0;
  for (const entry of files) {
    if (current.length > 0 && (bytes + entry.file.size > BATCH_BYTES || current.length >= BATCH_FILES)) {
      out.push(current);
      current = [];
      bytes = 0;
    }
    current.push(entry);
    bytes += entry.file.size;
  }
  if (current.length > 0) {
    out.push(current);
  }
  return out;
}

/** Imports dropped or pasted files in batches, keeping the folders they came from.
 * `onProgress` gets a running total in the same shape as a backend import job. */
export async function importFiles(
  files: DroppedFile[],
  folderId: string,
  jobId: string,
  onProgress: (progress: BoardImportProgress) => void,
  cancelled: () => boolean,
): Promise<BoardImportProgress> {
  const tooLarge = files.filter((entry) => entry.file.size > MAX_IMPORT_BYTES);
  const accepted = files.filter((entry) => entry.file.size <= MAX_IMPORT_BYTES);
  const progress: BoardImportProgress = {
    jobId,
    total: files.length,
    done: tooLarge.length,
    imported: 0,
    duplicates: 0,
    failed: tooLarge.length,
    current: "",
    finished: false,
    cancelled: false,
    error: tooLarge.length > 0 ? `${tooLarge[0].file.name} is larger than 200 MB.` : "",
    folderId: null,
  };
  onProgress({ ...progress });
  for (const batch of batches(accepted)) {
    if (cancelled()) {
      progress.cancelled = true;
      break;
    }
    progress.current = batch[0].relativeDir ? `${batch[0].relativeDir}/${batch[0].file.name}` : batch[0].file.name;
    onProgress({ ...progress });
    try {
      const items = await Promise.all(batch.map(toItem));
      const result = await importBoardItems(folderId, items);
      progress.imported += result.pins.length;
      progress.duplicates += result.duplicates;
      progress.failed += result.failed;
    } catch (reason) {
      progress.failed += batch.length;
      progress.error ||= reason instanceof Error ? reason.message : String(reason);
    }
    progress.done += batch.length;
    onProgress({ ...progress });
  }
  progress.finished = true;
  progress.current = "";
  onProgress({ ...progress });
  return progress;
}
