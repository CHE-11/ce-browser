import { useEffect, useRef, useState, type MouseEvent } from "react";
import { File, ImageOff } from "lucide-react";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import type { BoardsSnapshot, Pin } from "@type/browser";
import { DRAG_PINS, PIN_KIND_LABELS, hostOf, pinAddress } from "./board-tree";
import { PinMenu, type PinActions, type SelectMode } from "./PinGrid";
import { useThumbnail } from "./thumbnails";

type PinTableProps = {
  boards: BoardsSnapshot;
  pins: Pin[];
  selected: Set<string>;
  showBoard: boolean;
  onSelect: (pin: Pin, mode: SelectMode) => void;
  onSelectAll: () => void;
  onClear: () => void;
  actions: PinActions;
};

export function PinTable({ boards, pins, selected, showBoard, onSelect, onSelectAll, onClear, actions }: PinTableProps) {
  const [menu, setMenu] = useState<{ pin: Pin; x: number; y: number } | null>(null);
  const boardNames = new Map(boards.boards.map((board) => [board.id, board.name]));
  const allSelected = pins.length > 0 && pins.every((pin) => selected.has(pin.id));

  return (
    <>
      <div id="boards-table-wrap" className="overflow-x-auto">
        <table id="boards-table" className="w-full min-w-[46rem] border-collapse text-left text-sm">
          <thead id="boards-table-head">
            <tr id="boards-table-head-row" className="border-b border-zinc-800 text-xs text-zinc-500">
              <th id="boards-table-check" className="w-10 px-2 py-2">
                <input
                  id="boards-table-check-all"
                  type="checkbox"
                  checked={allSelected}
                  aria-label={allSelected ? "Clear selection" : "Select all"}
                  className="size-4 accent-sky-500"
                  onChange={() => (allSelected ? onClear() : onSelectAll())}
                />
              </th>
              <th id="boards-table-preview-label" className="w-14 px-2 py-2 font-medium">
                <span id="boards-table-preview-text" className="sr-only">
                  Preview
                </span>
              </th>
              <th id="boards-table-title-label" className="px-2 py-2 font-medium">
                Title
              </th>
              <th id="boards-table-kind-label" className="w-24 px-2 py-2 font-medium">
                Kind
              </th>
              {showBoard ? (
                <th id="boards-table-board-label" className="w-40 px-2 py-2 font-medium">
                  Board
                </th>
              ) : null}
              <th id="boards-table-source-label" className="w-40 px-2 py-2 font-medium">
                Source
              </th>
              <th id="boards-table-added-label" className="w-32 px-2 py-2 font-medium">
                Added
              </th>
              <th id="boards-table-note-label" className="w-56 px-2 py-2 font-medium">
                Note
              </th>
            </tr>
          </thead>
          <tbody id="boards-table-body">
            {pins.map((pin) => (
              <PinRow
                key={pin.id}
                pin={pin}
                showBoard={showBoard}
                boardName={boardNames.get(pin.boardId)}
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
          </tbody>
        </table>
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

function PinRow({
  pin,
  showBoard,
  boardName,
  selected,
  selection,
  onSelect,
  onMenu,
  actions,
}: {
  pin: Pin;
  showBoard: boolean;
  boardName?: string;
  selected: boolean;
  selection: Set<string>;
  onSelect: (pin: Pin, mode: SelectMode) => void;
  onMenu: (event: MouseEvent) => void;
  actions: PinActions;
}) {
  const base = `boards-row-${pin.id}`;
  const address = pinAddress(pin);
  const source = pin.kind === "file" ? pin.fileName || "File" : hostOf(address) || address;

  return (
    <tr
      id={base}
      tabIndex={0}
      draggable
      aria-selected={selected}
      className={cn(
        "cursor-pointer border-b border-zinc-800/80 text-zinc-200 hover:bg-zinc-900",
        selected && "bg-sky-950/40",
        focusRow,
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
      <td id={`${base}-check`} className="px-2 py-2">
        <input
          id={`${base}-checkbox`}
          type="checkbox"
          checked={selected}
          aria-label={selected ? "Deselect" : "Select"}
          className="size-4 accent-sky-500"
          onClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
            onSelect(pin, event.shiftKey ? "range" : "toggle");
          }}
          onChange={() => undefined}
        />
      </td>
      <td id={`${base}-preview`} className="px-2 py-2">
        <RowThumb pin={pin} />
      </td>
      <td id={`${base}-title`} className="max-w-xs truncate px-2 py-2 font-medium" title={pin.title}>
        {pin.title || source || "Untitled"}
      </td>
      <td id={`${base}-kind`} className="px-2 py-2 text-xs uppercase tracking-wide text-zinc-400">
        {PIN_KIND_LABELS[pin.kind]}
      </td>
      {showBoard ? (
        <td id={`${base}-board`} className="truncate px-2 py-2 text-zinc-400" title={boardName}>
          {boardName}
        </td>
      ) : null}
      <td id={`${base}-source`} className="truncate px-2 py-2 text-zinc-400" title={address || pin.fileName}>
        {source}
      </td>
      <td id={`${base}-added`} className="whitespace-nowrap px-2 py-2 text-zinc-400">
        {new Date(pin.createdAt).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" })}
      </td>
      <td id={`${base}-note`} className="max-w-56 truncate px-2 py-2 text-zinc-500" title={pin.note}>
        {pin.note}
      </td>
    </tr>
  );
}

function RowThumb({ pin }: { pin: Pin }) {
  const ref = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  const thumbnail = useThumbnail(pin.hasImage ? pin.id : null, visible);
  const base = `boards-row-${pin.id}-thumb`;

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
      { rootMargin: "200px 0px" },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [visible]);

  return (
    <div id={base} ref={ref} className="flex size-10 items-center justify-center overflow-hidden rounded-md bg-zinc-800">
      {pin.hasImage && thumbnail ? (
        <img id={`${base}-image`} src={thumbnail} alt="" draggable={false} className="size-full object-cover object-top" />
      ) : pin.kind === "file" ? (
        <File id={`${base}-file`} className="size-4 text-zinc-400" aria-hidden />
      ) : pin.hasImage && visible ? (
        <ImageOff id={`${base}-missing`} className="size-4 text-zinc-600" aria-hidden />
      ) : null}
    </div>
  );
}
