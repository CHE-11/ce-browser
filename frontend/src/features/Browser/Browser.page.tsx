
import { backdropSupport, clearFind, onBrowserCommand, openPopup, setUiInsets } from "@/lib/browser-client";
import { leaveTab } from "@/lib/tab-hover";
import { hostToolbarIds, splitToolbar } from "@/lib/toolbar-pin";
import { toolbarSide } from "@/lib/toolbar-side";
import { useTabs, useBookmarks, useDownloads, useHistory, useExtensions, useHostExtensions, useAppearanceFromSettings } from "@/lib/use-browser";
import { useRef, useState, useEffect, useLayoutEffect } from "react";
import type { PopupKind } from "@type/browser";
import { BrowserBody } from "./BrowserBody";
import { BrowserHeader } from "./BrowserHeader";
import { ExtensionsToolbar } from "../Extensions/ExtensionsToolbar";
import { openTabOverview } from "../Tabs/TabOverview";



export function BrowserPage() {
  const snapshot = useTabs();
  const bookmarks = useBookmarks();
  const downloads = useDownloads();
  const history = useHistory();
  const { extensions } = useExtensions();
  const hostExtensions = useHostExtensions().extensions;
  const settings = useAppearanceFromSettings();
  const headerRef = useRef<HTMLDivElement>(null);
  const sidebarRef = useRef<HTMLElement>(null);
  const toolbarRef = useRef<HTMLElement>(null);
  const [findToken, setFindToken] = useState<number | null>(null);
  const [findAllToken, setFindAllToken] = useState(0);
  const [findPanel, setFindPanel] = useState(false);
  const findOpen = useRef(false);
  const findTabId = useRef<number | undefined>(undefined);

  const activeTab = snapshot.tabs.find((tab) => tab.id === snapshot.activeId);
  const vertical = settings?.tabLayout === "vertical";
  const hoverPreview = settings?.tabHoverPreview;
  const toolExtensionIds = [
    ...extensions.filter((extension) => extension.enabled).map((extension) => extension.id),
    ...hostToolbarIds(hostExtensions),
  ];
  const toolSplit = splitToolbar(settings, toolExtensionIds);
  const toolSide = toolbarSide(settings);
  const showTools = !snapshot.fullscreen && settings?.showToolsToolbar !== false && toolSplit.tools.length > 0;
  const showToolStrip = showTools && (toolSide === "left" || toolSide === "right");
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

  findOpen.current = findToken !== null;
  findTabId.current = activeTab?.id;

  useEffect(() => {
    if (findToken === null) {
      setFindPanel(false);
    }
  }, [findToken]);

  useEffect(() => {
    /** Centred across the window, `top` px down. */
    const centered = (kind: PopupKind, top: number, params = "") =>
      void openPopup(kind, { x: 0, y: 0, width: window.innerWidth, height: top - 4 }, { params, placement: "bottom-center" });
    const unlisten = onBrowserCommand((command) => {
      switch (command) {
        case "find":
          if (findOpen.current) {
            const id = findTabId.current;
            if (id !== undefined) {
              void clearFind(id);
            }
            setFindToken(null);
            break;
          }
          setFindToken((token) => (token ?? 0) + 1);
          break;
        case "find-all":
          setFindToken((token) => (token ?? 0) + 1);
          setFindAllToken((token) => token + 1);
          break;
        case "palette":
          centered("command-palette", 52);
          break;
        case "tab-search":
          centered("tab-search", 52);
          break;
        case "shortcut-sheet":
          centered("shortcut-sheet", 52);
          break;
        case "tab-switcher":
        case "tab-switcher-back":
          centered("tab-switcher", Math.max(52, window.innerHeight / 2 - 220), `step=${command === "tab-switcher" ? 1 : -1}`);
          break;
        case "tab-overview":
          void openTabOverview(true);
          break;
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
      const height = header.getBoundingClientRect().height;
      const top = Math.ceil(height);
      // Pages are placed on the next whole pixel. This starts the strip beside them there too.
      const browser = header.closest("#browser");
      if (browser instanceof HTMLElement) {
        browser.style.setProperty("--chrome-top-slack", `${top - height}px`);
      }
      const sidebar = sidebarRef.current ? Math.ceil(sidebarRef.current.getBoundingClientRect().width) : 0;
      const toolBox = toolbarRef.current?.getBoundingClientRect();
      const toolWidth = toolBox ? Math.ceil(toolBox.width) : 0;
      const toolHeight = toolBox ? Math.ceil(toolBox.height) : 0;
      const results = document.getElementById("find-results");
      const resultsWidth = results ? Math.ceil(results.getBoundingClientRect().width) : 0;
      const left = sidebar + (toolSide === "left" ? toolWidth : 0);
      const right = (toolSide === "right" ? toolWidth : 0) + resultsWidth;
      const bottom = toolSide === "bottom" ? toolHeight : 0;
      void setUiInsets(top, left, right, bottom);
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
    const results = document.getElementById("find-results");
    if (results) {
      observer.observe(results);
    }
    return () => observer.disconnect();
  }, [vertical, snapshot.fullscreen, showToolStrip, toolSide, settings?.verticalToolbarLabels, findPanel]);

  const showBookmarksBar = settings?.showBookmarksBar ?? true;

  return (
    <div id="browser" className="flex h-full flex-col bg-zinc-950 text-zinc-100">
      <BrowserHeader
        headerRef={headerRef}
        snapshot={snapshot}
        vertical={vertical}
        bookmarks={bookmarks}
        downloads={downloads}
        history={history}
        extensions={extensions}
        settings={settings}
        showBookmarksBar={showBookmarksBar}
        showToolsToolbar={showTools && toolSide === "top"}
        toolbarRef={toolSide === "top" ? toolbarRef : undefined}
        findToken={findToken}
        findAllToken={findAllToken}
        findPanel={findPanel}
        onFindPanelChange={setFindPanel}
        activeTab={activeTab}
        onCloseFind={() => setFindToken(null)}
      />
      <BrowserBody
        snapshot={snapshot}
        settings={settings}
        vertical={vertical}
        sidebarRef={sidebarRef}
        toolbarRef={toolbarRef}
        activeTab={activeTab}
        split={split}
        splitShown={splitShown}
        showToolStrip={showToolStrip}
        toolSide={toolSide}
        extensions={extensions}
      />
      {showTools && toolSide === "bottom" ? (
        <ExtensionsToolbar ref={toolbarRef} side="bottom" activeTab={activeTab} extensions={extensions} settings={settings} />
      ) : null}
    </div>
  );
}
