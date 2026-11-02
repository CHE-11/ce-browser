import { useEffect, useState } from "react";
import { Search } from "lucide-react";
import { FavIcon } from "@/components/FavIcon";
import { PopupFrame } from "@/components/PopupFrame";
import { SkeletonRows } from "@/components/Skeleton";
import { activateTab, closePopup, findInPage, findInTabs } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import { hostOf } from "@/lib/format";
import type { FindTabResult } from "@type/browser";

/** "Find in all tabs": how many matches each tab in this workspace has. Choosing one jumps there. */
export function FindAllPopover({ params }: { params: URLSearchParams }) {
  const query = params.get("query") ?? "";
  const caseSensitive = params.get("case") === "1";
  const wholeWord = params.get("word") === "1";
  const [results, setResults] = useState<FindTabResult[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    findInTabs(query, caseSensitive, wholeWord)
      .then((found) => {
        if (!cancelled) {
          setResults(found);
        }
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setError(String(reason));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [caseSensitive, query, wholeWord]);

  const sorted = (results ?? []).slice().sort((left, right) => (right.count ?? -1) - (left.count ?? -1));
  const total = sorted.reduce((sum, item) => sum + (item.count ?? 0), 0);

  const open = (tab: FindTabResult) => {
    void activateTab(tab.id)
      .then(() => findInPage(tab.id, query, false, { caseSensitive, wholeWord, step: false }))
      .finally(() => closePopup());
  };

  return (
    <PopupFrame id="find-all" width={380} maxHeight={480}>
      <header id="find-all-header" className="flex items-center gap-2 px-4 pb-2 pt-3">
        <Search id="find-all-icon" className="size-4 text-zinc-400" aria-hidden />
        <h2 id="find-all-title" className="min-w-0 flex-1 truncate text-sm font-semibold">
          “{query}” in all tabs
        </h2>
        {results ? (
          <span id="find-all-total" className="shrink-0 text-xs text-zinc-500">
            {total} {total === 1 ? "match" : "matches"}
          </span>
        ) : null}
      </header>
      <div id="find-all-body" className="px-1.5 pb-1.5">
        {error ? (
          <p id="find-all-error" className="px-2.5 py-2 text-xs text-red-300">
            {error}
          </p>
        ) : !results ? (
          <SkeletonRows id="find-all-loading" rows={4} icon className="px-2.5 py-2" />
        ) : sorted.length === 0 ? (
          <p id="find-all-empty" className="px-2.5 py-3 text-xs text-zinc-500">
            There are no tabs in this workspace.
          </p>
        ) : (
          <ul id="find-all-list" className="flex flex-col gap-0.5">
            {sorted.map((tab) => {
              const searchable = tab.count !== null;
              return (
                <li id={`find-all-${tab.id}`} key={tab.id}>
                  <button
                    id={`find-all-${tab.id}-open`}
                    type="button"
                    disabled={!searchable || tab.count === 0}
                    className={cn(
                      "flex h-10 w-full items-center gap-3 rounded-lg px-2.5 text-left text-sm hover:bg-zinc-800 disabled:opacity-50 disabled:hover:bg-transparent",
                      focusRow,
                    )}
                    onClick={() => open(tab)}
                  >
                    <FavIcon id={`find-all-${tab.id}-icon`} url={tab.url} />
                    <span id={`find-all-${tab.id}-text`} className="min-w-0 flex-1">
                      <span id={`find-all-${tab.id}-title`} className="block truncate">
                        {tab.title || tab.url}
                      </span>
                      <span id={`find-all-${tab.id}-host`} className="block truncate text-xs text-zinc-500">
                        {hostOf(tab.url)}
                      </span>
                    </span>
                    <span
                      id={`find-all-${tab.id}-count`}
                      className={cn("shrink-0 text-xs tabular-nums", tab.count ? "text-sky-300" : "text-zinc-500")}
                    >
                      {searchable ? tab.count : tab.skipped}
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </PopupFrame>
  );
}
