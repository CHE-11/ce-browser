import { useEffect, useMemo, useState } from "react";
import { History, Search, Trash2 } from "lucide-react";
import { Button } from "@/components/Button";
import { FavIcon } from "@/components/FavIcon";
import { IconButton } from "@/components/IconButton";
import { PageShell } from "@/components/PageShell";
import { createTab, removeHistory, clearHistory, searchHistory } from "@/lib/browser-client";
import { hostOf } from "@/lib/format";
import { absoluteTime, relativeTime } from "@/lib/time";
import { useHistory } from "@/lib/use-browser";
import { HistoryHit, HistoryItem } from "../../../../types/browser";


function dayLabel(ms: number) {
  const date = new Date(ms);
  const today = new Date();
  const yesterday = new Date();
  yesterday.setDate(today.getDate() - 1);
  if (date.toDateString() === today.toDateString()) {
    return "Today";
  }
  if (date.toDateString() === yesterday.toDateString()) {
    return "Yesterday";
  }
  return date.toLocaleDateString(undefined, { dateStyle: "long" });
}

function HistoryRow({ item, snippet }: { item: HistoryItem; snippet?: string }) {
  const domId = `history-${item.id}`;
  const when = new Date(item.lastVisited).toISOString();
  return (
    <div id={domId} className="group flex items-center gap-3 rounded-md border border-zinc-800 bg-zinc-900 px-3 py-3 hover:bg-zinc-800/70">
      <FavIcon id={`${domId}-icon`} url={item.url} className="size-5" />
      <div id={`${domId}-body`} className="min-w-0 flex-1">
        <button
          id={`${domId}-title`}
          type="button"
          title={item.title}
          className="block max-w-full truncate text-left text-sm text-zinc-100 hover:underline"
          onClick={() => void createTab(item.url)}
        >
          {item.title || hostOf(item.url)}
        </button>
        <button
          id={`${domId}-url`}
          type="button"
          title={item.url}
          className="block max-w-full truncate text-left text-xs text-zinc-500 hover:text-sky-400"
          onClick={() => void createTab(item.url)}
        >
          {item.url}
        </button>
        {snippet ? (
          <p id={`${domId}-snippet`} className="mt-1 line-clamp-2 text-xs text-zinc-400">
            {snippet}
          </p>
        ) : null}
      </div>
      <div id={`${domId}-meta`} className="hidden shrink-0 text-right text-xs text-zinc-500 sm:block">
        <div id={`${domId}-time`} title={absoluteTime(when)}>
          {relativeTime(when)}
        </div>
        {item.visitCount > 1 ? (
          <div id={`${domId}-count`}>
            {item.visitCount} visits
          </div>
        ) : null}
      </div>
      <IconButton id={`${domId}-remove`} label="Remove from history" onClick={() => void removeHistory(item.id)}>
        <Trash2 id={`${domId}-remove-icon`} aria-hidden />
      </IconButton>
    </div>
  );
}

export function HistoryPage() {
  const history = useHistory();
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<HistoryHit[] | null>(null);
  const [hitsFor, setHitsFor] = useState("");

  useEffect(() => {
    const needle = query.trim();
    if (!needle) {
      setHits(null);
      setHitsFor("");
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void searchHistory(needle)
        .then((found) => {
          if (!cancelled) {
            setHits(found);
            setHitsFor(needle);
          }
        })
        .catch(() => {
          if (!cancelled) {
            setHits(null);
            setHitsFor("");
          }
        });
    }, 200);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [query]);

  const groups = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const searched = needle && hitsFor === needle ? hits : null;
    const filtered = searched
      ? searched
      : needle
        ? history.filter(
            (item) => item.title.toLowerCase().includes(needle) || item.url.toLowerCase().includes(needle),
          )
        : history;
    const byDay = new Map<string, HistoryHit[]>();
    for (const item of filtered) {
      const label = dayLabel(item.lastVisited);
      const snippet = "snippet" in item && typeof item.snippet === "string" ? item.snippet : "";
      byDay.set(label, [...(byDay.get(label) ?? []), { ...item, snippet }]);
    }
    return [...byDay.entries()];
  }, [history, hits, hitsFor, query]);

  return (
    <PageShell
      id="history-page"
      page="history"
      title="History"
      actions={
        <>
          <label id="history-page-search" className="flex h-8 w-64 items-center gap-2 rounded-md border border-zinc-700 bg-zinc-900 px-2">
            <Search id="history-page-search-icon" className="size-4 text-zinc-500" aria-hidden />
            <input
              id="history-page-search-input"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search titles, addresses, and page text"
              className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-zinc-500"
            />
          </label>
          <Button id="history-page-clear" variant="outline" disabled={history.length === 0} onClick={() => void clearHistory()}>
            Clear all
          </Button>
        </>
      }
    >
      <div id="history-page-list" className="mx-auto flex max-w-3xl flex-col gap-6">
        {groups.length === 0 ? (
          <div id="history-page-empty" className="flex flex-col items-center gap-3 py-20 text-sm text-zinc-500">
            <History id="history-page-empty-icon" className="size-8" aria-hidden />
            {query ? "No history matches that search." : "Pages you visit appear here."}
          </div>
        ) : (
          groups.map(([label, items]) => (
            <section key={label} id={`history-page-day-${label.replace(/\W+/g, "-")}`} className="flex flex-col gap-2">
              <h2 id={`history-page-day-${label.replace(/\W+/g, "-")}-title`} className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
                {label}
              </h2>
              {items.map((item) => (
                <HistoryRow key={item.id} item={item} snippet={item.snippet} />
              ))}
            </section>
          ))
        )}
      </div>
    </PageShell>
  );
}
