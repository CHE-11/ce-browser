import { useEffect, useMemo, useState } from "react";
import type { PageScript, ScriptLanguage } from "@type/browser";
import { SkeletonRows } from "@/components/Skeleton";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import { useAppearanceFromSettings, useScripting } from "@/lib/use-browser";

type ScriptsPageProps = {
  params: URLSearchParams;
};

const LANGUAGE_LABEL: Record<ScriptLanguage, string> = {
  javascript: "JavaScript",
  typescript: "TypeScript",
  python: "Python",
  css: "CSS",
};

function matchesQuery(script: PageScript, query: string) {
  if (!query) {
    return true;
  }
  const haystack = `${script.name} ${script.domain} ${script.language} ${script.source}`.toLowerCase();
  return haystack.includes(query.toLowerCase());
}

/** Every script saved by the Scripting panel. */
export function ScriptsPage({ params }: ScriptsPageProps) {
  useAppearanceFromSettings();
  const file = useScripting();
  const host = params.get("host") ?? "";
  const [query, setQuery] = useState(host);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const scripts = useMemo(
    () => (file?.scripts ?? [])
      .filter((script) => matchesQuery(script, query.trim()))
      .slice()
      .sort((left, right) => left.domain.localeCompare(right.domain) || left.name.localeCompare(right.name)),
    [file, query],
  );
  const selected = scripts.find((script) => script.id === selectedId) ?? scripts[0] ?? null;

  useEffect(() => {
    document.title = "Scripts";
  }, []);

  return (
    <main id="scripts-page" className="flex h-full bg-zinc-950 text-zinc-100">
      <aside id="scripts-list" className="flex w-72 shrink-0 flex-col gap-2 border-r border-zinc-800 p-3">
        <div id="scripts-list-title" className="px-2 text-sm font-semibold text-zinc-300">
          Scripts
        </div>
        <input
          id="scripts-filter"
          className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
          value={query}
          placeholder="Filter by name, site, or code"
          aria-label="Filter scripts"
          spellCheck={false}
          onChange={(event) => setQuery(event.target.value)}
        />
        <div id="scripts-list-items" className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto">
          {file === null ? <SkeletonRows id="scripts-loading" rows={4} className="px-2 py-1" /> : null}
          {file && scripts.length === 0 ? (
            <p id="scripts-empty" className="px-2 py-3 text-xs leading-relaxed text-zinc-500">
              {file.scripts.length === 0
                ? "No scripts yet. Open the Scripting panel beside a page to write one."
                : "Nothing matches this filter."}
            </p>
          ) : null}
          {scripts.map((script) => (
            <button
              id={`scripts-item-${script.id}`}
              key={script.id}
              type="button"
              aria-current={selected?.id === script.id ? "true" : undefined}
              className={cn("flex h-12 flex-col justify-center rounded-md px-2 text-left hover:bg-zinc-800", focusRow, selected?.id === script.id && "bg-zinc-800")}
              onClick={() => setSelectedId(script.id)}
            >
              <span id={`scripts-item-${script.id}-name`} className="flex items-center gap-2 text-sm">
                <span
                  id={`scripts-item-${script.id}-state`}
                  className={script.enabled ? "size-1.5 rounded-full bg-sky-400" : "size-1.5 rounded-full bg-zinc-600"}
                />
                <span id={`scripts-item-${script.id}-label`} className="min-w-0 flex-1 truncate">
                  {script.name || "Script"}
                </span>
              </span>
              <span id={`scripts-item-${script.id}-meta`} className="truncate pl-3.5 text-[11px] text-zinc-500">
                {script.domain} · {LANGUAGE_LABEL[script.language]}
              </span>
            </button>
          ))}
        </div>
      </aside>
      <section id="scripts-view" className="flex min-w-0 flex-1 flex-col gap-3 overflow-y-auto p-6">
        {selected ? (
          <>
            <div id="scripts-view-heading" className="flex flex-wrap items-center justify-between gap-3">
              <h1 id="scripts-view-title" className="text-lg font-semibold">
                {selected.name || "Script"}
              </h1>
              <p id="scripts-view-meta" className="text-xs text-zinc-500">
                {selected.domain} · {LANGUAGE_LABEL[selected.language]} · {selected.enabled ? "On" : "Off"}
                {selected.runOnLoad ? " · Runs when the site loads" : ""}
              </p>
            </div>
            <p id="scripts-hint" className="text-xs text-zinc-500">
              Open the Scripting panel beside this site to edit the script.
            </p>
            <pre id="scripts-source" className="min-h-40 flex-1 overflow-auto rounded-md border border-zinc-800 bg-zinc-950 p-3 font-mono text-xs leading-5 whitespace-pre-wrap text-zinc-200">
              {selected.source || "This script is empty."}
            </pre>
          </>
        ) : (
          <p id="scripts-view-empty" className="m-auto max-w-sm text-center text-sm text-zinc-500">
            Scripts you save in the Scripting panel show up here, for every site.
          </p>
        )}
      </section>
    </main>
  );
}
