import { FindBar } from "@/components/FindBar";
import { PageFailure } from "@/components/PageFailure";
import { SplitDivider } from "@/components/SplitDivider";
import { SyncToast } from "@/components/SyncToast";
import { AddressBar } from "@/components/TabsAddressAndBookmarks/Addressbar/AddressBar";
import { BookmarksBar } from "@/components/TabsAddressAndBookmarks/BookmarksBar/BookmarksBar";
import { TabStrip } from "@/components/TabsAddressAndBookmarks/Tabs/TabStrip";
import { VerticalTabs } from "@/components/TabsAddressAndBookmarks/Tabs/VerticalTabs";
import { VerticalToolbar } from "@/components/VerticalToolbar";
import { backdropSupport, onBrowserCommand, openPopup, setUiInsets } from "@/lib/browser-client";
import { leaveTab } from "@/lib/tab-hover";
import { splitToolbar, toolbarPinned } from "@/lib/toolbar-pin";
import { useTabs, useBookmarks, useDownloads, useHistory, useExtensions, useAppearanceFromSettings } from "@/lib/use-browser";
import { useRef, useState, useEffect, useLayoutEffect } from "react";
import type { PopupKind } from "@type/browser";


export function BrowserPage() {
  const snapshot = useTabs();
  const bookmarks = useBookmarks();
  const downloads = useDownloads();
  const history = useHistory();
  const { extensions } = useExtensions();
  const settings = useAppearanceFromSettings();
  const headerRef = useRef<HTMLDivElement>(null);
  const sidebarRef = useRef<HTMLElement>(null);
  const toolbarRef = useRef<HTMLElement>(null);
  const [findToken, setFindToken] = useState<number | null>(null);

  const activeTab = snapshot.tabs.find((tab) => tab.id === snapshot.activeId);
  const vertical = settings?.tabLayout === "vertical";
  const hoverPreview = settings?.tabHoverPreview;
  const toolExtensionIds = extensions
    .filter((extension) => extension.enabled && toolbarPinned(settings, extension.id))
    .map((extension) => extension.id);
  const showToolStrip = !snapshot.fullscreen && splitToolbar(settings, toolExtensionIds).vertical.length > 0;
  const split = snapshot.split;
  const splitShown =
    !!split && !snapshot.fullscreen && (snapshot.activeId === split.left || snapshot.activeId === split.right);

  useEffect(() => {
    document.title = activeTab ? `${activeTab.title || activeTab.url} - ce-browser` : "ce-browser";
  }, [activeTab]);

  const backdrop = settings?.windowBackdrop && settings.windowBackdrop !== "none" ? settings.windowBackdrop : null;
  useEffect(() => {
    const root = document.documentElement;
    if (!backdrop) {
      delete root.dataset.backdrop;
      return;
    }
    let cancelled = false;
    void backdropSupport()
      .then((support) => {
        if (cancelled) {
          return;
        }
        const drawn = !support.restartNeeded && (backdrop === "mica" ? support.mica : support.acrylic);
        if (drawn) {
          root.dataset.backdrop = backdrop;
          document.getElementById("ce-page-background")?.remove();
        } else {
          delete root.dataset.backdrop;
        }
      })
      .catch(() => delete root.dataset.backdrop);
    return () => {
      cancelled = true;
    };
  }, [backdrop]);

  useEffect(() => {
    if (hoverPreview === false) {
      leaveTab(true);
    }
  }, [hoverPreview]);

  useEffect(() => {
    const centered = (kind: PopupKind, width: number, height: number, params = "") =>
      void openPopup({
        kind,
        params,
        focus: true,
        x: Math.max(8, (window.innerWidth - width) / 2),
        y: 52,
        width,
        height: Math.min(height, window.innerHeight - 60),
      });
    const unlisten = onBrowserCommand((command) => {
      switch (command) {
        case "find":
          setFindToken((token) => (token ?? 0) + 1);
          break;
        case "palette":
          centered("command-palette", 560, 420);
          break;
        case "tab-search":
          centered("tab-search", 520, 480);
          break;
        case "shortcut-sheet":
          centered("shortcut-sheet", Math.min(760, window.innerWidth - 32), 560);
          break;
        case "tab-switcher":
        case "tab-switcher-back": {
          const width = Math.min(760, window.innerWidth - 32);
          void openPopup({
            kind: "tab-switcher",
            params: `step=${command === "tab-switcher" ? 1 : -1}`,
            focus: true,
            x: Math.max(8, (window.innerWidth - width) / 2),
            y: Math.max(52, window.innerHeight / 2 - 220),
            width,
            height: 440,
          });
          break;
        }
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
      const right = toolbarRef.current ? Math.ceil(toolbarRef.current.getBoundingClientRect().width) : 0;
      void setUiInsets(top, left, right, 0);
    };
    report();
    const observer = new ResizeObserver(report);
    observer.observe(header);
    if (sidebarRef.current) {
      observer.observe(sidebarRef.current);
    }
    if (toolbarRef.current) {
      observer.observe(toolbarRef.current);
    }
    return () => observer.disconnect();
  }, [vertical, snapshot.fullscreen, showToolStrip, settings?.verticalToolbarLabels]);

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
        {showBookmarksBar ? (
          <BookmarksBar
            tree={bookmarks}
            activeId={snapshot.activeId}
            textSize={settings?.bookmarksBarTextSize}
            padding={settings?.bookmarksBarPadding}
          />
        ) : null}
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
          {activeTab && !activeTab.loaded && !activeTab.failure ? (
            <span id="browser-page-loading" className="text-sm text-zinc-500">
              Loading {activeTab.title || activeTab.url}
            </span>
          ) : null}
          {splitShown && split
            ? [
                { id: split.left, style: { left: 0, width: `calc(${split.ratio * 100}% - 3px)` } },
                { id: split.right, style: { right: 0, width: `calc(${(1 - split.ratio) * 100}% - 3px)` } },
              ].map(({ id, style }) => {
                const tab = snapshot.tabs.find((item) => item.id === id);
                return tab?.failure ? (
                  <PageFailure key={id} tab={tab} failure={tab.failure} className="absolute inset-y-0" style={style} />
                ) : null;
              })
            : activeTab?.failure
              ? <PageFailure tab={activeTab} failure={activeTab.failure} className="absolute inset-0" />
              : null}
          {splitShown && split ? <SplitDivider split={split} activeId={snapshot.activeId} /> : null}
        </div>
        {showToolStrip ? (
          <VerticalToolbar ref={toolbarRef} activeTab={activeTab} extensions={extensions} settings={settings} />
        ) : null}
      </div>
    </div>
  );
}
