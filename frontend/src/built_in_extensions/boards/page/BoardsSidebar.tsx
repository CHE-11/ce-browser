import { useEffect, useMemo, useRef, useState, type DragEvent, type ReactNode } from "react";
import { Archive, ChevronDown, ChevronRight, Clock, FolderPlus, Hash, Inbox, Search, Shuffle, SquareStack, Tags, Trash2, X } from "lucide-react";
import { ContextMenu, ContextMenuItem } from "@/components/ContextMenu";
import { IconButton } from "@/components/IconButton";
import { createBoardFolder, duplicateBoardFolder, moveBoardFolder, renameBoardFolder, updateBoardFolder } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import type { BoardsSnapshot } from "@type/browser";
import { FolderGlyph } from "../folder-icons";
import { DRAG_FOLDER, DRAG_PINS, UNCATEGORIZED, childFolders, folderCounts, folderPath, tagCounts } from "../folder-tree";
import { dropEntries } from "../import/walk-drop";
import { canMoveFolder, dragHas, draggedFolder, draggedPins, isBoardsDrag, pinDropEffect } from "../drag";
import { LIBRARY_LABELS, RECENT_LIMIT, sameScope, type BoardsScope, type LibraryScopeKind } from "../scopes";
import { FolderContextMenu } from "./FolderContextMenu";

const EXPANDED_KEY = "ce-boards-expanded";
const FOLDERS_OPEN_KEY = "ce-boards-folders-open";

/** Where pins were dropped in the sidebar. */
export type PinDropTarget = { kind: "folder"; id: string } | { kind: "uncategorized" } | { kind: "trash" };

type BoardsSidebarProps = {
  boards: BoardsSnapshot;
  scope: BoardsScope;
  onScope: (scope: BoardsScope) => void;
  /** Id of the folder whose name is being edited. */
  renaming: string | null;
  onRenaming: (id: string | null) => void;
  onDeleteFolder: (id: string) => void;
  onExportFolder: (id: string) => void;
  /** `copy` is true when Ctrl was held: the pins stay where they were as well. */
  onDropPins: (ids: string[], target: PinDropTarget, copy: boolean) => void;
  /** Files and folders dropped from outside. `folderId` is a folder, or `UNCATEGORIZED` for the top level. */
  onDropFiles: (entries: FileSystemEntry[], folderId: string) => void;
  onError: (reason: unknown) => void;
};

function readExpanded() {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(EXPANDED_KEY) ?? "[]");
    return new Set(Array.isArray(stored) ? stored.filter((item): item is string => typeof item === "string") : []);
  } catch {
    return new Set<string>();
  }
}

/** Files or folders dragged in from outside the browser. */
function isFileDrag(event: DragEvent) {
  return dragHas(event, "Files") && !isBoardsDrag(event);
}

export function BoardsSidebar({ boards, scope, onScope, renaming, onRenaming, onDeleteFolder, onExportFolder, onDropPins, onDropFiles, onError }: BoardsSidebarProps) {
  const [expanded, setExpanded] = useState<Set<string>>(readExpanded);
  const [foldersOpen, setFoldersOpen] = useState(() => localStorage.getItem(FOLDERS_OPEN_KEY) !== "0");
  const [menu, setMenu] = useState<{ id: string | null; x: number; y: number } | null>(null);
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const counts = useMemo(() => folderCounts(boards), [boards]);

  const library = useMemo(() => {
    let all = 0;
    let uncategorized = 0;
    let untagged = 0;
    let recent = 0;
    let trash = 0;
    for (const pin of boards.pins) {
      if (pin.deletedAt !== null) {
        trash += 1;
        continue;
      }
      all += 1;
      if (pin.folderIds.length === 0) {
        uncategorized += 1;
      }
      if (pin.tags.length === 0) {
        untagged += 1;
      }
      if (pin.lastOpenedAt > 0) {
        recent += 1;
      }
    }
    return { all, uncategorized, untagged, recent: Math.min(recent, RECENT_LIMIT), trash, tags: tagCounts(boards).length };
  }, [boards]);

  useEffect(() => {
    localStorage.setItem(EXPANDED_KEY, JSON.stringify([...expanded]));
  }, [expanded]);

  useEffect(() => {
    localStorage.setItem(FOLDERS_OPEN_KEY, foldersOpen ? "1" : "0");
  }, [foldersOpen]);

  useEffect(() => {
    if (scope.kind !== "folder") {
      return;
    }
    const ancestors = folderPath(boards, scope.id).map((folder) => folder.id);
    ancestors.pop();
    if (ancestors.some((id) => !expanded.has(id))) {
      setExpanded((current) => new Set([...current, ...ancestors]));
    }
  }, [scope, boards, expanded]);

  /** Folders that match the filter, plus every folder above one that does. */
  const visible = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    if (!needle) {
      return null;
    }
    const shown = new Set<string>();
    for (const folder of boards.folders) {
      if (folder.name.toLowerCase().includes(needle)) {
        for (const item of folderPath(boards, folder.id)) {
          shown.add(item.id);
        }
      }
    }
    return shown;
  }, [boards, filter]);

  const expand = (id: string, open?: boolean) =>
    setExpanded((current) => {
      const next = new Set(current);
      if (open ?? !next.has(id)) {
        next.add(id);
      } else {
        next.delete(id);
      }
      return next;
    });

  const setTreeExpanded = (open: boolean) => setExpanded(open ? new Set(boards.folders.map((folder) => folder.id)) : new Set());

  const newFolder = (parentId: string | null) => {
    setFoldersOpen(true);
    void createBoardFolder("New folder", parentId)
      .then((folder) => {
        if (parentId) {
          expand(parentId, true);
        }
        onRenaming(folder.id);
      })
      .catch(onError);
  };

  const commitRename = (id: string, name: string) => {
    onRenaming(null);
    const trimmed = name.trim();
    if (trimmed) {
      void renameBoardFolder(id, trimmed).catch(onError);
    }
  };

  const allowDrop = (event: DragEvent, key: string, folders: boolean) => {
    if (folders && isFileDrag(event)) {
      event.preventDefault();
      event.stopPropagation();
      event.dataTransfer.dropEffect = "copy";
      setDropTarget(key);
      return;
    }
    const pins = dragHas(event, DRAG_PINS);
    if (!pins && !(folders && dragHas(event, DRAG_FOLDER))) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    event.dataTransfer.dropEffect = pins ? pinDropEffect(event) : "move";
    setDropTarget(key);
  };

  const dropOnFolder = (event: DragEvent, folderId: string | null) => {
    event.preventDefault();
    event.stopPropagation();
    setDropTarget(null);
    if (isFileDrag(event)) {
      const entries = dropEntries(event.dataTransfer);
      if (entries.length > 0) {
        onDropFiles(entries, folderId ?? UNCATEGORIZED);
        if (folderId) {
          expand(folderId, true);
        }
      }
      return;
    }
    const pins = draggedPins(event);
    if (pins.length > 0 && folderId) {
      onDropPins(pins, { kind: "folder", id: folderId }, event.ctrlKey);
      return;
    }
    const folder = draggedFolder(event);
    if (folder && canMoveFolder(boards, folder, folderId)) {
      void moveBoardFolder(folder, folderId).catch(onError);
      if (folderId) {
        expand(folderId, true);
      }
    }
  };

  const libraryRow = (kind: LibraryScopeKind, icon: ReactNode, count: number | null, drop?: PinDropTarget) => {
    const target: BoardsScope = kind === "random" ? { kind, seed: Date.now() } : ({ kind } as BoardsScope);
    const active = kind === "random" ? scope.kind === "random" : sameScope(scope, target);
    const key = `library-${kind}`;
    return (
      <TreeRow
        id={`boards-library-${kind}`}
        depth={0}
        icon={icon}
        label={LIBRARY_LABELS[kind]}
        count={count}
        active={active}
        dropActive={dropTarget === key}
        onClick={() => onScope(target)}
        onDragOver={drop ? (event) => allowDrop(event, key, false) : undefined}
        onDragLeave={() => setDropTarget((current) => (current === key ? null : current))}
        onDrop={
          drop
            ? (event) => {
                event.preventDefault();
                setDropTarget(null);
                const pins = draggedPins(event);
                if (pins.length > 0) {
                  onDropPins(pins, drop, false);
                }
              }
            : undefined
        }
      />
    );
  };

  const renderFolders = (parentId: string | null, depth: number): ReactNode =>
    childFolders(boards, parentId)
      .filter((folder) => !visible || visible.has(folder.id))
      .map((folder) => {
        const hasChildren = boards.folders.some((item) => item.parentId === folder.id);
        const open = visible ? true : expanded.has(folder.id);
        const key = `folder-${folder.id}`;
        return (
          <li id={`boards-tree-${key}`} key={key}>
            <TreeRow
              id={`boards-tree-${key}-row`}
              depth={depth}
              icon={<FolderGlyph id={`boards-tree-${key}-glyph`} folder={folder} open={open && hasChildren} />}
              label={folder.name}
              count={counts.get(folder.id) ?? 0}
              active={scope.kind === "folder" && scope.id === folder.id}
              expandable={hasChildren}
              expanded={open}
              renaming={renaming === folder.id}
              dropActive={dropTarget === key}
              onToggle={() => expand(folder.id)}
              onClick={() => onScope({ kind: "folder", id: folder.id })}
              onRename={(name) => commitRename(folder.id, name)}
              onCancelRename={() => onRenaming(null)}
              onContextMenu={(x, y) => setMenu({ id: folder.id, x, y })}
              onDragStart={(event) => {
                event.dataTransfer.setData(DRAG_FOLDER, folder.id);
                event.dataTransfer.effectAllowed = "move";
              }}
              onDragOver={(event) => allowDrop(event, key, true)}
              onDragLeave={() => setDropTarget((current) => (current === key ? null : current))}
              onDrop={(event) => dropOnFolder(event, folder.id)}
            />
            {open && hasChildren ? <ul id={`boards-tree-${key}-children`}>{renderFolders(folder.id, depth + 1)}</ul> : null}
          </li>
        );
      });

  const menuFolder = menu?.id ? boards.folders.find((folder) => folder.id === menu.id) : undefined;
  const closeMenu = () => setMenu(null);

  return (
    <aside
      id="boards-sidebar"
      className={cn("relative flex w-60 shrink-0 flex-col border-r border-zinc-800 bg-zinc-950", dropTarget === "sidebar" && "ring-2 ring-inset ring-sky-500")}
      onDragOver={(event) => {
        if (isFileDrag(event)) {
          event.preventDefault();
          event.dataTransfer.dropEffect = "copy";
          setDropTarget("sidebar");
        }
      }}
      onDragLeave={(event) => {
        const next = event.relatedTarget;
        if (!(next instanceof Node && event.currentTarget.contains(next))) {
          setDropTarget(null);
        }
      }}
      onDrop={(event) => {
        if (isFileDrag(event)) {
          dropOnFolder(event, null);
        }
      }}
    >
      <div id="boards-sidebar-header" className="flex items-center gap-1 px-4 pb-2 pt-4">
        <h1 id="boards-sidebar-title" className="flex-1 text-base font-semibold">
          Boards
        </h1>
        <IconButton id="boards-sidebar-new-folder" label="New folder" onClick={() => newFolder(null)}>
          <FolderPlus aria-hidden />
        </IconButton>
      </div>

      <div
        id="boards-sidebar-tree"
        className="min-h-0 flex-1 overflow-y-auto px-2 pb-4"
        onContextMenu={(event) => {
          event.preventDefault();
          setMenu({ id: null, x: event.clientX, y: event.clientY });
        }}
      >
        <ul id="boards-library">
          <li id="boards-library-all-item">{libraryRow("all", <SquareStack aria-hidden />, library.all)}</li>
          <li id="boards-library-uncategorized-item">
            {libraryRow("uncategorized", <Inbox aria-hidden />, library.uncategorized, { kind: "uncategorized" })}
          </li>
          <li id="boards-library-untagged-item">{libraryRow("untagged", <Hash aria-hidden />, library.untagged)}</li>
          <li id="boards-library-recent-item">{libraryRow("recent", <Clock aria-hidden />, library.recent)}</li>
          <li id="boards-library-random-item">{libraryRow("random", <Shuffle aria-hidden />, null)}</li>
          <li id="boards-library-tags-item">{libraryRow("tags", <Tags aria-hidden />, library.tags)}</li>
          <li id="boards-library-trash-item">{libraryRow("trash", <Trash2 aria-hidden />, library.trash, { kind: "trash" })}</li>
        </ul>

        <div
          id="boards-folders-header"
          className={cn("group mt-3 flex h-7 items-center gap-1 rounded-md pl-1 pr-1 text-xs font-medium text-zinc-500", (dropTarget === "root" || dropTarget === "root-files") && "bg-sky-900/60 ring-1 ring-sky-500")}
          onDragOver={(event) => {
            if (dragHas(event, DRAG_FOLDER) || isFileDrag(event)) {
              event.preventDefault();
              event.stopPropagation();
              setDropTarget(isFileDrag(event) ? "root-files" : "root");
            }
          }}
          onDragLeave={() => setDropTarget((current) => (current === "root" || current === "root-files" ? null : current))}
          onDrop={(event) => dropOnFolder(event, null)}
        >
          <button
            id="boards-folders-toggle"
            type="button"
            aria-expanded={foldersOpen}
            className={cn("flex min-w-0 flex-1 items-center gap-1 rounded text-left hover:text-zinc-300", focusRing)}
            onClick={() => setFoldersOpen((current) => !current)}
          >
            {foldersOpen ? <ChevronDown id="boards-folders-toggle-icon" className="size-3.5" aria-hidden /> : <ChevronRight id="boards-folders-toggle-icon" className="size-3.5" aria-hidden />}
            <span id="boards-folders-label">Folders ({boards.folders.length})</span>
          </button>
          <IconButton id="boards-folders-new" label="New folder" className="size-6 opacity-0 group-hover:opacity-100 focus-visible:opacity-100" onClick={() => newFolder(null)}>
            <FolderPlus aria-hidden />
          </IconButton>
        </div>

        {foldersOpen ? (
          boards.folders.length === 0 ? (
            <p id="boards-tree-empty" className="px-3 py-2 text-xs text-zinc-500">
              No folders yet. Create one, or import a folder from your computer.
            </p>
          ) : visible && visible.size === 0 ? (
            <p id="boards-tree-no-match" className="px-3 py-2 text-xs text-zinc-500">
              No folder matches.
            </p>
          ) : (
            <ul id="boards-tree-root">{renderFolders(null, 0)}</ul>
          )
        ) : null}
      </div>

      <label id="boards-sidebar-filter" className="m-2 flex h-8 items-center gap-2 rounded-md border border-zinc-800 bg-zinc-900 px-2 focus-within:border-sky-500">
        <Search id="boards-sidebar-filter-icon" className="size-3.5 shrink-0 text-zinc-500" aria-hidden />
        <input
          id="boards-sidebar-filter-input"
          value={filter}
          placeholder="Filter folders"
          className="min-w-0 flex-1 bg-transparent text-xs text-zinc-100 outline-none placeholder:text-zinc-500"
          onChange={(event) => setFilter(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              setFilter("");
            }
          }}
        />
        {filter ? (
          <button id="boards-sidebar-filter-clear" type="button" aria-label="Clear filter" className="text-zinc-500 hover:text-zinc-200" onClick={() => setFilter("")}>
            <X id="boards-sidebar-filter-clear-icon" className="size-3.5" aria-hidden />
          </button>
        ) : null}
      </label>

      {menu && menuFolder ? (
        <FolderContextMenu
          folder={menuFolder}
          x={menu.x}
          y={menu.y}
          onClose={closeMenu}
          onNewSubfolder={() => newFolder(menuFolder.id)}
          onRename={() => onRenaming(menuFolder.id)}
          onUpdate={(patch) => void updateBoardFolder(menuFolder.id, patch).catch(onError)}
          onDuplicate={() =>
            void duplicateBoardFolder(menuFolder.id)
              .then((copy) => onScope({ kind: "folder", id: copy.id }))
              .catch(onError)
          }
          onExport={() => onExportFolder(menuFolder.id)}
          onMoveToTop={() => void moveBoardFolder(menuFolder.id, null).catch(onError)}
          onExpandAll={() => setTreeExpanded(true)}
          onCollapseAll={() => setTreeExpanded(false)}
          onDelete={() => onDeleteFolder(menuFolder.id)}
        />
      ) : null}
      {menu && !menu.id ? (
        <ContextMenu id="boards-tree-menu" label="Folders" x={menu.x} y={menu.y} onClose={closeMenu}>
          <ContextMenuItem
            id="boards-tree-menu-new-folder"
            icon={<FolderPlus />}
            label="New folder"
            onClick={() => {
              closeMenu();
              newFolder(null);
            }}
          />
          <ContextMenuItem
            id="boards-tree-menu-expand"
            icon={<Archive />}
            label="Expand all folders"
            onClick={() => {
              closeMenu();
              setTreeExpanded(true);
            }}
          />
        </ContextMenu>
      ) : null}
      {dropTarget === "sidebar" || dropTarget === "root-files" ? (
        <p id="boards-sidebar-drop-hint" className="pointer-events-none absolute inset-x-2 bottom-12 rounded-md bg-sky-950/95 px-3 py-2 text-center text-xs text-sky-100 ring-1 ring-sky-500">
          Folders become new folders. Drop on a folder to import into it.
        </p>
      ) : null}
    </aside>
  );
}

type TreeRowProps = {
  id: string;
  depth: number;
  icon: ReactNode;
  label: string;
  /** Null hides the count. */
  count: number | null;
  active: boolean;
  expandable?: boolean;
  expanded?: boolean;
  renaming?: boolean;
  dropActive?: boolean;
  onToggle?: () => void;
  onClick: () => void;
  onRename?: (name: string) => void;
  onCancelRename?: () => void;
  onContextMenu?: (x: number, y: number) => void;
  onDragStart?: (event: DragEvent) => void;
  onDragOver?: (event: DragEvent) => void;
  onDragLeave?: () => void;
  onDrop?: (event: DragEvent) => void;
};

function TreeRow({
  id,
  depth,
  icon,
  label,
  count,
  active,
  expandable,
  expanded,
  renaming,
  dropActive,
  onToggle,
  onClick,
  onRename,
  onCancelRename,
  onContextMenu,
  onDragStart,
  onDragOver,
  onDragLeave,
  onDrop,
}: TreeRowProps) {
  const cancelled = useRef(false);
  const indent = { paddingLeft: `${depth * 14 + 4}px` };
  const rowClass = cn(
    "flex h-7 w-full items-center gap-1.5 rounded-md pr-2 text-left text-[13px] text-zinc-300 hover:bg-zinc-900 hover:text-zinc-100",
    active && "bg-sky-950 text-sky-200 hover:bg-sky-950 hover:text-sky-100",
    dropActive && "bg-sky-900/60 ring-1 ring-sky-500",
  );

  if (renaming) {
    return (
      <div id={id} className={rowClass} style={indent}>
        <span id={`${id}-spacer`} className="size-4 shrink-0" />
        <span id={`${id}-icon`} className="inline-flex size-4 shrink-0 items-center justify-center [&_svg]:size-4">
          {icon}
        </span>
        <input
          id={`${id}-rename`}
          autoFocus
          defaultValue={label}
          aria-label="Name"
          className="h-6 min-w-0 flex-1 rounded border border-sky-500 bg-zinc-950 px-1.5 text-sm text-zinc-100 outline-none"
          onFocus={(event) => {
            cancelled.current = false;
            event.currentTarget.select();
          }}
          onBlur={(event) => {
            if (!cancelled.current) {
              onRename?.(event.currentTarget.value);
            }
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.currentTarget.blur();
            } else if (event.key === "Escape") {
              event.preventDefault();
              cancelled.current = true;
              onCancelRename?.();
            }
          }}
        />
      </div>
    );
  }

  return (
    <div
      id={id}
      className={rowClass}
      style={indent}
      draggable={Boolean(onDragStart)}
      onDragStart={onDragStart}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
      onContextMenu={(event) => {
        if (onContextMenu) {
          event.preventDefault();
          event.stopPropagation();
          onContextMenu(event.clientX, event.clientY);
        }
      }}
    >
      {expandable ? (
        <button
          id={`${id}-toggle`}
          type="button"
          aria-label={expanded ? "Collapse" : "Expand"}
          aria-expanded={expanded}
          className={cn("inline-flex size-4 shrink-0 items-center justify-center rounded text-zinc-500 hover:text-zinc-200", focusRing)}
          onClick={onToggle}
        >
          <ChevronRight id={`${id}-toggle-icon`} className={cn("size-3.5 transition-transform", expanded && "rotate-90")} aria-hidden />
        </button>
      ) : (
        <span id={`${id}-spacer`} className="size-4 shrink-0" />
      )}
      <button
        id={`${id}-button`}
        type="button"
        aria-current={active ? "page" : undefined}
        className={cn("flex min-w-0 flex-1 items-center gap-2 rounded text-left", focusRing)}
        onClick={onClick}
        onDoubleClick={expandable ? onToggle : undefined}
      >
        <span id={`${id}-icon`} className={cn("inline-flex size-4 shrink-0 items-center justify-center [&_svg]:size-4", active ? "text-sky-300" : "text-zinc-500")}>
          {icon}
        </span>
        <span id={`${id}-label`} className="min-w-0 flex-1 truncate">
          {label}
        </span>
        {count !== null && count > 0 ? (
          <span id={`${id}-count`} className="shrink-0 text-[11px] tabular-nums text-zinc-500">
            {count}
          </span>
        ) : null}
      </button>
    </div>
  );
}
