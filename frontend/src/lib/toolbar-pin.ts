import type { DragEvent, MouseEvent } from "react";
import type { BrowserSettings, HostExtensionInfo, ToolbarPlacement } from "../../../types/browser";
import { BUILTIN_TOOLS, builtinTool } from "../features/BuiltInExtensions/builtin";
import { openPopup, saveSettings } from "./browser-client";

/** Toolbar ids for enabled host extensions that have a button. */
export function hostToolbarIds(extensions: HostExtensionInfo[]) {
  return extensions
    .filter((extension) => extension.enabled && (extension.surfaces.popup || extension.surfaces.panel))
    .map((extension) => `host:${extension.id}`);
}

/** Built-in tools, in the order they had before a custom order was saved. */
export const DEFAULT_TOOLBAR = BUILTIN_TOOLS.map((tool) => tool.id);

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

/**
 * Primary shows next to the address bar. Secondary shows on the tools toolbar.
 * An id with no saved choice keeps the previous default: built-in tools on the tools toolbar,
 * and extensions plus address-bar tools on the address bar. An id in the hidden list stays unpinned.
 */
export function toolbarPlacement(settings: BrowserSettings | null, id: string): ToolbarPlacement {
  const saved = settings?.toolbarPlacements?.[id];
  if (saved) {
    return { primary: !!saved.primary, secondary: !!saved.secondary };
  }
  if (settings?.hiddenToolbarExtensions?.includes(id)) {
    return { primary: false, secondary: false };
  }
  const tool = builtinTool(id);
  if (tool && !tool.address) {
    return { primary: false, secondary: true };
  }
  if (id.startsWith("host:")) {
    return { primary: false, secondary: true };
  }
  return { primary: true, secondary: false };
}

export type ToolbarSplit = {
  /** Ids pinned to the tools toolbar, in saved order. */
  tools: string[];
  /** Ids pinned next to the address bar, in saved order. */
  address: string[];
  /** Every id pinned to either bar, in saved order. Dragging uses this list. */
  all: string[];
};

/** A built-in tool stays on until the user turns it off. A missing list means every tool is on. */
export function builtinToolEnabled(settings: BrowserSettings | null, id: string) {
  return !(settings?.disabledBuiltinTools ?? []).includes(id);
}

export function saveBuiltinEnabled(settings: BrowserSettings, id: string, enabled: boolean) {
  const disabledBuiltinTools = (settings.disabledBuiltinTools ?? []).filter((item) => item !== id);
  if (!enabled) {
    disabledBuiltinTools.push(id);
  }
  return saveSettings({ ...settings, disabledBuiltinTools });
}

/** Splits pinned built-in tools and enabled extensions across the two bars. An id can be on both. */
export function splitToolbar(settings: BrowserSettings | null, extensionIds: string[]): ToolbarSplit {
  const builtinIds = BUILTIN_TOOLS.map((tool) => tool.id).filter((id) => builtinToolEnabled(settings, id));
  const extras = extensionIds.filter((id) => !builtinIds.includes(id));
  const available = [...builtinIds, ...extras].filter((id) => {
    const place = toolbarPlacement(settings, id);
    return place.primary || place.secondary;
  });
  const all = toolbarOrder(settings?.toolbarOrder, available);
  return {
    tools: all.filter((id) => toolbarPlacement(settings, id).secondary),
    address: all.filter((id) => toolbarPlacement(settings, id).primary),
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
export type ToolbarDropEdge = "before" | "after";

/** Gap under the pointer. Counts the slots between icons, including the slot after the last one. */
export function toolbarIndexAt(container: HTMLElement, client: number, axis: "x" | "y") {
  const items = container.querySelectorAll<HTMLElement>(":scope > [data-toolbar-index]");
  for (const item of items) {
    const rect = item.getBoundingClientRect();
    const start = axis === "x" ? rect.left : rect.top;
    const size = axis === "x" ? rect.width : rect.height;
    if (client < start + size / 2) {
      return Number(item.dataset.toolbarIndex);
    }
  }
  return items.length;
}

/** Hides the line when the icon would land in the gap it already occupies. */
export function toolbarDropEdge(
  index: number,
  count: number,
  fromIndex: number,
  dropIndex: number | null,
): ToolbarDropEdge | null {
  if (dropIndex === null || fromIndex < 0 || dropIndex === fromIndex || dropIndex === fromIndex + 1) {
    return null;
  }
  if (dropIndex === index) {
    return "before";
  }
  if (index === count - 1 && dropIndex === count) {
    return "after";
  }
  return null;
}

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

export function saveToolbarPlacement(settings: BrowserSettings, id: string, placement: ToolbarPlacement) {
  const toolbarPlacements = { ...(settings.toolbarPlacements ?? {}), [id]: placement };
  const hidden = (settings.hiddenToolbarExtensions ?? []).filter((item) => item !== id);
  if (!placement.primary && !placement.secondary) {
    hidden.push(id);
  }
  return saveSettings({ ...settings, toolbarPlacements, hiddenToolbarExtensions: hidden });
}

/** Right-click menu for an icon that is currently on a toolbar. */
export function openToolbarPinMenu(event: MouseEvent, id: string) {
  event.preventDefault();
  void openPopup("extension-pin", { x: event.clientX, y: event.clientY }, { params: `id=${encodeURIComponent(id)}` });
}
