import { useEffect, useMemo, useState } from "react";
import type { PopupShow } from "@type/browser";
import { AppMenu } from "@components/AppMenu";
import { PopupFrame } from "@/components/PopupFrame";
import { StatusPopover } from "@/components/StatusPopover";
import { Toast } from "@/components/Toast";
import { onPopupHide, onPopupShow, popupCurrent } from "@lib/browser-client";
import { PopupHostContext, type PopupHostState } from "@lib/popup-host";
import { useAppearanceState, useBookmarksState, useExtensions, useTabs } from "@lib/use-browser";
import { AddressBarEllipsisMenu } from "@/features/Browser/BrowserHeader/Addressbar/AddressBarEllipsisMenu";
import { ExtensionPinContextMenu } from "@/features/Browser/BrowserHeader/Addressbar/ExtensionPinContextMenu";
import { BookmarkAdd } from "@/features/Browser/BrowserHeader/BookmarksBar/BookmarkAdd";
import { BookmarkRename } from "@/features/Browser/BrowserHeader/BookmarksBar/BookmarkRename";
import { BookmarkEditor } from "@/features/Browser/BrowserHeader/BookmarksBar/BookmarkEditor";
import { BookmarkFolderMenu } from "@/features/Browser/BrowserHeader/BookmarksBar/BookmarkFolderMenu";
import { BookmarksBarContextMenu, type BookmarkMenuTarget } from "@/features/Browser/BrowserHeader/BookmarksBar/BookmarksBarContextMenu";
import { TabGroupFlyout } from "@/features/Browser/BrowserHeader/Tabs/TabGroupFlyout";
import { TabGroupMenu } from "@/features/Browser/BrowserHeader/Tabs/TabGroupMenu";
import { TabContextMenu } from "@/features/Browser/BrowserHeader/Tabs/TabContextMenu";
import { WorkspaceMenu } from "@/features/Browser/BrowserHeader/Workspace/WorkspaceMenu";
import { AdBlockerPopover } from "@features/BuiltInExtensions/ad-blocker/AdBlockerPopover";
import { BoardImagesPopover } from "@features/BuiltInExtensions/boards/BoardImagesPopover";
import { BoardsPopover } from "@features/BuiltInExtensions/boards/BoardsPopover";
import { PageControlsPopover } from "@features/BuiltInExtensions/page-controls/PageControlsPopover";
import { PageCapturePopover } from "@features/BuiltInExtensions/page-capture/PageCapturePopover";
import { PasswordPopover } from "@features/BuiltInExtensions/password/Password";
import { PrintPdfPopover } from "@features/BuiltInExtensions/print-pdf/PrintPdfPopover";
import { ReadingPopover } from "@features/BuiltInExtensions/reading/Reading";
import { ShoppingPopover } from "@/features/BuiltInExtensions/shopping/ShoppingPopover";
import { TranslatePopover } from "@features/BuiltInExtensions/translate/Translate";
import { SelectionTranslatePopover } from "@features/BuiltInExtensions/translate/SelectionTranslate";
import { WhatRunsPopover } from "@features/BuiltInExtensions/what-runs/WhatRunsPopover";
import { CommandPalette } from "@features/CommandPalette/CommandPalette";
import { TabOverview } from "@/features/Browser/BrowserHeader/Tabs/TabOverview";
import { TabSearch } from "@/features/Browser/BrowserHeader/Tabs/TabSearch";
import { TabSwitcher } from "@/features/Browser/BrowserHeader/Tabs/TabSwitcher";
import { TabHoverCard } from "@/features/Browser/BrowserHeader/Tabs/TabHoverCard";
import { UnresponsivePrompt } from "@features/PageRecovery/UnresponsivePrompt";
import { SiteInfoPanel } from "@features/SiteInfo/SiteInfoPanel";
import { FindAllPopover } from "@features/FindAll/FindAllPopover";
import { ShortcutSheet } from "@features/Shortcuts/ShortcutSheet";
import { NetworkPopover } from "@features/Network/NetworkPopover";
import { DownloadsPanel } from "@features/Downloads/DownloadsPanel";
import { MediaGrid } from "@/features/BuiltInExtensions/media/MediaGrid";
import { MediaHub } from "@/features/BuiltInExtensions/media/MediaHub";

type PopupPageProps = {
  popup: string;
  params: URLSearchParams;
};

/** `host` is a popup window that stays loaded and shows whatever it is sent. Toasts load
 * their own route. */
export function PopupPage({ popup, params }: PopupPageProps) {
  const { loaded } = useAppearanceState();
  useBookmarksState();
  useTabs();
  useExtensions();

  if (popup === "host") {
    return <PopupHost appearanceReady={loaded} />;
  }
  return <RoutedPopup key={`${popup}?${params.toString()}`} popup={popup} params={params} />;
}

function RoutedPopup({ popup, params }: PopupPageProps) {
  useEffect(() => {
    document.title = `ce-browser ${popup}`;
  }, [popup]);
  return <PopupContent popup={popup} params={params} />;
}

function PopupHost({ appearanceReady }: { appearanceReady: boolean }) {
  const [show, setShow] = useState<PopupShow | null>(null);

  useEffect(() => {
    let alive = true;
    const unlisteners = [onPopupShow((next) => setShow(next)), onPopupHide(() => setShow(null))];
    void popupCurrent()
      .then((current) => {
        if (alive && current) {
          setShow((shown) => (shown && shown.token >= current.token ? shown : current));
        }
      })
      .catch(() => undefined);
    return () => {
      alive = false;
      for (const unlisten of unlisteners) {
        void unlisten.then((stop) => stop());
      }
    };
  }, []);

  useEffect(() => {
    document.title = show ? `ce-browser ${show.kind}` : "ce-browser popup";
  }, [show]);

  const params = useMemo(() => new URLSearchParams(show?.params ?? ""), [show?.params]);
  const host = useMemo<PopupHostState | null>(
    () => (show ? { token: show.token, level: show.level, appearanceReady } : null),
    [show, appearanceReady],
  );

  if (!show || !host) {
    return null;
  }
  return (
    <PopupHostContext.Provider value={host}>
      <PopupContent key={show.token} popup={show.kind} params={params} />
    </PopupHostContext.Provider>
  );
}

function UnknownPopup() {
  return (
    <PopupFrame id="popup-unknown" width={240} className="p-3 text-sm text-zinc-500">
      Unknown popup.
    </PopupFrame>
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
        <BookmarkFolderMenu folderId={params.get("id") ?? "bar"} dragOpened={params.get("drag") === "1"} />
      );
    case "bookmarks-bar-menu":
      return (
        <BookmarksBarContextMenu
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
      return <TabContextMenu tabId={Number(params.get("id"))} />;
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
    case "shopping":
      return <ShoppingPopover params={params} />;
    case "reading":
      return <ReadingPopover params={params} />;
    case "extension-pin":
      return <ExtensionPinContextMenu extensionId={params.get("id") ?? ""} />;
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
    case "tab-overview":
      return <TabOverview params={params} />;
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
