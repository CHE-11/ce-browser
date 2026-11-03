import type { BoardImportItem } from "@type/browser";

export const MAX_IMPORT = 40;
export const MAX_IMPORT_BYTES = 20 * 1024 * 1024;

const URL_PATTERN = /https?:\/\/[^\s<>"']+/gi;

export function linksInText(text: string) {
  const found = text.match(URL_PATTERN) ?? [];
  const urls = found.map((url) => url.replace(/[),.;]+$/, ""));
  return [...new Set(urls)];
}

export function pathsToImports(paths: string[]): BoardImportItem[] {
  return paths.slice(0, MAX_IMPORT).map((path) => ({
    name: path.split(/[\\/]/).pop() ?? "",
    mime: "",
    url: "",
    data: "",
    path,
  }));
}

export function linksToImports(urls: string[]): BoardImportItem[] {
  return urls.slice(0, MAX_IMPORT).map((url) => ({
    name: "",
    mime: "",
    url,
    data: "",
    path: "",
  }));
}

/** Reads dropped or pasted files. Throws when one file is over the size limit. */
export async function filesToImports(files: File[]): Promise<{ items: BoardImportItem[]; skipped: number }> {
  const chosen = files.slice(0, MAX_IMPORT);
  for (const file of chosen) {
    if (file.size > MAX_IMPORT_BYTES) {
      throw new Error(`${file.name || "That file"} is larger than 20 MB.`);
    }
    if (file.size === 0) {
      throw new Error(`${file.name || "That file"} is empty.`);
    }
  }
  const items = await Promise.all(
    chosen.map(async (file) => ({
      name: file.name,
      mime: file.type,
      url: "",
      data: await fileToBase64(file),
      path: "",
    })),
  );
  return { items, skipped: Math.max(0, files.length - MAX_IMPORT) };
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
