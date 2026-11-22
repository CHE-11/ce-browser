import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Star } from "lucide-react";
import { FavIcon } from "@/components/FavIcon";
import { cn } from "@/lib/cn";
import type { BoardsSnapshot, Pin } from "@type/browser";
import { PIN_KIND_LABELS, formatBytes, formatDimensions, pinAddress, pinName, previewUrl } from "../folder-tree";
import { startPinDrag, type PinListHandlers } from "./PinGrid";

type PinTableProps = PinListHandlers & {
  id: string;
  boards: BoardsSnapshot;
  pins: Pin[];
  selected: Set<string>;
  anchor: number;
  keyboard: boolean;
  header?: ReactNode;
  empty?: ReactNode;
};

const ROW = 40;
const OVERSCAN_ROWS = 12;
const COLUMNS = "grid-cols-[minmax(0,2.4fr)_minmax(0,1.3fr)_minmax(0,1.3fr)_5rem_5rem_6.5rem_7rem_4.5rem]";

function isTyping(target: EventTarget | null) {
  return target instanceof HTMLElement && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT" || target.isContentEditable);
}

/** The List layout: one row per pin with its properties. */
export function PinTable({ id, boards, pins, selected, anchor, keyboard, header, empty, onSelect, onMarquee, onOpen, onMenu, onNavigate }: PinTableProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  const [range, setRange] = useState({ first: 0, last: 40 });
  const navigated = useRef(false);
  const folderNames = useMemo(() => new Map(boards.folders.map((folder) => [folder.id, folder.name])), [boards.folders]);

  const measure = useCallback(() => {
    const scroller = scrollRef.current;
    const body = bodyRef.current;
    if (!scroller || !body) {
      return;
    }
    const offset = body.getBoundingClientRect().top - scroller.getBoundingClientRect().top;
    const first = Math.max(0, Math.floor(-offset / ROW) - OVERSCAN_ROWS);
    const last = Math.ceil((-offset + scroller.clientHeight) / ROW) + OVERSCAN_ROWS;
    setRange((current) => (current.first === first && current.last === last ? current : { first, last }));
  }, []);

  useLayoutEffect(() => {
    const scroller = scrollRef.current;
    if (!scroller) {
      return;
    }
    const observer = new ResizeObserver(measure);
    observer.observe(scroller);
    measure();
    return () => observer.disconnect();
  }, [measure]);

  useLayoutEffect(measure, [measure, pins.length, header]);

  useEffect(() => {
    if (!navigated.current || anchor < 0) {
      return;
    }
    navigated.current = false;
    const scroller = scrollRef.current;
    const body = bodyRef.current;
    if (!scroller || !body) {
      return;
    }
    const offset = body.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop;
    const top = offset + anchor * ROW;
    if (top < scroller.scrollTop + ROW) {
      scroller.scrollTop = top - ROW;
    } else if (top + ROW > scroller.scrollTop + scroller.clientHeight) {
      scroller.scrollTop = top + ROW - scroller.clientHeight;
    }
  }, [anchor]);

  useEffect(() => {
    if (!keyboard) {
      return;
    }
    const onKey = (event: KeyboardEvent) => {
      if (isTyping(event.target) || event.altKey || event.ctrlKey || event.metaKey || pins.length === 0) {
        return;
      }
      if (event.key === "ArrowUp" || event.key === "ArrowDown") {
        event.preventDefault();
        navigated.current = true;
        const next = anchor < 0 ? 0 : Math.min(pins.length - 1, Math.max(0, anchor + (event.key === "ArrowDown" ? 1 : -1)));
        onNavigate(next, event.shiftKey);
      } else if ((event.key === "Enter" || event.key === " ") && pins[anchor]) {
        event.preventDefault();
        onOpen(pins[anchor]);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [keyboard, pins, anchor, onNavigate, onOpen]);

  const shown = pins.slice(range.first, range.last);

  return (
    <div
      id={id}
      ref={scrollRef}
      className="relative min-h-0 flex-1 overflow-y-auto"
      onScroll={measure}
      onClick={(event) => {
        if (!(event.target as HTMLElement).closest("[data-row],[data-folder-card],button")) {
          onMarquee([], false);
        }
      }}
    >
      <div id={`${id}-inner`} className="px-6 py-4">
        {header}
        {pins.length === 0 ? (
          empty
        ) : (
          <div id={`${id}-table`} role="listbox" aria-multiselectable className="text-xs">
            <div id={`${id}-head`} className={cn("sticky top-0 z-10 grid h-8 items-center gap-3 border-b border-zinc-800 bg-zinc-950 px-2 text-[11px] font-medium text-zinc-500", COLUMNS)}>
              <span id={`${id}-head-name`}>Name</span>
              <span id={`${id}-head-folders`}>Folders</span>
              <span id={`${id}-head-tags`}>Tags</span>
              <span id={`${id}-head-type`}>Type</span>
              <span id={`${id}-head-size`} className="text-right">
                Size
              </span>
              <span id={`${id}-head-dimensions`} className="text-right">
                Dimensions
              </span>
              <span id={`${id}-head-added`} className="text-right">
                Added
              </span>
              <span id={`${id}-head-rating`} className="text-right">
                Rating
              </span>
            </div>
            <div id={`${id}-body`} ref={bodyRef} className="relative" style={{ height: pins.length * ROW }}>
              {shown.map((pin, offset) => {
                const index = range.first + offset;
                const isSelected = selected.has(pin.id);
                const rowId = `${id}-row-${pin.id}`;
                const src = previewUrl(pin);
                return (
                  <div
                    id={rowId}
                    key={pin.id}
                    data-row={pin.id}
                    role="option"
                    aria-selected={isSelected}
                    draggable
                    className={cn(
                      "absolute inset-x-0 grid cursor-default select-none items-center gap-3 rounded-md px-2 text-zinc-300 hover:bg-zinc-900",
                      COLUMNS,
                      isSelected && "bg-sky-950 text-sky-100 hover:bg-sky-950",
                      index === anchor && !isSelected && "ring-1 ring-inset ring-zinc-600",
                    )}
                    style={{ top: index * ROW, height: ROW }}
                    onClick={(event) => onSelect(pin, event.shiftKey ? "range" : event.ctrlKey || event.metaKey ? "toggle" : "single")}
                    onDoubleClick={() => onOpen(pin)}
                    onContextMenu={(event) => {
                      event.preventDefault();
                      if (!isSelected) {
                        onSelect(pin, "single");
                      }
                      onMenu(pin, event.clientX, event.clientY);
                    }}
                    onDragStart={(event) => {
                      if (!isSelected) {
                        onSelect(pin, "single");
                      }
                      startPinDrag(event, isSelected ? pins.filter((item) => selected.has(item.id)) : [pin]);
                    }}
                  >
                    <span id={`${rowId}-name`} className="flex min-w-0 items-center gap-2">
                      {src ? (
                        <img id={`${rowId}-thumb`} src={src} alt="" loading="lazy" draggable={false} className="size-7 shrink-0 rounded object-cover" />
                      ) : (
                        <FavIcon id={`${rowId}-icon`} url={pinAddress(pin)} className="size-4 shrink-0" />
                      )}
                      <span id={`${rowId}-title`} className="truncate text-[13px] text-zinc-100" title={pinName(pin)}>
                        {pinName(pin)}
                      </span>
                    </span>
                    <span id={`${rowId}-folders`} className="truncate text-zinc-400">
                      {pin.folderIds.map((folder) => folderNames.get(folder)).filter(Boolean).join(", ") || "—"}
                    </span>
                    <span id={`${rowId}-tags`} className="truncate text-zinc-400">
                      {pin.tags.join(", ") || "—"}
                    </span>
                    <span id={`${rowId}-type`} className="truncate uppercase text-zinc-400">
                      {pin.ext || PIN_KIND_LABELS[pin.kind]}
                    </span>
                    <span id={`${rowId}-size`} className="text-right tabular-nums text-zinc-400">
                      {formatBytes(pin.size) || "—"}
                    </span>
                    <span id={`${rowId}-dimensions`} className="text-right tabular-nums text-zinc-400">
                      {formatDimensions(pin) || "—"}
                    </span>
                    <span id={`${rowId}-added`} className="text-right tabular-nums text-zinc-400">
                      {new Date(pin.createdAt).toLocaleDateString()}
                    </span>
                    <span id={`${rowId}-rating`} className="flex justify-end gap-px text-amber-300">
                      {Array.from({ length: pin.rating }, (_, star) => (
                        <Star id={`${rowId}-rating-${star}`} key={star} className="size-2.5 fill-current" aria-hidden />
                      ))}
                    </span>
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
