import { useEffect } from "react";
import { AppMenu } from "../components/AppMenu";
import { BookmarkAdd } from "../components/BookmarksBar/BookmarkAdd";
import { BookmarkEditor } from "../components/BookmarksBar/BookmarkEditor";
import { BookmarkFolderMenu } from "../components/BookmarksBar/BookmarkFolderMenu";
import { BookmarksBarMenu } from "../components/BookmarksBar/BookmarksBarMenu";
import { DownloadsPanel } from "../components/DownloadsPanel";
import { OmniboxSuggestions } from "../components/OmniboxSuggestions";
import { TabMenu } from "../components/TabMenu";
import { PageCapturePopover } from "../extensions/page-capture/PageCapturePopover";
import { PrintPdfPopover } from "../extensions/print-pdf/PrintPdfPopover";
import { useAppearanceFromSettings } from "../lib/use-browser";

type PopupPageProps = {
  popup: string;
  params: URLSearchParams;
};

export function PopupPage({ popup, params }: PopupPageProps) {
  useAppearanceFromSettings();

  useEffect(() => {
    document.title = `ce-browser ${popup}`;
  }, [popup]);

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
      return <OmniboxSuggestions />;
    case "print-pdf":
      return <PrintPdfPopover params={params} />;
    case "page-capture":
      return <PageCapturePopover params={params} />;
    default:
      return (
        <div id="popup-unknown" className="p-3 text-sm text-zinc-500">
          Unknown popup.
        </div>
      );
  }
}
