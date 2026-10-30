import { useCallback, useMemo, useState, type DragEvent, type MouseEvent } from "react";
import {
  ChevronDown,
  ChevronRight,
  ExternalLink,
  Folder,
  FolderOpen,
  FolderPlus,
  Pencil,
  Plus,
  Search,
  Trash2,
  Upload,
  Download,
} from "lucide-react";
import type { BookmarkNode } from "../../../types/browser";
import { Button } from "../components/Button";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "../components/ContextMenu";
import { Dialog } from "../components/Dialog";
import { FavIcon } from "../components/FavIcon";
import { IconButton } from "../components/IconButton";
import { PageShell } from "../components/PageShell";
import {
  addBookmark,
  createTab,
  deleteBookmark,
  exportBookmarksHtml,
  importBookmarksHtml,
  moveBookmark,
  updateBookmark,
} from "../lib/browser-client";
import { cn } from "../lib/cn";
import { findFolder, useBookmarks } from "../lib/use-browser";

type EditorState =
  | { mode: "add-link"; parentId: string }
  | { mode: "add-folder"; parentId: string }
  | { mode: "edit"; node: BookmarkNode };

type MenuState = { node: BookmarkNode; x: number; y: number };

const DRAG_TYPE = "application/x-ce-bookmark";

export function BookmarksPage() {
  const tree = useBookmarks();
  const [selectedId, setSelectedId] = useState("bar");
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set(["bar", "other"]));
  const [query, setQuery] = useState("");
  const [editor, setEditor] = useState<EditorState | null>(null);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<string | null>(null);

  const selected = findFolder(tree, selectedId) ?? tree?.bar ?? null;

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

  const report = (action: Promise<unknown>, success?: string) => {
    action
      .then(() => setStatus(success ?? null))
      .catch((reason: unknown) => setStatus(String(reason)));
  };

  const closeMenu = useCallback(() => setMenu(null), []);

  const toggleExpanded = (id: string) => {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const onDragStart = (event: DragEvent, node: BookmarkNode) => {
    event.dataTransfer.setData(DRAG_TYPE, node.id);
    if (node.url) {
      event.dataTransfer.setData("text/uri-list", node.url);
    }
    event.dataTransfer.effectAllowed = "move";
  };

  const draggedId = (event: DragEvent) => event.dataTransfer.getData(DRAG_TYPE);

  const allowDrop = (event: DragEvent, key: string) => {
    if (event.dataTransfer.types.includes(DRAG_TYPE)) {
      event.preventDefault();
      setDropTarget(key);
    }
  };

  const dropInto = (event: DragEvent, folder: BookmarkNode) => {
    event.preventDefault();
    setDropTarget(null);
    const id = draggedId(event);
    if (!id || id === folder.id) {
      return;
    }
    report(moveBookmark(id, folder.id));
  };

  const dropAt = (event: DragEvent, index: number) => {
    event.preventDefault();
    setDropTarget(null);
    const id = draggedId(event);
    if (!id || !selected || results) {
      return;
    }
    const rect = event.currentTarget.getBoundingClientRect();
    const after = event.clientY > rect.top + rect.height / 2;
    report(moveBookmark(id, selected.id, after ? index + 1 : index));
  };

  const openRowMenu = (event: MouseEvent, node: BookmarkNode) => {
    event.preventDefault();
    setMenu({ node, x: event.clientX, y: event.clientY });
  };

  const renderFolder = (node: BookmarkNode, depth: number) => {
    const folders = (node.children ?? []).filter((child) => !child.separator && !child.url);
    const isOpen = expanded.has(node.id);
    const domId = `bookmarks-folder-${node.id}`;
    return (
      <div key={node.id} id={domId}>
        <div
          id={`${domId}-row`}
          className={cn(
            "flex h-8 cursor-default items-center gap-1 rounded-md pr-2 text-sm hover:bg-zinc-900",
            selectedId === node.id && !results && "bg-sky-950 text-sky-300 hover:bg-sky-950",
            dropTarget === `folder:${node.id}` && "ring-1 ring-sky-500",
          )}
          style={{ paddingLeft: depth * 14 + 4 }}
          onClick={() => {
            setSelectedId(node.id);
            setQuery("");
          }}
          onDragOver={(event) => allowDrop(event, `folder:${node.id}`)}
          onDragLeave={() => setDropTarget(null)}
          onDrop={(event) => dropInto(event, node)}
        >
          <button
            id={`${domId}-toggle`}
            type="button"
            aria-label={isOpen ? "Collapse" : "Expand"}
            className={cn("inline-flex size-5 items-center justify-center text-zinc-500", folders.length === 0 && "invisible")}
            onClick={(event) => {
              event.stopPropagation();
              toggleExpanded(node.id);
            }}
          >
            {isOpen ? <ChevronDown id={`${domId}-open`} className="size-3.5" /> : <ChevronRight id={`${domId}-closed`} className="size-3.5" />}
          </button>
          {selectedId === node.id ? (
            <FolderOpen id={`${domId}-icon`} className="size-4 shrink-0 text-amber-400" aria-hidden />
          ) : (
            <Folder id={`${domId}-icon`} className="size-4 shrink-0 text-amber-400" aria-hidden />
          )}
          <span id={`${domId}-title`} className="ml-1 truncate">
            {node.title}
          </span>
        </div>
        {isOpen ? folders.map((child) => renderFolder(child, depth + 1)) : null}
      </div>
    );
  };

  return (
    <PageShell
      id="bookmarks-page"
      page="bookmarks"
      title="Bookmarks"
      actions={
        <>
          <label id="bookmarks-page-search" className="flex h-8 w-64 items-center gap-2 rounded-md border border-zinc-700 bg-zinc-900 px-2">
            <Search id="bookmarks-page-search-icon" className="size-4 text-zinc-500" aria-hidden />
            <input
              id="bookmarks-page-search-input"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search bookmarks"
              className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-zinc-500"
            />
          </label>
          <Button
            id="bookmarks-page-import"
            variant="outline"
            onClick={() =>
              void importBookmarksHtml()
                .then((count) => count !== null && setStatus(`Imported ${count} bookmarks into Other bookmarks.`))
                .catch((reason: unknown) => setStatus(String(reason)))
            }
          >
            <Upload id="bookmarks-page-import-icon" className="size-4" aria-hidden />
            Import
          </Button>
          <Button
            id="bookmarks-page-export"
            variant="outline"
            onClick={() =>
              void exportBookmarksHtml()
                .then((saved) => saved && setStatus("Bookmarks exported."))
                .catch((reason: unknown) => setStatus(String(reason)))
            }
          >
            <Download id="bookmarks-page-export-icon" className="size-4" aria-hidden />
            Export
          </Button>
        </>
      }
    >
      <div id="bookmarks-page-layout" className="flex min-h-full gap-6">
        <aside id="bookmarks-page-folders" className="w-64 shrink-0">
          {tree ? (
            <>
              {renderFolder(tree.bar, 0)}
              {renderFolder(tree.other, 0)}
            </>
          ) : null}
        </aside>
        <section id="bookmarks-page-items" className="min-w-0 flex-1">
          <div id="bookmarks-page-items-header" className="mb-3 flex items-center justify-between gap-3">
            <h2 id="bookmarks-page-items-title" className="truncate text-sm font-semibold text-zinc-300">
              {results ? `${results.length} results for “${query.trim()}”` : selected?.title}
            </h2>
            {!results && selected ? (
              <div id="bookmarks-page-items-actions" className="flex gap-2">
                <Button id="bookmarks-page-add-link" size="sm" onClick={() => setEditor({ mode: "add-link", parentId: selected.id })}>
                  <Plus id="bookmarks-page-add-link-icon" className="size-3.5" aria-hidden />
                  Add bookmark
                </Button>
                <Button id="bookmarks-page-add-folder" size="sm" onClick={() => setEditor({ mode: "add-folder", parentId: selected.id })}>
                  <FolderPlus id="bookmarks-page-add-folder-icon" className="size-3.5" aria-hidden />
                  New folder
                </Button>
              </div>
            ) : null}
          </div>
          {status ? (
            <div id="bookmarks-page-status" className="mb-3 rounded-md border border-zinc-800 bg-zinc-900 px-3 py-2 text-xs text-zinc-300">
              {status}
            </div>
          ) : null}
          <div id="bookmarks-page-rows" className="flex flex-col rounded-lg border border-zinc-800">
            {rows.length === 0 ? (
              <div id="bookmarks-page-empty" className="px-4 py-10 text-center text-sm text-zinc-500">
                {results ? "No bookmarks match that search." : "This folder is empty. Drag bookmarks here or add one."}
              </div>
            ) : null}
            {rows.map((node, index) => {
              const domId = `bookmarks-row-${node.id}`;
              if (node.separator) {
                return (
                  <div
                    key={node.id}
                    id={domId}
                    draggable
                    className={cn(
                      "group flex h-8 items-center gap-3 border-b border-zinc-800 px-3 last:border-b-0 hover:bg-zinc-900",
                      dropTarget === `row:${node.id}` && "bg-sky-950/40",
                    )}
                    onDragStart={(event) => onDragStart(event, node)}
                    onDragOver={(event) => allowDrop(event, `row:${node.id}`)}
                    onDragLeave={() => setDropTarget(null)}
                    onDrop={(event) => dropAt(event, index)}
                    onContextMenu={(event) => openRowMenu(event, node)}
                  >
                    <div id={`${domId}-line`} role="separator" className="h-px flex-1 bg-zinc-700" />
                    <span id={`${domId}-title`} className="shrink-0 text-xs text-zinc-500">
                      Divider
                    </span>
                    <IconButton id={`${domId}-delete`} label="Delete" onClick={() => report(deleteBookmark(node.id))}>
                      <Trash2 id={`${domId}-delete-icon`} aria-hidden />
                    </IconButton>
                  </div>
                );
              }
              return (
                <div
                  key={node.id}
                  id={domId}
                  draggable
                  className={cn(
                    "group flex h-11 items-center gap-3 border-b border-zinc-800 px-3 last:border-b-0 hover:bg-zinc-900",
                    dropTarget === `row:${node.id}` && "bg-sky-950/40",
                  )}
                  onDragStart={(event) => onDragStart(event, node)}
                  onDragOver={(event) => allowDrop(event, `row:${node.id}`)}
                  onDragLeave={() => setDropTarget(null)}
                  onDrop={(event) => (node.url ? dropAt(event, index) : dropInto(event, node))}
                  onContextMenu={(event) => openRowMenu(event, node)}
                  onDoubleClick={() => {
                    if (node.separator) {
                      return;
                    }
                    if (node.url) {
                      void createTab(node.url);
                    } else {
                      setSelectedId(node.id);
                      setExpanded((current) => new Set(current).add(selected?.id ?? "bar"));
                    }
                  }}
                >
                  {node.url ? (
                    <FavIcon id={`${domId}-icon`} url={node.url} />
                  ) : (
                    <Folder id={`${domId}-icon`} className="size-4 shrink-0 text-amber-400" aria-hidden />
                  )}
                  <span id={`${domId}-title`} className="min-w-0 max-w-[45%] truncate text-sm">
                    {node.title || node.url}
                  </span>
                  <span id={`${domId}-url`} className="min-w-0 flex-1 truncate text-xs text-zinc-500">
                    {node.url ?? `${node.children?.length ?? 0} items`}
                  </span>
                  <div id={`${domId}-actions`} className="flex shrink-0 items-center opacity-0 group-hover:opacity-100 focus-within:opacity-100">
                    {node.url ? (
                      <IconButton id={`${domId}-open`} label="Open in new tab" onClick={() => void createTab(node.url ?? "")}>
                        <ExternalLink id={`${domId}-open-icon`} aria-hidden />
                      </IconButton>
                    ) : null}
                    <IconButton id={`${domId}-edit`} label="Edit" onClick={() => setEditor({ mode: "edit", node })}>
                      <Pencil id={`${domId}-edit-icon`} aria-hidden />
                    </IconButton>
                    <IconButton id={`${domId}-delete`} label="Delete" onClick={() => report(deleteBookmark(node.id))}>
                      <Trash2 id={`${domId}-delete-icon`} aria-hidden />
                    </IconButton>
                  </div>
                </div>
              );
            })}
          </div>
        </section>
      </div>

      {menu ? (
        <ContextMenu id="bookmarks-menu" label="Bookmark" x={menu.x} y={menu.y} onClose={closeMenu}>
          {menu.node.separator ? null : menu.node.url ? (
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
          ) : (
            <ContextMenuItem
              id="bookmarks-menu-open-all"
              icon={<ExternalLink />}
              label="Open all in tabs"
              onClick={() => {
                for (const child of menu.node.children ?? []) {
                  if (child.url) {
                    void createTab(child.url, true);
                  }
                }
                closeMenu();
              }}
            />
          )}
          {menu.node.separator ? null : (
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
          <ContextMenuItem
            id="bookmarks-menu-delete"
            icon={<Trash2 />}
            label="Delete"
            danger
            onClick={() => {
              report(deleteBookmark(menu.node.id));
              closeMenu();
            }}
          />
        </ContextMenu>
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
    </PageShell>
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