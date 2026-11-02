import { useContext, useEffect } from "react";
import { AppMenu } from "@components/AppMenu";
import { StatusPopover } from "@/components/StatusPopover";
import { Toast } from "@/components/Toast";
import { PopupAppearanceContext, usePopupHostReady } from "@lib/popup-ready";
import { useAppearanceState, useBookmarksState, useExtensions, useTabs } from "@lib/use-browser";
import { AddressBarEllipsisMenu } from "@/features/Browser/BrowserHeader/Addressbar/AddressBarEllipsisMenu";
import { ExtensionPinMenu } from "@/features/Browser/BrowserHeader/Addressbar/ExtensionPinMenu";
import { BookmarkAdd } from "@/features/Browser/BrowserHeader/BookmarksBar/BookmarkAdd";
import { BookmarkRename } from "@/features/Browser/BrowserHeader/BookmarksBar/BookmarkRename";
import { BookmarkEditor } from "@/features/Browser/BrowserHeader/BookmarksBar/BookmarkEditor";
import { BookmarkFolderMenu } from "@/features/Browser/BrowserHeader/BookmarksBar/BookmarkFolderMenu";
import { BookmarksBarContextMenu, type BookmarkMenuTarget } from "@/features/Browser/BrowserHeader/BookmarksBar/BookmarksBarContextMenu";
import { TabGroupFlyout } from "@/features/Browser/BrowserHeader/Tabs/TabGroupFlyout";
import { TabGroupMenu } from "@/features/Browser/BrowserHeader/Tabs/TabGroupMenu";
import { TabMenu } from "@/features/Browser/BrowserHeader/Tabs/TabMenu";
import { WorkspaceMenu } from "@/features/Browser/BrowserHeader/Workspace/WorkspaceMenu";
import { AdBlockerPopover } from "@features/BuiltInExtensions/ad-blocker/AdBlockerPopover";
import { AssistantPopover } from "@features/BuiltInExtensions/assistant/Assistant";
import { BoardImagesPopover } from "@features/BuiltInExtensions/boards/BoardImagesPopover";
import { BoardsPopover } from "@features/BuiltInExtensions/boards/BoardsPopover";
import { PageControlsPopover } from "@features/BuiltInExtensions/page-controls/PageControlsPopover";
import { PageCapturePopover } from "@features/BuiltInExtensions/page-capture/PageCapturePopover";
import { PasswordPopover } from "@features/BuiltInExtensions/password/Password";
import { PricePopover } from "@features/BuiltInExtensions/price/Price";
import { PrintPdfPopover } from "@features/BuiltInExtensions/print-pdf/PrintPdfPopover";
import { ReadingPopover } from "@features/BuiltInExtensions/reading/Reading";
import { ShoppingPopover } from "@features/BuiltInExtensions/shopping/Shopping";
import { TranslatePopover } from "@features/BuiltInExtensions/translate/Translate";
import { SelectionTranslatePopover } from "@features/BuiltInExtensions/translate/SelectionTranslate";
import { WaybackPopover } from "@features/BuiltInExtensions/wayback/Wayback";
import { WhatRunsPopover } from "@features/BuiltInExtensions/what-runs/WhatRunsPopover";
import { CommandPalette } from "@features/CommandPalette/CommandPalette";
import { TabSearch } from "@/features/Browser/BrowserHeader/Tabs/TabSearch";
import { TabSwitcher } from "@/features/Browser/BrowserHeader/Tabs/TabSwitcher";
import { TabHoverCard } from "@/features/Browser/BrowserHeader/Tabs/TabHoverCard";
import { UnresponsivePrompt } from "@features/PageRecovery/UnresponsivePrompt";
import { SiteInfoPanel } from "@features/SiteInfo/SiteInfoPanel";
import { FindAllPopover } from "@features/FindAll/FindAllPopover";
import { ShortcutSheet } from "@features/Shortcuts/ShortcutSheet";
import { NetworkPopover } from "@features/Network/NetworkPopover";
import { DownloadsPanel } from "@features/Downloads/DownloadsPanel";
import { MediaGrid } from "@features/Media/MediaGrid";
import { MediaHub } from "@features/Media/MediaHub";

type PopupPageProps = {
  popup: string;
  params: URLSearchParams;
};

export function PopupPage({ popup, params }: PopupPageProps) {
  const { loaded } = useAppearanceState();
  useBookmarksState();
  useTabs();
  useExtensions();

  useEffect(() => {
    document.title = `ce-browser ${popup}`;
  }, [popup]);

  return (
    <PopupAppearanceContext.Provider value={loaded}>
      {popup === "ready" ? (
        <PopupStandby />
      ) : (
        <MountedPopup key={`${popup}?${params.toString()}`} popup={popup} params={params} />
      )}
    </PopupAppearanceContext.Provider>
  );
}

function PopupStandby() {
  const appearanceReady = useContext(PopupAppearanceContext);
  usePopupHostReady(appearanceReady);
  return null;
}

function MountedPopup({ popup, params }: PopupPageProps) {
  return <PopupContent popup={popup} params={params} />;
}

function UnknownPopup() {
  return (
    <div id="popup-unknown" className="p-3 text-sm text-zinc-500">
      Unknown popup.
    </div>
  );
}

function bookmarkMenuTarget(params: URLSearchParams): BookmarkMenuTarget | null {
  const id = params.get("target");
  if (!id) {
    return null;
  }
  const kind = params.get("kind");
  const url = params.get("url") ?? undefined;
  return { id, kind: kind === "folder" || kind === "separator" ? kind : "link", url };
}

function PopupContent({ popup, params }: PopupPageProps) {
  switch (popup) {
    case "downloads":
      return <DownloadsPanel />;
    case "bookmark-edit":
      return <BookmarkEditor url={params.get("url") ?? ""} title={params.get("title") ?? ""} />;
    case "bookmark-folder":
      return (
        <BookmarkFolderMenu
          folderId={params.get("id") ?? "bar"}
          left={Number(params.get("left")) || 0}
          dragOpened={params.get("drag") === "1"}
        />
      );
    case "bookmarks-bar-menu":
      return (
        <BookmarksBarContextMenu
          x={Number(params.get("x")) || 0}
          y={Number(params.get("y")) || 0}
          parentId={params.get("parent") || "bar"}
          divider={params.get("divider") === "horizontal" ? "horizontal" : "vertical"}
          target={bookmarkMenuTarget(params)}
        />
      );
    case "bookmark-add":
      return (
        <BookmarkAdd
          mode={params.get("mode") === "folder" ? "folder" : "link"}
          parentId={params.get("parent") || "bar"}
        />
      );
    case "bookmark-rename":
      return <BookmarkRename id={params.get("id") ?? ""} />;
    case "tab-menu":
      return <TabMenu tabId={Number(params.get("id"))} />;
    case "app-menu":
      return <AppMenu />;
    case "status":
      return <StatusPopover />;
    case "omnibox":
      return <AddressBarEllipsisMenu />;
    case "command-palette":
      return <CommandPalette />;
    case "print-pdf":
      return <PrintPdfPopover params={params} />;
    case "page-capture":
      return <PageCapturePopover params={params} />;
    case "boards":
      return <BoardsPopover params={params} />;
    case "board-images":
      return <BoardImagesPopover params={params} />;
    case "media-hub":
      return <MediaHub />;
    case "media-grid":
      return <MediaGrid tabId={Number(params.get("id"))} />;
    case "what-runs":
      return <WhatRunsPopover params={params} />;
    case "ad-blocker":
      return <AdBlockerPopover params={params} />;
    case "page-controls":
      return <PageControlsPopover params={params} />;
    case "network":
      return <NetworkPopover tabId={Number(params.get("id"))} />;
    case "translate":
      return <TranslatePopover params={params} />;
    case "password":
      return <PasswordPopover />;
    case "price":
      return <PricePopover params={params} />;
    case "shopping":
      return <ShoppingPopover params={params} />;
    case "wayback":
      return <WaybackPopover params={params} />;
    case "reading":
      return <ReadingPopover params={params} />;
    case "assistant":
      return <AssistantPopover params={params} />;
    case "extension-pin":
      return <ExtensionPinMenu extensionId={params.get("id") ?? ""} />;
    case "tab-group":
      return <TabGroupFlyout groupId={Number(params.get("id"))} />;
    case "tab-group-menu":
      return <TabGroupMenu groupId={Number(params.get("id"))} />;
    case "workspaces":
      return <WorkspaceMenu />;
    case "toast":
      return <Toast params={params} />;
    case "tab-search":
      return <TabSearch params={params} />;
    case "tab-switcher":
      return <TabSwitcher params={params} />;
    case "tab-hover":
      return <TabHoverCard tabId={Number(params.get("id"))} />;
    case "unresponsive":
      return <UnresponsivePrompt tabId={Number(params.get("id"))} />;
    case "site-info":
      return <SiteInfoPanel tabId={Number(params.get("id"))} />;
    case "selection-translate":
      return <SelectionTranslatePopover params={params} />;
    case "find-all":
      return <FindAllPopover params={params} />;
    case "shortcut-sheet":
      return <ShortcutSheet />;
    default:
      return <UnknownPopup />;
  }
}
