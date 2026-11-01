import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import Fuse, { type FuseResultMatch } from "fuse.js";
import {
  activateTab,
  clearSiteCache,
  closePopup,
  createEphemeralTab,
  createTab,
  groupBySite,
  navigate,
  openDevtools,
  openPopup,
  openReader,
  restoreClosed,
  saveSettings,
  searchHistory,
  sendUiCommand,
  sleepOtherTabs,
  switchWorkspace,
  windowPoint,
  zoomTab,
} from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { flattenBookmarks, useBookmarks, useExtensions, useHistory, useRecentlyClosed, useSettings, useTabs } from "@/lib/use-browser";
import { BUILTIN_TOOLS } from "@features/BuiltInExtensions/builtin";
import { siteKey } from "@features/BuiltInExtensions/page-controls/site";
import { tabParams } from "@features/BuiltInExtensions/shared";
import { SETTINGS_INDEX } from "@features/Settings/settings-index";
import { SETTINGS_SECTIONS } from "@features/Settings/settings-sections";
import type { ClosedEntry, HistoryHit, TabInfo } from "@type/browser";

type Hit = {
  id: string;
  title: string;
  hint: string;
  /** Extra words that match but are not shown. */
  keywords?: string;
  group: "Action" | "Tab" | "Recently closed" | "Workspace" | "Setting" | "Tool" | "Extension" | "Bookmark" | "History";
  run: () => Promise<unknown> | void;
};

type Ranked = { hit: Hit; matches: readonly FuseResultMatch[] };

const MAX_HITS = 14;
const GROUP_BOOST: Partial<Record<Hit["group"], number>> = { Action: 0.08, Tab: 0.06, Setting: 0.04, Tool: 0.04 };

function closedLabel(entry: ClosedEntry) {
  if (entry.kind === "group") {
    return `Reopen group ${entry.group?.name.trim() || "without a name"} (${entry.tabs.length} tabs)`;
  }
  if (entry.kind === "batch") {
    return `Reopen ${entry.tabs.length} tabs`;
  }
  return `Reopen ${entry.tabs[0]?.title || entry.tabs[0]?.url || "tab"}`;
}

/** Matched letters in bold. */
function Highlighted({ id, text, matches }: { id: string; text: string; matches: readonly FuseResultMatch[] }) {
  const ranges = matches.find((match) => match.key === "title")?.indices ?? [];
  if (ranges.length === 0) {
    return <span id={id}>{text}</span>;
  }
  const parts: ReactNode[] = [];
  let at = 0;
  ranges.forEach(([start, end], index) => {
    if (end - start < 1 && ranges.length > 1) {
      return;
    }
    if (start > at) {
      parts.push(text.slice(at, start));
    }
    parts.push(
      <mark id={`${id}-match-${index}`} key={index} className="bg-transparent font-semibold text-sky-300">
        {text.slice(start, end + 1)}
      </mark>,
    );
    at = end + 1;
  });
  parts.push(text.slice(at));
  return <span id={id}>{parts}</span>;
}

/** Opens a ce://settings row, reusing an open settings tab. */
async function openSetting(tabs: TabInfo[], id: string) {
  const url = `ce://settings?focus=${encodeURIComponent(id)}`;
  const open = tabs.find((tab) => tab.url.startsWith("ce://settings"));
  if (open) {
    await activateTab(open.id);
    return navigate(open.id, url);
  }
  return createTab(url);
}

export function CommandPalette() {
  const snapshot = useTabs();
  const bookmarks = useBookmarks();
  const history = useHistory();
  const settings = useSettings();
  const closed = useRecentlyClosed();
  const { extensions } = useExtensions();
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const [textHits, setTextHits] = useState<HistoryHit[]>([]);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const active = snapshot.tabs.find((tab) => tab.id === snapshot.activeId);

  /** Opens another popup where this one is. Opening it closes the palette. */
  const openHere = (kind: Parameters<typeof openPopup>[0]["kind"], params: string, width = 360, height = 480) =>
    windowPoint(0, 0).then((point) => openPopup({ kind, params, x: point.x, y: point.y, width, height }));

  const all = useMemo<Hit[]>(() => {
    const actions: Hit[] = [
      { id: "new-tab", title: "New tab", hint: "Ctrl+T", group: "Action", run: () => createTab() },
      { id: "ephemeral", title: "New ephemeral tab", hint: "InPrivate", group: "Action", run: () => createEphemeralTab() },
      { id: "tab-search", title: "Search tabs", hint: "Ctrl+Shift+A", group: "Action", run: () => sendUiCommand("tab-search") },
      { id: "shortcut-sheet", title: "Keyboard shortcut cheat sheet", hint: "Ctrl+/", group: "Action", run: () => sendUiCommand("shortcut-sheet") },
      { id: "group-by-site", title: "Group tabs by site", hint: "Tab groups", group: "Action", run: () => groupBySite() },
      { id: "bookmarks", title: "Bookmarks", hint: "ce://bookmarks", group: "Action", run: () => createTab("ce://bookmarks") },
      { id: "downloads", title: "Downloads", hint: "ce://downloads", group: "Action", run: () => createTab("ce://downloads") },
      { id: "files", title: "Files", hint: "ce://files", group: "Action", run: () => createTab("ce://files") },
      { id: "history", title: "History", hint: "ce://history", group: "Action", run: () => createTab("ce://history") },
      { id: "settings", title: "Settings", hint: "ce://settings", group: "Action", run: () => openSetting(snapshot.windowTabs, "settings-profiles") },
      { id: "extensions", title: "Extensions", hint: "ce://extensions", group: "Action", run: () => createTab("ce://extensions") },
      { id: "scripts", title: "Site scripts", hint: "ce://scripts", group: "Action", run: () => createTab("ce://scripts") },
      { id: "permissions", title: "Permissions", hint: "ce://permissions", group: "Action", run: () => createTab("ce://permissions") },
      { id: "shortcuts", title: "Keyboard shortcuts", hint: "ce://shortcuts", group: "Action", run: () => createTab("ce://shortcuts") },
      { id: "rewrites", title: "Request rules", hint: "ce://rewrites", group: "Action", run: () => createTab("ce://rewrites") },
      { id: "storage", title: "Site storage", hint: "ce://storage", group: "Action", run: () => createTab("ce://storage") },
      { id: "find", title: "Find in page", hint: "Ctrl+F", group: "Action", run: () => sendUiCommand("find") },
      {
        id: "vertical",
        title: settings?.tabLayout === "vertical" ? "Horizontal tabs" : "Vertical tabs",
        hint: "Tab layout",
        group: "Action",
        run: async () => {
          if (settings) {
            await saveSettings({ ...settings, tabLayout: settings.tabLayout === "vertical" ? "horizontal" : "vertical" });
          }
        },
      },
    ];
    if (active) {
      actions.push(
        { id: "reader", title: "Reader mode", hint: "This page", group: "Action", run: () => openReader(active.id) },
        { id: "devtools", title: "Developer tools", hint: "F12", group: "Action", run: () => openDevtools(active.id) },
        { id: "zoom-in", title: "Zoom in", hint: "Ctrl++", group: "Action", run: () => zoomTab(active.id, "in") },
        { id: "zoom-out", title: "Zoom out", hint: "Ctrl+-", group: "Action", run: () => zoomTab(active.id, "out") },
        { id: "zoom-reset", title: "Reset zoom", hint: "Ctrl+0", group: "Action", run: () => zoomTab(active.id, "reset") },
        { id: "sleep-others", title: "Sleep other tabs", hint: "Free memory", group: "Action", keywords: "suspend", run: () => sleepOtherTabs(active.id) },
      );
    }
    const tabs: Hit[] = snapshot.windowTabs.map((tab) => {
      const workspace = tab.workspace !== snapshot.workspace ? snapshot.workspaces.find((item) => item.id === tab.workspace)?.name : undefined;
      return {
        id: `tab-${tab.id}`,
        title: tab.title || tab.url,
        hint: workspace ? `Tab in ${workspace}` : "Switch to tab",
        keywords: tab.url,
        group: "Tab",
        run: () => activateTab(tab.id),
      };
    });
    const reopen: Hit[] = (closed ?? []).slice(0, 10).map((entry) => ({
      id: `closed-${entry.id}`,
      title: closedLabel(entry),
      hint: "Recently closed",
      keywords: entry.tabs.map((tab) => `${tab.title} ${tab.url}`).join(" "),
      group: "Recently closed",
      run: () => restoreClosed(entry.id),
    }));
    const workspaces: Hit[] = snapshot.workspaces
      .filter((workspace) => workspace.id !== snapshot.workspace)
      .map((workspace) => ({
        id: `workspace-${workspace.id}`,
        title: `Switch to ${workspace.name}`,
        hint: "Workspace",
        group: "Workspace",
        run: () => switchWorkspace(workspace.id),
      }));
    const sectionTitle = (id: string) => SETTINGS_SECTIONS.find((section) => section.id === id)?.title ?? "Settings";
    const settingsHits: Hit[] = SETTINGS_INDEX.map((entry) => ({
      id: `setting-${entry.id}`,
      title: entry.label,
      hint: `Settings, ${sectionTitle(entry.section)}`,
      keywords: entry.keywords,
      group: "Setting",
      run: () => openSetting(snapshot.windowTabs, entry.id),
    }));
    const tools: Hit[] = active
      ? BUILTIN_TOOLS.map((tool) => ({
          id: `tool-${tool.id}`,
          title: tool.name,
          hint: tool.description,
          group: "Tool",
          run: () => {
            if (tool.id === "clear-cache") {
              return clearSiteCache(active.id);
            }
            const params =
              tool.id === "page-controls" ? `${tabParams(active)}&host=${encodeURIComponent(siteKey(active.url))}` : tabParams(active);
            return openHere(tool.id as Parameters<typeof openPopup>[0]["kind"], params);
          },
        }))
      : [];
    const extensionHits: Hit[] = extensions
      .filter((extension) => extension.enabled && extension.popup)
      .map((extension) => ({
        id: `extension-${extension.id}`,
        title: extension.name,
        hint: "Extension",
        keywords: extension.description,
        group: "Extension",
        run: () => openHere("extension", extension.id, 380, 520),
      }));
    const pages: Hit[] = [
      ...flattenBookmarks(bookmarks).map((bookmark) => ({
        id: `bookmark-${bookmark.id}`,
        title: bookmark.title || bookmark.url,
        hint: bookmark.url,
        group: "Bookmark" as const,
        run: () => createTab(bookmark.url),
      })),
      ...history.slice(0, 200).map((item) => ({
        id: `history-${item.id}`,
        title: item.title || item.url,
        hint: item.url,
        group: "History" as const,
        run: () => createTab(item.url),
      })),
    ];
    return [...actions, ...tabs, ...reopen, ...workspaces, ...settingsHits, ...tools, ...extensionHits, ...pages];
  }, [active, bookmarks, closed, extensions, history, settings, snapshot.windowTabs, snapshot.workspace, snapshot.workspaces]);

  const fuse = useMemo(
    () =>
      new Fuse(all, {
        keys: [
          { name: "title", weight: 0.7 },
          { name: "hint", weight: 0.15 },
          { name: "keywords", weight: 0.15 },
        ],
        threshold: 0.4,
        ignoreLocation: true,
        includeScore: true,
        includeMatches: true,
        minMatchCharLength: 1,
      }),
    [all],
  );

  const ranked = useMemo<Ranked[]>(() => {
    const text = query.trim();
    if (!text) {
      const firsts = all.filter((hit) => hit.group === "Action").slice(0, 8);
      const recent = all.filter((hit) => hit.group === "Recently closed").slice(0, 3);
      return [...firsts, ...recent].map((hit) => ({ hit, matches: [] }));
    }
    const lower = text.toLowerCase();
    const scored = fuse.search(text, { limit: 60 }).map((result) => {
      const title = result.item.title.toLowerCase();
      const prefix = title.startsWith(lower) ? 0.3 : title.split(/\s+/).some((word) => word.startsWith(lower)) ? 0.15 : 0;
      const score = 1 - (result.score ?? 1) + prefix + (GROUP_BOOST[result.item.group] ?? 0);
      return { hit: result.item, matches: result.matches ?? [], score };
    });
    scored.sort((left, right) => right.score - left.score);
    const matched = scored.slice(0, MAX_HITS).map(({ hit, matches }) => ({ hit, matches }));
    const extra = textHits
      .filter((hit) => !matched.some((item) => item.hit.id === `history-${hit.id}`))
      .map((hit) => ({
        hit: {
          id: `history-${hit.id}`,
          title: hit.title || hit.url,
          hint: hit.snippet || hit.url,
          group: "History" as const,
          run: () => createTab(hit.url),
        },
        matches: [],
      }));
    return [...matched, ...extra].slice(0, MAX_HITS);
  }, [all, fuse, query, textHits]);

  useEffect(() => {
    const needle = query.trim();
    if (needle.length < 2) {
      setTextHits([]);
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void searchHistory(needle)
        .then((found) => {
          if (!cancelled) {
            setTextHits(found.filter((hit) => hit.snippet));
          }
        })
        .catch(() => {
          if (!cancelled) {
            setTextHits([]);
          }
        });
    }, 200);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [query]);

  useEffect(() => {
    setSelected(0);
  }, [query]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    listRef.current?.querySelector<HTMLElement>(`[data-index="${selected}"]`)?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  const choose = (hit: Hit | undefined) => {
    if (!hit) {
      return;
    }
    const opensPopup = hit.group === "Tool" || hit.group === "Extension" || hit.id === "tab-search" || hit.id === "shortcut-sheet";
    void Promise.resolve(hit.run()).finally(() => {
      if (!opensPopup) {
        void closePopup();
      }
    });
  };

  return (
    <div id="command-palette" className="flex h-full flex-col overflow-hidden rounded-xl bg-zinc-900 text-zinc-100 ring-1 ring-inset ring-zinc-700">
      <input
        id="command-palette-input"
        ref={inputRef}
        value={query}
        placeholder="Search actions, tabs, settings, tools and bookmarks"
        aria-label="Command palette"
        aria-controls="command-palette-list"
        className="h-12 shrink-0 border-b border-zinc-800 bg-transparent px-4 text-sm outline-none"
        onChange={(event) => setQuery(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown") {
            event.preventDefault();
            setSelected((index) => (ranked.length === 0 ? 0 : (index + 1) % ranked.length));
          } else if (event.key === "ArrowUp") {
            event.preventDefault();
            setSelected((index) => (ranked.length === 0 ? 0 : (index - 1 + ranked.length) % ranked.length));
          } else if (event.key === "Enter") {
            event.preventDefault();
            choose(ranked[selected]?.hit);
          } else if (event.key === "Escape") {
            void closePopup();
          }
        }}
      />
      <div id="command-palette-list" ref={listRef} role="listbox" aria-label="Results" className="min-h-0 flex-1 overflow-y-auto p-1.5">
        {ranked.length === 0 ? (
          <div id="command-palette-empty" className="px-3 py-4 text-sm text-zinc-500">
            No matches.
          </div>
        ) : (
          ranked.map(({ hit, matches }, index) => (
            <button
              id={`command-palette-${hit.id}`}
              key={hit.id}
              type="button"
              role="option"
              aria-selected={index === selected}
              data-index={index}
              className={cn(
                "flex h-10 w-full items-center gap-3 rounded-lg px-2.5 text-left text-sm",
                index === selected ? "bg-zinc-800" : "hover:bg-zinc-800/60",
              )}
              onMouseEnter={() => setSelected(index)}
              onClick={() => choose(hit)}
            >
              <span id={`command-palette-${hit.id}-group`} className="w-24 shrink-0 truncate text-[11px] uppercase tracking-wide text-zinc-500">
                {hit.group}
              </span>
              <span id={`command-palette-${hit.id}-title`} className="min-w-0 flex-1 truncate">
                <Highlighted id={`command-palette-${hit.id}-text`} text={hit.title} matches={matches} />
              </span>
              <span id={`command-palette-${hit.id}-hint`} className="max-w-56 shrink-0 truncate text-xs text-zinc-500">
                {hit.hint}
              </span>
            </button>
          ))
        )}
      </div>
    </div>
  );
}
