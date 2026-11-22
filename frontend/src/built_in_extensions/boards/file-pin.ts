import { chooseSavePath, defaultDownloadDir, savePinFile } from "@/lib/browser-client";
import type { Pin } from "@type/browser";
import { fileNameFor, joinPath } from "@/built_in_extensions/shared";

/** The pin keeps its own file: an image, a page screenshot, or an imported file. */
export function hasOriginal(pin: Pin) {
  return pin.hasImage || pin.kind === "file" || pin.kind === "image";
}

/** Asks where to save the pin's original file. Returns false when the dialog is cancelled. */
export async function savePinFileAs(pin: Pin) {
  const folder = await defaultDownloadDir();
  const ext = pin.ext || (pin.hasImage ? "png" : "bin");
  const name = pin.fileName || fileNameFor(pin.title, "", ext);
  const extension = name.includes(".") ? (name.split(".").pop() ?? "") : ext;
  const path = await chooseSavePath(joinPath(folder, name), "File", extension ? [extension] : ["*"]);
  if (!path) {
    return false;
  }
  await savePinFile(pin.id, path);
  return true;
}
