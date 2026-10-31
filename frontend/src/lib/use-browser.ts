import { useEffect, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { BookmarkNode, BookmarkTree, BrowserSettings, TabsSnapshot } from "../../../types/browser";
import { publishAppearance } from "./appearance";
import {
  getSettings,
  listBookmarks,
  listDownloads,
  listExtensions,
  listHistory,
  listTabs,
  onBookmarksChanged,
  onDownloadsChanged,
  onExtensionsChanged,
  onHistoryChanged,
  onSettingsChanged,
  onTabsChanged,
} from "./browser-client";
import { publishZoom } from "./zoom";

/** Loads a value once and keeps it current from a change event. */
function useLiveValue<T>(load: () => Promise<T>, subscribe: (handler: (value: T) => void) => Promise<UnlistenFn>) {
  const [value, setValue] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    const unlisten = subscribe((next) => {
      if (!disposed) {
        setValue(next);
      }
    });
    load()
      .then((next) => {
        if (!disposed) {
          setValue(next);
        }
      })
      .catch((reason: unknown) => {
        if (!disposed) {
          setError(String(reason));
        }
      });
    return () => {
      disposed = true;
      void unlisten.then((stop) => stop());
    };
  }, [load, subscribe]);

  return { value, error };
}

const EMPTY_TABS: TabsSnapshot = {
  tabs: [],
  activeId: null,
  selectedIds: [],
  canReopen: false,
  fullscreen: false,
  windows: [],
};

function currentWindowLabel() {
  try {
    return getCurrentWindow().label;
  } catch {
    return "main";
  }
}

export function useTabs() {
  const snapshot = useLiveValue(listTabs, onTabsChanged).value ?? EMPTY_TABS;
  const label = currentWindowLabel();
  const tabs = snapshot.tabs.filter((tab) => (tab.window || "main") === label);
  const mine = snapshot.windows?.find((item) => item.label === label);
  const activeId = mine ? mine.activeId : label === "main" ? snapshot.activeId : null;
  const fullscreen = mine ? mine.fullscreen : label === "main" && snapshot.fullscreen;
  const selectedIds = (snapshot.selectedIds ?? []).filter((id) => tabs.some((tab) => tab.id === id));
  return {
    ...snapshot,
    tabs,
    activeId,
    fullscreen,
    selectedIds: selectedIds.length > 0 ? selectedIds : activeId === null ? [] : [activeId],
  };
}

export function useBookmarks() {
  return useLiveValue(listBookmarks, onBookmarksChanged).value;
}

export function useDownloads() {
  return useLiveValue(listDownloads, onDownloadsChanged).value ?? [];
}

export function useHistory() {
  return useLiveValue(listHistory, onHistoryChanged).value ?? [];
}

export function useExtensions() {
  const { value, error } = useLiveValue(listExtensions, onExtensionsChanged);
  return { extensions: value ?? [], error };
}

export function useSettings(): BrowserSettings | null {
  return useLiveValue(getSettings, onSettingsChanged).value;
}

/** Applies theme, color mode and UI zoom from settings to this webview. */
export function useAppearanceFromSettings() {
  const settings = useSettings();
  useEffect(() => {
    if (!settings) {
      return;
    }
    publishAppearance(settings.theme, settings.colorMode);
    publishZoom(settings.uiZoom);
  }, [settings]);
  return settings;
}

export function flattenBookmarks(tree: BookmarkTree | null) {
  const links: { id: string; title: string; url: string }[] = [];
  const walk = (nodes: BookmarkTree["bar"][] | null) => {
    for (const node of nodes ?? []) {
      if (node.url) {
        links.push({ id: node.id, title: node.title, url: node.url });
      } else {
        walk(node.children);
      }
    }
  };
  if (tree) {
    walk([tree.bar, tree.other]);
  }
  return links;
}

export const BOOKMARK_DRAG_TYPE = "application/x-ce-bookmark";

export type BookmarkDropPlace = "before" | "into" | "after";

const ROOT_FOLDERS = new Set(["bar", "other"]);

/** Where a dragged bookmark lands on `node`. Roots only accept a drop inside. */
export function bookmarkDropPlace(
  event: { currentTarget: Element; clientX: number; clientY: number },
  node: BookmarkNode,
  axis: "x" | "y",
): BookmarkDropPlace {
  const rect = event.currentTarget.getBoundingClientRect();
  const size = axis === "x" ? rect.width : rect.height;
  const offset = axis === "x" ? event.clientX - rect.left : event.clientY - rect.top;
  const ratio = size <= 0 ? 0.5 : offset / size;
  const folder = !node.url && !node.separator;
  if (ROOT_FOLDERS.has(node.id) || !folder) {
    if (ROOT_FOLDERS.has(node.id)) {
      return "into";
    }
    return ratio > 0.5 ? "after" : "before";
  }
  if (ratio < 0.25) {
    return "before";
  }
  if (ratio > 0.75) {
    return "after";
  }
  return "into";
}

export function findBookmarkParent(tree: BookmarkTree | null, id: string): { parentId: string; index: number } | null {
  if (!tree) {
    return null;
  }
  const search = (node: BookmarkNode): { parentId: string; index: number } | null => {
    const children = node.children ?? [];
    const index = children.findIndex((child) => child.id === id);
    if (index >= 0) {
      return { parentId: node.id, index };
    }
    for (const child of children) {
      const found = search(child);
      if (found) {
        return found;
      }
    }
    return null;
  };
  return search(tree.bar) ?? search(tree.other);
}

/** True when `id` is `ancestorId` or sits inside that folder. */
export function bookmarkContains(tree: BookmarkTree | null, ancestorId: string, id: string): boolean {
  const ancestor = findFolder(tree, ancestorId);
  if (!ancestor) {
    return false;
  }
  if (ancestor.id === id) {
    return true;
  }
  const walk = (node: BookmarkNode): boolean => (node.children ?? []).some((child) => child.id === id || walk(child));
  return walk(ancestor);
}

/** Drops selected descendants so deleting a folder does not also delete its children one by one. */
export function bookmarkRootsOf(tree: BookmarkTree | null, ids: string[]) {
  const selected = new Set(ids);
  return ids.filter((id) => {
    let parent = findBookmarkParent(tree, id);
    while (parent) {
      if (selected.has(parent.parentId)) {
        return false;
      }
      parent = findBookmarkParent(tree, parent.parentId);
    }
    return true;
  });
}

export function findFolder(tree: BookmarkTree | null, id: string): BookmarkTree["bar"] | null {
  if (!tree) {
    return null;
  }
  const search = (node: BookmarkTree["bar"]): BookmarkTree["bar"] | null => {
    if (node.id === id) {
      return node;
    }
    for (const child of node.children ?? []) {
      const found = search(child);
      if (found) {
        return found;
      }
    }
    return null;
  };
  return search(tree.bar) ?? search(tree.other);
}

export function listFolders(tree: BookmarkTree | null) {
  const folders: { id: string; title: string; depth: number }[] = [];
  const walk = (node: BookmarkTree["bar"], depth: number) => {
    if (node.separator || node.url) {
      return;
    }
    folders.push({ id: node.id, title: node.title, depth });
    for (const child of node.children ?? []) {
      walk(child, depth + 1);
    }
  };
  if (tree) {
    walk(tree.bar, 0);
    walk(tree.other, 0);
  }
  return folders;
}
