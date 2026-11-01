import { useEffect, useState } from "react";
import { extractArticle, removeClutter } from "@features/BuiltInExtensions/print-pdf/cleanup";
import { readerPage, showReaderOriginal } from "@/lib/browser-client";
import { useAppearanceFromSettings } from "@/lib/use-browser";
import type { ExtractedPage } from "@type/browser";

const KINDS = ["ads", "navigation", "popups", "social"] as const;

function articleHtml(page: ExtractedPage) {
  const doc = new DOMParser().parseFromString(page.html, "text/html");
  for (const kind of KINDS) {
    removeClutter(doc, kind);
  }
  const article = extractArticle(doc);
  return article?.innerHTML || doc.body?.innerHTML || page.html;
}

export function ReaderPage() {
  useAppearanceFromSettings();
  const [page, setPage] = useState<ExtractedPage | null | undefined>(undefined);

  useEffect(() => {
    document.title = "Reader";
    void readerPage()
      .then((next) => {
        setPage(next);
        if (next?.title) {
          document.title = `${next.title} - Reader`;
        }
      })
      .catch(() => setPage(null));
  }, []);

  return (
    <main id="reader-page" className="h-full overflow-y-auto bg-zinc-950 text-zinc-100">
      <article id="reader-article" className="mx-auto flex max-w-2xl flex-col gap-4 px-6 py-8">
        <header id="reader-banner" className="flex items-center justify-between gap-3 border-b border-zinc-800 pb-3">
          <div id="reader-source" className="min-w-0">
            <p id="reader-kicker" className="text-xs font-medium uppercase tracking-wide text-sky-400">
              Reader
            </p>
            <p id="reader-url" className="truncate text-xs text-zinc-500">
              {page?.url || ""}
            </p>
          </div>
          <button
            id="reader-original"
            type="button"
            className="h-8 shrink-0 rounded-md border border-zinc-700 px-3 text-xs hover:bg-zinc-800"
            onClick={() => void showReaderOriginal()}
          >
            Show original
          </button>
        </header>
        {page === undefined ? (
          <p id="reader-loading" className="text-sm text-zinc-500">
            Reading the page
          </p>
        ) : page === null ? (
          <p id="reader-empty" className="text-sm text-zinc-500">
            Nothing to read. Open reader mode from a page first.
          </p>
        ) : (
          <>
            <h1 id="reader-title" className="text-3xl font-semibold leading-tight">
              {page.title}
            </h1>
            {page.byline || page.siteName ? (
              <p id="reader-byline" className="text-sm text-zinc-400">
                {[page.byline, page.siteName].filter(Boolean).join(" · ")}
              </p>
            ) : null}
            <div
              id="reader-body"
              className="flex flex-col gap-4 text-lg leading-8 text-zinc-200 [&_a]:text-sky-400 [&_h2]:mt-4 [&_h2]:text-2xl [&_h2]:font-semibold [&_img]:my-2 [&_img]:max-w-full [&_pre]:overflow-x-auto [&_pre]:rounded-lg [&_pre]:bg-zinc-900 [&_pre]:p-3 [&_pre]:text-sm"
              dangerouslySetInnerHTML={{ __html: articleHtml(page) }}
            />
          </>
        )}
      </article>
    </main>
  );
}
