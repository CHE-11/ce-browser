import { chooseSavePath, defaultDownloadDir, savePinFile } from "@/lib/browser-client";
import type { Pin } from "@type/browser";
import { fileNameFor, joinPath } from "@features/BuiltInExtensions/shared";

/** Asks where to save a file pin. Returns false when the dialog is cancelled. */
export async function savePinFileAs(pin: Pin) {
  const folder = await defaultDownloadDir();
  const name = pin.fileName || fileNameFor(pin.title, "", "bin");
  const extension = name.includes(".") ? (name.split(".").pop() ?? "") : "";
  const path = await chooseSavePath(joinPath(folder, name), "File", extension ? [extension] : ["*"]);
  if (!path) {
    return false;
  }
  await savePinFile(pin.id, path);
  return true;
}
