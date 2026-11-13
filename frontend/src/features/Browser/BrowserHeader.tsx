import type { RefObject } from "react";
import type { BookmarkTree, BrowserSettings, DownloadItem, ExtensionInfo, HistoryItem, TabInfo } from "@type/browser";
import type { WindowTabsState } from "@/lib/use-browser";
import { FindBar } from "@/components/FindBar";
import { SyncToast } from "@/components/SyncToast";
import { AddressBar } from "../AddressBar/AddressBar";
import { BookmarksBar } from "../Bookmarks/BookmarksBar/BookmarksBar";
import { HostBookmarksBar } from "../Extensions/HostBookmarksBar";
import { useHostExtensions } from "@/lib/use-browser";
import { TabStrip } from "../Tabs/TabStrip";
import { ExtensionsToolbar } from "../Extensions/ExtensionsToolbar";


type BrowserHeaderProps = {
  headerRef: RefObject<HTMLDivElement | null>;
  snapshot: WindowTabsState;
  vertical: boolean;
  bookmarks: BookmarkTree | null;
  downloads: DownloadItem[];
  history: HistoryItem[];
  extensions: ExtensionInfo[];
  settings: BrowserSettings | null;
  showBookmarksBar: boolean;
  /** Built-in tools in a row above the bookmarks bar. */
  showToolsToolbar: boolean;
  toolbarRef?: RefObject<HTMLElement | null>;
  findToken: number | null;
  /** Bumped by Ctrl+Shift+F. The find bar opens Find in all tabs when it has a query. */
  findAllToken: number;
  /** Results list beside the page. */
  findPanel: boolean;
  onFindPanelChange: (open: boolean) => void;
  activeTab: TabInfo | undefined;
  onCloseFind: () => void;
};

export function BrowserHeader({
  headerRef,
  snapshot,
  vertical,
  bookmarks,
  downloads,
  history,
  extensions,
  settings,
  showBookmarksBar,
  showToolsToolbar,
  toolbarRef,
  findToken,
  findAllToken,
  findPanel,
  onFindPanelChange,
  activeTab,
  onCloseFind,
}: BrowserHeaderProps) {
  const hostBarId = settings?.hostBookmarksBar ?? "";
  const hostBar = useHostExtensions().extensions.find(
    (extension) => extension.id === hostBarId && extension.enabled && extension.surfaces.bookmarksBar,
  );
  return (
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
      {showToolsToolbar ? (
        <ExtensionsToolbar ref={toolbarRef} side="top" activeTab={activeTab} extensions={extensions} settings={settings} />
      ) : null}
      {showBookmarksBar && hostBar ? (
        <HostBookmarksBar extensionId={hostBar.id} height={settings?.bookmarksBarHeight} />
      ) : showBookmarksBar ? (
        <BookmarksBar
          tree={bookmarks}
          activeId={snapshot.activeId}
          activePinned={!!activeTab?.pinned}
          textSize={settings?.bookmarksBarTextSize}
          padding={settings?.bookmarksBarPadding}
          iconSize={settings?.bookmarksBarIconSize}
          height={settings?.bookmarksBarHeight}
          folderIcons={settings?.bookmarksBarFolderIcons ?? true}
          align={settings?.bookmarksBarAlign}
        />
      ) : null}
      {findToken !== null && activeTab ? (
        <FindBar
          tabId={activeTab.id}
          focusToken={findToken}
          openAllToken={findAllToken}
          panelOpen={findPanel}
          onPanelChange={onFindPanelChange}
          fullscreen={snapshot.fullscreen}
          onClose={onCloseFind}
        />
      ) : null}
      <SyncToast />
    </div>
  );
}
