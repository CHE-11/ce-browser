/** A dropped file and the folder path it sat in, relative to what was dropped ("Trip/Day 1"). */
export type DroppedFile = { file: File; relativeDir: string };

const SKIPPED = new Set(["thumbs.db", "desktop.ini", ".ds_store", "ehthumbs.db"]);
const MAX_DEPTH = 32;

function skipped(name: string) {
  return name.startsWith(".") || SKIPPED.has(name.toLowerCase());
}

function readBatch(reader: FileSystemDirectoryReader) {
  return new Promise<FileSystemEntry[]>((resolve, reject) => reader.readEntries(resolve, reject));
}

/** `readEntries` returns at most about 100 entries a call, so read until it comes back empty. */
async function readAll(directory: FileSystemDirectoryEntry) {
  const reader = directory.createReader();
  const entries: FileSystemEntry[] = [];
  for (;;) {
    const batch = await readBatch(reader);
    if (batch.length === 0) {
      return entries;
    }
    entries.push(...batch);
  }
}

function fileOf(entry: FileSystemFileEntry) {
  return new Promise<File>((resolve, reject) => entry.file(resolve, reject));
}

async function walk(entry: FileSystemEntry, dir: string, depth: number, out: DroppedFile[]) {
  if (skipped(entry.name)) {
    return;
  }
  if (entry.isFile) {
    try {
      const file = await fileOf(entry as FileSystemFileEntry);
      if (file.size > 0) {
        out.push({ file, relativeDir: dir });
      }
    } catch {
      // Unreadable files are left out.
    }
    return;
  }
  if (entry.isDirectory && depth < MAX_DEPTH) {
    const path = dir ? `${dir}/${entry.name}` : entry.name;
    for (const child of await readAll(entry as FileSystemDirectoryEntry)) {
      await walk(child, path, depth + 1, out);
    }
  }
}

/** Entries must be taken from the drop event before it returns; they can be walked later. */
export function dropEntries(data: DataTransfer): FileSystemEntry[] {
  const entries: FileSystemEntry[] = [];
  for (const item of data.items) {
    if (item.kind !== "file") {
      continue;
    }
    const entry = item.webkitGetAsEntry();
    if (entry) {
      entries.push(entry);
    }
  }
  return entries;
}

/** True when the drop holds at least one folder. */
export function hasDirectory(entries: FileSystemEntry[]) {
  return entries.some((entry) => entry.isDirectory);
}

/** Every file in the dropped files and folders, with the folders they were in. */
export async function walkEntries(entries: FileSystemEntry[]): Promise<DroppedFile[]> {
  const out: DroppedFile[] = [];
  for (const entry of entries) {
    await walk(entry, "", 0, out);
  }
  return out;
}
