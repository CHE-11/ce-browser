import { useState } from "react";
import {
  ArrowDownUp,
  Check,
  ChevronLeft,
  ChevronRight,
  ChevronsDownUp,
  ChevronsUpDown,
  Copy,
  Download,
  FolderPlus,
  FolderTree,
  Palette,
  Pencil,
  Trash2,
  Undo2,
} from "lucide-react";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "@/components/ContextMenu";
import { cn } from "@/lib/cn";
import type { BoardFolder, FolderPatch } from "@type/browser";
import { FOLDER_ICONS, FOLDER_ICON_NAMES } from "../folder-icons";
import { FOLDER_COLORS, FOLDER_COLOR_CLASS, PIN_SORTS, PIN_SORT_LABELS } from "../folder-tree";

type FolderContextMenuProps = {
  folder: BoardFolder;
  x: number;
  y: number;
  onClose: () => void;
  onNewSubfolder: () => void;
  onRename: () => void;
  onUpdate: (patch: FolderPatch) => void;
  onDuplicate: () => void;
  onExport: () => void;
  onMoveToTop: () => void;
  /** Tree actions, shown only in the sidebar. */
  onExpandAll?: () => void;
  onCollapseAll?: () => void;
  onDelete: () => void;
};

type Panel = "main" | "appearance" | "sort";

/** Right-click menu for a folder in the sidebar or a folder card. */
export function FolderContextMenu({
  folder,
  x,
  y,
  onClose,
  onNewSubfolder,
  onRename,
  onUpdate,
  onDuplicate,
  onExport,
  onMoveToTop,
  onExpandAll,
  onCollapseAll,
  onDelete,
}: FolderContextMenuProps) {
  const [panel, setPanel] = useState<Panel>("main");
  const run = (work: () => void) => () => {
    work();
    onClose();
  };
  const back = (
    <ContextMenuItem id="boards-folder-menu-back" icon={<ChevronLeft />} label="Back" onClick={() => setPanel("main")} />
  );

  return (
    <ContextMenu
      id="boards-folder-menu"
      label={`Folder ${folder.name}`}
      x={x}
      y={y}
      onClose={onClose}
      onEscape={() => {
        if (panel !== "main") {
          setPanel("main");
          return true;
        }
        return false;
      }}
      className="w-56"
    >
      {panel === "main" ? (
        <>
          <ContextMenuItem id="boards-folder-menu-new" icon={<FolderPlus />} label="New subfolder" onClick={run(onNewSubfolder)} />
          <ContextMenuItem id="boards-folder-menu-rename" icon={<Pencil />} label="Rename" onClick={run(onRename)} />
          <ContextMenuItem
            id="boards-folder-menu-appearance"
            icon={<Palette />}
            label="Color and icon"
            hasPopup
            trailing={<ChevronRight id="boards-folder-menu-appearance-chevron" className="size-3.5 text-zinc-500" aria-hidden />}
            onClick={() => setPanel("appearance")}
          />
          <ContextMenuItem
            id="boards-folder-menu-sort"
            icon={<ArrowDownUp />}
            label="Sort by"
            hasPopup
            trailing={
              <span id="boards-folder-menu-sort-value" className="flex items-center gap-1 text-zinc-500">
                {folder.sort ? PIN_SORT_LABELS[folder.sort].replace(/ \(.*\)$/, "") : "Default"}
                <ChevronRight id="boards-folder-menu-sort-chevron" className="size-3.5" aria-hidden />
              </span>
            }
            onClick={() => setPanel("sort")}
          />
          <ContextMenuItem
            id="boards-folder-menu-subfolders"
            icon={<FolderTree />}
            label="Show subfolder contents"
            trailing={folder.showSubfolders ? <Check id="boards-folder-menu-subfolders-check" className="size-3.5 text-sky-400" aria-hidden /> : null}
            onClick={run(() => onUpdate({ showSubfolders: !folder.showSubfolders }))}
          />
          <ContextMenuSeparator id="boards-folder-menu-separator-1" />
          <ContextMenuItem id="boards-folder-menu-duplicate" icon={<Copy />} label="Duplicate" onClick={run(onDuplicate)} />
          <ContextMenuItem id="boards-folder-menu-export" icon={<Download />} label="Export…" onClick={run(onExport)} />
          {folder.parentId ? <ContextMenuItem id="boards-folder-menu-top" icon={<Undo2 />} label="Move to top level" onClick={run(onMoveToTop)} /> : null}
          {onExpandAll && onCollapseAll ? (
            <>
              <ContextMenuSeparator id="boards-folder-menu-separator-2" />
              <ContextMenuItem id="boards-folder-menu-expand" icon={<ChevronsUpDown />} label="Expand all" onClick={run(onExpandAll)} />
              <ContextMenuItem id="boards-folder-menu-collapse" icon={<ChevronsDownUp />} label="Collapse all" onClick={run(onCollapseAll)} />
            </>
          ) : null}
          <ContextMenuSeparator id="boards-folder-menu-separator-3" />
          <ContextMenuItem id="boards-folder-menu-delete" icon={<Trash2 />} label="Delete…" danger onClick={run(onDelete)} />
        </>
      ) : null}

      {panel === "appearance" ? (
        <>
          {back}
          <ContextMenuSeparator id="boards-folder-menu-appearance-separator" />
          <p id="boards-folder-menu-color-label" className="px-2 pb-1 pt-1.5 text-[11px] text-zinc-500">
            Color
          </p>
          <div id="boards-folder-menu-colors" className="flex flex-wrap gap-1.5 px-2 pb-2">
            <button
              id="boards-folder-menu-color-none"
              type="button"
              role="menuitem"
              title="No color"
              aria-label="No color"
              className={cn("size-5 rounded-full border border-zinc-600 bg-transparent", !folder.color && "ring-2 ring-sky-400 ring-offset-1 ring-offset-zinc-900")}
              onClick={() => onUpdate({ color: "" })}
            />
            {FOLDER_COLORS.map((color) => (
              <button
                id={`boards-folder-menu-color-${color}`}
                key={color}
                type="button"
                role="menuitem"
                title={color}
                aria-label={color}
                className={cn(
                  "size-5 rounded-full",
                  FOLDER_COLOR_CLASS[color].swatch,
                  folder.color === color && "ring-2 ring-sky-400 ring-offset-1 ring-offset-zinc-900",
                )}
                onClick={() => onUpdate({ color })}
              />
            ))}
          </div>
          <p id="boards-folder-menu-icon-label" className="px-2 pb-1 text-[11px] text-zinc-500">
            Icon
          </p>
          <div id="boards-folder-menu-icons" className="grid grid-cols-7 gap-0.5 px-1.5 pb-1.5">
            {FOLDER_ICON_NAMES.map((name) => {
              const Icon = FOLDER_ICONS[name];
              const active = (folder.icon || "folder") === name;
              return (
                <button
                  id={`boards-folder-menu-icon-${name}`}
                  key={name}
                  type="button"
                  role="menuitem"
                  title={name}
                  aria-label={name}
                  className={cn(
                    "inline-flex size-7 items-center justify-center rounded text-zinc-300 hover:bg-zinc-800",
                    active && "bg-sky-950 text-sky-300",
                  )}
                  onClick={() => onUpdate({ icon: name === "folder" ? "" : name })}
                >
                  <Icon id={`boards-folder-menu-icon-${name}-glyph`} className="size-4" aria-hidden />
                </button>
              );
            })}
          </div>
        </>
      ) : null}

      {panel === "sort" ? (
        <>
          {back}
          <ContextMenuSeparator id="boards-folder-menu-sort-separator" />
          <ContextMenuItem
            id="boards-folder-menu-sort-default"
            icon={folder.sort ? <span id="boards-folder-menu-sort-default-blank" /> : <Check />}
            label="Use the page sort"
            onClick={run(() => onUpdate({ sort: "" }))}
          />
          {PIN_SORTS.map((sort) => (
            <ContextMenuItem
              id={`boards-folder-menu-sort-${sort}`}
              key={sort}
              icon={folder.sort === sort ? <Check /> : <span id={`boards-folder-menu-sort-${sort}-blank`} />}
              label={PIN_SORT_LABELS[sort]}
              onClick={run(() => onUpdate({ sort }))}
            />
          ))}
        </>
      ) : null}
    </ContextMenu>
  );
}
