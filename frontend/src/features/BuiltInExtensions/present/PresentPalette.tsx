import { useEffect, useState } from "react";
import { ArrowUpRight, Circle, CircleDot, Crosshair, Eraser, Focus, MousePointer2, Pencil, Square, Trash2, Type, X, ZoomIn } from "lucide-react";
import { clearPresentMarks, onPresentChanged, presentState, setPresentColor, setPresentCursor, setPresentTool, togglePresent } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import type { PresentCursor, PresentState, PresentTool } from "@type/browser";
import { PRESENT_COLORS, PRESENT_CURSORS, PRESENT_TOOLS } from "./present-model";

const ICONS: Record<PresentTool, typeof Focus> = {
  spotlight: Focus,
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
  placing: false,
  clear: 0,
  spotlight: null,
  zoom: null,
};

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
        </div>
        <div id="present-controls-tools" className="flex items-center gap-0.5 rounded-full bg-zinc-950/95 px-1.5 py-1 text-zinc-100 shadow-lg ring-1 ring-white/10">
          {PRESENT_TOOLS.map((tool) => {
            const Icon = ICONS[tool.id];
            const selected = state.tool === tool.id;
            const locked = (tool.id === "spotlight" && state.spotlight) || (tool.id === "zoom" && state.zoom);
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
