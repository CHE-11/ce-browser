import { useEffect } from "react";
import { AppMenu } from "../components/AppMenu";
import { ExtensionPinMenu } from "../components/ExtensionPinMenu";
import { BookmarkAdd } from "../components/BookmarksBar/BookmarkAdd";
import { BookmarkEditor } from "../components/BookmarksBar/BookmarkEditor";
import { BookmarkFolderMenu } from "../components/BookmarksBar/BookmarkFolderMenu";
import { BookmarksBarMenu } from "../components/BookmarksBar/BookmarksBarMenu";
import { DownloadsPanel } from "../components/DownloadsPanel";
import { AddressBarEllipsisMenu } from "../components/AddressBarEllipsisMenu";
import { TabGroupFlyout } from "../components/TabGroupFlyout";
import { TabGroupMenu } from "../components/TabGroupMenu";
import { TabMenu } from "../components/TabMenu";
import { WorkspaceMenu } from "../components/WorkspaceMenu";
import { AdBlockerPopover } from "../extensions/ad-blocker/AdBlockerPopover";
import { PageCapturePopover } from "../extensions/page-capture/PageCapturePopover";
import { PrintPdfPopover } from "../extensions/print-pdf/PrintPdfPopover";
import { WhatRunsPopover } from "../extensions/what-runs/WhatRunsPopover";
import { PopupAppearanceContext, usePopupPainted } from "../lib/popup-ready";
import { useAppearanceState } from "../lib/use-browser";

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
    case "print-pdf":
      return <PrintPdfPopover params={params} />;
    case "page-capture":
      return <PageCapturePopover params={params} />;
    case "what-runs":
      return <WhatRunsPopover params={params} />;
    case "ad-blocker":
      return <AdBlockerPopover params={params} />;
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
