import { useEffect } from "react";
import { X } from "lucide-react";
import type { FindResult } from "@type/browser";
import { cn } from "../lib/cn";
import { focusRow } from "../lib/focus-ring";
import { IconButton } from "./IconButton";

type FindResultsPanelProps = {
  query: string;
  result: FindResult | null;
  onClose: () => void;
  onSelect: (index: number) => void;
};

/** Match list beside the page: where each hit sits, and the words around it. */
export function FindResultsPanel({ query, result, onClose, onSelect }: FindResultsPanelProps) {
  const hits = result?.hits ?? [];
  const count = result?.count ?? 0;
  const current = result?.index ?? -1;
  const first = hits[0]?.index ?? 0;
  const last = hits[hits.length - 1]?.index ?? -1;
  const partial = hits.length > 0 && (first > 0 || last < count - 1);

  useEffect(() => {
    if (current < 0) {
      return;
    }
    document.getElementById(`find-result-${current}`)?.scrollIntoView({ block: "nearest" });
  }, [current, hits]);

  return (
    <aside id="find-results" className="flex h-full w-96 shrink-0 flex-col border-l border-zinc-800 bg-zinc-950">
      <header id="find-results-header" className="flex h-11 shrink-0 items-center gap-2 border-b border-zinc-800 px-3">
        <h2 id="find-results-title" className="min-w-0 flex-1 truncate text-sm font-semibold">
          Results
        </h2>
        {query && result ? (
          <span id="find-results-count" className="shrink-0 text-xs tabular-nums text-zinc-500">
            {count}
            {count >= 2000 ? "+" : ""}
          </span>
        ) : null}
        <IconButton id="find-results-close" label="Close results" onClick={onClose}>
          <X id="find-results-close-icon" aria-hidden />
        </IconButton>
      </header>
      <div id="find-results-body" className="min-h-0 flex-1 overflow-y-auto px-2 py-2">
        {!query ? (
          <p id="find-results-empty" className="px-2 py-3 text-xs leading-5 text-zinc-500">
            Type in the find bar to list matches on this page.
          </p>
        ) : !result ? (
          <p id="find-results-loading" className="px-2 py-3 text-xs text-zinc-500">
            Searching…
          </p>
        ) : count === 0 ? (
          <p id="find-results-none" className="px-2 py-3 text-xs leading-5 text-zinc-500">
            No matches for “{query}”.
          </p>
        ) : (
          <ul id="find-results-items" className="flex flex-col gap-1">
            {hits.map((hit) => {
              const selected = hit.index === current;
              return (
                <li id={`find-result-${hit.index}`} key={hit.index}>
                  <button
                    id={`find-result-${hit.index}-open`}
                    type="button"
                    aria-current={selected ? "true" : undefined}
                    className={cn(
                      "flex w-full flex-col gap-1 rounded-lg border px-2.5 py-2 text-left",
                      selected ? "border-orange-500 bg-orange-500/10 ring-2 ring-orange-500" : "border-zinc-800 hover:border-zinc-600 hover:bg-zinc-900",
                      focusRow,
                    )}
                    onClick={() => onSelect(hit.index)}
                  >
                    <p id={`find-result-${hit.index}-text`} className="text-sm leading-5 text-zinc-300">
                      <span id={`find-result-${hit.index}-before`}>{hit.before}</span>
                      <mark
                        id={`find-result-${hit.index}-match`}
                        className={cn("rounded-sm px-0.5 text-zinc-950", selected ? "bg-orange-500" : "bg-yellow-300")}
                      >
                        {hit.text}
                      </mark>
                      <span id={`find-result-${hit.index}-after`}>{hit.after}</span>
                    </p>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
        {partial ? (
          <p id="find-results-more" className="px-2 py-2 text-xs text-zinc-500">
            Showing {first + 1}–{last + 1} of {count}
            {count >= 2000 ? "+" : ""}
          </p>
        ) : null}
      </div>
    </aside>
  );
}
