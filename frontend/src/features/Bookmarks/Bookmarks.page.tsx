import { Button } from "@/components/Button";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "@/components/ContextMenu";
import { Dialog } from "@/components/Dialog";
import { FavIcon } from "@/components/FavIcon";
import { IconButton } from "@/components/IconButton";
import { deleteBookmark, moveBookmark, createTab, importBookmarksHtml, exportBookmarksHtml, updateBookmark, addBookmark } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { hostOf } from "@/lib/format";
import { useRoute } from "@/lib/route";
import { BookmarkDropPlace, useAppearanceFromSettings, useBookmarks, findFolder, bookmarkRootsOf, bookmarkContains, BOOKMARK_DRAG_TYPE, bookmarkDropPlace, findBookmarkParent, listFolders } from "@/lib/use-browser";
import { ChevronRight, Search, LayoutGrid, List, Type, Upload, Download, Plus, FolderPlus, FolderInput, Trash2, ExternalLink, Folder, Pencil } from "lucide-react";
import { useRef, useState, useEffect, useMemo, useCallback, type DragEvent, type MouseEvent } from "react";
import { BookmarkNode, BookmarkTree } from "../../../../types/browser";

type EditorState =
  | { mode: "add-link"; parentId: string }
  | { mode: "add-folder"; parentId: string }
  | { mode: "edit"; node: BookmarkNode };

type MenuState = { node: BookmarkNode; x: number; y: number };

type DropState = { id: string; place: BookmarkDropPlace };

type BookmarkView = "cards" | "list" | "headlines";

const ROOT_FOLDERS = new Set(["bar", "other"]);
const VIEW_KEY = "ce-bookmarks-view";

function storedView(): BookmarkView {
  try {
    const value = localStorage.getItem(VIEW_KEY);
    if (value === "cards" || value === "list" || value === "headlines") {
      return value;
    }
  } catch {
    // Private mode can block storage. The cards view is the default.
  }
  return "cards";
}

function folderPath(tree: BookmarkTree | null, id: string): BookmarkNode[] {
  if (!tree) {
    return [];
  }
  const walk = (node: BookmarkNode, trail: BookmarkNode[]): BookmarkNode[] | null => {
    const next = [...trail, node];
    if (node.id === id) {
      return next;
    }
    for (const child of node.children ?? []) {
      if (child.url || child.separator) {
        continue;
      }
      const found = walk(child, next);
      if (found) {
        return found;
      }
    }
    return null;
  };
  return walk(tree.bar, []) ?? walk(tree.other, []) ?? [];
}

function dropChrome(drop: DropState | null, id: string, axis: "x" | "y") {
  if (!drop || drop.id !== id) {
    return "";
  }
  if (drop.place === "into") {
    return "ring-1 ring-sky-500";
  }
  if (axis === "x") {
    return drop.place === "before" ? "shadow-[inset_2px_0_0_0_#0ea5e9]" : "shadow-[inset_-2px_0_0_0_#0ea5e9]";
  }
  return drop.place === "before" ? "shadow-[inset_0_2px_0_0_#0ea5e9]" : "shadow-[inset_0_-2px_0_0_#0ea5e9]";
}

export function BookmarksPage() {
  useAppearanceFromSettings();
  const tree = useBookmarks();
  const route = useRoute();
  const requestedFolder = route.kind === "page" ? route.params.get("folder") : null;
  const appliedFolder = useRef<string | null>(null);
  const [selectedId, setSelectedId] = useState("bar");
  const [query, setQuery] = useState("");
  const [view, setView] = useState<BookmarkView>(storedView);
  const [editor, setEditor] = useState<EditorState | null>(null);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [drop, setDrop] = useState<DropState | null>(null);
  const [picked, setPicked] = useState<string[]>([]);
  const [moving, setMoving] = useState<string[] | null>(null);
  const anchorId = useRef<string | null>(null);
  const dragged = useRef(false);

  const selected = findFolder(tree, selectedId) ?? tree?.bar ?? null;
  const path = folderPath(tree, selected?.id ?? "bar");
  const axis: "x" | "y" = view === "cards" ? "x" : "y";

  useEffect(() => {
    document.title = "Bookmarks";
  }, []);

  useEffect(() => {
    if (!requestedFolder || !tree || appliedFolder.current === requestedFolder) {
      return;
    }
    if (findFolder(tree, requestedFolder)) {
      appliedFolder.current = requestedFolder;
      setSelectedId(requestedFolder);
    }
  }, [requestedFolder, tree]);

  const results = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle || !tree) {
      return null;
    }
    const found: BookmarkNode[] = [];
    const walk = (node: BookmarkNode) => {
      for (const child of node.children ?? []) {
        if (child.url && (child.title.toLowerCase().includes(needle) || child.url.toLowerCase().includes(needle))) {
          found.push(child);
        }
        walk(child);
      }
    };
    walk(tree.bar);
    walk(tree.other);
    return found;
  }, [query, tree]);

  const rows = results ?? selected?.children ?? [];

  useEffect(() => {
    setPicked([]);
    anchorId.current = null;
  }, [selectedId, query]);

  const chooseView = (next: BookmarkView) => {
    setView(next);
    try {
      localStorage.setItem(VIEW_KEY, next);
    } catch {
      // The view still changes for this visit.
    }
  };

  const report = (action: Promise<unknown>, success?: string) => {
    action
      .then(() => setStatus(success ?? null))
      .catch((reason: unknown) => setStatus(String(reason)));
  };

  const closeMenu = useCallback(() => setMenu(null), []);

  const orderedIds = (ids: string[]) => {
    const wanted = new Set(ids);
    const visible = rows.map((row) => row.id).filter((id) => wanted.has(id));
    return [...visible, ...ids.filter((id) => !visible.includes(id))];
  };

  const removeIds = async (ids: string[]) => {
    const targets = bookmarkRootsOf(tree, orderedIds(ids));
    try {
      for (const id of targets) {
        await deleteBookmark(id);
      }
      setPicked([]);
      setStatus(null);
    } catch (reason: unknown) {
      setStatus(String(reason));
    }
  };

  const moveIds = async (ids: string[], parentId: string) => {
    const targets = bookmarkRootsOf(tree, orderedIds(ids)).filter(
      (id) => id !== parentId && !ROOT_FOLDERS.has(id) && !bookmarkContains(tree, id, parentId),
    );
    try {
      for (const id of targets) {
        await moveBookmark(id, parentId);
      }
      setPicked([]);
      setStatus(null);
    } catch (reason: unknown) {
      setStatus(String(reason));
    }
  };

  const onDragStart = (event: DragEvent, node: BookmarkNode) => {
    if (ROOT_FOLDERS.has(node.id)) {
      event.preventDefault();
      return;
    }
    dragged.current = true;
    event.dataTransfer?.setData(BOOKMARK_DRAG_TYPE, node.id);
    if (node.url) {
      event.dataTransfer?.setData("text/uri-list", node.url);
    }
    event.dataTransfer!.effectAllowed = "move";
  };

  const allowDrop = (event: DragEvent, node: BookmarkNode) => {
    if (!event.dataTransfer?.types.includes(BOOKMARK_DRAG_TYPE)) {
      return;
    }
    event.preventDefault();
    const place = bookmarkDropPlace(event, node, axis);
    setDrop((current) => (current?.id === node.id && current.place === place ? current : { id: node.id, place }));
  };

  const commitDrop = (event: DragEvent, node: BookmarkNode) => {
    event.preventDefault();
    event.stopPropagation();
    const id = event.dataTransfer?.getData(BOOKMARK_DRAG_TYPE);
    const place = bookmarkDropPlace(event, node, axis);
    setDrop(null);
    if (!tree || !id || id === node.id || ROOT_FOLDERS.has(id)) {
      return;
    }
    if (place === "into") {
      if (node.url || node.separator) {
        return;
      }
      report(moveBookmark(id, node.id));
      return;
    }
    const parent = findBookmarkParent(tree, node.id);
    if (!parent) {
      return;
    }
    report(moveBookmark(id, parent.parentId, place === "after" ? parent.index + 1 : parent.index));
  };

  const dropIntoFolder = (event: DragEvent, folderId: string) => {
    event.preventDefault();
    const id = event.dataTransfer?.getData(BOOKMARK_DRAG_TYPE);
    setDrop(null);
    if (!id || id === folderId || ROOT_FOLDERS.has(id)) {
      return;
    }
    report(moveBookmark(id, folderId));
  };

  const pickRow = (event: MouseEvent, node: BookmarkNode, index: number) => {
    if (dragged.current) {
      dragged.current = false;
      return;
    }
    if (event.shiftKey && !event.ctrlKey && !event.metaKey) {
      const anchor = rows.findIndex((row) => row.id === anchorId.current);
      const start = anchor < 0 ? index : Math.min(anchor, index);
      const end = anchor < 0 ? index : Math.max(anchor, index);
      setPicked(rows.slice(start, end + 1).map((row) => row.id));
      return;
    }
    if (event.ctrlKey || event.metaKey) {
      setPicked((current) => (current.includes(node.id) ? current.filter((id) => id !== node.id) : [...current, node.id]));
      anchorId.current = node.id;
      return;
    }
    if (node.url) {
      void createTab(node.url);
      return;
    }
    if (!node.separator) {
      setSelectedId(node.id);
      setQuery("");
    }
  };

  const openRowMenu = (event: MouseEvent, node: BookmarkNode) => {
    event.preventDefault();
    if (!picked.includes(node.id)) {
      setPicked([node.id]);
      anchorId.current = node.id;
    }
    setMenu({ node, x: event.clientX, y: event.clientY });
  };

  const menuTargets = menu && picked.includes(menu.node.id) && picked.length > 1 ? picked : menu ? [menu.node.id] : [];
  const bulkMenu = menuTargets.length > 1;

  return (
    <div id="bookmarks-page" className="flex h-full flex-col bg-zinc-950 text-zinc-100">
      <header id="bookmarks-page-header" className="flex h-11 shrink-0 items-center gap-2 border-b border-zinc-800 px-3">
        <nav id="bookmarks-page-path" aria-label="Folder" className="flex min-w-0 items-center gap-0.5">
          {path.map((folder, index) => (
            <span key={folder.id} id={`bookmarks-page-crumb-${folder.id}`} className="flex min-w-0 items-center gap-0.5">
              {index > 0 ? <ChevronRight id={`bookmarks-page-crumb-${folder.id}-sep`} className="size-3 shrink-0 text-zinc-600" aria-hidden /> : null}
              <button
                id={`bookmarks-page-crumb-${folder.id}-button`}
                type="button"
                className={cn(
                  "truncate rounded px-1.5 py-0.5 text-sm hover:bg-zinc-800",
                  index === path.length - 1 ? "font-medium text-zinc-100" : "text-zinc-400",
                  drop?.id === folder.id && "ring-1 ring-sky-500",
                )}
                onClick={() => {
                  setSelectedId(folder.id);
                  setQuery("");
                }}
                onDragOver={(event) => allowDrop(event, folder)}
                onDrop={(event) => dropIntoFolder(event, folder.id)}
              >
                {folder.title}
              </button>
            </span>
          ))}
        </nav>
        <label id="bookmarks-page-search" className="ml-auto flex h-7 w-52 items-center gap-1.5 rounded-md border border-zinc-800 bg-zinc-900 px-2">
          <Search id="bookmarks-page-search-icon" className="size-3.5 text-zinc-500" aria-hidden />
          <input
            id="bookmarks-page-search-input"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search"
            className="min-w-0 flex-1 bg-transparent text-xs outline-none placeholder:text-zinc-500"
          />
        </label>
        <div id="bookmarks-page-views" className="flex items-center rounded-md border border-zinc-800 p-0.5">
          {(
            [
              ["cards", LayoutGrid, "Cards"],
              ["list", List, "List"],
              ["headlines", Type, "Headlines"],
            ] as const
          ).map(([id, Icon, label]) => (
            <button
              key={id}
              id={`bookmarks-page-view-${id}`}
              type="button"
              title={label}
              aria-label={label}
              aria-pressed={view === id}
              className={cn(
                "inline-flex size-6 items-center justify-center rounded text-zinc-400 hover:text-zinc-100",
                view === id && "bg-zinc-800 text-zinc-100",
              )}
              onClick={() => chooseView(id)}
            >
              <Icon id={`bookmarks-page-view-${id}-icon`} className="size-3.5" aria-hidden />
            </button>
          ))}
        </div>
        <IconButton
          id="bookmarks-page-import"
          label="Import"
          onClick={() =>
            void importBookmarksHtml()
              .then((count) => count !== null && setStatus(`Imported ${count} bookmarks into Other bookmarks.`))
              .catch((reason: unknown) => setStatus(String(reason)))
          }
        >
          <Upload id="bookmarks-page-import-icon" aria-hidden />
        </IconButton>
        <IconButton
          id="bookmarks-page-export"
          label="Export"
          onClick={() =>
            void exportBookmarksHtml()
              .then((saved) => saved && setStatus("Bookmarks exported."))
              .catch((reason: unknown) => setStatus(String(reason)))
          }
        >
          <Download id="bookmarks-page-export-icon" aria-hidden />
        </IconButton>
        {selected && !results ? (
          <>
            <IconButton id="bookmarks-page-add-link" label="Add bookmark" onClick={() => setEditor({ mode: "add-link", parentId: selected.id })}>
              <Plus id="bookmarks-page-add-link-icon" aria-hidden />
            </IconButton>
            <IconButton id="bookmarks-page-add-folder" label="New folder" onClick={() => setEditor({ mode: "add-folder", parentId: selected.id })}>
              <FolderPlus id="bookmarks-page-add-folder-icon" aria-hidden />
            </IconButton>
          </>
        ) : null}
      </header>
      {picked.length > 1 ? (
        <div id="bookmarks-page-selection" className="flex h-9 shrink-0 items-center gap-2 border-b border-zinc-800 px-3 text-xs text-zinc-300">
          <span id="bookmarks-page-selection-count">{picked.length} selected</span>
          <Button id="bookmarks-page-move" size="xs" onClick={() => setMoving(picked)}>
            <FolderInput id="bookmarks-page-move-icon" className="size-3.5" aria-hidden />
            Move
          </Button>
          <Button id="bookmarks-page-delete" size="xs" variant="destructive" onClick={() => void removeIds(picked)}>
            <Trash2 id="bookmarks-page-delete-icon" className="size-3.5" aria-hidden />
            Delete
          </Button>
        </div>
      ) : null}
      {status ? (
        <div id="bookmarks-page-status" className="shrink-0 border-b border-zinc-800 px-3 py-1.5 text-xs text-zinc-400">
          {status}
        </div>
      ) : null}
      <div
        id="bookmarks-page-items"
        className="min-h-0 flex-1 overflow-y-auto p-3"
        onClick={(event) => {
          if (event.target === event.currentTarget) {
            setPicked([]);
          }
        }}
        onDragOver={(event) => {
          if (event.dataTransfer.types.includes(BOOKMARK_DRAG_TYPE)) {
            event.preventDefault();
          }
        }}
        onDrop={(event) => {
          if (!selected || results) {
            return;
          }
          const id = event.dataTransfer.getData(BOOKMARK_DRAG_TYPE);
          if (!id) {
            return;
          }
          event.preventDefault();
          setDrop(null);
          report(moveBookmark(id, selected.id));
        }}
      >
        {results ? (
          <p id="bookmarks-page-results" className="mb-2 text-xs text-zinc-500">
            {results.length} results for “{query.trim()}”
          </p>
        ) : null}
        {rows.length === 0 ? (
          <p id="bookmarks-page-empty" className="px-1 py-8 text-center text-sm text-zinc-500">
            {results ? "No bookmarks match that search." : "This folder is empty. Drag bookmarks here or add one."}
          </p>
        ) : (
          <div
            id="bookmarks-page-grid"
            className={cn(
              view === "cards" && "grid grid-cols-[repeat(auto-fill,minmax(14rem,1fr))] gap-2",
              view === "list" && "flex flex-col gap-0.5",
              view === "headlines" && "flex flex-col",
            )}
          >
            {rows.map((node, index) => (
              <BookmarkItem
                key={node.id}
                node={node}
                view={view}
                chosen={picked.includes(node.id)}
                dropClass={dropChrome(drop, node.id, axis)}
                onClick={(event) => pickRow(event, node, index)}
                onContextMenu={(event) => openRowMenu(event, node)}
                onDragStart={(event) => onDragStart(event, node)}
                onDragEnd={() => {
                  setDrop(null);
                  window.setTimeout(() => {
                    dragged.current = false;
                  });
                }}
                onDragOver={(event) => allowDrop(event, node)}
                onDrop={(event) => commitDrop(event, node)}
                onEdit={() => setEditor({ mode: "edit", node })}
                onDelete={() => void removeIds([node.id])}
              />
            ))}
          </div>
        )}
      </div>

      {menu ? (
        <ContextMenu id="bookmarks-menu" label="Bookmark" x={menu.x} y={menu.y} onClose={closeMenu}>
          {bulkMenu || menu.node.separator ? null : menu.node.url ? (
            <>
              <ContextMenuItem
                id="bookmarks-menu-open"
                icon={<ExternalLink />}
                label="Open in new tab"
                onClick={() => {
                  void createTab(menu.node.url ?? "");
                  closeMenu();
                }}
              />
              <ContextMenuItem
                id="bookmarks-menu-open-background"
                icon={<ExternalLink />}
                label="Open in background tab"
                onClick={() => {
                  void createTab(menu.node.url ?? "", true);
                  closeMenu();
                }}
              />
            </>
          ) : bulkMenu ? null : (
            <ContextMenuItem
              id="bookmarks-menu-open-folder"
              icon={<Folder />}
              label="Open folder"
              onClick={() => {
                setSelectedId(menu.node.id);
                setQuery("");
                closeMenu();
              }}
            />
          )}
          {bulkMenu || menu.node.separator ? null : (
            <>
              <ContextMenuSeparator id="bookmarks-menu-separator" />
              <ContextMenuItem
                id="bookmarks-menu-edit"
                icon={<Pencil />}
                label="Edit"
                onClick={() => {
                  setEditor({ mode: "edit", node: menu.node });
                  closeMenu();
                }}
              />
            </>
          )}
          {menu.node.separator && !bulkMenu ? null : (
            <ContextMenuItem
              id="bookmarks-menu-move"
              icon={<FolderInput />}
              label={bulkMenu ? `Move ${menuTargets.length} to folder` : "Move to folder"}
              onClick={() => {
                setMoving(menuTargets);
                closeMenu();
              }}
            />
          )}
          <ContextMenuItem
            id="bookmarks-menu-delete"
            icon={<Trash2 />}
            label={bulkMenu ? `Delete ${menuTargets.length}` : "Delete"}
            danger
            onClick={() => {
              void removeIds(menuTargets);
              closeMenu();
            }}
          />
        </ContextMenu>
      ) : null}

      {moving ? (
        <MoveDialog
          tree={tree}
          ids={moving}
          onClose={() => setMoving(null)}
          onMove={(parentId) => {
            void moveIds(moving, parentId);
            setMoving(null);
          }}
        />
      ) : null}

      {editor ? (
        <BookmarkDialog
          key={editor.mode === "edit" ? editor.node.id : editor.mode}
          editor={editor}
          onClose={() => setEditor(null)}
          onSave={(title, url) => {
            const action =
              editor.mode === "edit"
                ? updateBookmark(editor.node.id, title, editor.node.url ? url : null)
                : addBookmark(editor.parentId, title, editor.mode === "add-link" ? url : null);
            report(action);
            setEditor(null);
          }}
        />
      ) : null}
    </div>
  );
}

type BookmarkItemProps = {
  node: BookmarkNode;
  view: BookmarkView;
  chosen: boolean;
  dropClass: string;
  onClick: (event: MouseEvent) => void;
  onContextMenu: (event: MouseEvent) => void;
  onDragStart: (event: DragEvent) => void;
  onDragEnd: () => void;
  onDragOver: (event: DragEvent) => void;
  onDrop: (event: DragEvent) => void;
  onEdit: () => void;
  onDelete: () => void;
};

function BookmarkItem({
  node,
  view,
  chosen,
  dropClass,
  onClick,
  onContextMenu,
  onDragStart,
  onDragEnd,
  onDragOver,
  onDrop,
  onEdit,
  onDelete,
}: BookmarkItemProps) {
  const domId = `bookmarks-item-${node.id}`;
  const folder = !node.url && !node.separator;
  return (
    <div
      id={domId}
      draggable={!ROOT_FOLDERS.has(node.id)}
      aria-selected={chosen}
      title={node.url ?? node.title}
      className={cn(
        "group relative flex min-w-0 cursor-default items-center text-left",
        view === "cards" && "h-[4.25rem] gap-3 rounded-xl border border-zinc-800 bg-zinc-900 px-3 shadow-sm hover:border-zinc-700 hover:shadow-md",
        view === "list" && "h-10 gap-3 rounded-lg px-2 hover:bg-zinc-900",
        view === "headlines" && "h-8 gap-2 rounded-md px-2 hover:bg-zinc-900",
        chosen && "bg-sky-950/50 ring-1 ring-sky-800",
        dropClass,
      )}
      onClick={onClick}
      onContextMenu={onContextMenu}
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onDragOver={onDragOver}
      onDrop={onDrop}
    >
      {node.separator ? (
        <div id={`${domId}-line`} role="separator" className="h-px flex-1 bg-zinc-700" />
      ) : node.url ? (
        <FavIcon id={`${domId}-icon`} url={node.url} className={view === "cards" ? "size-5" : "size-4"} />
      ) : (
        <Folder id={`${domId}-icon`} className={cn("shrink-0 text-amber-400", view === "cards" ? "size-5" : "size-4")} aria-hidden />
      )}
      {node.separator ? (
        <span id={`${domId}-title`} className="shrink-0 text-xs text-zinc-500">
          Divider
        </span>
      ) : (
        <span id={`${domId}-copy`} className="min-w-0 flex-1">
          <span id={`${domId}-title`} className={cn("block truncate", view === "headlines" ? "text-sm" : "text-sm font-medium")}>
            {node.title || node.url}
          </span>
          {view === "headlines" ? null : (
            <span id={`${domId}-meta`} className="block truncate text-xs text-zinc-500">
              {node.url ? hostOf(node.url) : `${node.children?.length ?? 0} items`}
            </span>
          )}
        </span>
      )}
      {view === "headlines" || node.separator ? null : (
        <span
          id={`${domId}-actions`}
          className="flex shrink-0 items-center opacity-0 group-hover:opacity-100 focus-within:opacity-100"
          onClick={(event) => event.stopPropagation()}
        >
          {folder ? null : (
            <IconButton id={`${domId}-edit`} label="Edit" className="size-7" onClick={onEdit}>
              <Pencil id={`${domId}-edit-icon`} aria-hidden />
            </IconButton>
          )}
          {folder ? (
            <IconButton id={`${domId}-edit`} label="Rename" className="size-7" onClick={onEdit}>
              <Pencil id={`${domId}-edit-icon`} aria-hidden />
            </IconButton>
          ) : null}
          <IconButton id={`${domId}-delete`} label="Delete" className="size-7" onClick={onDelete}>
            <Trash2 id={`${domId}-delete-icon`} aria-hidden />
          </IconButton>
        </span>
      )}
    </div>
  );
}

type MoveDialogProps = {
  tree: BookmarkTree | null;
  ids: string[];
  onClose: () => void;
  onMove: (parentId: string) => void;
};

function MoveDialog({ tree, ids, onClose, onMove }: MoveDialogProps) {
  const folders = listFolders(tree).filter(
    (folder) => !ids.includes(folder.id) && !ids.some((id) => bookmarkContains(tree, id, folder.id)),
  );
  const [parentId, setParentId] = useState(folders[0]?.id ?? "other");
  const blocked = ids.includes(parentId) || ids.some((id) => bookmarkContains(tree, id, parentId));

  return (
    <Dialog id="bookmarks-move-dialog" title={ids.length > 1 ? `Move ${ids.length} bookmarks` : "Move to folder"} open onClose={onClose} panelClassName="max-w-md">
      <form
        id="bookmarks-move-form"
        className="flex flex-col gap-3"
        onSubmit={(event) => {
          event.preventDefault();
          if (!blocked && parentId) {
            onMove(parentId);
          }
        }}
      >
        <label id="bookmarks-move-folder-label" className="flex flex-col gap-1 text-xs text-zinc-400">
          Folder
          <select
            id="bookmarks-move-folder"
            value={parentId}
            onChange={(event) => setParentId(event.target.value)}
            className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
          >
            {folders.map((folder) => (
              <option key={folder.id} id={`bookmarks-move-folder-${folder.id}`} value={folder.id}>
                {`${"  ".repeat(folder.depth)}${folder.title}`}
              </option>
            ))}
          </select>
        </label>
        <div id="bookmarks-move-actions" className="flex justify-end gap-2 pt-1">
          <Button id="bookmarks-move-cancel" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button id="bookmarks-move-save" type="submit" variant="primary" disabled={blocked || !parentId}>
            Move
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

type BookmarkDialogProps = {
  editor: EditorState;
  onClose: () => void;
  onSave: (title: string, url: string) => void;
};

function BookmarkDialog({ editor, onClose, onSave }: BookmarkDialogProps) {
  const node = editor.mode === "edit" ? editor.node : null;
  const isLink = editor.mode === "add-link" || !!node?.url;
  const [title, setTitle] = useState(node?.title ?? "");
  const [url, setUrl] = useState(node?.url ?? "https://");
  const valid = title.trim().length > 0 && (!isLink || url.trim().length > 0);
  const heading = editor.mode === "add-folder" ? "New folder" : editor.mode === "add-link" ? "Add bookmark" : "Edit";

  return (
    <Dialog id="bookmark-dialog" title={heading} open onClose={onClose} panelClassName="max-w-md">
      <form
        id="bookmark-dialog-form"
        className="flex flex-col gap-3"
        onSubmit={(event) => {
          event.preventDefault();
          if (valid) {
            onSave(title.trim(), url.trim());
          }
        }}
      >
        <label id="bookmark-dialog-title-label" className="flex flex-col gap-1 text-xs text-zinc-400">
          Name
          <input
            id="bookmark-dialog-title"
            autoFocus
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
          />
        </label>
        {isLink ? (
          <label id="bookmark-dialog-url-label" className="flex flex-col gap-1 text-xs text-zinc-400">
            URL
            <input
              id="bookmark-dialog-url"
              value={url}
              spellCheck={false}
              onChange={(event) => setUrl(event.target.value)}
              className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
            />
          </label>
        ) : null}
        <div id="bookmark-dialog-actions" className="flex justify-end gap-2 pt-1">
          <Button id="bookmark-dialog-cancel" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button id="bookmark-dialog-save" type="submit" variant="primary" disabled={!valid}>
            Save
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
