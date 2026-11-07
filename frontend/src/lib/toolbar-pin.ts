import type { DragEvent, MouseEvent } from "react";
import type { BrowserSettings } from "../../../types/browser";
import { BUILTIN_TOOLS } from "../features/BuiltInExtensions/builtin";
import { openPopup, saveSettings } from "./browser-client";

/** Built-in tools, in the order they had before a custom order was saved. Address-bar controls are placed directly. */
export const DEFAULT_TOOLBAR = BUILTIN_TOOLS.filter((tool) => !tool.address).map((tool) => tool.id);

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

/** A tool stays on its toolbar until its id is stored in settings. */
export function toolbarPinned(settings: BrowserSettings | null, id: string) {
  return !settings?.hiddenToolbarExtensions?.includes(id);
}

export type ToolbarSplit = {
  /** Pinned built-in tools on the tools toolbar. */
  tools: string[];
  /** Pinned extensions next to the address bar. */
  address: string[];
  /** Every pinned id, in saved order. Dragging uses this list. */
  all: string[];
};

/** Built-in tools stay on the tools toolbar. Extensions stay next to the address bar. */
export function splitToolbar(settings: BrowserSettings | null, extensionIds: string[]): ToolbarSplit {
  const pinnedExtensions = extensionIds.filter((id) => toolbarPinned(settings, id));
  const all = toolbarOrder(settings?.toolbarOrder, [
    ...DEFAULT_TOOLBAR.filter((id) => toolbarPinned(settings, id)),
    ...pinnedExtensions,
  ]);
  const extensions = new Set(pinnedExtensions);
  return {
    tools: all.filter((id) => !extensions.has(id)),
    address: all.filter((id) => extensions.has(id)),
    all,
  };
}

/** Moves `from` to sit just before `to` in the shared toolbar order. */
export function reorderToolbar(order: string[], from: string, to: string) {
  if (!from || from === to) {
    return null;
  }
  const next = order.filter((id) => id !== from);
  const index = next.indexOf(to);
  if (index < 0) {
    return null;
  }
  next.splice(index, 0, from);
  return next;
}

/**
 * Moves `from` to `dropIndex` among `visible` icons.
 * `dropIndex` counts gaps in that list, including the gap after the last icon.
 * Icons that are not in `visible` keep their places in `order`.
 */
export function reorderToolbarAt(order: string[], visible: string[], from: string, dropIndex: number) {
  const fromIndex = visible.indexOf(from);
  if (fromIndex < 0 || dropIndex < 0 || dropIndex > visible.length) {
    return null;
  }
  const target = fromIndex < dropIndex ? dropIndex - 1 : dropIndex;
  if (target === fromIndex) {
    return null;
  }
  const nextVisible = visible.filter((id) => id !== from);
  nextVisible.splice(target, 0, from);
  const visibleSet = new Set(visible);
  let cursor = 0;
  const next = order.map((id) => {
    if (!visibleSet.has(id)) {
      return id;
    }
    return nextVisible[cursor++] ?? id;
  });
  if (cursor !== nextVisible.length) {
    return null;
  }
  return next;
}

export function toolbarDragHandlers(id: string, onMove: (from: string, to: string) => void) {
  return {
    draggable: true,
    onDragStart: (event: DragEvent) => {
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData("text/plain", id);
    },
    onDragOver: (event: DragEvent) => {
      event.preventDefault();
      event.dataTransfer.dropEffect = "move";
    },
    onDrop: (event: DragEvent) => {
      event.preventDefault();
      onMove(event.dataTransfer.getData("text/plain"), id);
    },
  };
}

export function saveToolbarPin(settings: BrowserSettings, id: string, pinned: boolean) {
  const hidden = (settings.hiddenToolbarExtensions ?? []).filter((item) => item !== id);
  if (!pinned) {
    hidden.push(id);
  }
  return saveSettings({ ...settings, hiddenToolbarExtensions: hidden });
}

/** Right-click menu for an icon that is currently on a toolbar. */
export function openToolbarPinMenu(event: MouseEvent, id: string) {
  event.preventDefault();
  void openPopup("extension-pin", { x: event.clientX, y: event.clientY }, { params: `id=${encodeURIComponent(id)}` });
}
