import type { MouseEvent } from "react";
import type { BrowserSettings } from "../../../types/browser";
import { openPopup, saveSettings } from "./browser-client";

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
