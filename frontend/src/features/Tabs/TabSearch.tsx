import { useEffect, useMemo, useRef, useState } from "react";
import Fuse from "fuse.js";
import { Layers, Moon, RotateCcw, X } from "lucide-react";
import { FavIcon } from "@/components/FavIcon";
import { PopupFrame } from "@/components/PopupFrame";
import { activateTab, closePopup, closeTab, restoreClosed } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { hostOf } from "@/lib/format";
import { tabProblemTitle, useHeavyTabs } from "@/lib/tab-alert";
import { groupColorClasses } from "@/lib/tab-rows";
import { relativeTime } from "@/lib/time";
import { useRecentlyClosed, useTabs } from "@/lib/use-browser";
import type { ClosedEntry, TabInfo } from "@type/browser";
import { TabAlertDot } from "./TabAlertDot";

type Row =
  | { kind: "tab"; key: string; tab: TabInfo; workspace: string }
  | { kind: "closed"; key: string; entry: ClosedEntry };

type TabSearchProps = {
  params: URLSearchParams;
};

function closedTitle(entry: ClosedEntry) {
  if (entry.kind === "group") {
    const name = entry.group?.name.trim() || "Unnamed group";
    return `${name} (${entry.tabs.length} ${entry.tabs.length === 1 ? "tab" : "tabs"})`;
  }
  if (entry.kind === "batch") {
    return `${entry.tabs.length} tabs`;
  }
  return entry.tabs[0]?.title || entry.tabs[0]?.url || "Tab";
}

function closedHint(entry: ClosedEntry) {
  if (entry.kind === "tab") {
    return hostOf(entry.tabs[0]?.url ?? "");
  }
  return entry.tabs
    .slice(0, 3)
    .map((tab) => tab.title || hostOf(tab.url))
    .join(", ");
}

/** Search across every workspace's tabs and the recently closed list. `scope=workspace` lists
 * only the current workspace, for the tab strip's overflow button. */
export function TabSearch({ params }: TabSearchProps) {
  const snapshot = useTabs();
  const heavy = useHeavyTabs();
  const closed = useRecentlyClosed();
  const onlyWorkspace = params.get("scope") === "workspace";
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const workspaceName = (id: string) => snapshot.workspaces.find((item) => item.id === id)?.name ?? "Workspace";

  const rows = useMemo<Row[]>(() => {
    const source = onlyWorkspace ? snapshot.tabs : snapshot.windowTabs;
    const order = (tab: TabInfo) => (tab.workspace === snapshot.workspace ? 0 : 1);
    const tabs = [...source].sort(
      (left, right) => order(left) - order(right) || left.workspace.localeCompare(right.workspace) || right.lastActive - left.lastActive,
    );
    const tabRows: Row[] = tabs.map((tab) => ({ kind: "tab", key: `tab-${tab.id}`, tab, workspace: tab.workspace }));
    const closedRows: Row[] = onlyWorkspace
      ? []
      : (closed ?? []).map((entry) => ({ kind: "closed", key: `closed-${entry.id}`, entry }));
    const text = query.trim();
    if (!text) {
      return [...tabRows, ...closedRows];
    }
    const searchable = [...tabRows, ...closedRows].map((row) => ({
      row,
      title: row.kind === "tab" ? row.tab.title : closedTitle(row.entry),
      url: row.kind === "tab" ? row.tab.url : row.entry.tabs.map((tab) => `${tab.title} ${tab.url}`).join(" "),
    }));
    return new Fuse(searchable, { keys: [{ name: "title", weight: 0.6 }, { name: "url", weight: 0.4 }], threshold: 0.4, ignoreLocation: true })
      .search(text)
      .map((result) => result.item.row);
  }, [closed, onlyWorkspace, query, snapshot.tabs, snapshot.windowTabs, snapshot.workspace]);

  useEffect(() => {
    setSelected(0);
  }, [query]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    listRef.current?.querySelector<HTMLElement>(`[data-index="${selected}"]`)?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  const choose = (row: Row | undefined) => {
    if (!row) {
      return;
    }
    const done = row.kind === "tab" ? activateTab(row.tab.id) : restoreClosed(row.entry.id);
    void done.finally(() => closePopup());
  };

  const firstClosed = rows.findIndex((row) => row.kind === "closed");
  let lastWorkspace: string | null = null;

  return (
    <PopupFrame id="tab-search" width={onlyWorkspace ? 420 : 520} maxHeight={2000} ready={snapshot.loaded} className="flex flex-col">
      <input
        id="tab-search-input"
        ref={inputRef}
        value={query}
        placeholder={onlyWorkspace ? "Search tabs in this workspace" : "Search open and recently closed tabs"}
        aria-label="Search tabs"
        aria-controls="tab-search-list"
        className="h-11 shrink-0 border-b border-zinc-800 bg-transparent px-4 text-sm outline-none"
        onChange={(event) => setQuery(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown") {
            event.preventDefault();
            setSelected((index) => (rows.length === 0 ? 0 : (index + 1) % rows.length));
          } else if (event.key === "ArrowUp") {
            event.preventDefault();
            setSelected((index) => (rows.length === 0 ? 0 : (index - 1 + rows.length) % rows.length));
          } else if (event.key === "Enter") {
            event.preventDefault();
            choose(rows[selected]);
          } else if (event.key === "Escape") {
            void closePopup();
          }
        }}
      />
      <div id="tab-search-list" ref={listRef} role="listbox" aria-label="Tabs" className="max-h-104 overflow-y-auto p-1.5">
        {rows.length === 0 ? (
          <div id="tab-search-empty" className="px-3 py-6 text-center text-sm text-zinc-500">
            {query.trim() ? "No tabs match." : "No tabs."}
          </div>
        ) : (
          rows.map((row, index) => {
            const heading =
              row.kind === "tab" && !query.trim() && !onlyWorkspace && row.workspace !== lastWorkspace && snapshot.workspaces.length > 1
                ? workspaceName(row.workspace)
                : row.kind === "closed" && index === firstClosed
                  ? "Recently closed"
                  : null;
            if (row.kind === "tab") {
              lastWorkspace = row.workspace;
            }
            const problem = row.kind === "tab" ? tabProblemTitle(row.tab, heavy.get(row.tab.id) ?? null) : null;
            return (
              <div id={`tab-search-row-${row.key}`} key={row.key}>
                {heading ? (
                  <div id={`tab-search-heading-${row.key}`} className="px-2.5 pb-1 pt-2.5 text-[11px] font-medium uppercase tracking-wide text-zinc-500">
                    {heading}
                  </div>
                ) : null}
                {row.kind === "tab" ? (
                  <div
                    id={`tab-search-${row.key}`}
                    role="option"
                    aria-selected={index === selected}
                    aria-label={problem ? `${row.tab.title || row.tab.url}, ${problem}` : undefined}
                    data-index={index}
                    className={cn(
                      "group flex h-10 w-full cursor-default items-center gap-3 rounded-lg px-2.5 text-left text-sm",
                      index === selected ? "bg-zinc-800" : "hover:bg-zinc-800/60",
                    )}
                    onMouseEnter={() => setSelected(index)}
                    onClick={() => choose(row)}
                  >
                    <span id={`tab-search-${row.key}-icon-wrap`} className="relative inline-flex shrink-0">
                      <FavIcon id={`tab-search-${row.key}-icon`} url={row.tab.url} src={row.tab.favicon} />
                      {problem ? (
                        <TabAlertDot id={`tab-search-${row.key}-alert`} title={problem} className="-right-0.5 -top-0.5 ring-zinc-900" />
                      ) : null}
                    </span>
                    <span id={`tab-search-${row.key}-text`} className="min-w-0 flex-1">
                      <span id={`tab-search-${row.key}-title`} className="block truncate">
                        {row.tab.title || row.tab.url}
                      </span>
                      <span id={`tab-search-${row.key}-url`} className="block truncate text-xs text-zinc-500">
                        {hostOf(row.tab.url)}
                      </span>
                    </span>
                    {row.tab.sleeping ? <Moon id={`tab-search-${row.key}-asleep`} className="size-3.5 shrink-0 text-zinc-500" aria-label="Asleep" /> : null}
                    {row.tab.id === snapshot.activeId ? (
                      <span id={`tab-search-${row.key}-current`} className="shrink-0 text-[11px] text-sky-400">
                        Current
                      </span>
                    ) : null}
                    <button
                      id={`tab-search-${row.key}-close`}
                      type="button"
                      aria-label={`Close ${row.tab.title}`}
                      title="Close tab"
                      className="inline-flex size-6 shrink-0 items-center justify-center rounded text-zinc-400 opacity-0 hover:bg-zinc-700 hover:text-zinc-100 focus-visible:opacity-100 group-hover:opacity-100"
                      onClick={(event) => {
                        event.stopPropagation();
                        void closeTab(row.tab.id);
                      }}
                    >
                      <X id={`tab-search-${row.key}-close-icon`} className="size-3.5" aria-hidden />
                    </button>
                  </div>
                ) : (
                  <div
                    id={`tab-search-${row.key}`}
                    role="option"
                    aria-selected={index === selected}
                    data-index={index}
                    className={cn(
                      "flex h-10 w-full cursor-default items-center gap-3 rounded-lg px-2.5 text-left text-sm",
                      index === selected ? "bg-zinc-800" : "hover:bg-zinc-800/60",
                    )}
                    onMouseEnter={() => setSelected(index)}
                    onClick={() => choose(row)}
                  >
                    {row.entry.kind === "tab" ? (
                      <FavIcon id={`tab-search-${row.key}-icon`} url={row.entry.tabs[0]?.url ?? ""} src={row.entry.tabs[0]?.favicon} />
                    ) : row.entry.kind === "group" ? (
                      <span
                        id={`tab-search-${row.key}-swatch`}
                        className={cn("size-3 shrink-0 rounded-full", groupColorClasses(row.entry.group?.color ?? "grey").swatch)}
                      />
                    ) : (
                      <Layers id={`tab-search-${row.key}-icon`} className="size-4 shrink-0 text-zinc-400" aria-hidden />
                    )}
                    <span id={`tab-search-${row.key}-text`} className="min-w-0 flex-1">
                      <span id={`tab-search-${row.key}-title`} className="block truncate">
                        {closedTitle(row.entry)}
                      </span>
                      <span id={`tab-search-${row.key}-hint`} className="block truncate text-xs text-zinc-500">
                        {closedHint(row.entry)}
                      </span>
                    </span>
                    <span id={`tab-search-${row.key}-when`} className="shrink-0 text-[11px] text-zinc-500">
                      {row.entry.closedAt ? relativeTime(new Date(row.entry.closedAt).toISOString()) : ""}
                    </span>
                    <RotateCcw id={`tab-search-${row.key}-restore`} className="size-3.5 shrink-0 text-zinc-500" aria-label="Restore" />
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
    </PopupFrame>
  );
}
