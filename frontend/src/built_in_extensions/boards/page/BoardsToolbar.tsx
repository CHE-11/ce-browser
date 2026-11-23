import { useState, type MouseEvent } from "react";
import {
  ArrowDownUp,
  Check,
  ChevronLeft,
  ChevronRight,
  ClipboardPaste,
  FilePlus,
  Filter,
  FolderInput,
  LayoutDashboard,
  LayoutGrid,
  Link,
  List,
  PanelRight,
  Plus,
  Rows3,
  Search,
  Shuffle,
  Trash2,
  X,
} from "lucide-react";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "@/components/ContextMenu";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { cn } from "@/lib/cn";
import type { BoardsSnapshot, PinSort } from "@type/browser";
import { FolderGlyph } from "../folder-icons";
import { dragHas, draggedFolder, draggedPins, isBoardsDrag, pinDropEffect } from "../drag";
import { DRAG_PINS, PIN_SORTS, PIN_SORT_LABELS, folderPath } from "../folder-tree";
import { scopeTitle, type BoardsScope } from "../scopes";
import { MAX_TILE_SIZE, MIN_TILE_SIZE, PIN_LAYOUT_LABELS, type PinLayout, type ViewPrefs } from "../view-prefs";

type Menu = { kind: "layout" | "sort" | "add"; x: number; y: number };

type BoardsToolbarProps = {
  boards: BoardsSnapshot;
  scope: BoardsScope;
  /** Pins shown after search and filters. */
  count: number;
  selectedCount: number;
  canBack: boolean;
  canForward: boolean;
  onBack: () => void;
  onForward: () => void;
  onScope: (scope: BoardsScope) => void;
  query: string;
  onQuery: (query: string) => void;
  prefs: ViewPrefs;
  onPrefs: (patch: Partial<ViewPrefs>) => void;
  /** Sort in effect: the folder's own sort or the page sort. Null when the view keeps its own order. */
  sort: PinSort | null;
  /** The folder sets its own sort, so the menu changes the folder. */
  folderSort: boolean;
  onSort: (sort: PinSort) => void;
  filterOpen: boolean;
  filterCount: number;
  onToggleFilter: () => void;
  onAddFiles: () => void;
  onImportFolders: () => void;
  onAddLink: () => void;
  onPaste: () => void;
  onImportEagle: () => void;
  onReshuffle: () => void;
  onEmptyTrash: () => void;
  onClearSelection: () => void;
  /** Pins dropped on a parent folder in the path. `copy` is true when Ctrl was held. */
  onDropPins: (ids: string[], folderId: string, copy: boolean) => void;
  /** Folder `id` dropped on a parent folder in the path. */
  onDropFolder: (id: string, folderId: string) => void;
};

const LAYOUT_ICONS: Record<PinLayout, typeof LayoutGrid> = {
  justified: Rows3,
  masonry: LayoutDashboard,
  grid: LayoutGrid,
  list: List,
};

export function BoardsToolbar({
  boards,
  scope,
  count,
  selectedCount,
  canBack,
  canForward,
  onBack,
  onForward,
  onScope,
  query,
  onQuery,
  prefs,
  onPrefs,
  sort,
  folderSort,
  onSort,
  filterOpen,
  filterCount,
  onToggleFilter,
  onAddFiles,
  onImportFolders,
  onAddLink,
  onPaste,
  onImportEagle,
  onReshuffle,
  onEmptyTrash,
  onClearSelection,
  onDropPins,
  onDropFolder,
}: BoardsToolbarProps) {
  const [menu, setMenu] = useState<Menu | null>(null);
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  const path = scope.kind === "folder" ? folderPath(boards, scope.id) : [];
  const LayoutIcon = LAYOUT_ICONS[prefs.layout];

  const openMenu = (kind: Menu["kind"]) => (event: MouseEvent<HTMLButtonElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    setMenu({ kind, x: kind === "add" ? rect.right - 220 : rect.left, y: rect.bottom + 4 });
  };
  const close = () => setMenu(null);
  const run = (work: () => void) => () => {
    work();
    close();
  };

  return (
    <header id="boards-toolbar" className="flex h-12 shrink-0 items-center gap-2 border-b border-zinc-800 px-3">
      <IconButton id="boards-toolbar-back" label="Back" disabled={!canBack} onClick={onBack}>
        <ChevronLeft aria-hidden />
      </IconButton>
      <IconButton id="boards-toolbar-forward" label="Forward" disabled={!canForward} onClick={onForward}>
        <ChevronRight aria-hidden />
      </IconButton>

      <nav id="boards-toolbar-path" aria-label="Location" className="flex min-w-0 flex-1 items-center gap-1 text-sm">
        {path.length > 0 ? (
          path.map((folder, index) => {
            const last = index === path.length - 1;
            return (
              <span id={`boards-toolbar-path-${folder.id}`} key={folder.id} className="flex min-w-0 items-center gap-1">
                {index > 0 ? <ChevronRight id={`boards-toolbar-path-${folder.id}-separator`} className="size-3.5 shrink-0 text-zinc-600" aria-hidden /> : null}
                <button
                  id={`boards-toolbar-path-${folder.id}-button`}
                  type="button"
                  aria-current={last ? "page" : undefined}
                  className={cn(
                    "flex min-w-0 items-center gap-1.5 rounded px-1 py-0.5 hover:bg-zinc-800",
                    last ? "font-semibold text-zinc-100" : "text-zinc-400",
                    dropTarget === folder.id && "bg-sky-900/60 text-sky-100 ring-1 ring-sky-500",
                  )}
                  onClick={() => onScope({ kind: "folder", id: folder.id })}
                  onDragOver={
                    last
                      ? undefined
                      : (event) => {
                          if (isBoardsDrag(event)) {
                            event.preventDefault();
                            event.dataTransfer.dropEffect = dragHas(event, DRAG_PINS) ? pinDropEffect(event) : "move";
                            setDropTarget(folder.id);
                          }
                        }
                  }
                  onDragLeave={() => setDropTarget((current) => (current === folder.id ? null : current))}
                  onDrop={
                    last
                      ? undefined
                      : (event) => {
                          event.preventDefault();
                          setDropTarget(null);
                          const pins = draggedPins(event);
                          const dragged = draggedFolder(event);
                          if (pins.length > 0) {
                            onDropPins(pins, folder.id, event.ctrlKey);
                          } else if (dragged) {
                            onDropFolder(dragged, folder.id);
                          }
                        }
                  }
                >
                  {last ? <FolderGlyph id={`boards-toolbar-path-${folder.id}-icon`} folder={folder} className="text-zinc-400" /> : null}
                  <span id={`boards-toolbar-path-${folder.id}-name`} className="truncate">
                    {folder.name}
                  </span>
                </button>
              </span>
            );
          })
        ) : (
          <h2 id="boards-toolbar-title" className="truncate px-1 font-semibold text-zinc-100">
            {scopeTitle(boards, scope)}
          </h2>
        )}
        <span id="boards-toolbar-count" className="shrink-0 pl-1 text-xs tabular-nums text-zinc-500">
          {count}
        </span>
        {selectedCount > 0 ? (
          <span id="boards-toolbar-selection" className="ml-2 flex shrink-0 items-center gap-1 rounded-full bg-sky-950 py-0.5 pl-2.5 pr-1 text-xs text-sky-200">
            {selectedCount} selected
            <button id="boards-toolbar-selection-clear" type="button" aria-label="Clear selection" className="inline-flex size-4 items-center justify-center rounded-full hover:bg-sky-900" onClick={onClearSelection}>
              <X id="boards-toolbar-selection-clear-icon" className="size-3" aria-hidden />
            </button>
          </span>
        ) : null}
      </nav>

      {scope.kind === "random" ? (
        <Button id="boards-toolbar-reshuffle" size="sm" variant="ghost" onClick={onReshuffle}>
          <Shuffle id="boards-toolbar-reshuffle-icon" className="size-3.5" aria-hidden />
          Shuffle
        </Button>
      ) : null}
      {scope.kind === "trash" ? (
        <Button id="boards-toolbar-empty-trash" size="sm" variant="destructiveOutline" disabled={count === 0} onClick={onEmptyTrash}>
          <Trash2 id="boards-toolbar-empty-trash-icon" className="size-3.5" aria-hidden />
          Empty Trash
        </Button>
      ) : null}

      <label id="boards-toolbar-search" className="flex h-8 w-52 items-center gap-2 rounded-md border border-zinc-800 bg-zinc-900 px-2 focus-within:border-sky-500">
        <Search id="boards-toolbar-search-icon" className="size-3.5 shrink-0 text-zinc-500" aria-hidden />
        <input
          id="boards-toolbar-search-input"
          type="search"
          value={query}
          placeholder="Search"
          className="min-w-0 flex-1 bg-transparent text-sm text-zinc-100 outline-none placeholder:text-zinc-500"
          onChange={(event) => onQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              onQuery("");
              event.currentTarget.blur();
            }
          }}
        />
      </label>

      {prefs.layout !== "list" ? (
        <input
          id="boards-toolbar-size"
          type="range"
          aria-label="Thumbnail size"
          title="Thumbnail size"
          min={MIN_TILE_SIZE}
          max={MAX_TILE_SIZE}
          step={10}
          value={prefs.size}
          className="w-24 accent-sky-500"
          onChange={(event) => onPrefs({ size: Number(event.target.value) })}
        />
      ) : null}

      <IconButton id="boards-toolbar-layout" label={`Layout: ${PIN_LAYOUT_LABELS[prefs.layout]}`} active={menu?.kind === "layout"} onClick={openMenu("layout")}>
        <LayoutIcon aria-hidden />
      </IconButton>
      <IconButton id="boards-toolbar-sort" label={sort ? `Sort: ${PIN_SORT_LABELS[sort]}` : "This view keeps its own order"} disabled={!sort} active={menu?.kind === "sort"} onClick={openMenu("sort")}>
        <ArrowDownUp aria-hidden />
      </IconButton>
      <span id="boards-toolbar-filter-wrap" className="relative">
        <IconButton id="boards-toolbar-filter" label="Filter" active={filterOpen || filterCount > 0} onClick={onToggleFilter}>
          <Filter aria-hidden />
        </IconButton>
        {filterCount > 0 ? (
          <span id="boards-toolbar-filter-count" className="pointer-events-none absolute -right-0.5 -top-0.5 inline-flex size-4 items-center justify-center rounded-full bg-sky-500 text-[10px] font-semibold text-white">
            {filterCount}
          </span>
        ) : null}
      </span>
      <Button id="boards-toolbar-add" size="sm" variant="primary" onClick={openMenu("add")}>
        <Plus id="boards-toolbar-add-icon" className="size-3.5" aria-hidden />
        Add
      </Button>
      <IconButton id="boards-toolbar-inspector" label={prefs.inspector ? "Hide inspector" : "Show inspector"} active={prefs.inspector} onClick={() => onPrefs({ inspector: !prefs.inspector })}>
        <PanelRight aria-hidden />
      </IconButton>

      {menu?.kind === "layout" ? (
        <ContextMenu id="boards-layout-menu" label="Layout" x={menu.x} y={menu.y} onClose={close}>
          {(Object.keys(PIN_LAYOUT_LABELS) as PinLayout[]).map((layout) => {
            const Icon = LAYOUT_ICONS[layout];
            return (
              <ContextMenuItem
                id={`boards-layout-menu-${layout}`}
                key={layout}
                icon={<Icon />}
                label={PIN_LAYOUT_LABELS[layout]}
                trailing={prefs.layout === layout ? <Check id={`boards-layout-menu-${layout}-check`} className="size-3.5 text-sky-400" aria-hidden /> : null}
                onClick={run(() => onPrefs({ layout }))}
              />
            );
          })}
          <ContextMenuSeparator id="boards-layout-menu-separator" />
          <ContextMenuItem
            id="boards-layout-menu-captions"
            icon={<span id="boards-layout-menu-captions-blank" />}
            label="Show names and sizes"
            trailing={prefs.captions ? <Check id="boards-layout-menu-captions-check" className="size-3.5 text-sky-400" aria-hidden /> : null}
            onClick={run(() => onPrefs({ captions: !prefs.captions }))}
          />
        </ContextMenu>
      ) : null}

      {menu?.kind === "sort" && sort ? (
        <ContextMenu id="boards-sort-menu" label="Sort" x={menu.x} y={menu.y} onClose={close}>
          {folderSort ? (
            <p id="boards-sort-menu-note" className="px-2 py-1 text-[11px] text-zinc-500">
              Sorting this folder
            </p>
          ) : null}
          {PIN_SORTS.map((option) => (
            <ContextMenuItem
              id={`boards-sort-menu-${option}`}
              key={option}
              icon={sort === option ? <Check /> : <span id={`boards-sort-menu-${option}-blank`} />}
              label={PIN_SORT_LABELS[option]}
              onClick={run(() => onSort(option))}
            />
          ))}
        </ContextMenu>
      ) : null}

      {menu?.kind === "add" ? (
        <ContextMenu id="boards-add-menu" label="Add" x={menu.x} y={menu.y} onClose={close} className="w-56">
          <ContextMenuItem id="boards-add-menu-files" icon={<FilePlus />} label="Add files…" onClick={run(onAddFiles)} />
          <ContextMenuItem id="boards-add-menu-folders" icon={<FolderInput />} label="Import folders…" onClick={run(onImportFolders)} />
          <ContextMenuItem id="boards-add-menu-link" icon={<Link />} label="Add a link…" onClick={run(onAddLink)} />
          <ContextMenuItem id="boards-add-menu-paste" icon={<ClipboardPaste />} label="Paste from clipboard" onClick={run(onPaste)} />
          <ContextMenuSeparator id="boards-add-menu-separator" />
          <ContextMenuItem id="boards-add-menu-eagle" icon={<FolderInput />} label="Import Eagle library…" onClick={run(onImportEagle)} />
        </ContextMenu>
      ) : null}
    </header>
  );
}
