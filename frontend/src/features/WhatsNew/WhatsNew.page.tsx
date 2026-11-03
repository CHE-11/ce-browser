import { useEffect, useState } from "react";
import { ExternalLink, PartyPopper } from "lucide-react";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { SkeletonRows } from "@/components/Skeleton";
import { createTab, releaseNotes } from "@/lib/browser-client";
import { useAppearanceFromSettings, useUpdateStatus } from "@/lib/use-browser";
import type { ReleaseNotes } from "@type/browser";
import { ReleaseMarkdown } from "./ReleaseMarkdown";

type WhatsNewPageProps = {
  params: URLSearchParams;
};

/** Release notes for the version that is now running. */
export function WhatsNewPage({ params }: WhatsNewPageProps) {
  useAppearanceFromSettings();
  const update = useUpdateStatus();
  const version = params.get("version") || update?.current || "";
  const from = params.get("from") ?? "";
  const [notes, setNotes] = useState<ReleaseNotes | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    document.title = "What's new";
  }, []);

  useEffect(() => {
    if (!version) {
      return;
    }
    let cancelled = false;
    setError(null);
    releaseNotes(version)
      .then((next) => {
        if (!cancelled) {
          setNotes(next);
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
  }, [version]);

  return (
    <main id="whats-new-page" className="h-full overflow-y-auto bg-zinc-950 text-zinc-100">
      <div id="whats-new-body" className="mx-auto flex max-w-2xl flex-col gap-6 px-6 py-10">
        <header id="whats-new-header" className="flex items-start gap-4">
          <span id="whats-new-mark" className="inline-flex size-11 shrink-0 items-center justify-center rounded-xl bg-sky-500/15 text-sky-300">
            <PartyPopper id="whats-new-mark-icon" className="size-5" aria-hidden />
          </span>
          <div id="whats-new-heading" className="min-w-0 flex-1">
            <h1 id="whats-new-title" className="text-xl font-semibold">
              {version ? `What's new in ce-browser ${version}` : "What's new"}
            </h1>
            <p id="whats-new-subtitle" className="mt-1 text-sm text-zinc-500">
              {from ? `Updated from ${from}. Your tabs, bookmarks and settings came with you.` : "Release notes for this version."}
            </p>
          </div>
          {notes?.url ? (
            <Button id="whats-new-release" size="sm" variant="ghost" onClick={() => void createTab(notes.url)}>
              <ExternalLink id="whats-new-release-icon" className="size-3.5" aria-hidden />
              Release page
            </Button>
          ) : null}
        </header>
        <section id="whats-new-notes" className="rounded-lg border border-zinc-800 bg-zinc-900/60 p-6">
          {error ? (
            <EmptyState id="whats-new-error" icon={PartyPopper} title="The release notes couldn't load" hint={error} />
          ) : !notes ? (
            <SkeletonRows id="whats-new-loading" rows={6} />
          ) : notes.body.trim() ? (
            <ReleaseMarkdown id="whats-new-markdown" markdown={notes.body} />
          ) : (
            <EmptyState id="whats-new-empty" icon={PartyPopper} title="No notes for this release" hint="The release page may list the changes." />
          )}
        </section>
      </div>
    </main>
  );
}
