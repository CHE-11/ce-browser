import type { MouseEvent } from "react";
import { ChevronDown, ChevronUp, SquareTerminal, X } from "lucide-react";
import { startWindowDrag } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";

export type HudPanel = "ipc" | "webviews" | "state" | "logs";

export const HUD_PANELS: { id: HudPanel; label: string }[] = [
  { id: "ipc", label: "IPC" },
  { id: "webviews", label: "Webviews" },
  { id: "state", label: "State" },
  { id: "logs", label: "Logs" },
];

type HudHeaderProps = {
  panel: HudPanel;
  onPanel: (panel: HudPanel) => void;
  collapsed: boolean;
  onCollapse: (collapsed: boolean) => void;
  onClose: () => void;
  invokesPerSecond: number;
  errors: number;
  webviews: number | null;
};

/** The header is the HUD's title bar: dragging any empty part of it moves the window. */
export function HudHeader({ panel, onPanel, collapsed, onCollapse, onClose, invokesPerSecond, errors, webviews }: HudHeaderProps) {
  const onMouseDown = (event: MouseEvent<HTMLElement>) => {
    if (event.button !== 0 || (event.target as HTMLElement).closest("button, input, select")) {
      return;
    }
    void startWindowDrag();
  };

  return (
    <header
      id="debug-hud-header"
      className="flex h-10 shrink-0 select-none items-center gap-1 border-b border-zinc-800 px-2"
      onMouseDown={onMouseDown}
      onDoubleClick={(event) => {
        if (!(event.target as HTMLElement).closest("button")) {
          onCollapse(!collapsed);
        }
      }}
    >
      <SquareTerminal id="debug-hud-icon" aria-hidden className="size-4 shrink-0 text-sky-400" />
      <span id="debug-hud-title" className="mr-1 text-xs font-semibold text-zinc-200">
        Debug
      </span>
      <nav id="debug-hud-panels" className="flex min-w-0 items-center gap-0.5">
        {HUD_PANELS.map((item) => (
          <button
            id={`debug-hud-panel-${item.id}`}
            key={item.id}
            type="button"
            aria-pressed={!collapsed && panel === item.id}
            className={cn(
              "h-6 shrink-0 rounded px-2 text-[11px]",
              !collapsed && panel === item.id ? "bg-zinc-800 text-zinc-100" : "text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-100",
              focusRing,
            )}
            onClick={() => {
              onPanel(item.id);
              onCollapse(false);
            }}
          >
            {item.label}
          </button>
        ))}
      </nav>
      <div id="debug-hud-stats" className="ml-auto hidden items-center gap-2 text-[10px] tabular-nums text-zinc-500 min-[500px]:flex">
        <span id="debug-hud-stat-rate" title="Invokes per second over the last 5 seconds">
          {invokesPerSecond.toFixed(1)}/s
        </span>
        <span id="debug-hud-stat-errors" className={cn(errors > 0 && "text-red-300")} title="Failed invokes in the feed">
          {errors} err
        </span>
        <span id="debug-hud-stat-webviews" title="Native webviews">
          {webviews ?? "–"} views
        </span>
      </div>
      <button
        id="debug-hud-collapse"
        type="button"
        title={collapsed ? "Expand" : "Collapse"}
        className={cn("ml-1 inline-flex size-6 shrink-0 items-center justify-center rounded text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 min-[500px]:ml-0", focusRing)}
        onClick={() => onCollapse(!collapsed)}
      >
        {collapsed ? <ChevronDown id="debug-hud-expand-icon" aria-hidden className="size-3.5" /> : <ChevronUp id="debug-hud-collapse-icon" aria-hidden className="size-3.5" />}
      </button>
      <button
        id="debug-hud-close"
        type="button"
        title="Hide (Ctrl+Shift+F12)"
        className={cn("inline-flex size-6 shrink-0 items-center justify-center rounded text-zinc-400 hover:bg-red-500/20 hover:text-red-200", focusRing)}
        onClick={onClose}
      >
        <X id="debug-hud-close-icon" aria-hidden className="size-3.5" />
      </button>
    </header>
  );
}
