import { useEffect, useMemo, useState } from "react";
import type { PopupShow } from "@type/browser";
import { AddressBarEllipsisMenu } from "@/features/AddressBar/AddressBarEllipsisMenu";
import { AddressBarExtensionPopover } from "@/features/AddressBar/AddressBarExtensionPopover";
import { PopupFrame } from "@/components/PopupFrame";
import { StatusPopover } from "@/components/StatusPopover";
import { Toast } from "@/components/Toast";
import { onPopupHide, onPopupShow, popupCurrent } from "@lib/browser-client";
import { PopupHostContext, type PopupHostState } from "@lib/popup-host";
import { useAppearanceState, useBookmarksState, useExtensions, useTabs } from "@lib/use-browser";
import { AddressBarDropdownMenu } from "@/features/AddressBar/AddressBarDropdownMenu";
import { ProfileSwitcher } from "@/features/AddressBar/ProfileSwitcher";
import { FontPicker } from "@/features/BuiltInExtensions/font-picker/FontPicker";
import { StylePicker } from "@/features/BuiltInExtensions/style-picker/StylePicker";
import { ThemePicker } from "@/features/BuiltInExtensions/theme-picker/ThemePicker";
import { AddressBarExtensionPinContextMenu } from "@/features/AddressBar/AddressBarExtensionPinContextMenu";
import { PinContextMenu } from "@/features/AddressBar/PinContextMenu";
import { BookmarkAdd } from "../Bookmarks/BookmarksBar/BookmarkAdd";
import { BookmarkEditor } from "../Bookmarks/BookmarksBar/BookmarkEditor";
import { BookmarkFolderMenu } from "../Bookmarks/BookmarksBar/BookmarkFolderMenu";
import { BookmarkRename } from "../Bookmarks/BookmarksBar/BookmarkRename";
import { BookmarksBarOverflow } from "../Bookmarks/BookmarksBar/BookmarksBarOverflow";
import { AdBlockerPopover } from "../BuiltInExtensions/ad-blocker/AdBlockerPopover";
import { ExtractPopover } from "../BuiltInExtensions/automation/ExtractPopover";
import { BoardImagesPopover } from "../BuiltInExtensions/boards/BoardImagesPopover";
import { BoardDropPopover } from "../BuiltInExtensions/boards/popover/BoardDropPopover";
import { BoardsPopover } from "../BuiltInExtensions/boards/popover/BoardsPopover";
import { CommandPalette } from "../BuiltInExtensions/command-palette/CommandPalette";
import { MediaGrid } from "../BuiltInExtensions/media/MediaGrid";
import { MediaHub } from "../BuiltInExtensions/media/MediaHub";
import { NetworkPopover } from "../BuiltInExtensions/network/NetworkPopover";
import { NetworkSpeedPopover } from "../BuiltInExtensions/network-speed/NetworkSpeedPopover";
import { PageCapturePopover } from "../BuiltInExtensions/page-capture/PageCapturePopover";
import { RecordingPopover } from "../BuiltInExtensions/recording/RecordingPopover";
import { PageControlsPopover } from "../BuiltInExtensions/page-controls/PageControlsPopover";
import { PasswordPopover } from "../BuiltInExtensions/password/Password";
import { PrintPdfPopover } from "../BuiltInExtensions/print-to-pdf/PrintPdfPopover";
import { ReadingPopover } from "../BuiltInExtensions/reading/Reading";
import { ShoppingPopover } from "../BuiltInExtensions/shopping/ShoppingPopover";
import { SelectionTranslatePopover } from "../BuiltInExtensions/translate/SelectionTranslate";
import { TranslatePopover } from "../BuiltInExtensions/translate/Translate";
import { WebpageMarkdownPopover } from "../BuiltInExtensions/webpage-markdown/WebpageMarkdownPopover";
import { WhatRunsPopover } from "../BuiltInExtensions/what-runs/WhatRunsPopover";
import { DownloadsPopover } from "../Downloads/DownloadsPopover";
import { FindAllPopover } from "../FindAll/FindAllPopover";
import { HistoryPopover } from "../History/HistoryPopover";
import { UnresponsivePrompt } from "../PageRecovery/UnresponsivePrompt";
import { PermissionPrompt } from "../Permissions/PermissionPrompt";
import { ShortcutSheet } from "../Shortcuts/ShortcutSheet";
import { SiteInfoPanel } from "../SiteInfo/SiteInfoPanel";
import { TabContextMenu } from "../Tabs/TabContextMenu";
import { TabGroupFlyout } from "../Tabs/TabGroupFlyout";
import { TabGroupMenu } from "../Tabs/TabGroupMenu";
import { TabHoverCard } from "../Tabs/TabHoverCard";
import { TabOverview } from "../Tabs/TabOverview";
import { TabSearch } from "../Tabs/TabSearch";
import { TabSwitcher } from "../Tabs/TabSwitcher";
import { WorkspaceMenu } from "../Workspace/WorkspaceMenu";
import { BookmarkMenuTarget, BookmarksBarContextMenu } from "../Bookmarks/BookmarksBar/BookmarksBarContextMenu";


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

function menuIndex(params: URLSearchParams) {
  const raw = params.get("index");
  if (!raw) {
    return null;
  }
  const index = Number(raw);
  return Number.isInteger(index) && index >= 0 ? index : null;
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
      return <DownloadsPopover />;
    case "history":
      return <HistoryPopover />;
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
          anchorId={params.get("anchor")}
          index={menuIndex(params)}
        />
      );
    case "bookmarks-bar-overflow":
      return <BookmarksBarOverflow ids={(params.get("ids") ?? "").split(",").filter(Boolean)} />;
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
    case "address-bar-ellipsis-menu":
      return <AddressBarEllipsisMenu />;
    case "extensions-menu":
      return <AddressBarExtensionPopover />;
    case "status":
      return <StatusPopover />;
    case "omnibox":
      return <AddressBarDropdownMenu />;
    case "command-palette":
      return <CommandPalette />;
    case "print-pdf":
      return <PrintPdfPopover params={params} />;
    case "page-capture":
      return <PageCapturePopover params={params} />;
    case "recording":
      return <RecordingPopover params={params} />;
    case "webpage-markdown":
      return <WebpageMarkdownPopover params={params} />;
    case "boards":
      return <BoardsPopover params={params} />;
    case "board-images":
      return <BoardImagesPopover params={params} />;
    case "board-drop":
      return <BoardDropPopover params={params} />;
    case "media-hub":
      return <MediaHub />;
    case "media-grid":
      return <MediaGrid tabId={Number(params.get("id"))} />;
    case "what-runs":
      return <WhatRunsPopover params={params} />;
    case "extract":
      return <ExtractPopover params={params} />;
    case "ad-blocker":
      return <AdBlockerPopover params={params} />;
    case "page-controls":
      return <PageControlsPopover params={params} />;
    case "network":
      return <NetworkPopover tabId={Number(params.get("id"))} />;
    case "network-speed":
      return <NetworkSpeedPopover />;
    case "translate":
      return <TranslatePopover params={params} />;
    case "password":
      return <PasswordPopover />;
    case "shopping":
      return <ShoppingPopover params={params} />;
    case "reading":
      return <ReadingPopover params={params} />;
    case "extension-pin":
      return <AddressBarExtensionPinContextMenu extensionId={params.get("id") ?? ""} />;
    case "pin-menu":
      return <PinContextMenu url={params.get("url") ?? ""} />;
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
    case "permission":
      return <PermissionPrompt params={params} />;
    case "site-info":
      return <SiteInfoPanel tabId={Number(params.get("id"))} initialView={params.get("view") ?? ""} />;
    case "selection-translate":
      return <SelectionTranslatePopover params={params} />;
    case "find-all":
      return <FindAllPopover params={params} />;
    case "shortcut-sheet":
      return <ShortcutSheet />;
    case "style-picker":
      return <StylePicker />;
    case "theme-picker":
      return <ThemePicker />;
    case "font-picker":
      return <FontPicker />;
    case "profile-switcher":
      return <ProfileSwitcher />;
    default:
      return <UnknownPopup />;
  }
}
