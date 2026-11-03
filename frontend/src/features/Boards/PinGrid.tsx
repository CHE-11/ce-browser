import { useEffect, useRef, useState, type MouseEvent, type ReactNode } from "react";
import { Check, Copy, ExternalLink, File, FolderInput, ImageOff, Maximize2, Save, Star, Trash2 } from "lucide-react";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "@/components/ContextMenu";
import { FavIcon } from "@/components/FavIcon";
import { cn } from "@/lib/cn";
import type { BoardsSnapshot, Pin } from "@type/browser";
import { DRAG_PINS, PIN_KIND_LABELS, hostOf } from "./board-tree";
import { useThumbnail } from "./thumbnails";

/** Thumbnails keep at most this height for each unit of width. Matches the backend crop. */
const MAX_ASPECT = 2.5;

export type PinActions = {
  open: (pin: Pin) => void;
  openSource: (pin: Pin) => void;
  copyImage: (pin: Pin) => void;
  openFile: (pin: Pin) => void;
  saveFile: (pin: Pin) => void;
  move: (ids: string[], x: number, y: number) => void;
  setCover: (pin: Pin) => void;
  remove: (ids: string[]) => void;
};

type PinGridProps = {
  boards: BoardsSnapshot;
  pins: Pin[];
  selected: Set<string>;
  /** Show which board each pin is on, for views that mix boards. */
  showBoard: boolean;
  onSelect: (pin: Pin, mode: SelectMode) => void;
  actions: PinActions;
};

/** Toggle one pin, or select every pin between the last one picked and this one. */
export type SelectMode = "toggle" | "range";

export function PinGrid({ boards, pins, selected, showBoard, onSelect, actions }: PinGridProps) {
  const [menu, setMenu] = useState<{ pin: Pin; x: number; y: number } | null>(null);
  const boardNames = new Map(boards.boards.map((board) => [board.id, board.name]));

  return (
    <>
      <div id="boards-grid" className="columns-[15rem] gap-4">
        {pins.map((pin) => (
          <PinCard
            key={pin.id}
            pin={pin}
            boardName={showBoard ? boardNames.get(pin.boardId) : undefined}
            selected={selected.has(pin.id)}
            selection={selected}
            onSelect={onSelect}
            onMenu={(event) => {
              event.preventDefault();
              setMenu({ pin, x: event.clientX, y: event.clientY });
            }}
            actions={actions}
          />
        ))}
      </div>
      {menu ? (
        <PinMenu
          pin={menu.pin}
          x={menu.x}
          y={menu.y}
          ids={selected.has(menu.pin.id) ? [...selected] : [menu.pin.id]}
          actions={actions}
          onClose={() => setMenu(null)}
        />
      ) : null}
    </>
  );
}

type PinCardProps = {
  pin: Pin;
  boardName?: string;
  selected: boolean;
  selection: Set<string>;
  onSelect: (pin: Pin, mode: SelectMode) => void;
  onMenu: (event: MouseEvent) => void;
  actions: PinActions;
};

function PinCard({ pin, boardName, selected, selection, onSelect, onMenu, actions }: PinCardProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  const thumbnail = useThumbnail(pin.hasImage ? pin.id : null, visible);
  const host = hostOf(pin.pageUrl || pin.linkUrl);
  const address = pin.pageUrl || pin.linkUrl;
  const idBase = `boards-pin-${pin.id}`;

  useEffect(() => {
    const element = ref.current;
    if (!element || visible) {
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setVisible(true);
          observer.disconnect();
        }
      },
      { rootMargin: "600px 0px" },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [visible]);

  const aspect = pin.hasImage && pin.width > 0 && pin.height > 0 ? `${pin.width} / ${Math.min(pin.height, pin.width * MAX_ASPECT)}` : undefined;

  return (
    <div
      id={idBase}
      ref={ref}
      role="button"
      tabIndex={0}
      draggable
      aria-pressed={selected}
      className={cn(
        "group relative mb-4 cursor-zoom-in break-inside-avoid overflow-hidden rounded-xl bg-zinc-900 ring-1 ring-zinc-800 transition hover:ring-zinc-600 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sky-500",
        selected && "ring-2 ring-sky-500 hover:ring-sky-400",
      )}
      onClick={(event) => {
        if (event.shiftKey) {
          onSelect(pin, "range");
        } else if (event.ctrlKey || event.metaKey) {
          onSelect(pin, "toggle");
        } else {
          actions.open(pin);
        }
      }}
      onKeyDown={(event) => {
        if (event.target !== event.currentTarget) {
          return;
        }
        if (event.key === "Enter") {
          actions.open(pin);
        } else if (event.key === " ") {
          event.preventDefault();
          onSelect(pin, "toggle");
        }
      }}
      onContextMenu={onMenu}
      onDragStart={(event) => {
        const ids = selection.has(pin.id) ? [...selection] : [pin.id];
        event.dataTransfer.setData(DRAG_PINS, JSON.stringify(ids));
        event.dataTransfer.effectAllowed = "move";
      }}
    >
      {pin.hasImage ? (
        <div id={`${idBase}-media`} className="relative w-full bg-zinc-800" style={{ aspectRatio: aspect ?? "4 / 3" }}>
          {thumbnail ? (
            <img id={`${idBase}-image`} src={thumbnail} alt="" draggable={false} className="absolute inset-0 size-full object-cover object-top" />
          ) : visible ? null : (
            <div id={`${idBase}-placeholder`} className="absolute inset-0 animate-pulse bg-zinc-800" />
          )}
          {visible && thumbnail === null && pin.hasImage ? (
            <ImageOff id={`${idBase}-missing`} className="absolute left-1/2 top-1/2 size-5 -translate-x-1/2 -translate-y-1/2 text-zinc-600" aria-hidden />
          ) : null}
        </div>
      ) : (
        <p id={`${idBase}-excerpt`} className="line-clamp-[10] whitespace-pre-line px-3 pt-3 text-sm text-zinc-300">
          {pin.text || pin.title}
        </p>
      )}

      <button
        id={`${idBase}-select`}
        type="button"
        aria-label={selected ? "Deselect" : "Select"}
        className={cn(
          "absolute left-2 top-2 inline-flex size-6 items-center justify-center rounded-full border border-white/70 bg-black/40 text-white opacity-0 transition group-hover:opacity-100",
          selected && "border-sky-400 bg-sky-500 opacity-100",
        )}
        onClick={(event) => {
          event.stopPropagation();
          onSelect(pin, event.shiftKey ? "range" : "toggle");
        }}
      >
        {selected ? <Check id={`${idBase}-select-icon`} className="size-3.5" aria-hidden /> : null}
      </button>

      <div id={`${idBase}-actions`} className="absolute right-2 top-2 flex gap-1 opacity-0 transition group-hover:opacity-100">
        {address ? (
          <CardAction id={`${idBase}-source`} label="Open source" onClick={() => actions.openSource(pin)}>
            <ExternalLink aria-hidden />
          </CardAction>
        ) : null}
        {pin.kind === "file" ? (
          <CardAction id={`${idBase}-file`} label="Open file" onClick={() => actions.openFile(pin)}>
            <File aria-hidden />
          </CardAction>
        ) : null}
        {pin.hasImage ? (
          <CardAction id={`${idBase}-copy`} label="Copy image" onClick={() => actions.copyImage(pin)}>
            <Copy aria-hidden />
          </CardAction>
        ) : null}
        <CardAction
          id={`${idBase}-move`}
          label="Move to board"
          onClick={(event) => {
            const rect = event.currentTarget.getBoundingClientRect();
            actions.move(selection.has(pin.id) ? [...selection] : [pin.id], rect.left, rect.bottom + 4);
          }}
        >
          <FolderInput aria-hidden />
        </CardAction>
        <CardAction id={`${idBase}-delete`} label="Delete" danger onClick={() => actions.remove(selection.has(pin.id) ? [...selection] : [pin.id])}>
          <Trash2 aria-hidden />
        </CardAction>
      </div>

      <div id={`${idBase}-info`} className="flex flex-col gap-1 px-3 py-2.5">
        <p id={`${idBase}-title`} className="line-clamp-2 text-sm font-medium text-zinc-100" title={pin.title}>
          {pin.title || host || "Untitled"}
        </p>
        {pin.note ? (
          <p id={`${idBase}-note`} className="line-clamp-3 text-xs text-zinc-400">
            {pin.note}
          </p>
        ) : null}
        <div id={`${idBase}-meta`} className="flex min-w-0 items-center gap-1.5 text-[11px] text-zinc-500">
          <FavIcon id={`${idBase}-favicon`} url={pin.pageUrl} className="size-3.5" />
          <span id={`${idBase}-host`} className="min-w-0 flex-1 truncate">
            {host}
            {boardName ? ` · ${boardName}` : ""}
          </span>
          <span id={`${idBase}-kind`} className="shrink-0 rounded bg-zinc-800 px-1.5 py-px text-[10px] uppercase tracking-wide text-zinc-400">
            {PIN_KIND_LABELS[pin.kind]}
          </span>
        </div>
      </div>
    </div>
  );
}

function CardAction({
  id,
  label,
  danger,
  onClick,
  children,
}: {
  id: string;
  label: string;
  danger?: boolean;
  onClick: (event: MouseEvent<HTMLButtonElement>) => void;
  children: ReactNode;
}) {
  return (
    <button
      id={id}
      type="button"
      title={label}
      aria-label={label}
      className={cn(
        "inline-flex size-7 items-center justify-center rounded-full bg-black/60 text-white backdrop-blur transition hover:bg-black/80 [&_svg]:size-3.5",
        danger && "hover:bg-red-600",
      )}
      onClick={(event) => {
        event.stopPropagation();
        onClick(event);
      }}
    >
      {children}
    </button>
  );
}

export function PinMenu({ pin, ids, x, y, actions, onClose }: { pin: Pin; ids: string[]; x: number; y: number; actions: PinActions; onClose: () => void }) {
  const many = ids.length > 1;
  const item = (run: () => void) => () => {
    run();
    onClose();
  };
  return (
    <ContextMenu id="boards-pin-menu" label="Pin" x={x} y={y} onClose={onClose}>
      <ContextMenuItem id="boards-pin-menu-open" icon={<Maximize2 />} label="Open" disabled={many} onClick={item(() => actions.open(pin))} />
      <ContextMenuItem
        id="boards-pin-menu-source"
        icon={<ExternalLink />}
        label="Open source"
        disabled={many || (!pin.pageUrl && !pin.linkUrl)}
        onClick={item(() => actions.openSource(pin))}
      />
      {pin.kind === "file" ? (
        <>
          <ContextMenuItem id="boards-pin-menu-open-file" icon={<File />} label="Open file" disabled={many} onClick={item(() => actions.openFile(pin))} />
          <ContextMenuItem id="boards-pin-menu-save-file" icon={<Save />} label="Save file" disabled={many} onClick={item(() => actions.saveFile(pin))} />
        </>
      ) : null}
      <ContextMenuItem
        id="boards-pin-menu-copy"
        icon={<Copy />}
        label="Copy image"
        disabled={many || !pin.hasImage}
        onClick={item(() => actions.copyImage(pin))}
      />
      <ContextMenuItem
        id="boards-pin-menu-cover"
        icon={<Star />}
        label="Use as board cover"
        disabled={many || !pin.hasImage}
        onClick={item(() => actions.setCover(pin))}
      />
      <ContextMenuSeparator id="boards-pin-menu-separator" />
      <ContextMenuItem
        id="boards-pin-menu-move"
        icon={<FolderInput />}
        label={many ? `Move ${ids.length} pins to…` : "Move to…"}
        onClick={item(() => actions.move(ids, x, y))}
      />
      <ContextMenuItem
        id="boards-pin-menu-delete"
        icon={<Trash2 />}
        label={many ? `Delete ${ids.length} pins` : "Delete"}
        danger
        onClick={item(() => actions.remove(ids))}
      />
    </ContextMenu>
  );
}
