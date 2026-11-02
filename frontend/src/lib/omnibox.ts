import Fuse from "fuse.js";
import type { HistoryItem, OmniboxSuggestion, TabInfo, Workspace } from "../../../types/browser";

const MAX_SUGGESTIONS = 8;
export const OMNIBOX_ROW_HEIGHT = 36;
const DAY_MS = 24 * 60 * 60 * 1000;

type Candidate = {
  title: string;
  url: string;
  kind: "bookmark" | "tab" | "history";
  tabId?: number;
  workspace?: string;
  historyId?: number;
};

/** Matches the Rust `resolve_input` host detection closely enough to label the first row. */
export function looksLikeUrl(text: string) {
  const value = text.trim();
  if (!value || /\s/.test(value) || value.startsWith("?")) {
    return false;
  }
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(value) || /^(about|data|file|ce|chrome-extension|mailto):/i.test(value)) {
    return true;
  }
  const host = value.split(/[/?#]/)[0];
  return host === "localhost" || /^localhost:\d+$/.test(host) || /^[^.]+(\.[^.]+)+(:\d+)?$/.test(host);
}

/** An address as people type it: no scheme and no leading `www.`. */
export function bareAddress(url: string) {
  return url.replace(/^[a-z][a-z0-9+.-]*:\/\//i, "").replace(/^www\./i, "");
}

/** How often and how recently a page was visited, from 0 to 1. Typed visits count double. */
export function frecency(item: Pick<HistoryItem, "visitCount" | "typedCount" | "lastVisited"> | undefined, now = Date.now()) {
  if (!item) {
    return 0;
  }
  const age = now - item.lastVisited;
  const recency = age < 4 * DAY_MS ? 1 : age < 14 * DAY_MS ? 0.7 : age < 31 * DAY_MS ? 0.5 : age < 90 * DAY_MS ? 0.3 : 0.1;
  const visits = item.visitCount + 2 * (item.typedCount ?? 0);
  return Math.min(1, (Math.log2(1 + visits) * recency) / 6);
}

function historyByUrl(history: HistoryItem[]) {
  const map = new Map<string, HistoryItem>();
  for (const item of history) {
    const key = bareAddress(item.url).replace(/\/$/, "");
    if (!map.has(key)) {
      map.set(key, item);
    }
  }
  return map;
}

export function buildSuggestions(
  text: string,
  bookmarks: { title: string; url: string }[],
  history: HistoryItem[],
  tabs: TabInfo[],
  activeId: number | null,
  workspaces: Workspace[] = [],
  currentWorkspace = "default",
  completion: string | null = null,
): OmniboxSuggestion[] {
  const query = text.trim();
  if (!query) {
    return [];
  }
  const target = completion ?? query;
  const first: OmniboxSuggestion = looksLikeUrl(target)
    ? { kind: "url", title: target, url: target }
    : { kind: "search", title: query, url: query };

  const workspaceName = (id: string) => workspaces.find((item) => item.id === id)?.name;
  const candidates: Candidate[] = [
    ...tabs
      .filter((tab) => tab.id !== activeId)
      .map((tab) => ({
        kind: "tab" as const,
        title: tab.title,
        url: tab.url,
        tabId: tab.id,
        workspace: tab.workspace !== currentWorkspace ? workspaceName(tab.workspace) : undefined,
      })),
    ...bookmarks.map((bookmark) => ({ kind: "bookmark" as const, title: bookmark.title, url: bookmark.url })),
    ...history.slice(0, 600).map((item) => ({ kind: "history" as const, title: item.title, url: item.url, historyId: item.id })),
  ];
  const fuse = new Fuse(candidates, {
    keys: [
      { name: "title", weight: 0.6 },
      { name: "url", weight: 0.4 },
    ],
    threshold: 0.35,
    ignoreLocation: true,
    includeScore: true,
  });

  const visits = historyByUrl(history);
  const lower = query.toLowerCase();
  const now = Date.now();
  const ranked = fuse.search(query, { limit: 80 }).map((result) => {
    const item = result.item;
    const bare = bareAddress(item.url).toLowerCase();
    const relevance = 1 - (result.score ?? 1);
    const visited = frecency(visits.get(bare.replace(/\/$/, "")), now);
    const prefix = bare.startsWith(lower) ? 0.35 : bare.includes(`.${lower}`) || bare.includes(`/${lower}`) ? 0.1 : 0;
    const kindBoost = item.kind === "tab" ? 0.15 : item.kind === "bookmark" ? 0.1 : 0;
    return { item, score: relevance * 0.55 + visited * 0.45 + prefix + kindBoost };
  });
  ranked.sort((left, right) => right.score - left.score);

  const firstKey = bareAddress(first.url).replace(/\/$/, "").toLowerCase();
  const seen = new Set<string>([firstKey]);
  const matches: OmniboxSuggestion[] = [];
  for (const { item } of ranked) {
    const key = bareAddress(item.url).replace(/\/$/, "").toLowerCase();
    const tabKey = item.kind === "tab" ? `tab:${item.tabId}` : key;
    if (seen.has(tabKey) || (item.kind !== "tab" && seen.has(key))) {
      continue;
    }
    seen.add(tabKey);
    seen.add(key);
    matches.push({
      kind: item.kind,
      title: item.title || item.url,
      url: item.url,
      tabId: item.tabId,
      workspace: item.workspace,
      historyId: item.historyId,
    });
    if (matches.length >= MAX_SUGGESTIONS - 1) {
      break;
    }
  }
  return [first, ...matches];
}

/**
 * What the address bar fills in after `typed`: the best visited or bookmarked address that starts
 * with it. Without a `/` in the text only the host is completed. Null when nothing fits.
 */
export function inlineCompletion(typed: string, history: HistoryItem[], bookmarks: { url: string }[]) {
  const text = typed.trimStart();
  if (!text || /\s/.test(text) || text.startsWith("?")) {
    return null;
  }
  const lower = text.toLowerCase();
  const typedScheme = /^[a-z][a-z0-9+.-]*:\/\//i.test(text);
  const now = Date.now();
  let best: { completion: string; score: number } | null = null;
  const consider = (url: string, score: number) => {
    if (!/^https?:\/\//i.test(url)) {
      return;
    }
    const shown = typedScheme ? url : bareAddress(url);
    if (!shown.toLowerCase().startsWith(lower) || shown.length <= text.length) {
      return;
    }
    let completion = shown;
    if (!text.includes("/")) {
      const hostEnd = shown.indexOf("/", typedScheme ? shown.indexOf("//") + 2 : 0);
      completion = hostEnd >= 0 ? shown.slice(0, hostEnd) : shown;
    }
    completion = completion.replace(/\/$/, "");
    if (completion.length <= text.length) {
      return;
    }
    if (!best || score > best.score) {
      best = { completion, score };
    }
  };
  for (const item of history.slice(0, 600)) {
    consider(item.url, frecency(item, now) + (item.typedCount > 0 ? 0.2 : 0));
  }
  for (const bookmark of bookmarks) {
    consider(bookmark.url, 0.25);
  }
  const found = best as { completion: string; score: number } | null;
  return found ? text + found.completion.slice(text.length) : null;
}
