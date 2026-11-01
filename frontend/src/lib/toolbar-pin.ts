import type { MouseEvent } from "react";
import type { BrowserSettings } from "../../../types/browser";
import { openPopup, saveSettings } from "./browser-client";

/** Built-in tools, in the order they had before a custom order was saved. */
export const DEFAULT_TOOLBAR = [
  "print-pdf",
  "page-capture",
  "media-hub",
  "clear-cache",
  "what-runs",
  "ad-blocker",
  "page-controls",
  "translate",
  "password",
  "price",
  "wayback",
  "reading",
  "assistant",
];

/** Saved ids that are still pinned, then any pinned id the saved list does not mention. */
export function toolbarOrder(saved: string[] | undefined, available: string[]) {
  const ordered: string[] = [];
  for (const id of saved ?? []) {
    if (available.includes(id) && !ordered.includes(id)) {
      ordered.push(id);
    }
  }
  for (const id of available) {
    if (!ordered.includes(id)) {
      ordered.push(id);
    }
  }
  return ordered;
}

/** An extension stays on the address bar until its id is stored in settings. */
export function toolbarPinned(settings: BrowserSettings | null, id: string) {
  return !settings?.hiddenToolbarExtensions?.includes(id);
}

export function saveToolbarPin(settings: BrowserSettings, id: string, pinned: boolean) {
  const hidden = (settings.hiddenToolbarExtensions ?? []).filter((item) => item !== id);
  if (!pinned) {
    hidden.push(id);
  }
  return saveSettings({ ...settings, hiddenToolbarExtensions: hidden });
}

/** Right-click menu for an icon that is currently on the address bar. */
export function openToolbarPinMenu(event: MouseEvent, id: string) {
  event.preventDefault();
  void openPopup({
    kind: "extension-pin",
    params: `id=${encodeURIComponent(id)}`,
    x: event.clientX,
    y: event.clientY,
    width: 220,
    height: 64,
  });
}
