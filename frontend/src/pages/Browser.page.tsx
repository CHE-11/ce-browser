import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { AddressBar } from "../components/Addressbar/AddressBar";
import { BookmarksBar } from "../components/BookmarksBar/BookmarksBar";
import { FindBar } from "../components/FindBar";
import { SyncToast } from "../components/SyncToast";
import { TabStrip } from "../components/TabStrip";
import { onBrowserCommand, setUiHeight } from "../lib/browser-client";
import { useAppearanceFromSettings, useBookmarks, useDownloads, useExtensions, useHistory, useTabs } from "../lib/use-browser";

export function BrowserPage() {
  const snapshot = useTabs();
  const bookmarks = useBookmarks();
  const downloads = useDownloads();
  const history = useHistory();
  const { extensions } = useExtensions();
  const settings = useAppearanceFromSettings();
  const headerRef = useRef<HTMLDivElement>(null);
  const [findToken, setFindToken] = useState<number | null>(null);

  const activeTab = snapshot.tabs.find((tab) => tab.id === snapshot.activeId);

  useEffect(() => {
    document.title = activeTab ? `${activeTab.title || activeTab.url} - ce-browser` : "ce-browser";
  }, [activeTab]);

  useEffect(() => {
    const unlisten = onBrowserCommand((command) => {
      if (command === "find") {
        setFindToken((token) => (token ?? 0) + 1);
      }
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  useLayoutEffect(() => {
    const header = headerRef.current;
    if (!header) {
      return;
    }
    const report = () => void setUiHeight(Math.ceil(header.getBoundingClientRect().height));
    report();
    const observer = new ResizeObserver(report);
    observer.observe(header);
    return () => observer.disconnect();
  }, []);

  const showBookmarksBar = settings?.showBookmarksBar ?? true;

  return (
    <div id="browser" className="flex h-full flex-col bg-zinc-950 text-zinc-100">
      <div id="browser-header" ref={headerRef} className={snapshot.fullscreen ? "hidden" : "flex flex-col"}>
        <TabStrip snapshot={snapshot} />
        <AddressBar
          snapshot={snapshot}
          bookmarks={bookmarks}
          downloads={downloads}
          history={history}
          extensions={extensions}
          settings={settings}
        />
        {showBookmarksBar ? <BookmarksBar tree={bookmarks} activeId={snapshot.activeId} /> : null}
        {findToken !== null && activeTab ? (
          <FindBar tabId={activeTab.id} focusToken={findToken} onClose={() => setFindToken(null)} />
        ) : null}
        <SyncToast />
      </div>
      <div id="browser-page-area" className="flex flex-1 items-center justify-center bg-zinc-950">
        {activeTab && !activeTab.loaded ? (
          <span id="browser-page-loading" className="text-sm text-zinc-500">
            Loading {activeTab.title || activeTab.url}
          </span>
        ) : null}
      </div>
    </div>
  );
}
