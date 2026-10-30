import Fuse from "fuse.js";
import type { OmniboxSuggestion, TabInfo } from "../../../types/browser";

const MAX_SUGGESTIONS = 8;
export const OMNIBOX_ROW_HEIGHT = 36;
export const OMNIBOX_PADDING = 8;

type Candidate = { title: string; url: string; kind: "bookmark" | "tab" | "history"; tabId?: number };

/** Matches the Rust `resolve_input` host detection closely enough to label the first row. */
export function looksLikeUrl(text: string) {
  const value = text.trim();
  if (!value || /\s/.test(value)) {
    return false;
  }
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(value) || /^(about|data|file|ce|chrome-extension|mailto):/i.test(value)) {
    return true;
  }
  const host = value.split(/[/?#]/)[0];
  return host === "localhost" || /^localhost:\d+$/.test(host) || /^[^.]+(\.[^.]+)+(:\d+)?$/.test(host);
}

export function buildSuggestions(
  text: string,
  bookmarks: { title: string; url: string }[],
  history: { title: string; url: string }[],
  tabs: TabInfo[],
  activeId: number | null,
): OmniboxSuggestion[] {
  const query = text.trim();
  if (!query) {
    return [];
  }
  const first: OmniboxSuggestion = looksLikeUrl(query)
    ? { kind: "url", title: query, url: query }
    : { kind: "search", title: query, url: query };

  const candidates: Candidate[] = [
    ...tabs
      .filter((tab) => tab.id !== activeId)
      .map((tab) => ({ kind: "tab" as const, title: tab.title, url: tab.url, tabId: tab.id })),
    ...bookmarks.map((bookmark) => ({ kind: "bookmark" as const, title: bookmark.title, url: bookmark.url })),
    ...history.slice(0, 400).map((item) => ({ kind: "history" as const, title: item.title, url: item.url })),
  ];
  const fuse = new Fuse(candidates, {
    keys: [
      { name: "title", weight: 0.6 },
      { name: "url", weight: 0.4 },
    ],
    threshold: 0.35,
    ignoreLocation: true,
  });

  const seen = new Set<string>([first.url]);
  const matches: OmniboxSuggestion[] = [];
  for (const result of fuse.search(query)) {
    const item = result.item;
    const key = `${item.kind}:${item.url}`;
    if (seen.has(key) || seen.has(item.url)) {
      continue;
    }
    seen.add(key);
    matches.push({ kind: item.kind, title: item.title || item.url, url: item.url, tabId: item.tabId });
    if (matches.length >= MAX_SUGGESTIONS - 1) {
      break;
    }
  }
  return [first, ...matches];
}

export function omniboxHeight(count: number) {
  return count * OMNIBOX_ROW_HEIGHT + OMNIBOX_PADDING;
}
