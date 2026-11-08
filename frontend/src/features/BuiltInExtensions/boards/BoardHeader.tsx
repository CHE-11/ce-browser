import { useEffect, useRef, useState } from "react";
import { ChevronRight, Copy, ExternalLink, FolderInput, LayoutGrid, Pencil, Plus, Table2, Trash2, Upload, X } from "lucide-react";
import { Button } from "@/components/Button";
import { createBoard, renameBoard, renameBoardFolder } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import type { BoardsSnapshot, PinKind } from "@type/browser";
import { PIN_KINDS, PIN_KIND_LABELS, folderPath, type BoardsScope, type PinSort, type PinView } from "./board-tree";

type BoardHeaderProps = {
  boards: BoardsSnapshot;
  scope: BoardsScope;
  pinCount: number;
  selectedCount: number;
  view: PinView;
  sort: PinSort;
  kind: PinKind | null;
  onScope: (scope: BoardsScope) => void;
  onView: (view: PinView) => void;
  onSort: (sort: PinSort) => void;
  onKind: (kind: PinKind | null) => void;
  onMoveSelected: (x: number, y: number) => void;
  onDeleteSelected: () => void;
  onClearSelection: () => void;
  onCopySelected: () => void;
  onOpenSelected: () => void;
  onPickFiles: () => void;
  onAddLink: () => void;
  onError: (reason: unknown) => void;
};

export function BoardHeader({
  boards,
  scope,
  pinCount,
  selectedCount,
  view,
  sort,
  kind,
  onScope,
  onView,
  onSort,
  onKind,
  onMoveSelected,
  onDeleteSelected,
  onClearSelection,
  onCopySelected,
  onOpenSelected,
  onPickFiles,
  onAddLink,
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

      <div id="boards-header-actions" className="flex flex-wrap items-center justify-end gap-2">
        {selectedCount > 0 ? (
          <>
            <span id="boards-header-selected" className="text-sm text-sky-300">
              {selectedCount} selected
            </span>
            <Button id="boards-header-copy" onClick={onCopySelected}>
              <Copy id="boards-header-copy-icon" className="size-4" aria-hidden />
              Copy addresses
            </Button>
            <Button id="boards-header-open" onClick={onOpenSelected}>
              <ExternalLink id="boards-header-open-icon" className="size-4" aria-hidden />
              Open
            </Button>
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
        <AddMenu onPickFiles={onPickFiles} onAddLink={onAddLink} />
        <label id="boards-sort-label" className="sr-only" htmlFor="boards-sort">
          Sort
        </label>
        <select
          id="boards-sort"
          value={sort}
          aria-label="Sort"
          className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500"
          onChange={(event) => onSort(event.target.value as PinSort)}
        >
          <option id="boards-sort-newest" value="newest">
            Newest
          </option>
          <option id="boards-sort-oldest" value="oldest">
            Oldest
          </option>
          <option id="boards-sort-title" value="title">
            Title
          </option>
          <option id="boards-sort-kind" value="kind">
            Kind
          </option>
        </select>
        <div id="boards-view-toggle" className="flex overflow-hidden rounded-md border border-zinc-700">
          <button
            id="boards-view-grid"
            type="button"
            aria-pressed={view === "grid"}
            title="Grid"
            className={cn("inline-flex size-8 items-center justify-center", view === "grid" ? "bg-zinc-700 text-zinc-100" : "text-zinc-400 hover:bg-zinc-800", focusRing)}
            onClick={() => onView("grid")}
          >
            <LayoutGrid id="boards-view-grid-icon" className="size-4" aria-hidden />
          </button>
          <button
            id="boards-view-table"
            type="button"
            aria-pressed={view === "table"}
            title="Table"
            className={cn("inline-flex size-8 items-center justify-center", view === "table" ? "bg-zinc-700 text-zinc-100" : "text-zinc-400 hover:bg-zinc-800", focusRing)}
            onClick={() => onView("table")}
          >
            <Table2 id="boards-view-table-icon" className="size-4" aria-hidden />
          </button>
        </div>
      </div>
      <div id="boards-kind-filters" className="flex w-full flex-wrap gap-1">
        <KindChip id="boards-kind-all" label="All" pressed={kind === null} onClick={() => onKind(null)} />
        {PIN_KINDS.map((item) => (
          <KindChip
            key={item}
            id={`boards-kind-${item}`}
            label={PIN_KIND_LABELS[item]}
            pressed={kind === item}
            onClick={() => onKind(kind === item ? null : item)}
          />
        ))}
      </div>
    </header>
  );
}

function KindChip({ id, label, pressed, onClick }: { id: string; label: string; pressed: boolean; onClick: () => void }) {
  return (
    <button
      id={id}
      type="button"
      aria-pressed={pressed}
      className={cn("rounded-full px-2.5 py-1 text-xs", pressed ? "bg-sky-600 text-white" : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700 hover:text-zinc-200", focusRing)}
      onClick={onClick}
    >
      {label}
    </button>
  );
}

function AddMenu({ onPickFiles, onAddLink }: { onPickFiles: () => void; onAddLink: () => void }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    const onPointer = (event: PointerEvent) => {
      if (!ref.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    window.addEventListener("pointerdown", onPointer);
    return () => window.removeEventListener("pointerdown", onPointer);
  }, [open]);

  return (
    <div id="boards-add" ref={ref} className="relative">
      <Button
        id="boards-add-toggle"
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
      >
        <Plus id="boards-add-icon" className="size-4" aria-hidden />
        Add
      </Button>
      {open ? (
        <div id="boards-add-menu" className="absolute right-0 top-full z-20 mt-1 w-60 rounded-lg border border-zinc-700 bg-zinc-900 p-1 shadow-xl">
          <button
            id="boards-add-files"
            type="button"
            className={cn("flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm text-zinc-100 hover:bg-zinc-800", focusRing)}
            onClick={() => {
              setOpen(false);
              onPickFiles();
            }}
          >
            <Upload id="boards-add-files-icon" className="size-4 text-zinc-400" aria-hidden />
            Upload files
          </button>
          <button
            id="boards-add-link"
            type="button"
            className={cn("flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm text-zinc-100 hover:bg-zinc-800", focusRing)}
            onClick={() => {
              setOpen(false);
              onAddLink();
            }}
          >
            <ExternalLink id="boards-add-link-icon" className="size-4 text-zinc-400" aria-hidden />
            Add a link
          </button>
          <p id="boards-add-hint" className="px-2 py-1.5 text-xs text-zinc-500">
            You can also paste or drop files, images, and links.
          </p>
        </div>
      ) : null}
    </div>
  );
}
