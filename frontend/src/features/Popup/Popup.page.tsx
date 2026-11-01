import { useEffect } from "react";
import { AppMenu } from "@components/AppMenu";
import { PopupAppearanceContext, usePopupPainted } from "@lib/popup-ready";
import { useAppearanceState } from "@lib/use-browser";
import { AddressBarEllipsisMenu } from "@/components/TabsAddressAndBookmarks/Addressbar/AddressBarEllipsisMenu";
import { ExtensionPinMenu } from "@/components/TabsAddressAndBookmarks/Addressbar/ExtensionPinMenu";
import { BookmarkAdd } from "@/components/TabsAddressAndBookmarks/BookmarksBar/BookmarkAdd";
import { BookmarkEditor } from "@/components/TabsAddressAndBookmarks/BookmarksBar/BookmarkEditor";
import { BookmarkFolderMenu } from "@/components/TabsAddressAndBookmarks/BookmarksBar/BookmarkFolderMenu";
import { BookmarksBarMenu } from "@/components/TabsAddressAndBookmarks/BookmarksBar/BookmarksBarMenu";
import { TabGroupFlyout } from "@/components/TabsAddressAndBookmarks/Tabs/TabGroupFlyout";
import { TabGroupMenu } from "@/components/TabsAddressAndBookmarks/Tabs/TabGroupMenu";
import { TabMenu } from "@/components/TabsAddressAndBookmarks/Tabs/TabMenu";
import { WorkspaceMenu } from "@/components/WorkspaceMenu";
import { AdBlockerPopover } from "@features/BuiltInExtensions/ad-blocker/AdBlockerPopover";
import { PageControlsPopover } from "@features/BuiltInExtensions/page-controls/PageControlsPopover";
import { CommandPalette } from "@features/CommandPalette/CommandPalette";
import { NetworkPopover } from "@features/Network/NetworkPopover";
import { PageCapturePopover } from "@features/BuiltInExtensions/page-capture/PageCapturePopover";
import { PrintPdfPopover } from "@features/BuiltInExtensions/print-pdf/PrintPdfPopover";
import { WhatRunsPopover } from "@features/BuiltInExtensions/what-runs/WhatRunsPopover";
import { DownloadsPanel } from "@features/Downloads/DownloadsPanel";

type PopupPageProps = {
  popup: string;
  params: URLSearchParams;
};

export function PopupPage({ popup, params }: PopupPageProps) {
  const { loaded } = useAppearanceState();

  useEffect(() => {
    document.title = `ce-browser ${popup}`;
  }, [popup]);

  return (
    <PopupAppearanceContext.Provider value={loaded}>
      <PopupContent popup={popup} params={params} />
    </PopupAppearanceContext.Provider>
  );
}

function UnknownPopup() {
  usePopupPainted(true);
  return (
    <div id="popup-unknown" className="p-3 text-sm text-zinc-500">
      Unknown popup.
    </div>
  );
}

function PopupContent({ popup, params }: PopupPageProps) {
  switch (popup) {
    case "downloads":
      return <DownloadsPanel />;
    case "bookmark-edit":
      return <BookmarkEditor url={params.get("url") ?? ""} title={params.get("title") ?? ""} />;
    case "bookmark-folder":
      return <BookmarkFolderMenu folderId={params.get("id") ?? "bar"} />;
    case "bookmarks-bar-menu":
      return (
        <BookmarksBarMenu
          x={Number(params.get("x")) || 0}
          y={Number(params.get("y")) || 0}
          parentId={params.get("parent") || "bar"}
          divider={params.get("divider") === "horizontal" ? "horizontal" : "vertical"}
        />
      );
    case "bookmark-add":
      return (
        <BookmarkAdd
          mode={params.get("mode") === "folder" ? "folder" : "link"}
          parentId={params.get("parent") || "bar"}
        />
      );
    case "tab-menu":
      return <TabMenu tabId={Number(params.get("id"))} />;
    case "app-menu":
      return <AppMenu />;
    case "omnibox":
      return <AddressBarEllipsisMenu />;
    case "command-palette":
      return <CommandPalette />;
    case "print-pdf":
      return <PrintPdfPopover params={params} />;
    case "page-capture":
      return <PageCapturePopover params={params} />;
    case "what-runs":
      return <WhatRunsPopover params={params} />;
    case "ad-blocker":
      return <AdBlockerPopover params={params} />;
    case "page-controls":
      return <PageControlsPopover params={params} />;
    case "network":
      return <NetworkPopover tabId={Number(params.get("id"))} />;
    case "extension-pin":
      return <ExtensionPinMenu extensionId={params.get("id") ?? ""} />;
    case "tab-group":
      return <TabGroupFlyout groupId={Number(params.get("id"))} />;
    case "tab-group-menu":
      return <TabGroupMenu groupId={Number(params.get("id"))} />;
    case "workspaces":
      return <WorkspaceMenu />;
    default:
      return <UnknownPopup />;
  }
}
