import { memo, useState, type DragEvent, type MouseEvent } from "react";
import { FileText, Music, Play, Star } from "lucide-react";
import { FavIcon } from "@/components/FavIcon";
import { cn } from "@/lib/cn";
import type { Pin } from "@type/browser";
import { PIN_KIND_LABELS, formatBytes, formatDimensions, hostOf, pinAddress, pinName, previewUrl, viewerKind } from "../folder-tree";

/** Height of the name and dimensions under a tile. */
export const CAPTION_HEIGHT = 36;

type PinTileProps = {
  pin: Pin;
  x: number;
  y: number;
  width: number;
  height: number;
  captions: boolean;
  /** Grid cells fit the whole preview; other layouts fill the tile. */
  fit: "cover" | "contain";
  selected: boolean;
  focused: boolean;
  onClick: (pin: Pin, event: MouseEvent) => void;
  onDoubleClick: (pin: Pin) => void;
  onContextMenu: (pin: Pin, event: MouseEvent) => void;
  onDragStart: (pin: Pin, event: DragEvent) => void;
};

export const PinTile = memo(function PinTile({
  pin,
  x,
  y,
  width,
  height,
  captions,
  fit,
  selected,
  focused,
  onClick,
  onDoubleClick,
  onContextMenu,
  onDragStart,
}: PinTileProps) {
  const idBase = `boards-pin-${pin.id}`;
  const [failed, setFailed] = useState(false);
  const src = failed ? "" : previewUrl(pin);
  const kind = viewerKind(pin);
  const detail = formatDimensions(pin) || [pin.ext.toUpperCase(), formatBytes(pin.size)].filter(Boolean).join(" · ") || PIN_KIND_LABELS[pin.kind];
  const showExt = pin.ext && kind !== "image" && pin.kind !== "page" && pin.kind !== "region" && pin.kind !== "element";

  return (
    <div
      id={idBase}
      data-tile={pin.id}
      role="option"
      aria-selected={selected}
      draggable
      className="group absolute select-none"
      style={{ left: x, top: y, width }}
      onClick={(event) => onClick(pin, event)}
      onDoubleClick={() => onDoubleClick(pin)}
      onContextMenu={(event) => onContextMenu(pin, event)}
      onDragStart={(event) => onDragStart(pin, event)}
    >
      <div
        id={`${idBase}-media`}
        className={cn(
          "relative overflow-hidden rounded-md bg-zinc-900 ring-1 ring-zinc-800 transition-shadow group-hover:ring-zinc-600",
          selected && "ring-[3px] ring-sky-500 group-hover:ring-sky-400",
          focused && !selected && "ring-2 ring-zinc-400",
        )}
        style={{ height }}
      >
        {src ? (
          <img
            id={`${idBase}-image`}
            src={src}
            alt=""
            draggable={false}
            loading="lazy"
            decoding="async"
            className={cn("absolute inset-0 size-full", fit === "contain" ? "object-contain p-1" : "object-cover object-top")}
            onError={() => setFailed(true)}
          />
        ) : (
          <PinCard pin={pin} idBase={idBase} />
        )}
        {kind === "video" ? (
          <span id={`${idBase}-play`} className="absolute bottom-1.5 right-1.5 inline-flex size-6 items-center justify-center rounded-full bg-black/60 text-white">
            <Play id={`${idBase}-play-icon`} className="size-3 fill-current" aria-hidden />
          </span>
        ) : null}
        {showExt || pin.ext === "gif" ? (
          <span id={`${idBase}-ext`} className="absolute bottom-1.5 left-1.5 rounded bg-black/65 px-1.5 py-px text-[10px] font-semibold uppercase tracking-wide text-white">
            {pin.ext}
          </span>
        ) : null}
        {pin.rating > 0 ? (
          <span id={`${idBase}-rating`} className="absolute right-1.5 top-1.5 hidden items-center gap-0.5 rounded bg-black/60 px-1 py-px text-[10px] text-amber-300 group-hover:inline-flex">
            <Star id={`${idBase}-rating-icon`} className="size-2.5 fill-current" aria-hidden />
            {pin.rating}
          </span>
        ) : null}
      </div>
      {captions ? (
        <div id={`${idBase}-caption`} className="flex flex-col items-center px-1 pt-1" style={{ height: CAPTION_HEIGHT }}>
          <span
            id={`${idBase}-name`}
            className={cn("max-w-full truncate rounded px-1.5 text-xs leading-4 text-zinc-200", selected && "bg-sky-600 text-white")}
            title={pinName(pin)}
          >
            {pinName(pin)}
          </span>
          <span id={`${idBase}-detail`} className="max-w-full truncate text-[11px] leading-4 text-zinc-500">
            {detail}
          </span>
        </div>
      ) : null}
    </div>
  );
});

/** What a tile shows when the pin has no image. */
function PinCard({ pin, idBase }: { pin: Pin; idBase: string }) {
  const kind = viewerKind(pin);
  const address = pinAddress(pin);
  if (pin.kind === "link" || (kind === "link" && address)) {
    return (
      <div id={`${idBase}-link`} className="flex size-full flex-col justify-between gap-2 bg-gradient-to-br from-zinc-800 to-zinc-900 p-3">
        <FavIcon id={`${idBase}-link-icon`} url={address} className="size-6" />
        <div id={`${idBase}-link-text`} className="min-w-0">
          <p id={`${idBase}-link-title`} className="line-clamp-3 text-sm font-medium text-zinc-100">
            {pin.title || address}
          </p>
          <p id={`${idBase}-link-host`} className="truncate text-[11px] text-zinc-500">
            {hostOf(address)}
          </p>
        </div>
      </div>
    );
  }
  if (pin.text && pin.kind !== "file") {
    return (
      <p id={`${idBase}-excerpt`} className="size-full overflow-hidden whitespace-pre-line p-3 text-xs leading-relaxed text-zinc-300">
        {pin.text}
      </p>
    );
  }
  const Icon = kind === "audio" ? Music : FileText;
  return (
    <div id={`${idBase}-file`} className="flex size-full flex-col items-center justify-center gap-2 text-zinc-500">
      <Icon id={`${idBase}-file-icon`} className="size-8" aria-hidden />
      <span id={`${idBase}-file-ext`} className="text-xs font-semibold uppercase tracking-wide">
        {pin.ext || "file"}
      </span>
    </div>
  );
}
