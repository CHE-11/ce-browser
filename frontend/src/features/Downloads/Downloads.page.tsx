import { useMemo, useState } from "react";
import { Download, Search, SearchX } from "lucide-react";
import { Button } from "@components/Button";
import { EmptyState } from "@components/EmptyState";
import { SkeletonRows } from "@components/Skeleton";
import { PageShell } from "@components/PageShell";
import { clearDownloads } from "@lib/browser-client";
import { useDownloadsState } from "@lib/use-browser";
import { DownloadRow } from "./DownloadRow";

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
  const { downloads, loaded } = useDownloadsState();
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
        {!loaded ? (
          <SkeletonRows id="downloads-page-loading" rows={5} icon />
        ) : groups.length === 0 ? (
          query ? (
            <EmptyState id="downloads-page-empty" icon={SearchX} title="No downloads match that search" hint="Search looks at file names and addresses." />
          ) : (
            <EmptyState id="downloads-page-empty" icon={Download} title="No downloads yet" hint="Files you download appear here, with their progress and where they were saved." />
          )
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
