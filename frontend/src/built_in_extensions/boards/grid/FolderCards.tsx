import { useState, type DragEvent } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { cn } from "@/lib/cn";
import type { BoardFolder, BoardsSnapshot } from "@type/browser";
import { FolderGlyph } from "../folder-icons";
import { DRAG_PINS, folderPreviewPins, pinsInFolder, previewUrl } from "../folder-tree";

const OPEN_KEY = "ce-boards-subfolders-open";

type FolderCardsProps = {
  boards: BoardsSnapshot;
  folders: BoardFolder[];
  /** Card width in pixels, following the size slider. */
  size: number;
  onOpen: (folder: BoardFolder) => void;
  onMenu: (folder: BoardFolder, x: number, y: number) => void;
  /** `move` is true when Shift was held. */
  onDropPins: (ids: string[], folder: BoardFolder, move: boolean) => void;
};

/** "Subfolder (n)" cards above a folder's pins, each a mosaic of the newest images inside. */
export function FolderCards({ boards, folders, size, onOpen, onMenu, onDropPins }: FolderCardsProps) {
  const [open, setOpen] = useState(() => localStorage.getItem(OPEN_KEY) !== "0");
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  if (folders.length === 0) {
    return null;
  }
  const width = Math.max(160, Math.min(320, size * 1.2));

  const toggle = () => {
    setOpen((current) => {
      localStorage.setItem(OPEN_KEY, current ? "0" : "1");
      return !current;
    });
  };

  const drop = (event: DragEvent, folder: BoardFolder) => {
    event.preventDefault();
    setDropTarget(null);
    try {
      const ids: unknown = JSON.parse(event.dataTransfer.getData(DRAG_PINS) || "[]");
      if (Array.isArray(ids) && ids.length > 0) {
        onDropPins(
          ids.filter((id): id is string => typeof id === "string"),
          folder,
          event.shiftKey,
        );
      }
    } catch {
      // Not pins.
    }
  };

  return (
    <section id="boards-subfolders" className="mb-5">
      <button id="boards-subfolders-toggle" type="button" aria-expanded={open} className="mb-2 flex items-center gap-1 text-xs font-medium text-zinc-400 hover:text-zinc-200" onClick={toggle}>
        {open ? <ChevronDown id="boards-subfolders-toggle-icon" className="size-3.5" aria-hidden /> : <ChevronRight id="boards-subfolders-toggle-icon" className="size-3.5" aria-hidden />}
        Subfolder ({folders.length})
      </button>
      {open ? (
        <ul id="boards-subfolders-list" className="grid gap-4" style={{ gridTemplateColumns: `repeat(auto-fill, minmax(${width}px, 1fr))` }}>
          {folders.map((folder) => {
            const previews = folderPreviewPins(boards, folder, 3);
            const count = pinsInFolder(boards, folder.id, true).length;
            const idBase = `boards-subfolder-${folder.id}`;
            return (
              <li id={idBase} key={folder.id} data-folder-card={folder.id}>
                <button
                  id={`${idBase}-button`}
                  type="button"
                  className="group flex w-full flex-col gap-1.5 text-left"
                  onClick={() => onOpen(folder)}
                  onContextMenu={(event) => {
                    event.preventDefault();
                    onMenu(folder, event.clientX, event.clientY);
                  }}
                  onDragOver={(event) => {
                    if (event.dataTransfer.types.includes(DRAG_PINS)) {
                      event.preventDefault();
                      event.dataTransfer.dropEffect = event.shiftKey ? "move" : "copy";
                      setDropTarget(folder.id);
                    }
                  }}
                  onDragLeave={() => setDropTarget((current) => (current === folder.id ? null : current))}
                  onDrop={(event) => drop(event, folder)}
                >
                  <span
                    id={`${idBase}-mosaic`}
                    className={cn(
                      "grid aspect-[4/3] w-full grid-cols-3 grid-rows-2 gap-0.5 overflow-hidden rounded-lg bg-zinc-900 ring-1 ring-zinc-800 transition group-hover:ring-zinc-600",
                      dropTarget === folder.id && "ring-2 ring-sky-500",
                    )}
                  >
                    {previews.length === 0 ? (
                      <span id={`${idBase}-empty`} className="col-span-3 row-span-2 flex items-center justify-center">
                        <FolderGlyph id={`${idBase}-empty-icon`} folder={folder} className="size-10 text-zinc-700" />
                      </span>
                    ) : (
                      previews.map((pin, index) => (
                        <img
                          id={`${idBase}-preview-${index}`}
                          key={pin.id}
                          src={previewUrl(pin)}
                          alt=""
                          loading="lazy"
                          draggable={false}
                          className={cn(
                            "size-full bg-zinc-800 object-cover",
                            index === 0 && (previews.length === 1 ? "col-span-3 row-span-2" : "col-span-2 row-span-2"),
                            index > 0 && previews.length === 2 && "row-span-2",
                          )}
                        />
                      ))
                    )}
                  </span>
                  <span id={`${idBase}-label`} className="flex min-w-0 items-center gap-1.5 px-0.5">
                    <FolderGlyph id={`${idBase}-icon`} folder={folder} className="text-zinc-500" />
                    <span id={`${idBase}-name`} className="min-w-0 flex-1 truncate text-sm text-zinc-200">
                      {folder.name}
                    </span>
                    <span id={`${idBase}-count`} className="shrink-0 text-xs tabular-nums text-zinc-500">
                      {count}
                    </span>
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      ) : null}
    </section>
  );
}
