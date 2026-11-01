import { useEffect, useMemo, useRef, useState } from "react";
import { emitTo } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import Fuse from "fuse.js";
import {
  activateTab,
  closePopup,
  createEphemeralTab,
  createTab,
  openDevtools,
  openReader,
  reopenClosedTab,
  saveSettings,
  zoomTab,
} from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { flattenBookmarks, useBookmarks, useHistory, useSettings, useTabs } from "@/lib/use-browser";

type Hit = {
  id: string;
  title: string;
  hint: string;
  run: () => Promise<unknown> | void;
};

export function CommandPalette() {
  const snapshot = useTabs();
  const bookmarks = useBookmarks();
  const history = useHistory();
  const settings = useSettings();
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const active = snapshot.tabs.find((tab) => tab.id === snapshot.activeId);

  const hits = useMemo(() => {
    const actions: Hit[] = [
      { id: "new-tab", title: "New tab", hint: "Action", run: () => createTab() },
      { id: "ephemeral", title: "New ephemeral tab", hint: "Action", run: () => createEphemeralTab() },
      { id: "reopen", title: "Reopen closed tab", hint: "Action", run: () => reopenClosedTab() },
      { id: "bookmarks", title: "Bookmarks", hint: "Page", run: () => createTab("ce://bookmarks") },
      { id: "downloads", title: "Downloads", hint: "Page", run: () => createTab("ce://downloads") },
      { id: "files", title: "Files", hint: "Page", run: () => createTab("ce://files") },
      { id: "history", title: "History", hint: "Page", run: () => createTab("ce://history") },
      { id: "settings", title: "Settings", hint: "Page", run: () => createTab("ce://settings") },
      { id: "extensions", title: "Extensions", hint: "Page", run: () => createTab("ce://extensions") },
      { id: "scripts", title: "Site scripts", hint: "Page", run: () => createTab("ce://scripts") },
      { id: "permissions", title: "Permissions", hint: "Page", run: () => createTab("ce://permissions") },
      { id: "shortcuts", title: "Keyboard shortcuts", hint: "Page", run: () => createTab("ce://shortcuts") },
      { id: "rewrites", title: "Request rules", hint: "Page", run: () => createTab("ce://rewrites") },
      { id: "storage", title: "Site storage", hint: "Page", run: () => createTab("ce://storage") },
      {
        id: "find",
        title: "Find in page",
        hint: "Action",
        run: () => {
          const label = getCurrentWindow().label;
          const ui = label === "main" ? "ui" : label.replace(/^main-/, "ui-");
          return emitTo(ui, "browser-command", "find");
        },
      },
      {
        id: "vertical",
        title: settings?.tabLayout === "vertical" ? "Horizontal tabs" : "Vertical tabs",
        hint: "Action",
        run: async () => {
          if (settings) {
            await saveSettings({ ...settings, tabLayout: settings.tabLayout === "vertical" ? "horizontal" : "vertical" });
          }
        },
      },
    ];
    if (active) {
      actions.push(
        { id: "reader", title: "Reader mode", hint: "Action", run: () => openReader(active.id) },
        { id: "devtools", title: "Developer tools", hint: "Action", run: () => openDevtools(active.id) },
        { id: "zoom-in", title: "Zoom in", hint: "Action", run: () => zoomTab(active.id, "in") },
        { id: "zoom-out", title: "Zoom out", hint: "Action", run: () => zoomTab(active.id, "out") },
        { id: "zoom-reset", title: "Reset zoom", hint: "Action", run: () => zoomTab(active.id, "reset") },
      );
    }
    const pages: Hit[] = [
      ...snapshot.tabs.map((tab) => ({
        id: `tab-${tab.id}`,
        title: tab.title || tab.url,
        hint: "Tab",
        run: () => activateTab(tab.id),
      })),
      ...flattenBookmarks(bookmarks).map((bookmark) => ({
        id: `bookmark-${bookmark.id}`,
        title: bookmark.title || bookmark.url,
        hint: bookmark.url,
        run: () => createTab(bookmark.url),
      })),
      ...history.slice(0, 200).map((item) => ({
        id: `history-${item.id}`,
        title: item.title || item.url,
        hint: item.url,
        run: () => createTab(item.url),
      })),
    ];
    const all = [...actions, ...pages];
    const text = query.trim();
    if (!text) {
      return all.slice(0, 12);
    }
    return new Fuse(all, { keys: ["title", "hint"], threshold: 0.4, ignoreLocation: true })
      .search(text)
      .slice(0, 12)
      .map((result) => result.item);
  }, [active, bookmarks, history, query, settings, snapshot.tabs]);

  useEffect(() => {
    setSelected(0);
  }, [query]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const choose = (hit: Hit | undefined) => {
    if (!hit) {
      return;
    }
    void Promise.resolve(hit.run()).finally(() => closePopup());
  };

  return (
    <div id="command-palette" className="flex h-full flex-col overflow-hidden rounded-xl bg-zinc-900 text-zinc-100 ring-1 ring-inset ring-zinc-700">
      <input
        id="command-palette-input"
        ref={inputRef}
        value={query}
        placeholder="Search tabs, bookmarks and actions"
        className="h-12 shrink-0 border-b border-zinc-800 bg-transparent px-4 text-sm outline-none"
        onChange={(event) => setQuery(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown") {
            event.preventDefault();
            setSelected((index) => (hits.length === 0 ? 0 : (index + 1) % hits.length));
          } else if (event.key === "ArrowUp") {
            event.preventDefault();
            setSelected((index) => (hits.length === 0 ? 0 : (index - 1 + hits.length) % hits.length));
          } else if (event.key === "Enter") {
            event.preventDefault();
            choose(hits[selected]);
          } else if (event.key === "Escape") {
            void closePopup();
          }
        }}
      />
      <div id="command-palette-list" role="listbox" className="min-h-0 flex-1 overflow-y-auto p-1.5">
        {hits.length === 0 ? (
          <div id="command-palette-empty" className="px-3 py-4 text-sm text-zinc-500">
            No matches.
          </div>
        ) : (
          hits.map((hit, index) => (
            <button
              id={`command-palette-${hit.id}`}
              key={hit.id}
              type="button"
              role="option"
              aria-selected={index === selected}
              className={cn(
                "flex h-10 w-full items-center gap-3 rounded-lg px-2.5 text-left text-sm",
                index === selected ? "bg-zinc-800" : "hover:bg-zinc-800/60",
              )}
              onMouseEnter={() => setSelected(index)}
              onClick={() => choose(hit)}
            >
              <span id={`command-palette-${hit.id}-title`} className="min-w-0 flex-1 truncate">
                {hit.title}
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
