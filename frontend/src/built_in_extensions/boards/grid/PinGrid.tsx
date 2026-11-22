import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type DragEvent, type MouseEvent, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import type { Pin } from "@type/browser";
import { DRAG_PINS, pinAddress } from "../folder-tree";
import { computeLayout, intersects, neighbor, tilesInBand, type LayoutKind } from "./layout";
import { CAPTION_HEIGHT, PinTile } from "./PinTile";

/** Pick one pin, add or remove one, or take everything between the anchor and this one. */
export type SelectMode = "single" | "toggle" | "range";

export type PinListHandlers = {
  onSelect: (pin: Pin, mode: SelectMode) => void;
  /** Pins inside a drag rectangle. `additive` keeps the earlier selection. */
  onMarquee: (ids: string[], additive: boolean) => void;
  onOpen: (pin: Pin) => void;
  onMenu: (pin: Pin, x: number, y: number) => void;
  /** Arrow keys moved to `index`; `extend` when Shift was held. */
  onNavigate: (index: number, extend: boolean) => void;
};

type PinGridProps = PinListHandlers & {
  id: string;
  pins: Pin[];
  layout: LayoutKind;
  size: number;
  captions: boolean;
  selected: Set<string>;
  /** Index in `pins` of the last pin picked; arrow keys move from it. */
  anchor: number;
  /** Arrow keys and Enter act on the grid. Off while a dialog or the viewer has focus. */
  keyboard: boolean;
  /** Shown above the pins and scrolled with them, such as subfolder cards. */
  header?: ReactNode;
  /** Shown instead of the pins when there are none. */
  empty?: ReactNode;
  /** Right-click on empty space. */
  onBackgroundMenu?: (x: number, y: number) => void;
};

const OVERSCAN = 800;
const EDGE = 48;

function isTyping(target: EventTarget | null) {
  return target instanceof HTMLElement && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT" || target.isContentEditable);
}

/** Sets the drag data for pins, so the sidebar and other windows can take them. */
export function startPinDrag(event: DragEvent, pins: Pin[]) {
  event.dataTransfer.setData(DRAG_PINS, JSON.stringify(pins.map((pin) => pin.id)));
  const urls = pins.map(pinAddress).filter(Boolean);
  if (urls.length > 0) {
    event.dataTransfer.setData("text/uri-list", urls.join("\r\n"));
    event.dataTransfer.setData("text/plain", urls.join("\n"));
  }
  event.dataTransfer.effectAllowed = "copyMove";
}

export function PinGrid({
  id,
  pins,
  layout: kind,
  size,
  captions,
  selected,
  anchor,
  keyboard,
  header,
  empty,
  onSelect,
  onMarquee,
  onOpen,
  onMenu,
  onNavigate,
  onBackgroundMenu,
}: PinGridProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  const [band, setBand] = useState({ top: 0, bottom: 1200 });
  const [marquee, setMarquee] = useState<{ left: number; top: number; right: number; bottom: number } | null>(null);
  const caption = captions ? CAPTION_HEIGHT : 0;
  const navigated = useRef(false);

  const items = useMemo(
    () => pins.map((pin) => ({ id: pin.id, aspect: pin.width > 0 && pin.height > 0 ? pin.width / pin.height : pin.kind === "link" ? 1.33 : 1 })),
    [pins],
  );
  const layout = useMemo(() => computeLayout(kind, items, width, size, caption), [kind, items, width, size, caption]);
  const byId = useMemo(() => new Map(pins.map((pin) => [pin.id, pin])), [pins]);

  const measure = useCallback(() => {
    const scroller = scrollRef.current;
    const canvas = canvasRef.current;
    if (!scroller || !canvas) {
      return;
    }
    const offset = canvas.getBoundingClientRect().top - scroller.getBoundingClientRect().top;
    setBand({ top: -offset - OVERSCAN, bottom: -offset + scroller.clientHeight + OVERSCAN });
  }, []);

  useLayoutEffect(() => {
    const canvas = canvasRef.current;
    const scroller = scrollRef.current;
    if (!canvas || !scroller) {
      return;
    }
    const observer = new ResizeObserver(() => {
      setWidth(canvas.clientWidth);
      measure();
    });
    observer.observe(canvas);
    observer.observe(scroller);
    setWidth(canvas.clientWidth);
    measure();
    return () => observer.disconnect();
  }, [measure]);

  useLayoutEffect(measure, [measure, layout.height, header]);

  const visible = useMemo(() => tilesInBand(layout, band.top, band.bottom, caption), [layout, band, caption]);

  const scrollToIndex = useCallback(
    (index: number) => {
      const tile = layout.tiles.find((item) => item.index === index);
      const scroller = scrollRef.current;
      const canvas = canvasRef.current;
      if (!tile || !scroller || !canvas) {
        return;
      }
      const offset = canvas.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop;
      const top = offset + tile.y;
      const bottom = top + tile.height + caption;
      if (top < scroller.scrollTop + 8) {
        scroller.scrollTop = top - 8;
      } else if (bottom > scroller.scrollTop + scroller.clientHeight - 8) {
        scroller.scrollTop = bottom - scroller.clientHeight + 8;
      }
    },
    [layout, caption],
  );

  useEffect(() => {
    if (navigated.current && anchor >= 0) {
      navigated.current = false;
      scrollToIndex(anchor);
    }
  }, [anchor, scrollToIndex]);

  useEffect(() => {
    if (!keyboard) {
      return;
    }
    const onKey = (event: KeyboardEvent) => {
      if (isTyping(event.target) || event.altKey || event.ctrlKey || event.metaKey || pins.length === 0) {
        return;
      }
      if (event.key === "ArrowLeft" || event.key === "ArrowRight" || event.key === "ArrowUp" || event.key === "ArrowDown") {
        event.preventDefault();
        const next = anchor < 0 ? 0 : neighbor(layout, anchor, event.key);
        if (next >= 0) {
          navigated.current = true;
          onNavigate(next, event.shiftKey);
        }
      } else if ((event.key === "Enter" || event.key === " ") && anchor >= 0 && pins[anchor]) {
        event.preventDefault();
        onOpen(pins[anchor]);
      } else if (event.key === "Home" || event.key === "End") {
        event.preventDefault();
        navigated.current = true;
        onNavigate(event.key === "Home" ? 0 : pins.length - 1, event.shiftKey);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [keyboard, pins, anchor, layout, onNavigate, onOpen]);

  const onTileClick = useCallback(
    (pin: Pin, event: MouseEvent) => {
      event.stopPropagation();
      onSelect(pin, event.shiftKey ? "range" : event.ctrlKey || event.metaKey ? "toggle" : "single");
    },
    [onSelect],
  );

  const onTileMenu = useCallback(
    (pin: Pin, event: MouseEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (!selected.has(pin.id)) {
        onSelect(pin, "single");
      }
      onMenu(pin, event.clientX, event.clientY);
    },
    [selected, onSelect, onMenu],
  );

  const onTileDrag = useCallback(
    (pin: Pin, event: DragEvent) => {
      const dragged = selected.has(pin.id) ? pins.filter((item) => selected.has(item.id)) : [pin];
      if (!selected.has(pin.id)) {
        onSelect(pin, "single");
      }
      startPinDrag(event, dragged);
    },
    [pins, selected, onSelect],
  );

  const startMarquee = (event: ReactPointerEvent<HTMLDivElement>) => {
    const target = event.target as HTMLElement;
    if (event.button !== 0 || target.closest("[data-tile],[data-folder-card],button,input,a,textarea,select")) {
      return;
    }
    const scroller = scrollRef.current;
    const canvas = canvasRef.current;
    if (!scroller || !canvas || event.clientX >= scroller.getBoundingClientRect().left + scroller.clientWidth) {
      return;
    }
    event.preventDefault();
    const additive = event.ctrlKey || event.metaKey || event.shiftKey;
    const base = additive ? [...selected] : [];
    const startRect = canvas.getBoundingClientRect();
    const start = { x: event.clientX - startRect.left, y: event.clientY - startRect.top };
    let pointer = { x: event.clientX, y: event.clientY };
    let moved = false;
    let frame = 0;

    const update = () => {
      const rect = canvas.getBoundingClientRect();
      const current = { x: pointer.x - rect.left, y: pointer.y - rect.top };
      const box = {
        left: Math.min(start.x, current.x),
        top: Math.min(start.y, current.y),
        right: Math.max(start.x, current.x),
        bottom: Math.max(start.y, current.y),
      };
      if (!moved && box.right - box.left < 4 && box.bottom - box.top < 4) {
        return;
      }
      moved = true;
      setMarquee(box);
      const hit = layout.tiles.filter((tile) => intersects(tile, caption, box)).map((tile) => tile.id);
      onMarquee([...new Set([...base, ...hit])], additive);
    };

    const tick = () => {
      const bounds = scroller.getBoundingClientRect();
      if (moved) {
        if (pointer.y < bounds.top + EDGE) {
          scroller.scrollTop -= Math.ceil((bounds.top + EDGE - pointer.y) / 3);
          update();
        } else if (pointer.y > bounds.bottom - EDGE) {
          scroller.scrollTop += Math.ceil((pointer.y - (bounds.bottom - EDGE)) / 3);
          update();
        }
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);

    const onMove = (move: PointerEvent) => {
      pointer = { x: move.clientX, y: move.clientY };
      update();
    };
    const onUp = () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      setMarquee(null);
      if (!moved && !additive) {
        onMarquee([], false);
      }
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  };

  return (
    <div
      id={id}
      ref={scrollRef}
      className="relative min-h-0 flex-1 overflow-y-auto"
      onScroll={measure}
      onPointerDown={startMarquee}
      onContextMenu={(event) => {
        const target = event.target as HTMLElement;
        if (onBackgroundMenu && !target.closest("[data-tile],[data-folder-card]")) {
          event.preventDefault();
          onBackgroundMenu(event.clientX, event.clientY);
        }
      }}
    >
      <div id={`${id}-inner`} className="flex min-h-full flex-col px-6 py-4">
        {header}
        {pins.length === 0 ? empty : null}
        <div id={`${id}-canvas`} ref={canvasRef} role="listbox" aria-multiselectable className="relative w-full shrink-0" style={{ height: layout.height }}>
            {visible.map((tile) => {
              const pin = byId.get(tile.id);
              return pin ? (
                <PinTile
                  key={pin.id}
                  pin={pin}
                  x={tile.x}
                  y={tile.y}
                  width={tile.width}
                  height={tile.height}
                  captions={captions}
                  fit={kind === "grid" ? "contain" : "cover"}
                  selected={selected.has(pin.id)}
                  focused={tile.index === anchor}
                  onClick={onTileClick}
                  onDoubleClick={onOpen}
                  onContextMenu={onTileMenu}
                  onDragStart={onTileDrag}
                />
              ) : null;
            })}
            {marquee ? (
              <div
                id={`${id}-marquee`}
                className="pointer-events-none absolute z-10 border border-sky-400 bg-sky-400/15"
                style={{ left: marquee.left, top: marquee.top, width: marquee.right - marquee.left, height: marquee.bottom - marquee.top }}
              />
            ) : null}
        </div>
        <div id={`${id}-spacer`} className="min-h-16 flex-1" />
      </div>
    </div>
  );
}
