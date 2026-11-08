import { useEffect, useMemo, useRef, useState } from "react";
import { AppWindow, ListTree, RefreshCw } from "lucide-react";
import { PopoverHeader } from "@features/BuiltInExtensions/PopoverParts";
import { clearNetworkLog, closeThisPopup, listTabs, networkLog, openNetworkWindow } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { hostOf } from "@/lib/format";
import { authBadge, entryMatches, formatClock, networkColumnClass } from "@/lib/network-log";
import type { NetworkEntry } from "@type/browser";
import { NetworkRow } from "./NetworkRow";

type NetworkLogProps = {
  tabId: number;
  variant: "popup" | "window";
};

function failureText(reason: unknown, fallback: string) {
  return typeof reason === "string" && reason.trim() ? reason : fallback;
}

export function NetworkLog({ tabId, variant }: NetworkLogProps) {
  const [entries, setEntries] = useState<NetworkEntry[] | null>(null);
  const [pageLabel, setPageLabel] = useState("");
  const [query, setQuery] = useState("");
  const [authOnly, setAuthOnly] = useState(false);
  const [live, setLive] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [updatedAt, setUpdatedAt] = useState<number | null>(null);
  const [error, setError] = useState("");
  const loadRef = useRef<(manual: boolean) => void>(() => undefined);
  const generation = useRef(0);
  const validTab = Number.isInteger(tabId) && tabId >= 0;

  useEffect(() => {
    if (!validTab) {
      return;
    }
    let disposed = false;
    let stamp = "";
    const load = (manual: boolean) => {
      const request = ++generation.current;
      if (manual) {
        setRefreshing(true);
      }
      void Promise.all([networkLog(tabId), listTabs()])
        .then(([next, snapshot]) => {
          if (disposed || request !== generation.current) {
            return;
          }
          const nextStamp = `${next.length}:${next.at(-1)?.id ?? 0}`;
          if (manual || nextStamp !== stamp) {
            stamp = nextStamp;
            setEntries(next);
          }
          const tab = snapshot.tabs.find((item) => item.id === tabId);
          const label = tab ? tab.title || hostOf(tab.url) : "";
          setPageLabel((current) => (current === label ? current : label));
          setUpdatedAt(Date.now());
          setError("");
        })
        .catch((reason: unknown) => {
          if (!disposed && request === generation.current) {
            setError(failureText(reason, "The network log could not be read."));
          }
        })
        .finally(() => {
          if (!disposed && manual && request === generation.current) {
            setRefreshing(false);
          }
        });
    };
    loadRef.current = load;
    load(false);
    const timer = window.setInterval(() => {
      if (live) {
        load(false);
      }
    }, 1000);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [live, tabId, validTab]);

  const filtered = useMemo(() => {
    const rows = (entries ?? []).filter((entry) => entryMatches(entry, query, authOnly));
    return rows.reverse();
  }, [authOnly, entries, query]);
  const credentialCount = useMemo(() => (entries ?? []).filter((entry) => authBadge(entry)).length, [entries]);
  const total = entries?.length ?? 0;
  const narrowed = query.trim().length > 0 || authOnly;
  const subtitle = [
    pageLabel,
    narrowed ? `${filtered.length} of ${total}` : `${total} requests`,
    credentialCount > 0 ? `${credentialCount} with credentials` : "",
    live ? "Live" : "Paused",
  ]
    .filter(Boolean)
    .join(" · ");

  if (!validTab) {
    return (
      <p id="network-missing" className="px-4 py-6 text-sm text-zinc-500">
        This tab is no longer available.
      </p>
    );
  }

  return (
    <div id={variant === "window" ? "network-window" : "network-log"} className={cn("flex min-h-0 min-w-0 flex-col", variant === "popup" ? "h-[32rem]" : "h-full")}>
      <PopoverHeader id="network-header" icon={<ListTree id="network-header-glyph" aria-hidden />} title="Network" subtitle={subtitle} />
      <div id="network-toolbar" className="flex flex-wrap items-center gap-1.5 px-3 pb-2">
        <label id="network-search" className="flex h-7 min-w-40 flex-1 items-center rounded-md border border-zinc-800 bg-zinc-950 px-2 focus-within:border-sky-500">
          <input
            id="network-search-input"
            type="search"
            value={query}
            placeholder="Filter URL, header, or token"
            className="min-w-0 flex-1 bg-transparent text-xs text-zinc-100 outline-none placeholder:text-zinc-500"
            onChange={(event) => setQuery(event.target.value)}
          />
        </label>
        <button
          id="network-auth-filter"
          type="button"
          aria-pressed={authOnly}
          className={cn("h-7 rounded-md px-2 text-xs", authOnly ? "bg-sky-500/15 text-sky-200" : "text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100")}
          onClick={() => setAuthOnly((current) => !current)}
        >
          Auth
        </button>
        <button
          id="network-live"
          type="button"
          aria-pressed={live}
          className={cn("inline-flex h-7 items-center gap-1.5 rounded-md px-2 text-xs", live ? "text-emerald-300 hover:bg-zinc-800" : "text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100")}
          onClick={() => setLive((current) => !current)}
        >
          <span id="network-live-dot" className={cn("size-1.5 rounded-full", live ? "bg-emerald-400" : "bg-zinc-600")} />
          {live ? "Live" : "Paused"}
        </button>
        <button
          id="network-refresh"
          type="button"
          className="inline-flex h-7 items-center gap-1 rounded-md px-2 text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
          title={updatedAt ? `Updated ${formatClock(updatedAt)}` : "Refresh"}
          onClick={() => loadRef.current(true)}
        >
          <RefreshCw id="network-refresh-icon" aria-hidden className={cn("size-3.5", refreshing && "animate-spin")} />
          Refresh
        </button>
        <button
          id="network-clear"
          type="button"
          className="h-7 rounded-md px-2 text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
          onClick={() => {
            const request = ++generation.current;
            void clearNetworkLog(tabId)
              .then(() => {
                if (request !== generation.current) {
                  return;
                }
                setEntries([]);
                setError("");
              })
              .catch((reason: unknown) => setError(failureText(reason, "The network log could not be cleared.")));
          }}
        >
          Clear
        </button>
        {variant === "popup" ? (
          <button
            id="network-popout"
            type="button"
            className="inline-flex h-7 items-center gap-1 rounded-md px-2 text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
            onClick={() => {
              void openNetworkWindow(tabId)
                .then(() => closeThisPopup())
                .catch((reason: unknown) => setError(failureText(reason, "The network window did not open.")));
            }}
          >
            <AppWindow id="network-popout-icon" aria-hidden className="size-3.5" />
            Pop out
          </button>
        ) : null}
      </div>
      {error ? (
        <p id="network-error" className="px-4 pb-1 text-xs text-red-300">
          {error}
        </p>
      ) : null}
      <div id="network-scroller" className="min-h-0 flex-1 overflow-auto px-2 pb-2">
        {entries === null ? (
          <p id="network-loading" className="px-2 py-3 text-sm text-zinc-500">
            Loading requests…
          </p>
        ) : filtered.length === 0 ? (
          <p id="network-empty" className="px-2 py-3 text-sm text-zinc-500">
            {total === 0 ? "No requests yet." : "Nothing matches this filter."}
          </p>
        ) : (
          <div id="network-table" className="min-w-[52rem]">
            <div id="network-columns" className="sticky top-0 z-10 flex items-center gap-2 bg-zinc-900 px-2 py-1 text-[10px] uppercase tracking-wide text-zinc-500">
              <span id="network-column-toggle" className={networkColumnClass.toggle} />
              <span id="network-column-status" className={cn(networkColumnClass.status, "truncate")}>
                Status
              </span>
              <span id="network-column-method" className={cn(networkColumnClass.method, "truncate")}>
                Method
              </span>
              <span id="network-column-when" className={cn(networkColumnClass.when, "truncate")}>
                Date time
              </span>
              <span id="network-column-duration" className={cn(networkColumnClass.duration, "truncate")}>
                Request
              </span>
              <span id="network-column-auth" className={cn(networkColumnClass.auth, "truncate")}>
                Auth
              </span>
              <span id="network-column-url" className={cn(networkColumnClass.url, "truncate")}>
                URL
              </span>
            </div>
            <ul id="network-list">
              {filtered.map((entry) => (
                <NetworkRow key={entry.id} entry={entry} />
              ))}
            </ul>
          </div>
        )}
      </div>
    </div>
  );
}
