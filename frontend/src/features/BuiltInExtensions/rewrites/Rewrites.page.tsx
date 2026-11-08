import { useEffect, useState } from "react";
import { Route } from "lucide-react";
import { listRewriteRules, saveRewriteRules } from "@/lib/browser-client";
import { useAppearanceFromSettings } from "@/lib/use-browser";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { SkeletonRows } from "@/components/Skeleton";
import type { RewriteRule } from "@type/browser";

const EMPTY: RewriteRule = { id: 0, enabled: true, contains: "", action: "block", target: "" };

export function RewritesPage() {
  const ready = useAppearanceFromSettings();
  const [rules, setRules] = useState<RewriteRule[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [draft, setDraft] = useState<RewriteRule>(EMPTY);
  const [error, setError] = useState("");

  useEffect(() => {
    document.title = "Request rules";
    void listRewriteRules()
      .then(setRules)
      .catch((reason: unknown) => setError(String(reason)))
      .finally(() => setLoaded(true));
  }, []);

  const save = async (next: RewriteRule[]) => {
    setError("");
    try {
      setRules(await saveRewriteRules(next));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Couldn't save the rules.");
    }
  };

  if (!ready) {
    return null;
  }

  return (
    <div id="rewrites-page" className="mx-auto flex h-full max-w-3xl flex-col gap-4 overflow-auto bg-zinc-950 p-8 text-zinc-100">
      <h1 id="rewrites-title" className="text-xl font-semibold">
        Request rules
      </h1>
      <p id="rewrites-hint" className="text-sm text-zinc-500">
        When a request URL contains the text, it is blocked, sent to another address, or answered with a local file.
      </p>
      <form
        id="rewrites-form"
        className="grid grid-cols-[1fr_8rem_1fr_auto] gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          if (!draft.contains.trim()) {
            return;
          }
          void save([...rules, { ...draft, id: 0 }]);
          setDraft(EMPTY);
        }}
      >
        <input
          id="rewrites-contains"
          value={draft.contains}
          placeholder="URL contains"
          className="h-8 rounded-md border border-zinc-700 bg-zinc-900 px-2 text-sm"
          onChange={(event) => setDraft({ ...draft, contains: event.target.value })}
        />
        <select
          id="rewrites-action"
          value={draft.action}
          className="h-8 rounded-md border border-zinc-700 bg-zinc-900 px-2 text-sm"
          onChange={(event) => setDraft({ ...draft, action: event.target.value as RewriteRule["action"] })}
        >
          <option id="rewrites-action-block" value="block">
            Block
          </option>
          <option id="rewrites-action-redirect" value="redirect">
            Redirect
          </option>
          <option id="rewrites-action-file" value="file">
            Local file
          </option>
        </select>
        <input
          id="rewrites-target"
          value={draft.target}
          placeholder={draft.action === "file" ? "C:\\path\\file.js" : "https://example.com/new.js"}
          disabled={draft.action === "block"}
          className="h-8 rounded-md border border-zinc-700 bg-zinc-900 px-2 text-sm disabled:opacity-40"
          onChange={(event) => setDraft({ ...draft, target: event.target.value })}
        />
        <Button id="rewrites-add" type="submit" variant="primary">
          Add
        </Button>
      </form>
      {error ? (
        <p id="rewrites-error" className="text-sm text-amber-300">
          {error}
        </p>
      ) : null}
      <ul id="rewrites-list" className="flex flex-col gap-2">
        {!loaded ? (
          <li id="rewrites-loading">
            <SkeletonRows id="rewrites-loading-rows" rows={3} />
          </li>
        ) : rules.length === 0 ? (
          <li id="rewrites-empty">
            <EmptyState
              id="rewrites-empty-state"
              icon={Route}
              title="No request rules yet"
              hint="Add one above to block a URL, send it somewhere else, or answer it with a file on this computer."
              className="rounded-lg border border-dashed border-zinc-800"
            />
          </li>
        ) : (
          rules.map((rule) => (
            <li id={`rewrites-rule-${rule.id}`} key={rule.id} className="flex items-center gap-3 rounded-lg bg-zinc-900 px-3 py-2">
              <input
                id={`rewrites-enabled-${rule.id}`}
                type="checkbox"
                checked={rule.enabled}
                onChange={(event) => void save(rules.map((item) => (item.id === rule.id ? { ...item, enabled: event.target.checked } : item)))}
              />
              <span id={`rewrites-label-${rule.id}`} className="min-w-0 flex-1 truncate text-sm">
                {rule.contains} · {rule.action}
                {rule.target ? ` · ${rule.target}` : ""}
              </span>
              <Button id={`rewrites-delete-${rule.id}`} onClick={() => void save(rules.filter((item) => item.id !== rule.id))}>
                Delete
              </Button>
            </li>
          ))
        )}
      </ul>
    </div>
  );
}
