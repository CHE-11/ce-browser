import { useRef, useState } from "react";
import { ChevronRight, FolderInput, Pencil, Plus, Trash2, X } from "lucide-react";
import { Button } from "@/components/Button";
import { createBoard, renameBoard, renameBoardFolder } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import type { BoardsSnapshot } from "@type/browser";
import { folderPath, type BoardsScope } from "./board-tree";

type BoardHeaderProps = {
  boards: BoardsSnapshot;
  scope: BoardsScope;
  pinCount: number;
  selectedCount: number;
  onScope: (scope: BoardsScope) => void;
  onMoveSelected: (x: number, y: number) => void;
  onDeleteSelected: () => void;
  onClearSelection: () => void;
  onError: (reason: unknown) => void;
};

export function BoardHeader({
  boards,
  scope,
  pinCount,
  selectedCount,
  onScope,
  onMoveSelected,
  onDeleteSelected,
  onClearSelection,
  onError,
}: BoardHeaderProps) {
  const [editing, setEditing] = useState(false);
  const cancelled = useRef(false);
  const board = scope.kind === "board" ? boards.boards.find((item) => item.id === scope.id) : undefined;
  const folder = scope.kind === "folder" ? boards.folders.find((item) => item.id === scope.id) : undefined;
  const parentId = board ? board.folderId : folder ? folder.parentId : null;
  const crumbs = scope.kind === "all" ? [] : folderPath(boards, parentId);
  const title = board?.name ?? folder?.name ?? "All pins";
  const renamable = Boolean(board || folder);

  const rename = (name: string) => {
    setEditing(false);
    const trimmed = name.trim();
    if (!trimmed || trimmed === title) {
      return;
    }
    if (board) {
      void renameBoard(board.id, trimmed).catch(onError);
    } else if (folder) {
      void renameBoardFolder(folder.id, trimmed).catch(onError);
    }
  };

  return (
    <header id="boards-header" className="flex min-h-16 shrink-0 flex-wrap items-center gap-x-4 gap-y-2 border-b border-zinc-800 px-8 py-3">
      <div id="boards-header-title-block" className="min-w-0 flex-1">
        {crumbs.length > 0 ? (
          <nav id="boards-header-crumbs" aria-label="Folders" className="flex min-w-0 items-center gap-1 text-xs text-zinc-500">
            {crumbs.map((crumb, index) => (
              <span id={`boards-header-crumb-${crumb.id}`} key={crumb.id} className="flex min-w-0 items-center gap-1">
                {index > 0 ? <ChevronRight id={`boards-header-crumb-${crumb.id}-chevron`} className="size-3 shrink-0" aria-hidden /> : null}
                <button
                  id={`boards-header-crumb-${crumb.id}-button`}
                  type="button"
                  className={cn("truncate rounded hover:text-zinc-200", focusRing)}
                  onClick={() => onScope({ kind: "folder", id: crumb.id })}
                >
                  {crumb.name}
                </button>
              </span>
            ))}
          </nav>
        ) : null}
        <div id="boards-header-title-row" className="flex min-w-0 items-center gap-2">
          {editing ? (
            <input
              id="boards-header-rename"
              autoFocus
              defaultValue={title}
              aria-label="Name"
              className="h-8 min-w-0 max-w-md flex-1 rounded-md border border-sky-500 bg-zinc-950 px-2 text-xl font-semibold text-zinc-100 outline-none"
              onFocus={(event) => {
                cancelled.current = false;
                event.currentTarget.select();
              }}
              onBlur={(event) => {
                if (!cancelled.current) {
                  rename(event.currentTarget.value);
                }
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.currentTarget.blur();
                } else if (event.key === "Escape") {
                  cancelled.current = true;
                  setEditing(false);
                }
              }}
            />
          ) : (
            <h2 id="boards-header-title" className="truncate text-xl font-semibold">
              {title}
            </h2>
          )}
          {renamable && !editing ? (
            <button
              id="boards-header-rename-button"
              type="button"
              aria-label="Rename"
              title="Rename"
              className={cn("inline-flex size-7 shrink-0 items-center justify-center rounded-md text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200", focusRing)}
              onClick={() => setEditing(true)}
            >
              <Pencil id="boards-header-rename-icon" className="size-3.5" aria-hidden />
            </button>
          ) : null}
          <span id="boards-header-count" className="shrink-0 text-sm text-zinc-500">
            {pinCount === 1 ? "1 pin" : `${pinCount} pins`}
          </span>
        </div>
      </div>

      <div id="boards-header-actions" className="flex items-center gap-2">
        {selectedCount > 0 ? (
          <>
            <span id="boards-header-selected" className="text-sm text-sky-300">
              {selectedCount} selected
            </span>
            <Button
              id="boards-header-move"
              onClick={(event) => {
                const rect = event.currentTarget.getBoundingClientRect();
                onMoveSelected(rect.left, rect.bottom + 4);
              }}
            >
              <FolderInput id="boards-header-move-icon" className="size-4" aria-hidden />
              Move to…
            </Button>
            <Button id="boards-header-delete" variant="destructive" onClick={onDeleteSelected}>
              <Trash2 id="boards-header-delete-icon" className="size-4" aria-hidden />
              Delete
            </Button>
            <Button id="boards-header-clear" variant="ghost" onClick={onClearSelection}>
              <X id="boards-header-clear-icon" className="size-4" aria-hidden />
              Clear
            </Button>
          </>
        ) : folder ? (
          <Button
            id="boards-header-new-board"
            onClick={() =>
              void createBoard("New board", folder.id)
                .then((created) => onScope({ kind: "board", id: created.id }))
                .catch(onError)
            }
          >
            <Plus id="boards-header-new-board-icon" className="size-4" aria-hidden />
            New board here
          </Button>
        ) : null}
      </div>
    </header>
  );
}
