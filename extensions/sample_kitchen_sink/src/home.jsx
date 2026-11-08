import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { explain, hostOf, linksIn, studio } from "./format.js";
import { ActionButton, Brand, Empty, ErrorNote, Icon, Monogram, Stat } from "./ui.jsx";

function Home() {
  const [count, setCount] = useState(0);
  const [links, setLinks] = useState([]);
  const [tabs, setTabs] = useState([]);
  const [downloads, setDownloads] = useState([]);
  const [lastPage, setLastPage] = useState(null);
  const [error, setError] = useState("");

  useEffect(() => {
    document.title = "Studio";
    const ce = studio();
    if (!ce) {
      setError("Studio runs inside the browser.");
      return;
    }
    let live = true;
    const load = () => {
      Promise.all([ce.bookmarks.list(), ce.tabs.list(), ce.downloads.list(), ce.storage.get("lastPage")]).then(
        ([tree, nextTabs, nextDownloads, stored]) => {
          if (!live) return;
          const nextLinks = linksIn(tree);
          setLinks(nextLinks);
          setCount(nextLinks.length);
          setTabs(Array.isArray(nextTabs) ? nextTabs : []);
          setDownloads(Array.isArray(nextDownloads) ? nextDownloads : []);
          setLastPage(stored);
        },
        (reason) => {
          if (live) setError(explain(reason));
        },
      );
    };
    load();
    const stopTabs = ce.tabs.onChanged(load);
    const stopDownloads = ce.downloads.onChanged(load);
    return () => {
      live = false;
      stopTabs();
      stopDownloads();
    };
  }, []);

  const run = (action) => {
    setError("");
    const ce = studio();
    if (!ce) {
      setError("Studio runs inside the browser.");
      return;
    }
    Promise.resolve()
      .then(() => action(ce))
      .catch((reason) => setError(explain(reason)));
  };

  const openTabs = tabs.slice().sort((a, b) => Number(b.active) - Number(a.active));
  const lastTitle = lastPage && lastPage.title ? lastPage.title : "";

  return (
    <div id="studio-home" className="relative h-full overflow-auto bg-zinc-950 text-zinc-100">
      <div id="studio-home-glow" className="pointer-events-none absolute top-0 left-1/2 h-64 w-[36rem] -translate-x-1/2 rounded-full bg-indigo-500/10 blur-3xl" />
      <div id="studio-home-body-wrap" className="relative mx-auto flex w-full max-w-3xl flex-col gap-6 px-5 py-8">
        <Brand id="studio-home-brand" titleId="studio-home-title" title="Studio" large />
        <p id="studio-home-body" className="max-w-xl text-sm leading-6 text-zinc-400">
          This page, the toolbar popup, the side panel, the page toolbar, the bookmarks bar, and the downloads page all belong to one host extension.
        </p>
        <section id="studio-home-stats" className="grid grid-cols-3 gap-2">
          <Stat id="studio-home-bookmarks" label="Bookmarks" valueId="studio-bookmark-count" value={count} />
          <Stat id="studio-home-tabs" label="Tabs" valueId="studio-home-tab-count" value={tabs.length} />
          <Stat id="studio-home-downloads" label="Downloads" valueId="studio-home-download-count" value={downloads.length} />
        </section>
        <aside id="studio-home-last" className="rounded-2xl bg-zinc-900 px-4 py-3 ring-1 ring-zinc-800">
          <p id="studio-home-last-label" className="text-[10px] font-medium tracking-[0.16em] text-zinc-500 uppercase">
            example.com
          </p>
          <p id="studio-home-last-title" className="mt-1 truncate text-sm text-zinc-100">
            {lastTitle || "Open example.com to record a page."}
          </p>
          {lastPage && lastPage.href ? (
            <p id="studio-home-last-href" className="mt-1 truncate text-xs text-zinc-500">
              {lastPage.href}
            </p>
          ) : null}
        </aside>
        <div id="studio-home-columns" className="grid gap-6 md:grid-cols-2">
          <section id="studio-home-tab-section" className="flex flex-col gap-2">
            <h2 id="studio-home-tab-heading" className="px-1 text-[10px] font-medium tracking-[0.16em] text-zinc-500 uppercase">
              Open tabs
            </h2>
            {openTabs.length === 0 ? (
              <Empty id="studio-home-tabs-empty">No tabs are open.</Empty>
            ) : (
              <ul id="studio-home-tab-list" className="flex flex-col gap-1">
                {openTabs.slice(0, 6).map((tab) => {
                  const host = hostOf(tab.url);
                  const title = tab.title || host || "New tab";
                  return (
                    <li id={`studio-home-tab-${tab.id}`} key={tab.id}>
                      <button
                        id={`studio-home-tab-open-${tab.id}`}
                        type="button"
                        onClick={() => run((ce) => ce.tabs.activate(tab.id))}
                        className="flex w-full items-center gap-3 rounded-xl px-2 py-2 text-left hover:bg-zinc-900"
                      >
                        <Monogram id={`studio-home-tab-mark-${tab.id}`} label={host || title} />
                        <span id={`studio-home-tab-copy-${tab.id}`} className="min-w-0">
                          <span id={`studio-home-tab-name-${tab.id}`} className="block truncate text-sm font-medium text-zinc-100">
                            {title}
                          </span>
                          <span id={`studio-home-tab-host-${tab.id}`} className="block truncate text-[11px] text-zinc-500">
                            {host || "No address"}
                          </span>
                        </span>
                      </button>
                    </li>
                  );
                })}
              </ul>
            )}
          </section>
          <section id="studio-home-bookmark-section" className="flex flex-col gap-2">
            <h2 id="studio-home-bookmark-heading" className="px-1 text-[10px] font-medium tracking-[0.16em] text-zinc-500 uppercase">
              Bookmarks
            </h2>
            {links.length === 0 ? (
              <Empty id="studio-home-bookmarks-empty">No bookmarks yet.</Empty>
            ) : (
              <ul id="studio-home-bookmark-list" className="flex flex-col gap-1">
                {links.slice(0, 6).map((link) => {
                  const host = hostOf(link.url);
                  return (
                    <li id={`studio-home-bookmark-${link.id}`} key={link.id}>
                      <button
                        id={`studio-home-bookmark-open-${link.id}`}
                        type="button"
                        onClick={() => run((ce) => ce.tabs.create(link.url))}
                        className="flex w-full items-center gap-3 rounded-xl px-2 py-2 text-left hover:bg-zinc-900"
                      >
                        <Monogram id={`studio-home-bookmark-mark-${link.id}`} label={host || link.title} />
                        <span id={`studio-home-bookmark-copy-${link.id}`} className="min-w-0">
                          <span id={`studio-home-bookmark-name-${link.id}`} className="block truncate text-sm font-medium text-zinc-100">
                            {link.title || host}
                          </span>
                          <span id={`studio-home-bookmark-host-${link.id}`} className="block truncate text-[11px] text-zinc-500">
                            {host}
                          </span>
                        </span>
                      </button>
                    </li>
                  );
                })}
              </ul>
            )}
          </section>
        </div>
        <section id="studio-home-surfaces" className="flex flex-col gap-2">
          <h2 id="studio-home-surfaces-heading" className="px-1 text-[10px] font-medium tracking-[0.16em] text-zinc-500 uppercase">
            Open a surface
          </h2>
          <div id="studio-home-actions" className="grid gap-2 sm:grid-cols-2">
            <ActionButton
              id="studio-home-open-panel"
              primary
              icon={<Icon id="studio-home-open-panel-glyph" name="panel" />}
              label="Side panel"
              hint="Switch tabs without leaving the page"
              onClick={() => run((ce) => ce.ui.togglePanel())}
            />
            <ActionButton
              id="studio-home-open-popout"
              icon={<Icon id="studio-home-open-popout-glyph" name="window" />}
              label="Window"
              hint="Studio in its own window"
              onClick={() => run((ce) => ce.ui.openPopout())}
            />
            <ActionButton
              id="studio-home-open-downloads"
              icon={<Icon id="studio-home-open-downloads-glyph" name="download" />}
              label="Downloads"
              hint="Files saved by the browser"
              onClick={() => run((ce) => ce.tabs.create("ce://downloads"))}
            />
          </div>
        </section>
        <ErrorNote id="studio-home-error">{error}</ErrorNote>
      </div>
    </div>
  );
}

createRoot(document.getElementById("root")).render(<Home />);
