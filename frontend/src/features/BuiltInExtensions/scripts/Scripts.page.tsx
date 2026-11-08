import { useEffect, useState } from "react";
import { Trash2 } from "lucide-react";
import { Button } from "@/components/Button";
import { SkeletonRows } from "@/components/Skeleton";
import { Toggle } from "@/components/Toggle";
import { deleteUserScript, saveUserScript } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import { useAppearanceFromSettings, usePageRules } from "@/lib/use-browser";
import type { UserScript } from "@type/browser";

type ScriptsPageProps = {
  params: URLSearchParams;
};

const fieldClass =
  "w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-sm text-zinc-100 outline-none focus:border-sky-500";

function blank(host: string): UserScript {
  return { id: "", name: "", enabled: true, hosts: host ? [host] : [], css: "", js: "" };
}

export function ScriptsPage({ params }: ScriptsPageProps) {
  useAppearanceFromSettings();
  const rules = usePageRules();
  const host = params.get("host") ?? "";
  const [draft, setDraft] = useState<UserScript>(() => blank(host));
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    document.title = "Scripts";
  }, []);

  const save = () => {
    setError(null);
    const hosts = draft.hosts
      .flatMap((item) => item.split(/[\s,]+/))
      .map((item) => item.trim())
      .filter(Boolean);
    const before = new Set((rules?.scripts ?? []).map((script) => script.id));
    void saveUserScript({ ...draft, hosts })
      .then((next) => {
        const created = next.scripts.find((script) => !before.has(script.id));
        if (created) {
          setDraft(created);
        }
      })
      .catch((reason: unknown) => setError(String(reason)));
  };

  return (
    <main id="scripts-page" className="flex h-full bg-zinc-950 text-zinc-100">
      <aside id="scripts-list" className="flex w-64 shrink-0 flex-col gap-1 border-r border-zinc-800 p-3">
        <div id="scripts-list-title" className="px-2 pb-2 text-sm font-semibold text-zinc-300">
          Scripts
        </div>
        <button
          id="scripts-new"
          type="button"
          className={cn("h-8 rounded-md px-2 text-left text-sm text-sky-300 hover:bg-zinc-800", focusRow)}
          onClick={() => setDraft(blank(host))}
        >
          New script
        </button>
        {rules === null ? <SkeletonRows id="scripts-loading" rows={3} className="px-2 py-1" /> : null}
        {rules && rules.scripts.length === 0 ? (
          <p id="scripts-empty" className="px-2 py-3 text-xs leading-relaxed text-zinc-500">
            No scripts yet. A script adds CSS or JavaScript to the sites you choose.
          </p>
        ) : null}
        {(rules?.scripts ?? []).map((script) => (
          <button
            id={`scripts-item-${script.id}`}
            key={script.id}
            type="button"
            aria-current={draft.id === script.id ? "true" : undefined}
            className={cn("flex h-9 items-center gap-2 rounded-md px-2 text-left text-sm hover:bg-zinc-800", focusRow, draft.id === script.id && "bg-zinc-800")}
            onClick={() => setDraft(script)}
          >
            <span
              id={`scripts-item-${script.id}-state`}
              className={script.enabled ? "size-1.5 rounded-full bg-sky-400" : "size-1.5 rounded-full bg-zinc-600"}
            />
            <span id={`scripts-item-${script.id}-name`} className="min-w-0 flex-1 truncate">
              {script.name || "Script"}
            </span>
          </button>
        ))}
      </aside>
      <section id="scripts-editor" className="flex min-w-0 flex-1 flex-col gap-3 overflow-y-auto p-6">
        <div id="scripts-editor-heading" className="flex items-center justify-between gap-3">
          <h1 id="scripts-editor-title" className="text-lg font-semibold">
            {draft.id ? draft.name || "Script" : "New script"}
          </h1>
          <div id="scripts-editor-actions" className="flex items-center gap-2">
            <Toggle id="scripts-enabled" checked={draft.enabled} onChange={(enabled) => setDraft({ ...draft, enabled })} aria-label="Enabled" />
            {draft.id ? (
              <Button
                id="scripts-delete"
                size="sm"
                variant="ghost"
                onClick={() => {
                  void deleteUserScript(draft.id)
                    .then(() => setDraft(blank(host)))
                    .catch((reason: unknown) => setError(String(reason)));
                }}
              >
                <Trash2 id="scripts-delete-icon" className="size-3.5" aria-hidden />
                Delete
              </Button>
            ) : null}
            <Button id="scripts-save" size="sm" variant="primary" onClick={save}>
              Save
            </Button>
          </div>
        </div>
        <p id="scripts-hint" className="text-xs text-zinc-500">
          CSS and JS run on matching sites when the page loads. Leave the sites blank to run everywhere. An open page picks
          up a save immediately.
        </p>
        <label id="scripts-name" className="flex flex-col gap-1 text-sm">
          Name
          <input
            id="scripts-name-input"
            className={fieldClass}
            value={draft.name}
            maxLength={80}
            onChange={(event) => setDraft({ ...draft, name: event.target.value })}
          />
        </label>
        <label id="scripts-hosts" className="flex flex-col gap-1 text-sm">
          Sites
          <input
            id="scripts-hosts-input"
            className={fieldClass}
            value={draft.hosts.join(", ")}
            placeholder="example.com, news.example.com"
            spellCheck={false}
            onChange={(event) => setDraft({ ...draft, hosts: [event.target.value] })}
          />
        </label>
        <label id="scripts-css" className="flex min-h-0 flex-1 flex-col gap-1 text-sm">
          CSS
          <textarea
            id="scripts-css-input"
            className={`${fieldClass} min-h-28 flex-1 font-mono`}
            value={draft.css}
            spellCheck={false}
            onChange={(event) => setDraft({ ...draft, css: event.target.value })}
          />
        </label>
        <label id="scripts-js" className="flex min-h-0 flex-1 flex-col gap-1 text-sm">
          JavaScript
          <textarea
            id="scripts-js-input"
            className={`${fieldClass} min-h-40 flex-1 font-mono`}
            value={draft.js}
            spellCheck={false}
            onChange={(event) => setDraft({ ...draft, js: event.target.value })}
          />
        </label>
        {error ? (
          <p id="scripts-error" className="text-sm text-red-300">
            {error}
          </p>
        ) : null}
      </section>
    </main>
  );
}
