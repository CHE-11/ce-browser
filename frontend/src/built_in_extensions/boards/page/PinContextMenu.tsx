import {
  Copy,
  Download,
  ExternalLink,
  File,
  FolderInput,
  FolderMinus,
  FolderOpen,
  FolderPlus,
  ImageIcon,
  Link,
  Maximize2,
  RotateCcw,
  Save,
  Star,
  Trash2,
} from "lucide-react";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "@/components/ContextMenu";
import { cn } from "@/lib/cn";
import type { Pin } from "@type/browser";
import { hasPreview, pinAddress } from "../folder-tree";

export type PinMenuActions = {
  open: (pin: Pin) => void;
  openSource: (pins: Pin[]) => void;
  openFile: (pin: Pin) => void;
  reveal: (pin: Pin) => void;
  copyImage: (pin: Pin) => void;
  copyAddress: (pins: Pin[]) => void;
  saveAs: (pin: Pin) => void;
  exportPins: (pins: Pin[]) => void;
  addToFolder: (pins: Pin[], x: number, y: number) => void;
  moveToFolder: (pins: Pin[], x: number, y: number) => void;
  removeFromFolder: (pins: Pin[]) => void;
  setCover: (pin: Pin) => void;
  rate: (pins: Pin[], rating: number) => void;
  trash: (pins: Pin[]) => void;
  restore: (pins: Pin[]) => void;
  deleteForever: (pins: Pin[]) => void;
};

type PinContextMenuProps = {
  pins: Pin[];
  x: number;
  y: number;
  /** The folder being viewed, for Remove from folder and Set as cover. */
  folderId: string | null;
  inTrash: boolean;
  actions: PinMenuActions;
  onClose: () => void;
};

/** Right-click menu for one or more selected pins. */
export function PinContextMenu({ pins, x, y, folderId, inTrash, actions, onClose }: PinContextMenuProps) {
  const pin = pins[0];
  if (!pin) {
    return null;
  }
  const many = pins.length > 1;
  const count = many ? ` ${pins.length} items` : "";
  const run = (work: () => void) => () => {
    work();
    onClose();
  };
  const withAddress = pins.filter((item) => pinAddress(item));
  const isFile = pin.kind === "file" || pin.kind === "image";

  if (inTrash) {
    return (
      <ContextMenu id="boards-pin-menu" label="Pin" x={x} y={y} onClose={onClose}>
        <ContextMenuItem id="boards-pin-menu-restore" icon={<RotateCcw />} label={`Restore${count}`} onClick={run(() => actions.restore(pins))} />
        <ContextMenuItem id="boards-pin-menu-delete-forever" icon={<Trash2 />} label={`Delete forever${count}`} danger onClick={run(() => actions.deleteForever(pins))} />
      </ContextMenu>
    );
  }

  const rating = pins.every((item) => item.rating === pin.rating) ? pin.rating : 0;

  return (
    <ContextMenu id="boards-pin-menu" label="Pin" x={x} y={y} onClose={onClose} className="w-60">
      <ContextMenuItem id="boards-pin-menu-open" icon={<Maximize2 />} label="Open" disabled={many} onClick={run(() => actions.open(pin))} />
      <ContextMenuItem
        id="boards-pin-menu-source"
        icon={<ExternalLink />}
        label={many ? `Open ${withAddress.length} addresses` : "Open source page"}
        disabled={withAddress.length === 0}
        onClick={run(() => actions.openSource(withAddress))}
      />
      {isFile ? (
        <>
          <ContextMenuItem id="boards-pin-menu-open-file" icon={<File />} label="Open with default app" disabled={many} onClick={run(() => actions.openFile(pin))} />
          <ContextMenuItem id="boards-pin-menu-reveal" icon={<FolderOpen />} label="Show in File Explorer" disabled={many} onClick={run(() => actions.reveal(pin))} />
        </>
      ) : null}
      <ContextMenuSeparator id="boards-pin-menu-separator-1" />
      <ContextMenuItem id="boards-pin-menu-copy" icon={<Copy />} label="Copy image" disabled={many || !hasPreview(pin)} onClick={run(() => actions.copyImage(pin))} />
      <ContextMenuItem
        id="boards-pin-menu-copy-address"
        icon={<Link />}
        label="Copy address"
        disabled={withAddress.length === 0}
        onClick={run(() => actions.copyAddress(withAddress))}
      />
      <ContextMenuItem id="boards-pin-menu-save" icon={<Save />} label="Save as…" disabled={many} onClick={run(() => actions.saveAs(pin))} />
      <ContextMenuItem id="boards-pin-menu-export" icon={<Download />} label={`Export${count}…`} onClick={run(() => actions.exportPins(pins))} />
      <ContextMenuSeparator id="boards-pin-menu-separator-2" />
      <ContextMenuItem id="boards-pin-menu-add" icon={<FolderPlus />} label="Add to folder…" onClick={run(() => actions.addToFolder(pins, x, y))} />
      <ContextMenuItem id="boards-pin-menu-move" icon={<FolderInput />} label="Move to folder…" onClick={run(() => actions.moveToFolder(pins, x, y))} />
      {folderId ? (
        <ContextMenuItem id="boards-pin-menu-remove" icon={<FolderMinus />} label="Remove from this folder" onClick={run(() => actions.removeFromFolder(pins))} />
      ) : null}
      {folderId && !many && hasPreview(pin) ? (
        <ContextMenuItem id="boards-pin-menu-cover" icon={<ImageIcon />} label="Set as folder cover" onClick={run(() => actions.setCover(pin))} />
      ) : null}
      <ContextMenuSeparator id="boards-pin-menu-separator-3" />
      <div id="boards-pin-menu-rating" className="flex items-center gap-1 px-2 py-1">
        <span id="boards-pin-menu-rating-label" className="mr-1 text-xs text-zinc-400">
          Rating
        </span>
        {[1, 2, 3, 4, 5].map((stars) => (
          <button
            id={`boards-pin-menu-rating-${stars}`}
            key={stars}
            type="button"
            role="menuitem"
            aria-label={`${stars} ${stars === 1 ? "star" : "stars"}`}
            className="inline-flex size-5 items-center justify-center rounded text-zinc-500 hover:text-amber-300"
            onClick={run(() => actions.rate(pins, stars === rating ? 0 : stars))}
          >
            <Star id={`boards-pin-menu-rating-${stars}-icon`} className={cn("size-3.5", stars <= rating && "fill-current text-amber-300")} aria-hidden />
          </button>
        ))}
      </div>
      <ContextMenuSeparator id="boards-pin-menu-separator-4" />
      <ContextMenuItem id="boards-pin-menu-trash" icon={<Trash2 />} label={`Move to Trash${count}`} danger onClick={run(() => actions.trash(pins))} />
    </ContextMenu>
  );
}
