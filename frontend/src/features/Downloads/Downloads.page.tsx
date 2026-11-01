import { useMemo, useState } from "react";
import { Download, Search } from "lucide-react";
import { Button } from "@components/Button";
import { DownloadRow } from "@features/Downloads/DownloadRow";
import { PageShell } from "@components/PageShell";
import { clearDownloads } from "@lib/browser-client";
import { useDownloads } from "@lib/use-browser";

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

export function DownloadsPage() {
  const downloads = useDownloads();
  const [query, setQuery] = useState("");

  const groups = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const filtered = needle
      ? downloads.filter(
          (item) => item.fileName.toLowerCase().includes(needle) || item.url.toLowerCase().includes(needle),
        )
      : downloads;
    const byDay = new Map<string, typeof downloads>();
    for (const item of filtered) {
      const label = dayLabel(item.startedAt);
      byDay.set(label, [...(byDay.get(label) ?? []), item]);
    }
    return [...byDay.entries()];
  }, [downloads, query]);

  const finished = downloads.some((item) => item.state !== "inProgress" && item.state !== "paused");

  return (
    <PageShell
      id="downloads-page"
      page="downloads"
      title="Downloads"
      actions={
        <>
          <label id="downloads-page-search" className="flex h-8 w-64 items-center gap-2 rounded-md border border-zinc-700 bg-zinc-900 px-2">
            <Search id="downloads-page-search-icon" className="size-4 text-zinc-500" aria-hidden />
            <input
              id="downloads-page-search-input"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search downloads"
              className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-zinc-500"
            />
          </label>
          <Button id="downloads-page-clear" variant="outline" disabled={!finished} onClick={() => void clearDownloads()}>
            Clear all
          </Button>
        </>
      }
    >
      <div id="downloads-page-list" className="mx-auto flex max-w-3xl flex-col gap-6">
        {groups.length === 0 ? (
          <div id="downloads-page-empty" className="flex flex-col items-center gap-3 py-20 text-sm text-zinc-500">
            <Download id="downloads-page-empty-icon" className="size-8" aria-hidden />
            {query ? "No downloads match that search." : "Files you download appear here."}
          </div>
        ) : (
          groups.map(([label, items]) => (
            <section key={label} id={`downloads-page-day-${label.replace(/\W+/g, "-")}`} className="flex flex-col gap-2">
              <h2 id={`downloads-page-day-${label.replace(/\W+/g, "-")}-title`} className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
                {label}
              </h2>
              {items.map((item) => (
                <DownloadRow key={item.id} item={item} />
              ))}
            </section>
          ))
        )}
      </div>
    </PageShell>
  );
}
