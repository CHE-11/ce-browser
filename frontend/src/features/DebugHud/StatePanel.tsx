import { useState } from "react";
import { Copy } from "lucide-react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  copyText,
  getSettings,
  listBookmarks,
  listDownloads,
  listExtensions,
  listTabs,
  onBookmarksChanged,
  onDownloadsChanged,
  onExtensionsChanged,
  onSettingsChanged,
  onTabsChanged,
} from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { useLiveJson } from "@/lib/use-live-json";
import { useNow } from "@/lib/use-now";
import { HudButton, HudEmpty, HudSearch, HudToolbar } from "./HudControls";
import { JsonTree } from "./JsonTree";
import { PathsView } from "./PathsView";

type Source = {
  id: string;
  label: string;
  load: () => Promise<unknown>;
  subscribe: (handler: (value: unknown) => void) => Promise<UnlistenFn>;
};

const SOURCES: Source[] = [
  { id: "tabs", label: "Tabs", load: listTabs, subscribe: onTabsChanged },
  { id: "settings", label: "Settings", load: getSettings, subscribe: onSettingsChanged },
  { id: "bookmarks", label: "Bookmarks", load: listBookmarks, subscribe: onBookmarksChanged },
  { id: "downloads", label: "Downloads", load: listDownloads, subscribe: onDownloadsChanged },
  { id: "extensions", label: "Extensions", load: listExtensions, subscribe: onExtensionsChanged },
];

const VIEWS = [...SOURCES.map((source) => ({ id: source.id, label: source.label })), { id: "paths", label: "Paths" }];

function agoText(updatedAt: number, now: number) {
  if (!updatedAt) {
    return "";
  }
  const seconds = Math.max(0, Math.round((now - updatedAt) / 1000));
  if (seconds < 60) {
    return `updated ${seconds}s ago`;
  }
  return `updated ${Math.round(seconds / 60)}m ago`;
}

/** Ticks on its own so the tree beside it does not redraw every second. */
function UpdatedBadge({ id, updatedAt, changes }: { id: string; updatedAt: number; changes: number }) {
  const now = useNow(1000);
  const recent = updatedAt > 0 && now - updatedAt < 4000 && changes > 0;
  return (
    <span
      id={id}
      title={changes > 0 ? `${changes} changed paths in the last update` : undefined}
      className={cn("shrink-0 rounded px-1 text-[10px] tabular-nums", recent ? "bg-amber-400/15 text-amber-200" : "text-zinc-500")}
    >
      {agoText(updatedAt, now)}
    </span>
  );
}

function SnapshotView({ source }: { source: Source }) {
  const [query, setQuery] = useState("");
  const [copied, setCopied] = useState(false);
  const live = useLiveJson(source.load, source.subscribe, true);

  return (
    <div id={`debug-state-${source.id}`} className="flex min-h-0 flex-1 flex-col">
      <HudToolbar id={`debug-state-${source.id}-toolbar`}>
        <HudSearch id={`debug-state-${source.id}-search`} value={query} placeholder="Filter keys and values" onChange={setQuery} />
        <UpdatedBadge id={`debug-state-${source.id}-updated`} updatedAt={live.updatedAt} changes={live.changed.size} />
        <HudButton
          id={`debug-state-${source.id}-copy`}
          title="Copy as JSON"
          disabled={live.value === undefined}
          onClick={() => {
            void copyText(JSON.stringify(live.value, null, 2)).then(() => {
              setCopied(true);
              window.setTimeout(() => setCopied(false), 1200);
            });
          }}
        >
          <Copy id={`debug-state-${source.id}-copy-icon`} aria-hidden className="size-3" />
          {copied ? "Copied" : "Copy"}
        </HudButton>
      </HudToolbar>
      {live.error ? (
        <p id={`debug-state-${source.id}-error`} className="px-3 py-1 text-[11px] text-red-300">
          {live.error}
        </p>
      ) : null}
      <div id={`debug-state-${source.id}-scroller`} className="min-h-0 flex-1 overflow-auto">
        {live.value === undefined ? (
          <HudEmpty id={`debug-state-${source.id}-loading`}>Loading…</HudEmpty>
        ) : (
          <JsonTree id={`debug-state-${source.id}-tree`} value={live.value} changed={live.changed} query={query} />
        )}
      </div>
    </div>
  );
}

export function StatePanel({ view, onView }: { view: string; onView: (view: string) => void }) {
  const source = SOURCES.find((item) => item.id === view);
  return (
    <section id="debug-state" className="flex min-h-0 flex-1 flex-col">
      <nav id="debug-state-views" className="flex shrink-0 gap-0.5 overflow-x-auto border-b border-zinc-800 px-2 pt-1">
        {VIEWS.map((item) => (
          <button
            id={`debug-state-view-${item.id}`}
            key={item.id}
            type="button"
            aria-pressed={item.id === view}
            className={cn(
              "h-6 shrink-0 rounded-t px-2 text-[11px]",
              item.id === view ? "bg-zinc-800 text-zinc-100" : "text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-100",
              focusRing,
            )}
            onClick={() => onView(item.id)}
          >
            {item.label}
          </button>
        ))}
      </nav>
      {source ? <SnapshotView key={source.id} source={source} /> : <PathsView />}
    </section>
  );
}

export const STATE_VIEWS = VIEWS.map((item) => item.id);
