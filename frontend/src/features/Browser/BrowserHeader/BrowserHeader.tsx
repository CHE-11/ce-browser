import type { RefObject } from "react";
import type { BookmarkTree, BrowserSettings, DownloadItem, ExtensionInfo, HistoryItem, TabInfo } from "@type/browser";
import type { WindowTabsState } from "@/lib/use-browser";
import { FindBar } from "@/components/FindBar";
import { SyncToast } from "@/components/SyncToast";
import { AddressBar } from "./Addressbar/AddressBar";
import { BookmarksBar } from "./BookmarksBar/BookmarksBar";
import { TabStrip } from "./Tabs/TabStrip";

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
  findToken,
  activeTab,
  onCloseFind,
}: BrowserHeaderProps) {
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
      {showBookmarksBar ? (
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
