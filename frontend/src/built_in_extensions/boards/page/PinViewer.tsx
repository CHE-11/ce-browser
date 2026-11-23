import { useCallback, useEffect, useLayoutEffect, useRef, useState, type PointerEvent as ReactPointerEvent, type WheelEvent as ReactWheelEvent } from "react";
import { ArrowLeft, ChevronLeft, ChevronRight, ExternalLink, Eye, File, FileText, Maximize, Minus, Music, Plus, Scan } from "lucide-react";
import { Button } from "@/components/Button";
import { FavIcon } from "@/components/FavIcon";
import { IconButton } from "@/components/IconButton";
import { boardFileUrl } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import type { Pin } from "@type/browser";
import { formatDimensions, hostOf, pinAddress, pinName, previewUrl, previewableAddress, viewerKind } from "../folder-tree";
import type { PinMenuActions } from "./PinContextMenu";

type PinViewerProps = {
  pins: Pin[];
  index: number;
  onIndex: (index: number) => void;
  onClose: () => void;
  actions: PinMenuActions;
};

const MIN_ZOOM = 0.05;
const MAX_ZOOM = 16;
const FILMSTRIP_RADIUS = 40;
const TEXT_LIMIT = 1024 * 1024;

function isTyping(target: EventTarget | null) {
  return target instanceof HTMLElement && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT" || target.isContentEditable);
}

/** Full view of one pin inside the main pane, with zoom, a filmstrip, and players for media. */
export function PinViewer({ pins, index, onIndex, onClose, actions }: PinViewerProps) {
  const pin = pins[index];
  const previous = index > 0 ? () => onIndex(index - 1) : undefined;
  const next = index < pins.length - 1 ? () => onIndex(index + 1) : undefined;
  const [zoomApi, setZoomApi] = useState<ZoomApi | null>(null);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (isTyping(event.target) || event.ctrlKey || event.metaKey || event.altKey) {
        return;
      }
      if (event.key === "Escape" || event.key === " ") {
        event.preventDefault();
        onClose();
      } else if (event.key === "ArrowLeft" && previous) {
        event.preventDefault();
        previous();
      } else if (event.key === "ArrowRight" && next) {
        event.preventDefault();
        next();
      } else if ((event.key === "+" || event.key === "=") && zoomApi) {
        zoomApi.step(1.25);
      } else if (event.key === "-" && zoomApi) {
        zoomApi.step(0.8);
      } else if (event.key === "0" && zoomApi) {
        zoomApi.fit();
      } else if (event.key === "1" && zoomApi) {
        zoomApi.actual();
      } else if (event.key.toLowerCase() === "p" && pin && previewableAddress(pin)) {
        event.preventDefault();
        actions.previewPage(pin);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, previous, next, zoomApi, pin, actions]);

  if (!pin) {
    return null;
  }
  const kind = viewerKind(pin);
  const address = pinAddress(pin);
  const isFile = pin.kind === "file" || pin.kind === "image";
  const openLink = pin.kind === "link" && address ? () => actions.openSource([pin]) : undefined;

  return (
    <div id="boards-viewer" className="absolute inset-0 z-30 flex flex-col bg-zinc-950">
      <header id="boards-viewer-toolbar" className="flex h-12 shrink-0 items-center gap-2 border-b border-zinc-800 px-3">
        <IconButton id="boards-viewer-close" label="Back to the grid (Esc)" onClick={onClose}>
          <ArrowLeft aria-hidden />
        </IconButton>
        <h2 id="boards-viewer-title" className="min-w-0 flex-1 truncate text-sm font-medium text-zinc-100" title={pinName(pin)}>
          {pinName(pin)}
          <span id="boards-viewer-dimensions" className="ml-2 text-xs font-normal text-zinc-500">
            {formatDimensions(pin)}
          </span>
        </h2>
        {kind === "image" && zoomApi ? <ZoomControls api={zoomApi} /> : null}
        {previewableAddress(pin) ? (
          <IconButton id="boards-viewer-preview" label="Preview page (P)" onClick={() => actions.previewPage(pin)}>
            <Eye aria-hidden />
          </IconButton>
        ) : null}
        {address ? (
          <IconButton id="boards-viewer-source" label="Open source page" onClick={() => actions.openSource([pin])}>
            <ExternalLink aria-hidden />
          </IconButton>
        ) : null}
        {isFile ? (
          <IconButton id="boards-viewer-open-file" label="Open with default app" onClick={() => actions.openFile(pin)}>
            <File aria-hidden />
          </IconButton>
        ) : null}
        <span id="boards-viewer-position" className="px-1 text-xs tabular-nums text-zinc-500">
          {index + 1} / {pins.length}
        </span>
        <IconButton id="boards-viewer-previous" label="Previous (←)" disabled={!previous} onClick={previous}>
          <ChevronLeft aria-hidden />
        </IconButton>
        <IconButton id="boards-viewer-next" label="Next (→)" disabled={!next} onClick={next}>
          <ChevronRight aria-hidden />
        </IconButton>
      </header>

      <div id="boards-viewer-stage" className="relative min-h-0 flex-1 overflow-hidden">
        {kind === "image" ? (
          <ImageStage key={pin.id} pin={pin} onApi={setZoomApi} onOpen={openLink} />
        ) : kind === "video" ? (
          <video id="boards-viewer-video" key={pin.id} src={boardFileUrl(pin)} controls autoPlay loop className="size-full bg-black object-contain" />
        ) : kind === "audio" ? (
          <div id="boards-viewer-audio" className="flex size-full flex-col items-center justify-center gap-4">
            <Music id="boards-viewer-audio-icon" className="size-16 text-zinc-600" aria-hidden />
            <audio id="boards-viewer-audio-player" key={pin.id} src={boardFileUrl(pin)} controls autoPlay className="w-[28rem] max-w-full" />
          </div>
        ) : kind === "pdf" ? (
          <iframe id="boards-viewer-pdf" key={pin.id} title={pinName(pin)} src={boardFileUrl(pin)} className="size-full border-0 bg-white" />
        ) : kind === "text" ? (
          <TextStage key={pin.id} pin={pin} />
        ) : kind === "link" ? (
          <LinkStage pin={pin} onOpen={() => actions.openSource([pin])} onPreview={() => actions.previewPage(pin)} />
        ) : (
          <FileStage pin={pin} onOpen={() => actions.openFile(pin)} onReveal={() => actions.reveal(pin)} />
        )}
        {previous ? <NavButton id="boards-viewer-stage-previous" side="left" label="Previous" onClick={previous} /> : null}
        {next ? <NavButton id="boards-viewer-stage-next" side="right" label="Next" onClick={next} /> : null}
      </div>

      <Filmstrip pins={pins} index={index} onIndex={onIndex} />
    </div>
  );
}

function NavButton({ id, side, label, onClick }: { id: string; side: "left" | "right"; label: string; onClick: () => void }) {
  return (
    <button
      id={id}
      type="button"
      aria-label={label}
      className={cn(
        "absolute top-1/2 inline-flex size-10 -translate-y-1/2 items-center justify-center rounded-full bg-black/50 text-white opacity-50 transition hover:bg-black/75 hover:opacity-100 focus-visible:opacity-100",
        side === "left" ? "left-3" : "right-3",
      )}
      onClick={onClick}
    >
      {side === "left" ? <ChevronLeft id={`${id}-icon`} className="size-5" aria-hidden /> : <ChevronRight id={`${id}-icon`} className="size-5" aria-hidden />}
    </button>
  );
}

type ZoomApi = {
  zoom: number;
  /** Zoom at which the whole image fits. */
  fitZoom: number;
  set: (zoom: number) => void;
  step: (factor: number) => void;
  fit: () => void;
  fill: () => void;
  actual: () => void;
};

function ZoomControls({ api }: { api: ZoomApi }) {
  const percent = Math.round(api.zoom * 100);
  // A log scale so the slider moves evenly from 5% to 1600%.
  const toSlider = (zoom: number) => Math.log(zoom / MIN_ZOOM) / Math.log(MAX_ZOOM / MIN_ZOOM);
  const fromSlider = (value: number) => MIN_ZOOM * Math.pow(MAX_ZOOM / MIN_ZOOM, value);
  return (
    <div id="boards-viewer-zoom" className="flex items-center gap-1">
      <IconButton id="boards-viewer-zoom-out" label="Zoom out (-)" className="size-7" onClick={() => api.step(0.8)}>
        <Minus aria-hidden />
      </IconButton>
      <input
        id="boards-viewer-zoom-slider"
        type="range"
        aria-label="Zoom"
        min={0}
        max={1}
        step={0.001}
        value={toSlider(api.zoom)}
        className="w-28 accent-sky-500"
        onChange={(event) => api.set(fromSlider(Number(event.target.value)))}
      />
      <IconButton id="boards-viewer-zoom-in" label="Zoom in (+)" className="size-7" onClick={() => api.step(1.25)}>
        <Plus aria-hidden />
      </IconButton>
      <span id="boards-viewer-zoom-value" className="w-11 text-right text-xs tabular-nums text-zinc-400">
        {percent}%
      </span>
      <Button id="boards-viewer-zoom-fit" size="xs" variant="ghost" title="Fit (0)" onClick={api.fit}>
        <Scan id="boards-viewer-zoom-fit-icon" className="size-3.5" aria-hidden />
        Fit
      </Button>
      <Button id="boards-viewer-zoom-actual" size="xs" variant="ghost" title="Actual size (1)" onClick={api.actual}>
        100%
      </Button>
      <Button id="boards-viewer-zoom-fill" size="xs" variant="ghost" title="Fill the window" onClick={api.fill}>
        <Maximize id="boards-viewer-zoom-fill-icon" className="size-3.5" aria-hidden />
        Fill
      </Button>
    </div>
  );
}

/** An image that zooms around the pointer with the wheel and pans by dragging. */
/** `onOpen` replaces double-click zoom: link pins open their page instead. */
function ImageStage({ pin, onApi, onOpen }: { pin: Pin; onApi: (api: ZoomApi | null) => void; onOpen?: () => void }) {
  const stageRef = useRef<HTMLDivElement>(null);
  const [natural, setNatural] = useState({ width: pin.width, height: pin.height });
  const [stage, setStage] = useState({ width: 0, height: 0 });
  const [view, setView] = useState<{ zoom: number; x: number; y: number } | null>(null);
  const [loaded, setLoaded] = useState(false);
  const drag = useRef<{ x: number; y: number; startX: number; startY: number } | null>(null);
  const [fallback, setFallback] = useState(false);
  const original = pin.hasImage || pin.ext === "svg" || pin.kind === "image";
  const src = original && !fallback ? boardFileUrl(pin) : previewUrl(pin);

  useLayoutEffect(() => {
    const element = stageRef.current;
    if (!element) {
      return;
    }
    const observer = new ResizeObserver(() => setStage({ width: element.clientWidth, height: element.clientHeight }));
    observer.observe(element);
    setStage({ width: element.clientWidth, height: element.clientHeight });
    return () => observer.disconnect();
  }, []);

  const fitZoom = natural.width > 0 && natural.height > 0 && stage.width > 0 ? Math.min(1, (stage.width - 32) / natural.width, (stage.height - 32) / natural.height) : 1;
  const fillZoom = natural.width > 0 && natural.height > 0 && stage.width > 0 ? Math.max(stage.width / natural.width, stage.height / natural.height) : 1;

  const centered = useCallback(
    (zoom: number) => ({ zoom, x: (stage.width - natural.width * zoom) / 2, y: (stage.height - natural.height * zoom) / 2 }),
    [stage, natural],
  );

  /** Zoom keeping the point under (`cx`, `cy`) still. */
  const zoomAt = useCallback(
    (zoom: number, cx = stage.width / 2, cy = stage.height / 2) => {
      setView((current) => {
        const base = current ?? centered(fitZoom);
        const next = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom));
        const ratio = next / base.zoom;
        const placed = { zoom: next, x: cx - (cx - base.x) * ratio, y: cy - (cy - base.y) * ratio };
        // Keep a small image centered rather than drifting.
        if (natural.width * next <= stage.width && natural.height * next <= stage.height) {
          return centered(next);
        }
        return placed;
      });
    },
    [stage, natural, centered, fitZoom],
  );

  useEffect(() => {
    if (loaded && !view && stage.width > 0) {
      setView(centered(fitZoom));
    }
  }, [loaded, view, stage, centered, fitZoom]);

  useEffect(() => {
    if (!view) {
      onApi(null);
      return;
    }
    onApi({
      zoom: view.zoom,
      fitZoom,
      set: (zoom) => zoomAt(zoom),
      step: (factor) => zoomAt(view.zoom * factor),
      fit: () => setView(centered(fitZoom)),
      fill: () => setView(centered(fillZoom)),
      actual: () => zoomAt(1),
    });
  }, [view, fitZoom, fillZoom, zoomAt, centered, onApi]);

  useEffect(() => () => onApi(null), [onApi]);

  const onWheel = (event: ReactWheelEvent) => {
    if (!view) {
      return;
    }
    const rect = event.currentTarget.getBoundingClientRect();
    const factor = Math.exp(-event.deltaY * 0.0015);
    zoomAt(view.zoom * factor, event.clientX - rect.left, event.clientY - rect.top);
  };

  const onPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || !view) {
      return;
    }
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = { x: view.x, y: view.y, startX: event.clientX, startY: event.clientY };
  };

  const onPointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const start = drag.current;
    if (!start) {
      return;
    }
    setView((current) => (current ? { ...current, x: start.x + event.clientX - start.startX, y: start.y + event.clientY - start.startY } : current));
  };

  const zoomable = view !== null;
  const panning = zoomable && (natural.width * view.zoom > stage.width || natural.height * view.zoom > stage.height);

  return (
    <div
      id="boards-viewer-image-stage"
      ref={stageRef}
      title={onOpen ? "Double-click to open the page in a new tab" : undefined}
      className={cn(
        "relative size-full touch-none select-none overflow-hidden",
        panning ? "cursor-grab active:cursor-grabbing" : onOpen ? "cursor-pointer" : "cursor-zoom-in",
      )}
      style={{ backgroundImage: "repeating-conic-gradient(#18181b 0% 25%, #111113 0% 50%)", backgroundSize: "24px 24px" }}
      onWheel={onWheel}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={() => (drag.current = null)}
      onPointerCancel={() => (drag.current = null)}
      onDoubleClick={(event) => {
        if (onOpen) {
          onOpen();
          return;
        }
        if (!view) {
          return;
        }
        const rect = event.currentTarget.getBoundingClientRect();
        if (Math.abs(view.zoom - fitZoom) < 0.01) {
          zoomAt(1, event.clientX - rect.left, event.clientY - rect.top);
        } else {
          setView(centered(fitZoom));
        }
      }}
    >
      <img
        id="boards-viewer-image"
        src={src}
        alt=""
        draggable={false}
        className={cn("absolute left-0 top-0 max-w-none origin-top-left", !loaded && "opacity-0")}
        style={
          view
            ? { width: natural.width, height: natural.height, transform: `translate(${view.x}px, ${view.y}px) scale(${view.zoom})`, imageRendering: view.zoom > 2 ? "pixelated" : undefined }
            : undefined
        }
        onLoad={(event) => {
          const image = event.currentTarget;
          if (image.naturalWidth > 0 && image.naturalHeight > 0) {
            setNatural({ width: image.naturalWidth, height: image.naturalHeight });
          }
          setLoaded(true);
        }}
        onError={() => {
          if (original && !fallback) {
            setFallback(true);
          }
        }}
      />
    </div>
  );
}

function TextStage({ pin }: { pin: Pin }) {
  const [text, setText] = useState<string | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    let gone = false;
    const controller = new AbortController();
    void fetch(boardFileUrl(pin), { signal: controller.signal, headers: { Range: `bytes=0-${TEXT_LIMIT - 1}` } })
      .then((response) => {
        if (!response.ok) {
          throw new Error(`The file couldn't be read (${response.status}).`);
        }
        return response.text();
      })
      .then((value) => {
        if (!gone) {
          setText(value);
        }
      })
      .catch((reason: unknown) => {
        if (!gone && !controller.signal.aborted) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      });
    return () => {
      gone = true;
      controller.abort();
    };
  }, [pin]);
  return (
    <div id="boards-viewer-text" className="size-full overflow-auto p-6">
      {error ? (
        <p id="boards-viewer-text-error" className="text-sm text-amber-300">
          {error}
        </p>
      ) : (
        <pre id="boards-viewer-text-value" className="mx-auto max-w-5xl whitespace-pre-wrap break-words font-mono text-xs leading-relaxed text-zinc-200">
          {text ?? "Loading…"}
          {text !== null && pin.size > TEXT_LIMIT ? "\n\n… The rest of the file is not shown." : ""}
        </pre>
      )}
    </div>
  );
}

function LinkStage({ pin, onOpen, onPreview }: { pin: Pin; onOpen: () => void; onPreview: () => void }) {
  const address = pinAddress(pin);
  const previewable = Boolean(previewableAddress(pin));
  return (
    <div id="boards-viewer-link" className="flex size-full items-center justify-center p-8">
      <div
        id="boards-viewer-link-card"
        title={address ? "Double-click to open the page in a new tab" : undefined}
        className="flex max-w-xl flex-col gap-4 rounded-xl bg-zinc-900 p-6 ring-1 ring-zinc-800"
        onDoubleClick={(event) => {
          if (address && !(event.target instanceof Element && event.target.closest("button"))) {
            onOpen();
          }
        }}
      >
        <div id="boards-viewer-link-heading" className="flex items-center gap-3">
          <FavIcon id="boards-viewer-link-icon" url={address} className="size-8" />
          <div id="boards-viewer-link-text" className="min-w-0">
            <p id="boards-viewer-link-title" className="text-base font-medium text-zinc-100">
              {pin.title || address}
            </p>
            <p id="boards-viewer-link-host" className="truncate text-xs text-zinc-500" title={address}>
              {hostOf(address) || address}
            </p>
          </div>
        </div>
        {pin.text ? (
          <p id="boards-viewer-link-excerpt" className="max-h-80 overflow-y-auto whitespace-pre-line text-sm leading-relaxed text-zinc-300">
            {pin.text}
          </p>
        ) : null}
        {address ? (
          <div id="boards-viewer-link-actions" className="flex gap-2">
            {previewable ? (
              <Button id="boards-viewer-link-preview" className="flex-1" onClick={onPreview}>
                <Eye id="boards-viewer-link-preview-icon" className="size-4" aria-hidden />
                Preview page
              </Button>
            ) : null}
            <Button id="boards-viewer-link-open" variant="primary" className="flex-1" onClick={onOpen}>
              <ExternalLink id="boards-viewer-link-open-icon" className="size-4" aria-hidden />
              Open page
            </Button>
          </div>
        ) : null}
      </div>
    </div>
  );
}

function FileStage({ pin, onOpen, onReveal }: { pin: Pin; onOpen: () => void; onReveal: () => void }) {
  const src = previewUrl(pin);
  return (
    <div id="boards-viewer-file" className="flex size-full flex-col items-center justify-center gap-4 p-8">
      {src ? (
        <img id="boards-viewer-file-thumb" src={src} alt="" className="max-h-[60%] max-w-full rounded-md object-contain shadow-xl" />
      ) : (
        <FileText id="boards-viewer-file-icon" className="size-16 text-zinc-600" aria-hidden />
      )}
      <p id="boards-viewer-file-name" className="max-w-xl truncate text-sm text-zinc-300">
        {pin.fileName || pinName(pin)}
      </p>
      <div id="boards-viewer-file-actions" className="flex gap-2">
        <Button id="boards-viewer-file-open" variant="primary" onClick={onOpen}>
          <File id="boards-viewer-file-open-icon" className="size-4" aria-hidden />
          Open with default app
        </Button>
        <Button id="boards-viewer-file-reveal" onClick={onReveal}>
          Show in File Explorer
        </Button>
      </div>
    </div>
  );
}

function Filmstrip({ pins, index, onIndex }: { pins: Pin[]; index: number; onIndex: (index: number) => void }) {
  const stripRef = useRef<HTMLDivElement>(null);
  const first = Math.max(0, index - FILMSTRIP_RADIUS);
  const shown = pins.slice(first, index + FILMSTRIP_RADIUS + 1);

  useLayoutEffect(() => {
    const current = stripRef.current?.querySelector<HTMLElement>('[aria-current="true"]');
    current?.scrollIntoView({ block: "nearest", inline: "center" });
  }, [index]);

  if (pins.length <= 1) {
    return null;
  }
  return (
    <div id="boards-viewer-filmstrip" ref={stripRef} className="flex h-20 shrink-0 items-center gap-1.5 overflow-x-auto border-t border-zinc-800 px-3">
      {shown.map((pin, offset) => {
        const at = first + offset;
        const src = previewUrl(pin);
        return (
          <button
            id={`boards-viewer-filmstrip-${pin.id}`}
            key={pin.id}
            type="button"
            aria-current={at === index}
            aria-label={pinName(pin)}
            title={pinName(pin)}
            className={cn("size-14 shrink-0 overflow-hidden rounded bg-zinc-900 ring-1 ring-zinc-800 hover:ring-zinc-500", at === index && "ring-2 ring-sky-500")}
            onClick={() => onIndex(at)}
          >
            {src ? (
              <img id={`boards-viewer-filmstrip-${pin.id}-image`} src={src} alt="" loading="lazy" draggable={false} className="size-full object-cover" />
            ) : (
              <span id={`boards-viewer-filmstrip-${pin.id}-ext`} className="text-[10px] uppercase text-zinc-500">
                {pin.ext || pin.kind}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
