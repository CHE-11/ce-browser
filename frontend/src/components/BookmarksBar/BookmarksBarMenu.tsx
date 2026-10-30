import { FolderPlus, Plus, SeparatorHorizontal, SeparatorVertical } from "lucide-react";
import { addBookmark, closePopup, openPopup } from "../../lib/browser-client";
import { PopupFrame, PopupMenu, PopupMenuItem, PopupMenuSeparator } from "../PopupFrame";

type DividerOrientation = "vertical" | "horizontal";

type BookmarksBarMenuItemsProps = {
  idPrefix: string;
  parentId: string;
  divider: DividerOrientation;
  x: number;
  y: number;
  onDone: () => void;
};

export function BookmarksBarMenuItems({ idPrefix, parentId, divider, x, y, onDone }: BookmarksBarMenuItemsProps) {
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
    </PopupMenu>
  );
}

type BookmarksBarMenuProps = {
  x: number;
  y: number;
  parentId: string;
  divider: DividerOrientation;
};

export function BookmarksBarMenu({ x, y, parentId, divider }: BookmarksBarMenuProps) {
  return (
    <PopupFrame id="bookmarks-bar-menu">
      <BookmarksBarMenuItems
        idPrefix="bookmarks-bar-menu"
        parentId={parentId}
        divider={divider}
        x={x}
        y={y}
        onDone={() => void closePopup()}
      />
    </PopupFrame>
  );
}
