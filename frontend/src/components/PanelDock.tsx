import { useRef, type KeyboardEvent, type PointerEvent } from "react";
import { ChevronsLeft, ChevronsRight, GripVertical, X } from "lucide-react";
import type { SidePanelInfo } from "@type/browser";
import { PanelSide } from "@/components/PanelSide";
import { PANEL_RAIL } from "@/features/DevTools/page-column";
import { closeSidePanel, collapseSidePanel, nudgeSidePanel, reorderSidePanel, resizeSidePanel, setSidePanelEdge } from "@/lib/browser-client";
import { cn } from "@/lib/cn";

function domId(id: string) {
  return id.replace(/[^a-zA-Z0-9_-]/g, "-");
}

type PanelDockProps = {
  panels: SidePanelInfo[];
};

/** Chrome for every docked panel. Content webviews sit under the header and beside the resize seam. */
export function PanelDock({ panels }: PanelDockProps) {
  const left = panels.filter((panel) => panel.edge === "left");
  const right = panels.filter((panel) => panel.edge !== "left");
  if (left.length === 0 && right.length === 0) {
    return null;
  }
  return (
    <>
      {left.length > 0 ? (
        <div id="panel-dock-left" className="pointer-events-none absolute inset-y-0 left-0 z-20 flex">
          {[...left].reverse().map((panel) => (
            <PanelColumn key={panel.id} panel={panel} />
          ))}
        </div>
      ) : null}
      {right.length > 0 ? (
        <div id="panel-dock-right" className="pointer-events-none absolute inset-y-0 right-0 z-20 flex justify-end">
          {right.map((panel) => (
            <PanelColumn key={panel.id} panel={panel} />
          ))}
        </div>
      ) : null}
    </>
  );
}

function PanelColumn({ panel }: { panel: SidePanelInfo }) {
  const id = domId(panel.id);
  const busy = useRef(false);
  const width = panel.collapsed ? PANEL_RAIL : panel.width;
  const run = (action: () => Promise<unknown>) => {
    if (busy.current) {
      return;
    }
    busy.current = true;
    void action().finally(() => {
      busy.current = false;
    });
  };
  const onGrip = (event: PointerEvent<HTMLButtonElement>) => {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    run(() => reorderSidePanel(panel.id));
  };
  const onResize = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    run(() => resizeSidePanel(panel.id));
  };
  const onResizeKey = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = event.shiftKey ? 64 : 16;
    const grow = panel.edge === "left" ? "ArrowRight" : "ArrowLeft";
    const shrink = panel.edge === "left" ? "ArrowLeft" : "ArrowRight";
    if (event.key === grow) {
      event.preventDefault();
      void nudgeSidePanel(panel.id, step);
    } else if (event.key === shrink) {
      event.preventDefault();
      void nudgeSidePanel(panel.id, -step);
    }
  };
  return (
    <section
      id={`panel-dock-${id}`}
      style={{ width }}
      className={cn(
        "pointer-events-auto relative flex h-full shrink-0 flex-col bg-zinc-950 text-zinc-100",
        panel.edge === "left" ? "border-r border-zinc-800" : "border-l border-zinc-800",
      )}
    >
      {panel.collapsed ? (
        <div id={`panel-dock-${id}-rail`} className="flex h-full w-full flex-col items-center gap-2 py-2">
          <div id={`panel-dock-${id}-grip-expand-wrap`} className="flex flex-col items-center gap-1">
            <GripButton id={`panel-dock-${id}-grip`} label={`Reorder ${panel.title}`} onPointerDown={onGrip} />
            <button
              id={`panel-dock-${id}-expand`}
              type="button"
              className="flex size-6 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
              aria-label={`Collapse ${panel.title}`}
              onClick={() => void collapseSidePanel(panel.id, false)}
            >
              {panel.edge === "right" ? (
                <ChevronsLeft id={`panel-dock-${id}-expand-icon`} className="size-3.5" aria-hidden />
              ) : (
                <ChevronsRight id={`panel-dock-${id}-expand-icon`} className="size-3.5" aria-hidden />
              )}
            </button>
            <button
              id={`panel-dock-${id}-close`}
              type="button"
              className="flex size-6 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
              aria-label={`Close ${panel.title}`}
              onClick={() => void closeSidePanel(panel.id)}
            >
              <X id={`panel-dock-${id}-close-icon`} className="size-3.5" aria-hidden />
            </button>
          </div>

          <button
            id={`panel-dock-${id}-expand`}
            type="button"
            className="flex min-h-0 w-full flex-1 items-center justify-center text-xs font-medium text-zinc-200 [writing-mode:vertical-rl] hover:text-zinc-50"
            aria-label={`Expand ${panel.title}`}
            onClick={() => void collapseSidePanel(panel.id, false)}
          >
            <span id={`panel-dock-${id}-name`}>{panel.title}</span>
          </button>
        </div>
      ) : (
        <header id={`panel-dock-${id}-header`} className="flex h-[32px] shrink-0 items-center gap-1 px-1">
          <GripButton id={`panel-dock-${id}-grip`} label={`Reorder ${panel.title}`} onPointerDown={onGrip} />
          <span id={`panel-dock-${id}-title`} className="min-w-0 flex-1 truncate text-sm font-medium">
            {panel.title}
          </span>
          {panel.extensionId ? (
            <div id={`panel-dock-${id}-side-wrap`} className="shrink-0">
              <PanelSide
                id={`panel-dock-${id}-side`}
                side={panel.edge}
                onChange={(side) => run(() => setSidePanelEdge(panel.id, side))}
              />
            </div>
          ) : null}
          <button
            id={`panel-dock-${id}-collapse`}
            type="button"
            className="flex size-6 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
            aria-label={`Collapse ${panel.title}`}
            onClick={() => void collapseSidePanel(panel.id, true)}
          >
            {panel.edge === "left" ? (
              <ChevronsLeft id={`panel-dock-${id}-collapse-icon`} className="size-3.5" aria-hidden />
            ) : (
              <ChevronsRight id={`panel-dock-${id}-collapse-icon`} className="size-3.5" aria-hidden />
            )}
          </button>
          <button
            id={`panel-dock-${id}-close`}
            type="button"
            className="flex size-6 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
            aria-label={`Close ${panel.title}`}
            onClick={() => void closeSidePanel(panel.id)}
          >
            <X id={`panel-dock-${id}-close-icon`} className="size-3.5" aria-hidden />
          </button>
        </header>
      )}
      {!panel.collapsed && panel.resizable ? (
        <div
          id={`panel-dock-${id}-resize`}
          role="separator"
          aria-orientation="vertical"
          aria-label={`Resize ${panel.title}`}
          aria-valuenow={Math.round(panel.width)}
          tabIndex={0}
          className={cn(
            "absolute bottom-0 top-[32px] z-10 w-[6px] cursor-col-resize touch-none hover:bg-sky-500/40 focus-visible:bg-sky-500/40 focus-visible:outline-none",
            panel.edge === "left" ? "right-0" : "left-0",
          )}
          onPointerDown={onResize}
          onKeyDown={onResizeKey}
        />
      ) : null}
    </section>
  );
}

function GripButton({
  id,
  label,
  onPointerDown,
}: {
  id: string;
  label: string;
  onPointerDown: (event: PointerEvent<HTMLButtonElement>) => void;
}) {
  return (
    <button
      id={id}
      type="button"
      aria-label={label}
      title={label}
      className="flex size-6 shrink-0 cursor-grab items-center justify-center rounded text-zinc-500 hover:bg-zinc-800 hover:text-zinc-100 active:cursor-grabbing"
      onPointerDown={onPointerDown}
      onClick={(event) => event.stopPropagation()}
    >
      <GripVertical id={`${id}-icon`} className="size-3.5" aria-hidden />
    </button>
  );
}
