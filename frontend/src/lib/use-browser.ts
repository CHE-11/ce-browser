import { useEffect, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { BookmarkTree, BrowserSettings, TabsSnapshot } from "../../../types/browser";
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

const EMPTY_TABS: TabsSnapshot = { tabs: [], activeId: null, selectedIds: [], canReopen: false, fullscreen: false };

export function useTabs() {
  const snapshot = useLiveValue(listTabs, onTabsChanged).value ?? EMPTY_TABS;
  if (snapshot.selectedIds) {
    return snapshot;
  }
  return { ...snapshot, selectedIds: snapshot.activeId === null ? [] : [snapshot.activeId] };
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
