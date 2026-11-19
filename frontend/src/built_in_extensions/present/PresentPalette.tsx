import { useEffect, useState } from "react";
import { ArrowUpRight, Circle, CircleDot, Crosshair, Eraser, Focus, Lasso, MousePointer2, Pencil, Pointer, Square, Trash2, Type, Undo2, X, ZoomIn } from "lucide-react";
import { clearPresentMarks, onPresentChanged, presentState, setPresentCircle, setPresentColor, setPresentCursor, setPresentDim, setPresentRadius, setPresentSpread, setPresentTool, setPresentWidth, togglePresent, undoPresentMark } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import type { PresentCursor, PresentState, PresentTool } from "@type/browser";
import { PRESENT_COLORS, PRESENT_CURSORS, PRESENT_TOOLS, PRESENT_WIDTHS } from "./present-model";

const ICONS: Record<PresentTool, typeof Focus> = {
  none: Pointer,
  lasso: Lasso,
  spotlight: Focus,
  cursor: Pointer,
  zoom: ZoomIn,
  highlight: Square,
  pen: Pencil,
  arrow: ArrowUpRight,
  text: Type,
  eraser: Eraser,
};

const CURSOR_ICONS: Record<PresentCursor, typeof Focus> = {
  dot: CircleDot,
  ring: Circle,
  arrow: MousePointer2,
  crosshair: Crosshair,
  off: X,
};

const EMPTY: PresentState = {
  on: true,
  tool: "spotlight",
  cursor: "dot",
  color: "#f59e0b",
  width: 1,
  dim: 0.62,
  radius: 12,
  spread: 240,
  circle: false,
  placing: false,
  clear: 0,
  undo: 0,
  spotlight: null,
  zoom: null,
};

const WIDTH_LABEL = ["Thin stroke", "Medium stroke", "Thick stroke"];
const WIDTH_MARK = [4, 7, 11];

export function PresentPalette() {
  const [state, setState] = useState<PresentState>(EMPTY);

  useEffect(() => {
    document.title = "Present controls";
    document.documentElement.dataset.presentClear = "true";
    let stop = false;
    const apply = (next: PresentState) => {
      if (!stop) setState(next);
    };
    void presentState().then(apply).catch(() => undefined);
    const listen = onPresentChanged(apply);
    return () => {
      stop = true;
      delete document.documentElement.dataset.presentClear;
      void listen.then((unlisten) => unlisten());
    };
  }, []);

  return (
    <div id="present-controls" className="flex h-full items-center justify-center bg-transparent">
      <div id="present-controls-bar" className="flex flex-col items-center gap-1">
        {state.tool === "spotlight" || state.tool === "cursor" ? (
        <div id="present-controls-cursors" className="flex items-center gap-0.5 rounded-full bg-zinc-950/95 px-1.5 py-1 text-zinc-100 shadow-lg ring-1 ring-white/10">
          {PRESENT_CURSORS.map((cursor) => {
            const Icon = CURSOR_ICONS[cursor.id];
            const selected = state.cursor === cursor.id;
            return (
              <button
                id={`present-controls-cursor-${cursor.id}`}
                key={cursor.id}
                type="button"
                aria-label={cursor.label}
                aria-pressed={selected}
                title={cursor.label}
                className={cn("inline-flex size-6 items-center justify-center rounded-full hover:bg-zinc-800", selected && "bg-zinc-700")}
                onClick={() => void setPresentCursor(cursor.id).then(setState)}
              >
                <Icon id={`present-controls-cursor-${cursor.id}-icon`} className="size-3" aria-hidden />
              </button>
            );
          })}
          {state.tool === "cursor" ? (
            <>
              <button
                id="present-controls-circle"
                type="button"
                aria-label="Circle around cursor"
                aria-pressed={state.circle}
                title="Circle around cursor"
                className={cn("ml-0.5 inline-flex size-6 items-center justify-center rounded-full hover:bg-zinc-800", state.circle && "bg-zinc-700")}
                onClick={() => void setPresentCircle(!state.circle).then(setState)}
              >
                <Circle id="present-controls-circle-icon" className="size-3" aria-hidden />
              </button>
              <label id="present-controls-spread" className="flex items-center" title="Circle size">
                <input
                  id="present-controls-spread-input"
                  type="range"
                  min={80}
                  max={480}
                  step={8}
                  value={Math.round(state.spread || 240)}
                  aria-label="Circle size"
                  className="h-1 w-16 cursor-pointer accent-zinc-100"
                  onChange={(event) => void setPresentSpread(Number(event.target.value)).then(setState)}
                />
              </label>
            </>
          ) : null}
          {state.tool === "spotlight" ? (
            <>
              <label id="present-controls-spread" className="ml-1 flex items-center" title="Circle size">
                <input
                  id="present-controls-spread-input"
                  type="range"
                  min={80}
                  max={480}
                  step={8}
                  value={Math.round(state.spread || 240)}
                  aria-label="Circle size"
                  className="h-1 w-16 cursor-pointer accent-zinc-100"
                  onChange={(event) => void setPresentSpread(Number(event.target.value)).then(setState)}
                />
              </label>
              <label id="present-controls-dim" className="flex items-center" title="Background dim">
                <input
                  id="present-controls-dim-input"
                  type="range"
                  min={0}
                  max={92}
                  step={1}
                  value={Math.round((state.dim || 0) * 100)}
                  aria-label="Background dim"
                  className="h-1 w-16 cursor-pointer accent-zinc-100"
                  onChange={(event) => void setPresentDim(Number(event.target.value) / 100).then(setState)}
                />
              </label>
            </>
          ) : null}
        </div>
        ) : null}
        {state.tool === "highlight" ? (
          <div id="present-controls-radius" className="flex items-center rounded-full bg-zinc-950/95 px-2 py-1 text-zinc-100 shadow-lg ring-1 ring-white/10" title="Corner radius">
            <input
              id="present-controls-radius-input"
              type="range"
              min={0}
              max={40}
              step={1}
              value={Math.round(state.radius || 0)}
              aria-label="Corner radius"
              className="h-1 w-16 cursor-pointer accent-zinc-100"
              onChange={(event) => void setPresentRadius(Number(event.target.value)).then(setState)}
            />
          </div>
        ) : null}
        <div id="present-controls-tools" className="flex items-center gap-0.5 rounded-full bg-zinc-950/95 px-1.5 py-1 text-zinc-100 shadow-lg ring-1 ring-white/10">
          {PRESENT_TOOLS.map((tool) => {
            const Icon = ICONS[tool.id];
            const selected = state.tool === tool.id;
            const locked = ((tool.id === "spotlight" || tool.id === "cursor") && state.spotlight) || (tool.id === "zoom" && state.zoom);
            return (
              <button
                id={`present-controls-tool-${tool.id}`}
                key={tool.id}
                type="button"
                aria-label={tool.label}
                aria-pressed={selected}
                title={tool.label}
                className={cn(
                  "inline-flex size-7 items-center justify-center rounded-full hover:bg-zinc-800",
                  selected && "bg-zinc-700",
                  state.placing && selected && "ring-1 ring-sky-300",
                  locked && selected && "text-amber-300",
                )}
                onClick={() => void setPresentTool(tool.id).then(setState)}
              >
                <Icon id={`present-controls-tool-${tool.id}-icon`} className="size-3.5" aria-hidden />
              </button>
            );
          })}
          <span id="present-controls-color-divider" className="mx-0.5 h-4 w-px bg-white/15" />
          {PRESENT_COLORS.map((color) => (
            <button
              id={`present-controls-color-${color.slice(1)}`}
              key={color}
              type="button"
              aria-label={color}
              aria-pressed={state.color === color}
              title={color}
              className={cn("size-3.5 rounded-full ring-1 ring-white/25 ring-offset-1 ring-offset-zinc-950", state.color === color && "ring-2 ring-sky-300")}
              style={{ background: color }}
              onClick={() => void setPresentColor(color).then(setState)}
            />
          ))}
          <span id="present-controls-width-divider" className="mx-0.5 h-4 w-px bg-white/15" />
          {PRESENT_WIDTHS.map((width, index) => {
            const selected = Math.abs(state.width - width) < 0.05;
            const slug = String(width).replace(".", "-");
            return (
              <button
                id={`present-controls-width-${slug}`}
                key={width}
                type="button"
                aria-label={WIDTH_LABEL[index]}
                aria-pressed={selected}
                title={WIDTH_LABEL[index]}
                className={cn("inline-flex size-6 items-center justify-center rounded-full hover:bg-zinc-800", selected && "bg-zinc-700")}
                onClick={() => void setPresentWidth(width).then(setState)}
              >
                <span id={`present-controls-width-${slug}-mark`} className="rounded-full bg-zinc-100" style={{ width: WIDTH_MARK[index], height: WIDTH_MARK[index] }} />
              </button>
            );
          })}
          <button
            id="present-controls-undo"
            type="button"
            aria-label="Undo"
            title="Undo"
            className="inline-flex size-7 items-center justify-center rounded-full hover:bg-zinc-800"
            onClick={() => void undoPresentMark().then(setState)}
          >
            <Undo2 id="present-controls-undo-icon" className="size-3.5" aria-hidden />
          </button>
          <span id="present-controls-clear-divider" className="mx-0.5 h-4 w-px bg-white/15" />
          <button
            id="present-controls-clear"
            type="button"
            aria-label="Clear marks"
            title="Clear marks"
            className="inline-flex size-7 items-center justify-center rounded-full hover:bg-zinc-800"
            onClick={() => void clearPresentMarks().then(setState)}
          >
            <Trash2 id="present-controls-clear-icon" className="size-3.5" aria-hidden />
          </button>
          <button
            id="present-controls-close"
            type="button"
            aria-label="Stop presenting"
            title="Stop presenting"
            className="inline-flex size-7 items-center justify-center rounded-full hover:bg-zinc-800"
            onClick={() => void togglePresent()}
          >
            <X id="present-controls-close-icon" className="size-3.5" aria-hidden />
          </button>
        </div>
      </div>
    </div>
  );
}
