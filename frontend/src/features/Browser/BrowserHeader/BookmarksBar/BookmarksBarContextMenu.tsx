import { PopupMenu, PopupMenuItem, PopupMenuSeparator, PopupFrame } from "@/components/PopupFrame";
import { openPopup, addBookmark, closePopup, createTab, deleteBookmark, getSettings, openFolderAsGroup, openInWindow, saveSettings } from "@/lib/browser-client";
import { BOOKMARK_SIZE, bookmarkItemStyle } from "@/lib/bookmark-size";
import { useSettings } from "@/lib/use-browser";
import { AppWindow, ExternalLink, FolderPlus, Group, Library, PencilLine, Pin, Plus, SeparatorHorizontal, SeparatorVertical, Trash2 } from "lucide-react";

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
  target?: BookmarkMenuTarget | null;
  onDone: () => void;
};

/** Top-left of this menu. The add and rename forms replace it there. */
const MENU_CORNER = { x: 0, y: 0 };

function deleteLabel(kind: BookmarkMenuTarget["kind"]) {
  if (kind === "folder") {
    return "Delete folder";
  }
  if (kind === "separator") {
    return "Delete divider";
  }
  return "Delete bookmark";
}

/** Same comparison the settings list uses: one trailing slash still matches. */
function pinKey(url: string) {
  const trimmed = url.trim();
  if (!trimmed.endsWith("/")) {
    return trimmed;
  }
  const scheme = trimmed.indexOf("://");
  if (scheme < 0 || trimmed.length <= scheme + 4) {
    return trimmed;
  }
  return trimmed.slice(0, -1);
}

export function BookmarksBarContextMenuItems({ idPrefix, parentId, divider, target, onDone }: BookmarksBarMenuItemsProps) {
  const settings = useSettings();
  const itemStyle = bookmarkItemStyle(settings?.bookmarksBarMenuTextSize, settings?.bookmarksBarMenuPadding, {
    text: BOOKMARK_SIZE.menuText,
    padding: BOOKMARK_SIZE.menuPadding,
  });
  const openAdd = (mode: "link" | "folder") => {
    const params = new URLSearchParams({ mode, parent: parentId });
    void openPopup("bookmark-add", MENU_CORNER, { params: params.toString() });
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
    void openPopup("bookmark-rename", MENU_CORNER, { params: params.toString() });
  };

  const addToStartupPins = async () => {
    if (!linkUrl) {
      return;
    }
    const base = settings ?? (await getSettings());
    const url = linkUrl.trim();
    const key = pinKey(url);
    if (base.pinnedTabs.some((pin) => pinKey(pin) === key)) {
      return;
    }
    await saveSettings({ ...base, pinnedTabs: [...base.pinnedTabs, url] });
  };

  return (
    <PopupMenu id={`${idPrefix}-items`} label={vertical ? "Bookmarks bar" : "Bookmark folder"}>
      {linkUrl ? (
        <>
          <PopupMenuItem
            id={`${idPrefix}-open-tab`}
            icon={<ExternalLink />}
            label="Open in new tab"
            style={itemStyle}
            onClick={() => void createTab(linkUrl).finally(onDone)}
          />
          <PopupMenuItem
            id={`${idPrefix}-open-window`}
            icon={<AppWindow />}
            label="Open in new window"
            style={itemStyle}
            onClick={() => void openInWindow(linkUrl).finally(onDone)}
          />
          <PopupMenuItem
            id={`${idPrefix}-startup-pin`}
            icon={<Pin />}
            label="Add to startup pinned tabs"
            style={itemStyle}
            onClick={() => void addToStartupPins().finally(onDone)}
          />
          <PopupMenuSeparator id={`${idPrefix}-open-separator`} />
        </>
      ) : null}
      <PopupMenuItem id={`${idPrefix}-add`} icon={<Plus />} label="Add bookmark" style={itemStyle} onClick={() => openAdd("link")} />
      <PopupMenuItem
        id={`${idPrefix}-add-folder`}
        icon={<FolderPlus />}
        label="Add folder"
        style={itemStyle}
        onClick={() => openAdd("folder")}
      />
      <PopupMenuSeparator id={`${idPrefix}-separator`} />
      <PopupMenuItem
        id={`${idPrefix}-add-divider`}
        icon={vertical ? <SeparatorVertical /> : <SeparatorHorizontal />}
        label={vertical ? "Add vertical divider" : "Add horizontal divider"}
        style={itemStyle}
        onClick={addDivider}
      />
      {target?.kind === "folder" ? (
        <>
          <PopupMenuSeparator id={`${idPrefix}-group-separator`} />
          <PopupMenuItem
            id={`${idPrefix}-open-group`}
            icon={<Group />}
            label="Open all as a tab group"
            style={itemStyle}
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
              style={itemStyle}
              onClick={openRename}
            />
          ) : null}
          <PopupMenuItem
            id={`${idPrefix}-delete`}
            icon={<Trash2 />}
            label={deleteLabel(target.kind)}
            style={itemStyle}
            danger
            onClick={remove}
          />
        </>
      ) : null}
      <PopupMenuSeparator id={`${idPrefix}-manager-separator`} />
      <PopupMenuItem
        id={`${idPrefix}-manager`}
        icon={<Library />}
        label="Bookmarks Manager"
        style={itemStyle}
        onClick={() => void createTab("ce://bookmarks").finally(onDone)}
      />
    </PopupMenu>
  );
}

type BookmarksBarMenuProps = {
  parentId: string;
  divider: DividerOrientation;
  target?: BookmarkMenuTarget | null;
};

export function BookmarksBarContextMenu({ parentId, divider, target }: BookmarksBarMenuProps) {
  return (
    <PopupFrame id="bookmarks-bar-menu" width={280}>
      <BookmarksBarContextMenuItems
        idPrefix="bookmarks-bar-menu"
        parentId={parentId}
        divider={divider}
        target={target}
        onDone={() => void closePopup()}
      />
    </PopupFrame>
  );
}
