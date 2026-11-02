import { PopupMenu, PopupMenuItem, PopupMenuSeparator, PopupFrame } from "@/components/PopupFrame";
import { openPopup, addBookmark, closePopup, createTab, deleteBookmark, openFolderAsGroup, openInWindow } from "@/lib/browser-client";
import { AppWindow, ExternalLink, FolderPlus, Group, PencilLine, Plus, SeparatorHorizontal, SeparatorVertical, Trash2 } from "lucide-react";

type DividerOrientation = "vertical" | "horizontal";

export type BookmarkMenuTarget = {
  id: string;
  kind: "link" | "folder" | "separator";
  url?: string;
};

const PROTECTED_FOLDERS = new Set(["bar", "other"]);

type BookmarksBarMenuItemsProps = {
  idPrefix: string;
  parentId: string;
  divider: DividerOrientation;
  x: number;
  y: number;
  target?: BookmarkMenuTarget | null;
  onDone: () => void;
};

function deleteLabel(kind: BookmarkMenuTarget["kind"]) {
  if (kind === "folder") {
    return "Delete folder";
  }
  if (kind === "separator") {
    return "Delete divider";
  }
  return "Delete bookmark";
}

export function BookmarksBarContextMenuItems({ idPrefix, parentId, divider, x, y, target, onDone }: BookmarksBarMenuItemsProps) {
  const openAdd = (mode: "link" | "folder") => {
    const params = new URLSearchParams({ mode, parent: parentId });
    void openPopup({
      kind: "bookmark-add",
      params: params.toString(),
      x,
      y,
      width: 340,
      height: mode === "folder" ? 248 : 300,
    });
  };

  const addDivider = () => {
    void addBookmark(parentId, "", null, undefined, true).then(onDone);
  };

  const vertical = divider === "vertical";
  const canDelete = !!target && !PROTECTED_FOLDERS.has(target.id);
  const canRename = !!target && target.kind !== "separator" && !PROTECTED_FOLDERS.has(target.id);
  const linkUrl = target?.kind === "link" ? target.url?.trim() : "";

  const remove = () => {
    if (!target || !canDelete) {
      return;
    }
    void deleteBookmark(target.id).then(onDone);
  };

  const openRename = () => {
    if (!target || !canRename) {
      return;
    }
    const params = new URLSearchParams({ id: target.id });
    void openPopup({
      kind: "bookmark-rename",
      params: params.toString(),
      x,
      y,
      width: 340,
      height: 188,
    });
  };

  return (
    <PopupMenu id={`${idPrefix}-items`} label={vertical ? "Bookmarks bar" : "Bookmark folder"}>
      {linkUrl ? (
        <>
          <PopupMenuItem
            id={`${idPrefix}-open-tab`}
            icon={<ExternalLink />}
            label="Open in new tab"
            onClick={() => void createTab(linkUrl).finally(onDone)}
          />
          <PopupMenuItem
            id={`${idPrefix}-open-window`}
            icon={<AppWindow />}
            label="Open in new window"
            onClick={() => void openInWindow(linkUrl).finally(onDone)}
          />
          <PopupMenuSeparator id={`${idPrefix}-open-separator`} />
        </>
      ) : null}
      <PopupMenuItem id={`${idPrefix}-add`} icon={<Plus />} label="Add bookmark" onClick={() => openAdd("link")} />
      <PopupMenuItem
        id={`${idPrefix}-add-folder`}
        icon={<FolderPlus />}
        label="Add folder"
        onClick={() => openAdd("folder")}
      />
      <PopupMenuSeparator id={`${idPrefix}-separator`} />
      <PopupMenuItem
        id={`${idPrefix}-add-divider`}
        icon={vertical ? <SeparatorVertical /> : <SeparatorHorizontal />}
        label={vertical ? "Add vertical divider" : "Add horizontal divider"}
        onClick={addDivider}
      />
      {target?.kind === "folder" ? (
        <>
          <PopupMenuSeparator id={`${idPrefix}-group-separator`} />
          <PopupMenuItem
            id={`${idPrefix}-open-group`}
            icon={<Group />}
            label="Open all as a tab group"
            onClick={() => void openFolderAsGroup(target.id).finally(onDone)}
          />
        </>
      ) : null}
      {canDelete && target ? (
        <>
          <PopupMenuSeparator id={`${idPrefix}-delete-separator`} />
          {canRename ? (
            <PopupMenuItem
              id={`${idPrefix}-rename`}
              icon={<PencilLine />}
              label="Rename"
              onClick={openRename}
            />
          ) : null}
          <PopupMenuItem
            id={`${idPrefix}-delete`}
            icon={<Trash2 />}
            label={deleteLabel(target.kind)}
            danger
            onClick={remove}
          />
        </>
      ) : null}
    </PopupMenu>
  );
}

type BookmarksBarMenuProps = {
  x: number;
  y: number;
  parentId: string;
  divider: DividerOrientation;
  target?: BookmarkMenuTarget | null;
};

export function BookmarksBarContextMenu({ x, y, parentId, divider, target }: BookmarksBarMenuProps) {
  return (
    <PopupFrame id="bookmarks-bar-menu">
      <BookmarksBarContextMenuItems
        idPrefix="bookmarks-bar-menu"
        parentId={parentId}
        divider={divider}
        x={x}
        y={y}
        target={target}
        onDone={() => void closePopup()}
      />
    </PopupFrame>
  );
}
