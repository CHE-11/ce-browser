import { PopupMenu, PopupMenuItem, PopupMenuSeparator, PopupFrame } from "@/components/PopupFrame";
import { openPopup, addBookmark, closePopup, deleteBookmark, openFolderAsGroup } from "@/lib/browser-client";
import { FolderPlus, Group, Plus, SeparatorHorizontal, SeparatorVertical, Trash2 } from "lucide-react";

type DividerOrientation = "vertical" | "horizontal";

export type BookmarkMenuTarget = {
  id: string;
  kind: "link" | "folder" | "separator";
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

export function BookmarksBarMenuItems({ idPrefix, parentId, divider, x, y, target, onDone }: BookmarksBarMenuItemsProps) {
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

  const remove = () => {
    if (!target || !canDelete) {
      return;
    }
    void deleteBookmark(target.id).then(onDone);
  };

  return (
    <PopupMenu id={`${idPrefix}-items`} label={vertical ? "Bookmarks bar" : "Bookmark folder"}>
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

export function BookmarksBarMenu({ x, y, parentId, divider, target }: BookmarksBarMenuProps) {
  return (
    <PopupFrame id="bookmarks-bar-menu">
      <BookmarksBarMenuItems
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
