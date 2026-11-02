import { useEffect, useLayoutEffect, useMemo, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { BookmarkNode, BookmarkTree, BrowserSettings, ClosedEntry, PageRules, TabsSnapshot } from "../../../types/browser";
import { publishAppearance } from "./appearance";
import {
  currentWindowLabel as browserWindowLabel,
  getPageRules,
  getSettings,
  listBookmarks,
  listDownloads,
  listExtensions,
  listHistory,
  listRecentlyClosed,
  listTabs,
  onBookmarksChanged,
  onDownloadsChanged,
  onFaviconsChanged,
  onExtensionsChanged,
  onHistoryChanged,
  onPageRulesChanged,
  onSettingsChanged,
  onTabsChanged,
  onUpdateStatus,
  siteFavicons,
  updateStatus,
} from "./browser-client";
import { faviconKey } from "./format";
import { publishZoom } from "./zoom";

type LiveSlot<T> = {
  value: T | null;
  error: string | null;
  listeners: Set<(value: T | null, error: string | null) => void>;
  started: boolean;
};

const liveSlots = new Map<string, LiveSlot<unknown>>();

function liveSlot<T>(key: string): LiveSlot<T> {
  let slot = liveSlots.get(key) as LiveSlot<T> | undefined;
  if (!slot) {
    slot = { value: null, error: null, listeners: new Set(), started: false };
    liveSlots.set(key, slot as LiveSlot<unknown>);
  }
  return slot;
}

function publishLive<T>(slot: LiveSlot<T>, value: T | null, error: string | null) {
  slot.value = value;
  slot.error = error;
  for (const listener of slot.listeners) {
    listener(value, error);
  }
}

/**
 * Loads a value once per webview and keeps it current from a change event.
 * A later subscriber sees the value on its first render.
 */
function useLiveValue<T>(key: string, load: () => Promise<T>, subscribe: (handler: (value: T) => void) => Promise<UnlistenFn>) {
  const slot = liveSlot<T>(key);
  const [value, setValue] = useState<T | null>(slot.value);
  const [error, setError] = useState<string | null>(slot.error);

  useEffect(() => {
    const listener = (next: T | null, nextError: string | null) => {
      setValue(next);
      setError(nextError);
    };
    slot.listeners.add(listener);
    listener(slot.value, slot.error);
    if (!slot.started) {
      slot.started = true;
      let sawEvent = false;
      void subscribe((next) => {
        sawEvent = true;
        publishLive(slot, next, null);
      });
      load()
        .then((next) => {
          if (!sawEvent) {
            publishLive(slot, next, null);
          }
        })
        .catch((reason: unknown) => {
          if (!sawEvent && slot.value === null) {
            publishLive(slot, null, String(reason));
          }
        });
    }
    return () => {
      slot.listeners.delete(listener);
    };
  }, [key, load, subscribe, slot]);

  /** The first load finished, with a value or an error. */
  const loaded = value !== null || error !== null;
  return { value, error, loaded };
}

const EMPTY_TABS: TabsSnapshot = {
  tabs: [],
  activeId: null,
  selectedIds: [],
  canReopen: false,
  fullscreen: false,
  windows: [],
  groups: [],
  workspaces: [],
};

function currentWindowLabel() {
  try {
    return browserWindowLabel();
  } catch {
    return "main";
  }
}

export function useTabs() {
  const live = useLiveValue("tabs", listTabs, onTabsChanged);
  const snapshot = live.value ?? EMPTY_TABS;
  const label = currentWindowLabel();
  const mine = snapshot.windows?.find((item) => item.label === label);
  const workspace = mine?.workspace ?? "default";
  /** Every tab in this window, across workspaces. */
  const windowTabs = snapshot.tabs.filter((tab) => (tab.window || "main") === label);
  const tabs = windowTabs.filter((tab) => (tab.workspace || "default") === workspace);
  const activeId = mine ? mine.activeId : label === "main" ? snapshot.activeId : null;
  const fullscreen = mine ? mine.fullscreen : label === "main" && snapshot.fullscreen;
  const selectedIds = (snapshot.selectedIds ?? []).filter((id) => tabs.some((tab) => tab.id === id));
  const groups = (snapshot.groups ?? []).filter((group) => group.window === label);
  return {
    ...snapshot,
    tabs,
    windowTabs,
    activeId,
    fullscreen,
    workspace,
    workspaces: snapshot.workspaces ?? [],
    split: mine?.split ?? null,
    peekUrl: mine?.peekUrl ?? null,
    groups,
    selectedIds: selectedIds.length > 0 ? selectedIds : activeId === null ? [] : [activeId],
    loaded: live.loaded,
  };
}

export type WindowTabsState = ReturnType<typeof useTabs>;

/** Recently closed tabs and groups, newest first. Refreshes whenever the tabs change. */
export function useRecentlyClosed() {
  const [entries, setEntries] = useState<ClosedEntry[] | null>(null);
  useEffect(() => {
    let disposed = false;
    const load = () =>
      void listRecentlyClosed()
        .then((list) => {
          if (!disposed) {
            setEntries(list);
          }
        })
        .catch(() => {
          if (!disposed) {
            setEntries([]);
          }
        });
    load();
    const unlisten = onTabsChanged(load);
    return () => {
      disposed = true;
      void unlisten.then((stop) => stop());
    };
  }, []);
  return entries;
}

export function useBookmarksState() {
  const { value, loaded } = useLiveValue("bookmarks", listBookmarks, onBookmarksChanged);
  return { tree: value, loaded };
}

export function useBookmarks() {
  return useBookmarksState().tree;
}

/**
 * Cached site icons for `urls`, keyed by `faviconKey`. Hosts the backend has not seen yet are
 * fetched in the background and fill in as they arrive.
 */
export function useSiteFavicons(urls: (string | null | undefined)[]) {
  const [icons, setIcons] = useState<Record<string, string>>({});
  const byHost = new Map<string, string>();
  for (const url of urls) {
    const host = faviconKey(url);
    if (host && url && !byHost.has(host)) {
      byHost.set(host, url);
    }
  }
  const requestKey = [...byHost.values()].sort().join("\n");
  const requested = useMemo(() => (requestKey ? requestKey.split("\n") : []), [requestKey]);

  useEffect(() => {
    let disposed = false;
    const unlisten = onFaviconsChanged((icon) => {
      if (!disposed) {
        setIcons((current) => (current[icon.host] === icon.dataUrl ? current : { ...current, [icon.host]: icon.dataUrl }));
      }
    });
    return () => {
      disposed = true;
      void unlisten.then((stop) => stop());
    };
  }, []);

  useEffect(() => {
    if (requested.length === 0) {
      return;
    }
    let disposed = false;
    siteFavicons(requested)
      .then((found) => {
        if (!disposed) {
          setIcons((current) => ({ ...current, ...found }));
        }
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
    };
  }, [requested]);

  return icons;
}

export function useDownloadsState() {
  const { value, loaded } = useLiveValue("downloads", listDownloads, onDownloadsChanged);
  return { downloads: value ?? [], loaded };
}

export function useDownloads() {
  return useDownloadsState().downloads;
}

/** Version, waiting update and check state. Refreshes while an update check runs. */
export function useUpdateStatus() {
  return useLiveValue("update-status", updateStatus, onUpdateStatus).value;
}

export function useHistoryState() {
  const { value, loaded } = useLiveValue("history", listHistory, onHistoryChanged);
  return { history: value ?? [], loaded };
}

export function useHistory() {
  return useHistoryState().history;
}

export function useExtensions() {
  const { value, error, loaded } = useLiveValue("extensions", listExtensions, onExtensionsChanged);
  return { extensions: value ?? [], error, loaded };
}

export function useSettingsState() {
  const { value, loaded } = useLiveValue("settings", getSettings, onSettingsChanged);
  return { settings: value, loaded };
}

export function usePageRules(): PageRules | null {
  return useLiveValue("page-rules", getPageRules, onPageRulesChanged).value;
}

export function useSettings(): BrowserSettings | null {
  return useSettingsState().settings;
}

/**
 * Applies theme, color mode and UI zoom from settings to this webview. `loaded` turns true in
 * the same commit that publishes the saved appearance, or when settings fail to load.
 */
export function useAppearanceState() {
  const { settings, loaded } = useSettingsState();
  useLayoutEffect(() => {
    if (!settings) {
      return;
    }
    publishAppearance(settings.theme, settings.colorMode);
    publishZoom(settings.uiZoom);
  }, [settings]);
  return { settings, loaded };
}

export function useAppearanceFromSettings() {
  return useAppearanceState().settings;
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
