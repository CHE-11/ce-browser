import { useEffect, useMemo, useState } from "react";
import { Bookmark, Download, History, Puzzle, Search, Settings } from "lucide-react";
import { FavIcon } from "../components/FavIcon";
import { navigate } from "../lib/browser-client";
import { hostOf } from "../lib/format";
import { flattenBookmarks, useAppearanceFromSettings, useBookmarks, useTabs } from "../lib/use-browser";

const TILE_LIMIT = 12;

const SHORTCUTS = [
  { url: "ce://bookmarks", label: "Bookmarks", icon: Bookmark },
  { url: "ce://downloads", label: "Downloads", icon: Download },
  { url: "ce://history", label: "History", icon: History },
  { url: "ce://extensions", label: "Extensions", icon: Puzzle },
  { url: "ce://settings", label: "Settings", icon: Settings },
];

export function NewTabPage() {
  useAppearanceFromSettings();
  const tree = useBookmarks();
  const { activeId } = useTabs();
  const [query, setQuery] = useState("");

  useEffect(() => {
    document.title = "New Tab";
  }, []);

  const tiles = useMemo(() => {
    const bar = flattenBookmarks(tree ? { bar: tree.bar, other: { ...tree.other, children: [] } } : null);
    const other = flattenBookmarks(tree ? { bar: { ...tree.bar, children: [] }, other: tree.other } : null);
    return [...bar, ...other].slice(0, TILE_LIMIT);
  }, [tree]);

  const submit = () => {
    const text = query.trim();
    if (text && activeId !== null) {
      void navigate(activeId, text);
    }
  };

  return (
    <div id="newtab" className="flex h-full flex-col items-center overflow-y-auto bg-zinc-950 px-6 pt-[18vh] text-zinc-100">
      <h1 id="newtab-title" className="mb-8 text-4xl font-semibold tracking-tight text-zinc-200">
        ce-browser
      </h1>
      <form
        id="newtab-search"
        className="flex h-12 w-full max-w-xl items-center gap-3 rounded-full border border-zinc-700 bg-zinc-900 px-5 focus-within:border-sky-500"
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <Search id="newtab-search-icon" className="size-5 text-zinc-400" aria-hidden />
        <input
          id="newtab-search-input"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search the web or type a URL"
          spellCheck={false}
          className="min-w-0 flex-1 bg-transparent text-base outline-none placeholder:text-zinc-500"
        />
      </form>

      {tiles.length > 0 ? (
        <div id="newtab-tiles" className="mt-12 grid w-full max-w-3xl grid-cols-[repeat(auto-fill,minmax(8.5rem,1fr))] gap-3">
          {tiles.map((tile) => (
            <a
              key={tile.id}
              id={`newtab-tile-${tile.id}`}
              href={tile.url}
              title={tile.url}
              className="flex flex-col items-center gap-2 rounded-xl border border-zinc-800 bg-zinc-900 px-3 py-4 text-center hover:border-zinc-700 hover:bg-zinc-800"
            >
              <span id={`newtab-tile-${tile.id}-icon`} className="flex size-10 items-center justify-center rounded-full bg-zinc-800">
                <FavIcon id={`newtab-tile-${tile.id}-favicon`} url={tile.url} className="size-5" />
              </span>
              <span id={`newtab-tile-${tile.id}-title`} className="w-full truncate text-xs text-zinc-200">
                {tile.title || hostOf(tile.url)}
              </span>
            </a>
          ))}
        </div>
      ) : null}

      <div id="newtab-shortcuts" className="mt-10 flex flex-wrap justify-center gap-2 pb-10">
        {SHORTCUTS.map((shortcut) => (
          <button
            key={shortcut.url}
            id={`newtab-shortcut-${shortcut.label.toLowerCase()}`}
            type="button"
            className="flex h-8 items-center gap-2 rounded-full border border-zinc-800 px-3 text-xs text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100"
            onClick={() => activeId !== null && void navigate(activeId, shortcut.url)}
          >
            <shortcut.icon id={`newtab-shortcut-${shortcut.label.toLowerCase()}-icon`} className="size-3.5" aria-hidden />
            {shortcut.label}
          </button>
        ))}
      </div>
    </div>
  );
}
