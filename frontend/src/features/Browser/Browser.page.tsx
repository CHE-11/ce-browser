import { FindBar } from "@/components/FindBar";
import { SplitDivider } from "@/components/SplitDivider";
import { SyncToast } from "@/components/SyncToast";
import { AddressBar } from "@/components/TabsAddressAndBookmarks/Addressbar/AddressBar";
import { BookmarksBar } from "@/components/TabsAddressAndBookmarks/BookmarksBar/BookmarksBar";
import { TabStrip } from "@/components/TabsAddressAndBookmarks/Tabs/TabStrip";
import { VerticalTabs } from "@/components/TabsAddressAndBookmarks/Tabs/VerticalTabs";
import { onBrowserCommand, setUiInsets } from "@/lib/browser-client";
import { useTabs, useBookmarks, useDownloads, useHistory, useExtensions, useAppearanceFromSettings } from "@/lib/use-browser";
import { useRef, useState, useEffect, useLayoutEffect } from "react";


export function BrowserPage() {
  const snapshot = useTabs();
  const bookmarks = useBookmarks();
  const downloads = useDownloads();
  const history = useHistory();
  const { extensions } = useExtensions();
  const settings = useAppearanceFromSettings();
  const headerRef = useRef<HTMLDivElement>(null);
  const sidebarRef = useRef<HTMLElement>(null);
  const [findToken, setFindToken] = useState<number | null>(null);

  const activeTab = snapshot.tabs.find((tab) => tab.id === snapshot.activeId);
  const vertical = settings?.tabLayout === "vertical";
  const split = snapshot.split;
  const splitShown =
    !!split && !snapshot.fullscreen && (snapshot.activeId === split.left || snapshot.activeId === split.right);

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
    const report = () => {
      const top = Math.ceil(header.getBoundingClientRect().height);
      const left = sidebarRef.current ? Math.ceil(sidebarRef.current.getBoundingClientRect().width) : 0;
      void setUiInsets(top, left);
    };
    report();
    const observer = new ResizeObserver(report);
    observer.observe(header);
    if (sidebarRef.current) {
      observer.observe(sidebarRef.current);
    }
    return () => observer.disconnect();
  }, [vertical, snapshot.fullscreen]);

  const showBookmarksBar = settings?.showBookmarksBar ?? true;

  return (
    <div id="browser" className="flex h-full flex-col bg-zinc-950 text-zinc-100">
      <div id="browser-header" ref={headerRef} className={snapshot.fullscreen ? "hidden" : "flex flex-col"}>
        {vertical ? null : <TabStrip snapshot={snapshot} />}
        <AddressBar
          snapshot={snapshot}
          bookmarks={bookmarks}
          downloads={downloads}
          history={history}
          extensions={extensions}
          settings={settings}
          windowControls={vertical}
        />
        {showBookmarksBar ? <BookmarksBar tree={bookmarks} activeId={snapshot.activeId} /> : null}
        {findToken !== null && activeTab ? (
          <FindBar tabId={activeTab.id} focusToken={findToken} onClose={() => setFindToken(null)} />
        ) : null}
        <SyncToast />
      </div>
      <div id="browser-body" className="flex min-h-0 flex-1">
        {vertical && settings && !snapshot.fullscreen ? (
          <VerticalTabs ref={sidebarRef} snapshot={snapshot} settings={settings} />
        ) : null}
        <div id="browser-page-area" className="relative flex min-w-0 flex-1 items-center justify-center bg-zinc-950">
          {activeTab && !activeTab.loaded ? (
            <span id="browser-page-loading" className="text-sm text-zinc-500">
              Loading {activeTab.title || activeTab.url}
            </span>
          ) : null}
          {splitShown && split ? <SplitDivider split={split} activeId={snapshot.activeId} /> : null}
        </div>
      </div>
    </div>
  );
}
