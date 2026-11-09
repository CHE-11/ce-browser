import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import Fuse, { type FuseResultMatch } from "fuse.js";
import {
  activateTab,
  clearSiteCache,
  closePopup,
  debugToggleHud,
  createEphemeralTab,
  createTab,
  groupBySite,
  navigate,
  openDevtools,
  openExtensionPanel,
  openPopup,
  openReader,
  restoreClosed,
  saveSettings,
  searchHistory,
  sendUiCommand,
  sleepOtherTabs,
  startFlowRecording,
  toggleAgentPanel,
  toggleAutomationPanel,
  switchWorkspace,
  zoomTab,
} from "@/lib/browser-client";
import { PopupFrame } from "@/components/PopupFrame";
import { CHROME, metric, paletteHintClass, paletteInputClass, paletteRowClass } from "@/lib/chrome-metrics";
import { cn } from "@/lib/cn";
import { extensionOpenAs } from "@/lib/extension-open";
import { flattenBookmarks, useBookmarks, useExtensions, useHistory, useRecentlyClosed, useSettings, useTabs } from "@/lib/use-browser";
import { BUILTIN_TOOLS } from "@features/BuiltInExtensions/builtin";
import { builtinToolEnabled } from "@/lib/toolbar-pin";
import { siteKey } from "@features/BuiltInExtensions/page-controls/site";
import { tabParams } from "@features/BuiltInExtensions/shared";
import { SETTINGS_INDEX } from "@features/Settings/settings-index";
import { SETTINGS_SECTIONS } from "@/features/Settings/settings-sections";
import type { ClosedEntry, HistoryHit, PopupKind, TabInfo } from "@type/browser";

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

type PaletteFilter = {
  key: string;
  label: string;
  group: Hit["group"];
};

/** A leading letter, alone or followed by a space, limits the palette to one group. */
const PALETTE_FILTERS: readonly PaletteFilter[] = [
  { key: "b", label: "Bookmarks", group: "Bookmark" },
  { key: "t", label: "Tabs", group: "Tab" },
  { key: "h", label: "History", group: "History" },
  { key: "s", label: "Settings", group: "Setting" },
  { key: "a", label: "Actions", group: "Action" },
  { key: "e", label: "Extensions", group: "Extension" },
  { key: "w", label: "Workspaces", group: "Workspace" },
  { key: "r", label: "Recent", group: "Recently closed" },
  { key: "p", label: "Tools", group: "Tool" },
];

function parsePaletteQuery(query: string): { filter: PaletteFilter | null; text: string } {
  const raw = query.replace(/^\s+/, "");
  const filter = PALETTE_FILTERS.find((item) => item.key === raw.slice(0, 1).toLowerCase()) ?? null;
  if (filter && (raw.length === 1 || raw[1] === " ")) {
    return { filter, text: raw.slice(1).trim() };
  }
  return { filter: null, text: query.trim() };
}

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
  const openHere = (kind: PopupKind, params: string) => openPopup(kind, { x: 0, y: 0 }, { params });

  const all = useMemo<Hit[]>(() => {
    const actions: Hit[] = [
      { id: "new-tab", title: "New tab", hint: "Ctrl+T", group: "Action", run: () => createTab() },
      { id: "ephemeral", title: "New ephemeral tab", hint: "InPrivate", group: "Action", run: () => createEphemeralTab() },
      { id: "tab-search", title: "Search tabs", hint: "Ctrl+Shift+A", group: "Action", run: () => sendUiCommand("tab-search") },
      { id: "shortcut-sheet", title: "Keyboard shortcut cheat sheet", hint: "Ctrl+/", group: "Action", run: () => sendUiCommand("shortcut-sheet") },
      { id: "group-by-site", title: "Group tabs by site", hint: "Tab groups", group: "Action", run: () => groupBySite() },
      { id: "bookmarks", title: "Bookmarks", hint: "ce://bookmarks", group: "Action", run: () => createTab("ce://bookmarks") },
      { id: "boards", title: "Boards", hint: "ce://boards", group: "Action", run: () => createTab("ce://boards") },
      { id: "downloads", title: "Downloads", hint: "ce://downloads", group: "Action", run: () => createTab("ce://downloads") },
      { id: "files", title: "Files", hint: "ce://files", group: "Action", run: () => createTab("ce://files") },
      { id: "history", title: "History", hint: "ce://history", group: "Action", run: () => createTab("ce://history") },
      { id: "settings", title: "Settings", hint: "ce://settings", group: "Action", run: () => openSetting(snapshot.windowTabs, "settings-profiles") },
      { id: "styles", title: "Styles", hint: "ce://styles", group: "Action", run: () => createTab("ce://styles") },
      { id: "extensions", title: "Extensions", hint: "ce://extensions", group: "Action", run: () => createTab("ce://extensions") },
      { id: "scripts", title: "Site scripts", hint: "ce://scripts", group: "Action", run: () => createTab("ce://scripts") },
      { id: "permissions", title: "Permissions", hint: "ce://permissions", group: "Action", run: () => createTab("ce://permissions") },
      { id: "shortcuts", title: "Keyboard shortcuts", hint: "ce://shortcuts", group: "Action", run: () => createTab("ce://shortcuts") },
      { id: "rewrites", title: "Request rules", hint: "ce://rewrites", group: "Action", run: () => createTab("ce://rewrites") },
      { id: "storage", title: "Site storage", hint: "ce://storage", group: "Action", run: () => createTab("ce://storage") },
      { id: "find", title: "Find in page", hint: "Ctrl+F", group: "Action", run: () => sendUiCommand("find") },
      { id: "find-all", title: "Find in all tabs", hint: "Ctrl+Shift+F", group: "Action", run: () => sendUiCommand("find-all") },
      { id: "agent-panel", title: "Agent", hint: "Ctrl+Shift+Y", group: "Action", keywords: "ai automate browser task", run: () => toggleAgentPanel(true) },
      { id: "automations-panel", title: "Automations panel", hint: "Beside the page", group: "Action", keywords: "flows record macros", run: () => toggleAutomationPanel(true) },
      { id: "automations", title: "Automations", hint: "ce://automations", group: "Action", keywords: "flows macros schedule datasets rpa", run: () => createTab("ce://automations") },
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
        { id: "record-flow", title: "Record a flow", hint: "Opens beside the page", group: "Action", keywords: "macro automation rpa", run: () => startFlowRecording(active.id) },
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
    const workspaces: Hit[] =
      settings?.workspacesEnabled === false
        ? []
        : snapshot.workspaces
            .filter((workspace) => workspace.id !== snapshot.workspace)
            .map((workspace) => ({
              id: `workspace-${workspace.id}`,
              title: `Switch to ${workspace.name}`,
              hint: "Workspace",
              group: "Workspace" as const,
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
    const tools: Hit[] = BUILTIN_TOOLS.filter((tool) => builtinToolEnabled(settings, tool.id) && (active || tool.dev)).map((tool) => ({
      id: `tool-${tool.id}`,
      title: tool.name,
      hint: tool.description,
      group: "Tool" as const,
      run: () => {
        if (tool.id === "debug-hud") {
          return debugToggleHud();
        }
        if (!active) {
          return;
        }
        if (tool.id === "clear-cache") {
          return clearSiteCache(active.id);
        }
        const params =
          tool.id === "page-controls" ? `${tabParams(active)}&host=${encodeURIComponent(siteKey(active.url))}` : tabParams(active);
        return openHere(tool.id as PopupKind, params);
      },
    }));
    const extensionHits: Hit[] = extensions
      .filter((extension) => extension.enabled && (extension.popup || extension.sidePanel))
      .map((extension) => ({
        id: `extension-${extension.id}`,
        title: extension.defaultTitle || extension.name,
        hint: "Extension",
        keywords: extension.description,
        group: "Extension",
        run: () => {
          if (!extension.popup && extension.sidePanel) {
            return openExtensionPanel(extension.id);
          }
          if (extensionOpenAs(settings, extension.id) === "panel") {
            return openExtensionPanel(extension.id);
          }
          return openHere("extension", `id=${extension.id}`);
        },
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

  const parsed = useMemo(() => parsePaletteQuery(query), [query]);

  const fuse = useMemo(() => {
    const source = parsed.filter ? all.filter((hit) => hit.group === parsed.filter?.group) : all;
    return new Fuse(source, {
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
    });
  }, [all, parsed.filter]);

  const ranked = useMemo<Ranked[]>(() => {
    const text = parsed.text;
    if (!text) {
      const shown = parsed.filter
        ? all.filter((hit) => hit.group === parsed.filter?.group).slice(0, MAX_HITS)
        : [...all.filter((hit) => hit.group === "Action").slice(0, 8), ...all.filter((hit) => hit.group === "Recently closed").slice(0, 3)];
      return shown.map((hit) => ({ hit, matches: [] }));
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
    const extra =
      parsed.filter && parsed.filter.group !== "History"
        ? []
        : textHits
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
  }, [all, fuse, parsed.filter, parsed.text, textHits]);

  useEffect(() => {
    const needle = parsed.text;
    if ((parsed.filter && parsed.filter.group !== "History") || needle.length < 2) {
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
  }, [parsed.filter, parsed.text]);

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
    const pending = Promise.resolve(hit.run());
    if (!opensPopup) {
      void closePopup();
    }
    void pending.catch(() => undefined);
  };

  const applyFilter = (filter: PaletteFilter) => {
    const next = parsed.filter?.key === filter.key ? "" : `${filter.key} `;
    setQuery(next);
    requestAnimationFrame(() => {
      const input = inputRef.current;
      if (!input) {
        return;
      }
      input.focus();
      input.setSelectionRange(next.length, next.length);
    });
  };

  return (
    <PopupFrame id="command-palette" width={metric(settings?.commandPaletteWidth, CHROME.commandPaletteWidth)} maxHeight={2000} className="flex flex-col">
      <input
        id="command-palette-input"
        ref={inputRef}
        type="search"
        value={query}
        spellCheck={false}
        autoComplete="off"
        placeholder={parsed.filter ? `Search ${parsed.filter.label.toLowerCase()}` : "Search actions, tabs, settings, tools and bookmarks"}
        aria-label="Command palette"
        aria-controls="command-palette-list"
        aria-describedby="command-palette-filters"
        className={cn(paletteInputClass, "[&::-webkit-search-cancel-button]:hidden [&::-webkit-search-decoration]:hidden")}
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
      <div id="command-palette-list" ref={listRef} role="listbox" aria-label="Results" className="max-h-93 overflow-y-auto p-1.5">
        {ranked.length === 0 ? (
          <div id="command-palette-empty" className="px-3 py-4 text-sm text-zinc-500">
            {parsed.filter ? `No matching ${parsed.filter.label.toLowerCase()}.` : "No matches."}
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
                paletteRowClass,
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
              <span id={`command-palette-${hit.id}-hint`} className={paletteHintClass}>
                {hit.hint}
              </span>
            </button>
          ))
        )}
      </div>
      <div id="command-palette-filters" role="group" aria-label="Search filters" className="flex shrink-0 flex-wrap items-center justify-center gap-x-0.5 gap-y-0.5 border-t border-zinc-800 px-2 py-1">
        {PALETTE_FILTERS.map((filter) => {
          const active = parsed.filter?.key === filter.key;
          return (
            <button
              id={`command-palette-filter-${filter.key}`}
              key={filter.key}
              type="button"
              aria-pressed={active}
              className={cn(
                "flex items-center gap-1 rounded px-1 py-0.5 text-[10px]",
                active ? "bg-zinc-800 text-zinc-100" : "text-zinc-500 hover:bg-zinc-800/60 hover:text-zinc-300",
              )}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => applyFilter(filter)}
            >
              <kbd
                id={`command-palette-filter-${filter.key}-key`}
                className={cn(
                  "rounded border px-0.5 font-sans leading-none",
                  active ? "border-sky-800 bg-sky-950 text-sky-300" : "border-zinc-700 bg-zinc-800 text-zinc-300",
                )}
              >
                {filter.key}
              </kbd>
              <span id={`command-palette-filter-${filter.key}-label`}>{filter.label}</span>
            </button>
          );
        })}
      </div>
    </PopupFrame>
  );
}
