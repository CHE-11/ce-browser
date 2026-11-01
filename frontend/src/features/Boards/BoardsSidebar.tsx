import { useEffect, useMemo, useRef, useState, type DragEvent, type ReactNode } from "react";
import { ChevronRight, Folder, FolderOpen, FolderPlus, LayoutGrid, Pencil, Plus, Search, SquareStack, Trash2, Undo2 } from "lucide-react";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "@/components/ContextMenu";
import { IconButton } from "@/components/IconButton";
import {
  createBoard,
  createBoardFolder,
  moveBoard,
  moveBoardFolder,
  movePins,
  renameBoard,
  renameBoardFolder,
} from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import type { BoardsSnapshot } from "@type/browser";
import { DRAG_BOARD, DRAG_FOLDER, DRAG_PINS, boardsIn, childFolders, folderPath, folderTree, pinCounts, type BoardsScope } from "./board-tree";

const EXPANDED_KEY = "ce-boards-expanded";

type MenuTarget = { kind: "root" } | { kind: "folder"; id: string } | { kind: "board"; id: string };

type BoardsSidebarProps = {
  boards: BoardsSnapshot;
  scope: BoardsScope;
  onScope: (scope: BoardsScope) => void;
  query: string;
  onQuery: (query: string) => void;
  /** Id of the folder or board whose name is being edited. */
  renaming: string | null;
  onRenaming: (id: string | null) => void;
  onDeleteFolder: (id: string) => void;
  onDeleteBoard: (id: string) => void;
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

function dragHas(event: DragEvent, type: string) {
  return event.dataTransfer.types.includes(type);
}

export function BoardsSidebar({
  boards,
  scope,
  onScope,
  query,
  onQuery,
  renaming,
  onRenaming,
  onDeleteFolder,
  onDeleteBoard,
  onError,
}: BoardsSidebarProps) {
  const [expanded, setExpanded] = useState<Set<string>>(readExpanded);
  const [menu, setMenu] = useState<{ target: MenuTarget; x: number; y: number } | null>(null);
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  const counts = useMemo(() => pinCounts(boards), [boards]);

  useEffect(() => {
    localStorage.setItem(EXPANDED_KEY, JSON.stringify([...expanded]));
  }, [expanded]);

  useEffect(() => {
    const folderId =
      scope.kind === "board" ? (boards.boards.find((board) => board.id === scope.id)?.folderId ?? null) : scope.kind === "folder" ? scope.id : null;
    const ancestors = folderPath(boards, folderId).map((folder) => folder.id);
    if (scope.kind === "folder") {
      ancestors.pop();
    }
    if (ancestors.some((id) => !expanded.has(id))) {
      setExpanded((current) => new Set([...current, ...ancestors]));
    }
  }, [scope, boards, expanded]);

  const folderCount = (folderId: string) => {
    const folders = folderTree(boards, folderId);
    return boards.boards.reduce((total, board) => total + (board.folderId && folders.has(board.folderId) ? (counts.get(board.id) ?? 0) : 0), 0);
  };

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

  const newFolder = (parentId: string | null) => {
    void createBoardFolder("New folder", parentId)
      .then((folder) => {
        if (parentId) {
          expand(parentId, true);
        }
        onRenaming(folder.id);
      })
      .catch(onError);
  };

  const newBoard = (folderId: string | null) => {
    void createBoard("New board", folderId)
      .then((board) => {
        if (folderId) {
          expand(folderId, true);
        }
        onScope({ kind: "board", id: board.id });
        onRenaming(board.id);
      })
      .catch(onError);
  };

  const commitRename = (kind: "folder" | "board", id: string, name: string) => {
    onRenaming(null);
    const trimmed = name.trim();
    if (!trimmed) {
      return;
    }
    void (kind === "folder" ? renameBoardFolder(id, trimmed) : renameBoard(id, trimmed)).catch(onError);
  };

  const dropOn = (event: DragEvent, folderId: string | null) => {
    event.preventDefault();
    event.stopPropagation();
    setDropTarget(null);
    const board = event.dataTransfer.getData(DRAG_BOARD);
    const folder = event.dataTransfer.getData(DRAG_FOLDER);
    if (board) {
      void moveBoard(board, folderId).catch(onError);
    } else if (folder && folder !== folderId) {
      void moveBoardFolder(folder, folderId).catch(onError);
    }
    if (folderId) {
      expand(folderId, true);
    }
  };

  const dropPins = (event: DragEvent, boardId: string) => {
    event.preventDefault();
    event.stopPropagation();
    setDropTarget(null);
    try {
      const ids: unknown = JSON.parse(event.dataTransfer.getData(DRAG_PINS) || "[]");
      if (Array.isArray(ids) && ids.length > 0) {
        void movePins(ids.filter((id): id is string => typeof id === "string"), boardId).catch(onError);
      }
    } catch (reason) {
      onError(reason);
    }
  };

  const renderFolder = (folderId: string | null, depth: number): ReactNode => (
    <>
      {childFolders(boards, folderId).map((folder) => {
        const open = expanded.has(folder.id);
        const key = `folder-${folder.id}`;
        return (
          <li id={`boards-tree-${key}`} key={key}>
            <TreeRow
              id={`boards-tree-${key}-row`}
              depth={depth}
              icon={open ? <FolderOpen aria-hidden /> : <Folder aria-hidden />}
              label={folder.name}
              count={folderCount(folder.id)}
              active={scope.kind === "folder" && scope.id === folder.id}
              expandable
              expanded={open}
              renaming={renaming === folder.id}
              dropActive={dropTarget === key}
              onToggle={() => expand(folder.id)}
              onClick={() => onScope({ kind: "folder", id: folder.id })}
              onRename={(name) => commitRename("folder", folder.id, name)}
              onCancelRename={() => onRenaming(null)}
              onContextMenu={(x, y) => setMenu({ target: { kind: "folder", id: folder.id }, x, y })}
              onDragStart={(event) => {
                event.dataTransfer.setData(DRAG_FOLDER, folder.id);
                event.dataTransfer.effectAllowed = "move";
              }}
              onDragOver={(event) => {
                if (dragHas(event, DRAG_BOARD) || dragHas(event, DRAG_FOLDER)) {
                  event.preventDefault();
                  event.stopPropagation();
                  setDropTarget(key);
                }
              }}
              onDragLeave={() => setDropTarget((current) => (current === key ? null : current))}
              onDrop={(event) => dropOn(event, folder.id)}
            />
            {open ? <ul id={`boards-tree-${key}-children`}>{renderFolder(folder.id, depth + 1)}</ul> : null}
          </li>
        );
      })}
      {boardsIn(boards, folderId).map((board) => {
        const key = `board-${board.id}`;
        return (
          <li id={`boards-tree-${key}`} key={key}>
            <TreeRow
              id={`boards-tree-${key}-row`}
              depth={depth}
              icon={<LayoutGrid aria-hidden />}
              label={board.name}
              count={counts.get(board.id) ?? 0}
              active={scope.kind === "board" && scope.id === board.id}
              renaming={renaming === board.id}
              dropActive={dropTarget === key}
              onClick={() => onScope({ kind: "board", id: board.id })}
              onRename={(name) => commitRename("board", board.id, name)}
              onCancelRename={() => onRenaming(null)}
              onContextMenu={(x, y) => setMenu({ target: { kind: "board", id: board.id }, x, y })}
              onDragStart={(event) => {
                event.dataTransfer.setData(DRAG_BOARD, board.id);
                event.dataTransfer.effectAllowed = "move";
              }}
              onDragOver={(event) => {
                if (dragHas(event, DRAG_PINS)) {
                  event.preventDefault();
                  event.stopPropagation();
                  setDropTarget(key);
                }
              }}
              onDragLeave={() => setDropTarget((current) => (current === key ? null : current))}
              onDrop={(event) => dropPins(event, board.id)}
            />
          </li>
        );
      })}
    </>
  );

  const target = menu?.target;
  const menuFolder = target?.kind === "folder" ? boards.folders.find((folder) => folder.id === target.id) : undefined;
  const menuBoard = target?.kind === "board" ? boards.boards.find((board) => board.id === target.id) : undefined;
  const closeMenu = () => setMenu(null);
  const item = (run: () => void) => () => {
    run();
    closeMenu();
  };

  return (
    <aside id="boards-sidebar" className="flex w-64 shrink-0 flex-col border-r border-zinc-800 bg-zinc-950">
      <div id="boards-sidebar-header" className="flex items-center gap-1 px-4 pb-2 pt-4">
        <h1 id="boards-sidebar-title" className="flex-1 text-base font-semibold">
          Boards
        </h1>
        <IconButton id="boards-sidebar-new-folder" label="New folder" onClick={() => newFolder(null)}>
          <FolderPlus aria-hidden />
        </IconButton>
        <IconButton id="boards-sidebar-new-board" label="New board" onClick={() => newBoard(null)}>
          <Plus aria-hidden />
        </IconButton>
      </div>
      <label id="boards-sidebar-search" className="mx-3 mb-2 flex h-8 items-center gap-2 rounded-md border border-zinc-800 bg-zinc-900 px-2 focus-within:border-sky-500">
        <Search id="boards-sidebar-search-icon" className="size-3.5 shrink-0 text-zinc-500" aria-hidden />
        <input
          id="boards-sidebar-search-input"
          type="search"
          value={query}
          placeholder="Search pins"
          className="min-w-0 flex-1 bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-500"
          onChange={(event) => onQuery(event.target.value)}
        />
      </label>
      <div
        id="boards-sidebar-tree"
        className={cn("min-h-0 flex-1 overflow-y-auto px-2 pb-4", dropTarget === "root" && "bg-sky-950/30")}
        onContextMenu={(event) => {
          event.preventDefault();
          setMenu({ target: { kind: "root" }, x: event.clientX, y: event.clientY });
        }}
        onDragOver={(event) => {
          if (dragHas(event, DRAG_BOARD) || dragHas(event, DRAG_FOLDER)) {
            event.preventDefault();
            setDropTarget("root");
          }
        }}
        onDragLeave={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
            setDropTarget((current) => (current === "root" ? null : current));
          }
        }}
        onDrop={(event) => dropOn(event, null)}
      >
        <TreeRow
          id="boards-tree-all"
          depth={0}
          icon={<SquareStack aria-hidden />}
          label="All pins"
          count={boards.pins.length}
          active={scope.kind === "all"}
          onClick={() => onScope({ kind: "all" })}
        />
        <div id="boards-tree-divider" className="mx-2 my-2 h-px bg-zinc-800" />
        {boards.folders.length === 0 && boards.boards.length === 0 ? (
          <p id="boards-tree-empty" className="px-3 py-2 text-xs text-zinc-500">
            No boards yet. Create one, or save something with the Boards button in the address bar.
          </p>
        ) : (
          <ul id="boards-tree-root">{renderFolder(null, 0)}</ul>
        )}
      </div>

      {menu ? (
        <ContextMenu id="boards-tree-menu" label="Boards" x={menu.x} y={menu.y} onClose={closeMenu}>
          {menu.target.kind === "root" ? (
            <>
              <ContextMenuItem id="boards-tree-menu-new-folder" icon={<FolderPlus />} label="New folder" onClick={item(() => newFolder(null))} />
              <ContextMenuItem id="boards-tree-menu-new-board" icon={<Plus />} label="New board" onClick={item(() => newBoard(null))} />
            </>
          ) : null}
          {menuFolder ? (
            <>
              <ContextMenuItem id="boards-tree-menu-folder-board" icon={<Plus />} label="New board here" onClick={item(() => newBoard(menuFolder.id))} />
              <ContextMenuItem
                id="boards-tree-menu-folder-folder"
                icon={<FolderPlus />}
                label="New folder here"
                onClick={item(() => newFolder(menuFolder.id))}
              />
              <ContextMenuSeparator id="boards-tree-menu-folder-separator" />
              <ContextMenuItem id="boards-tree-menu-folder-rename" icon={<Pencil />} label="Rename" onClick={item(() => onRenaming(menuFolder.id))} />
              {menuFolder.parentId ? (
                <ContextMenuItem
                  id="boards-tree-menu-folder-top"
                  icon={<Undo2 />}
                  label="Move to top level"
                  onClick={item(() => void moveBoardFolder(menuFolder.id, null).catch(onError))}
                />
              ) : null}
              <ContextMenuItem id="boards-tree-menu-folder-delete" icon={<Trash2 />} label="Delete…" danger onClick={item(() => onDeleteFolder(menuFolder.id))} />
            </>
          ) : null}
          {menuBoard ? (
            <>
              <ContextMenuItem id="boards-tree-menu-board-rename" icon={<Pencil />} label="Rename" onClick={item(() => onRenaming(menuBoard.id))} />
              {menuBoard.folderId ? (
                <ContextMenuItem
                  id="boards-tree-menu-board-top"
                  icon={<Undo2 />}
                  label="Move to top level"
                  onClick={item(() => void moveBoard(menuBoard.id, null).catch(onError))}
                />
              ) : null}
              <ContextMenuItem id="boards-tree-menu-board-delete" icon={<Trash2 />} label="Delete…" danger onClick={item(() => onDeleteBoard(menuBoard.id))} />
            </>
          ) : null}
        </ContextMenu>
      ) : null}
    </aside>
  );
}

type TreeRowProps = {
  id: string;
  depth: number;
  icon: ReactNode;
  label: string;
  count: number;
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
    "flex h-8 w-full items-center gap-1.5 rounded-md pr-2 text-left text-sm text-zinc-300 hover:bg-zinc-900 hover:text-zinc-100",
    active && "bg-sky-950 text-sky-300 hover:bg-sky-950 hover:text-sky-200",
    dropActive && "bg-sky-900/60 ring-1 ring-sky-500",
  );

  if (renaming) {
    return (
      <div id={id} className={rowClass} style={indent}>
        <span id={`${id}-spacer`} className="size-4 shrink-0" />
        <span id={`${id}-icon`} className="inline-flex size-4 shrink-0 items-center justify-center text-zinc-400 [&_svg]:size-4">
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
        {count > 0 ? (
          <span id={`${id}-count`} className="shrink-0 text-[11px] tabular-nums text-zinc-500">
            {count}
          </span>
        ) : null}
      </button>
    </div>
  );
}
