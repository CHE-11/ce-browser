import { Button } from "@/components/Button";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "@/components/ContextMenu";
import { Dialog } from "@/components/Dialog";
import { createTab, openFolderAsGroup, openInWindow } from "@/lib/browser-client";
import { bookmarkContains, findFolder } from "@/lib/use-browser";
import { BookmarkNode, BookmarkTree } from "@type/browser";
import { AppWindow, ExternalLink, FolderPlus, Group, PencilLine, Plus, SeparatorHorizontal, Trash2 } from "lucide-react";
import { useState } from "react";

const PROTECTED_FOLDERS = new Set(["bar", "other"]);

export type BookmarkEditorMenuTarget = {
  /** The row under the pointer. Null when the click landed on empty list space. */
  node: BookmarkNode | null;
  parentId: string | null;
  index: number;
};

export type BookmarkEditorCompose =
  | { kind: "add-link"; parentId: string; index: number }
  | { kind: "add-folder"; parentId: string; index: number }
  | { kind: "rename"; node: BookmarkNode };

type BookmarkEditorContextMenuProps = {
  x: number;
  y: number;
  target: BookmarkEditorMenuTarget;
  bookmarkId: string;
  tree: BookmarkTree | null;
  onClose: () => void;
  onAddBookmark: (parentId: string, index: number) => void;
  onAddFolder: (parentId: string, index: number) => void;
  onAddDivider: (parentId: string, index: number) => void;
  onRename: (node: BookmarkNode) => void;
  onDelete: (node: BookmarkNode, parentId: string, index: number) => void;
};

function isFolder(node: BookmarkNode) {
  return !node.url && !node.separator;
}

function insertAt(target: BookmarkEditorMenuTarget, tree: BookmarkTree | null) {
  if (target.node && isFolder(target.node)) {
    return { parentId: target.node.id, index: target.node.children?.length ?? 0 };
  }
  if (!target.parentId) {
    return null;
  }
  if (target.node) {
    return { parentId: target.parentId, index: target.index + 1 };
  }
  const folder = findFolder(tree, target.parentId);
  return { parentId: target.parentId, index: folder?.children?.length ?? 0 };
}

function deleteLabel(node: BookmarkNode) {
  if (isFolder(node)) {
    return "Delete folder";
  }
  if (node.separator) {
    return "Delete divider";
  }
  return "Delete bookmark";
}

function deleteTitle(node: BookmarkNode, protectedNode: boolean, holdsEdit: boolean) {
  if (protectedNode) {
    return "The Bookmarks bar and Other bookmarks stay.";
  }
  if (holdsEdit && isFolder(node)) {
    return "This folder contains the bookmark you're editing.";
  }
  if (holdsEdit) {
    return "Use Remove to delete the bookmark you're editing.";
  }
  return undefined;
}

export function BookmarkEditorContextMenu({
  x,
  y,
  target,
  bookmarkId,
  tree,
  onClose,
  onAddBookmark,
  onAddFolder,
  onAddDivider,
  onRename,
  onDelete,
}: BookmarkEditorContextMenuProps) {
  const node = target.node;
  const spot = insertAt(target, tree);
  const folder = !!node && isFolder(node);
  const link = node?.url?.trim() ?? "";
  const protectedNode = !!node && PROTECTED_FOLDERS.has(node.id);
  const holdsEdit = !!node && bookmarkContains(tree, node.id, bookmarkId);
  const canDelete = !!node && !!target.parentId && !protectedNode && !holdsEdit;
  const canRename = !!node && !node.separator && !protectedNode;
  const addLabel = node && !folder ? "below" : "here";

  const run = (action: () => void) => {
    action();
    onClose();
  };

  return (
    <ContextMenu id="bookmark-editor-menu" label="Bookmark" x={x} y={y} onClose={onClose} scrollParentId="bookmark-place-tree">
      {link ? (
        <>
          <ContextMenuItem
            id="bookmark-editor-menu-open-tab"
            icon={<ExternalLink />}
            label="Open in new tab"
            onClick={() => run(() => void createTab(link))}
          />
          <ContextMenuItem
            id="bookmark-editor-menu-open-window"
            icon={<AppWindow />}
            label="Open in new window"
            onClick={() => run(() => void openInWindow(link))}
          />
          <ContextMenuSeparator id="bookmark-editor-menu-open-separator" />
        </>
      ) : null}
      {folder && node ? (
        <>
          <ContextMenuItem
            id="bookmark-editor-menu-open-group"
            icon={<Group />}
            label="Open all as a tab group"
            onClick={() => run(() => void openFolderAsGroup(node.id))}
          />
          <ContextMenuSeparator id="bookmark-editor-menu-group-separator" />
        </>
      ) : null}
      <ContextMenuItem
        id="bookmark-editor-menu-add"
        icon={<Plus />}
        label={`Add bookmark ${addLabel}`}
        disabled={!spot}
        onClick={() => {
          if (spot) {
            run(() => onAddBookmark(spot.parentId, spot.index));
          }
        }}
      />
      <ContextMenuItem
        id="bookmark-editor-menu-add-folder"
        icon={<FolderPlus />}
        label={`Add folder ${addLabel}`}
        disabled={!spot}
        onClick={() => {
          if (spot) {
            run(() => onAddFolder(spot.parentId, spot.index));
          }
        }}
      />
      <ContextMenuItem
        id="bookmark-editor-menu-add-divider"
        icon={<SeparatorHorizontal />}
        label={`Add divider ${addLabel}`}
        disabled={!spot}
        onClick={() => {
          if (spot) {
            run(() => onAddDivider(spot.parentId, spot.index));
          }
        }}
      />
      {node ? (
        <>
          <ContextMenuSeparator id="bookmark-editor-menu-edit-separator" />
          {canRename ? (
            <ContextMenuItem
              id="bookmark-editor-menu-rename"
              icon={<PencilLine />}
              label={link ? "Edit" : "Rename"}
              onClick={() => run(() => onRename(node))}
            />
          ) : null}
          <ContextMenuItem
            id="bookmark-editor-menu-delete"
            icon={<Trash2 />}
            label={deleteLabel(node)}
            danger
            disabled={!canDelete}
            title={deleteTitle(node, protectedNode, holdsEdit)}
            onClick={() => {
              const parentId = target.parentId;
              if (!canDelete || !parentId) {
                return;
              }
              run(() => onDelete(node, parentId, target.index));
            }}
          />
        </>
      ) : null}
    </ContextMenu>
  );
}

type BookmarkEditorComposeDialogProps = {
  compose: BookmarkEditorCompose;
  onClose: () => void;
  onSave: (title: string, url: string) => Promise<void>;
};

export function BookmarkEditorComposeDialog({ compose, onClose, onSave }: BookmarkEditorComposeDialogProps) {
  const renaming = compose.kind === "rename" ? compose.node : null;
  const isLink = compose.kind === "add-link" || !!renaming?.url;
  const [name, setName] = useState(renaming?.title ?? "");
  const [url, setUrl] = useState(renaming?.url ?? "https://");
  const [error, setError] = useState<string | null>(null);
  const valid = name.trim().length > 0 && (!isLink || url.trim().length > 0);
  const heading = compose.kind === "add-folder" ? "New folder" : compose.kind === "add-link" ? "Add bookmark" : isLink ? "Edit bookmark" : "Rename folder";

  const save = async () => {
    if (!valid) {
      return;
    }
    try {
      await onSave(name.trim(), url.trim());
    } catch (reason) {
      setError(String(reason));
    }
  };

  return (
    <Dialog id="bookmark-editor-compose" title={heading} open onClose={onClose} panelClassName="max-w-sm">
      <form
        id="bookmark-editor-compose-form"
        className="flex flex-col gap-3"
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <label id="bookmark-editor-compose-name-label" className="flex flex-col gap-1 text-xs text-zinc-400">
          Name
          <input
            id="bookmark-editor-compose-name"
            autoFocus
            value={name}
            onChange={(event) => setName(event.target.value)}
            className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
          />
        </label>
        {isLink ? (
          <label id="bookmark-editor-compose-url-label" className="flex flex-col gap-1 text-xs text-zinc-400">
            URL
            <input
              id="bookmark-editor-compose-url"
              value={url}
              spellCheck={false}
              onChange={(event) => setUrl(event.target.value)}
              className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
            />
          </label>
        ) : null}
        {error ? (
          <p id="bookmark-editor-compose-error" className="text-xs text-red-300">
            {error}
          </p>
        ) : null}
        <div id="bookmark-editor-compose-actions" className="flex justify-end gap-2 pt-1">
          <Button id="bookmark-editor-compose-cancel" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button id="bookmark-editor-compose-save" type="submit" variant="primary" disabled={!valid}>
            Save
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
