import type { RefObject } from "react";
import type { BookmarkTree, BrowserSettings, DownloadItem, ExtensionInfo, HistoryItem, TabInfo } from "@type/browser";
import type { WindowTabsState } from "@/lib/use-browser";
import { FindBar } from "@/components/FindBar";
import { SyncToast } from "@/components/SyncToast";
import { AddressBar } from "../AddressBar/AddressBar";
import { BookmarksBar } from "../Bookmarks/BookmarksBar/BookmarksBar";
import { HostBookmarksBar } from "../HostExtensions/HostBookmarksBar";
import { useHostExtensions } from "@/lib/use-browser";
import { TabStrip } from "../Tabs/TabStrip";
import { ExtensionsToolbar } from "../ExtensionsToolbar/ExtensionsToolbar";


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
        />
      ) : null}
      {findToken !== null && activeTab ? (
        <FindBar tabId={activeTab.id} focusToken={findToken} onClose={onCloseFind} />
      ) : null}
      <SyncToast />
    </div>
  );
}
