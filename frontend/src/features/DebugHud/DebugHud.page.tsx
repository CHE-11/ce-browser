import { useEffect, useMemo } from "react";
import { debugToggleHud, setWindowLogicalSize, startWindowResize, windowLogicalSize } from "@/lib/browser-client";
import { useDebugFeed } from "@/lib/use-debug-feed";
import { useDebugWebviews } from "@/lib/use-debug-webviews";
import { useNow } from "@/lib/use-now";
import { isBoolean, useStoredState } from "@/lib/use-stored-state";
import { HUD_PANELS, HudHeader, type HudPanel } from "./HudHeader";
import { IpcPanel } from "./IpcPanel";
import { LogsPanel } from "./LogsPanel";
import { STATE_VIEWS, StatePanel } from "./StatePanel";
import { WebviewsPanel } from "./WebviewsPanel";

/** Logical px. Matches `MIN_HEIGHT` in `src-tauri/src/debug/hud.rs`. */
const HEADER_HEIGHT = 40;
const DEFAULT_HEIGHT = 620;
/** The invoke rate in the header is averaged over this window. */
const RATE_WINDOW_MS = 5000;

function isPanel(value: unknown): value is HudPanel {
  return HUD_PANELS.some((item) => item.id === value);
}

function isStateView(value: unknown): value is string {
  return typeof value === "string" && STATE_VIEWS.includes(value);
}

function isHeight(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value > HEADER_HEIGHT;
}

/** The dev-only debug HUD. It runs in its own window, `debug-<browser window>`. */
export function DebugHudPage() {
  const [panel, setPanel] = useStoredState<HudPanel>("debug-hud-panel", "ipc", isPanel);
  const [stateView, setStateView] = useStoredState("debug-hud-state-view", "tabs", isStateView);
  const [collapsed, setCollapsed] = useStoredState("debug-hud-collapsed", false, isBoolean);
  const [height, setHeight] = useStoredState("debug-hud-height", DEFAULT_HEIGHT, isHeight);
  const feed = useDebugFeed();
  const { webviews, error: webviewsError, refresh: refreshWebviews } = useDebugWebviews(panel === "webviews" && !collapsed ? 1000 : 3000);
  const now = useNow(1000);

  useEffect(() => {
    document.title = "Debug HUD";
    document.documentElement.dataset.popup = "true";
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.ctrlKey && event.shiftKey && event.key === "F12") {
        event.preventDefault();
        void debugToggleHud();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Collapsing shrinks the window to its header and remembers the height to come back to.
  useEffect(() => {
    void windowLogicalSize()
      .then((size) => {
        if (collapsed) {
          if (size.height > HEADER_HEIGHT + 1) {
            setHeight(Math.round(size.height));
            return setWindowLogicalSize(size.width, HEADER_HEIGHT);
          }
          return;
        }
        if (size.height <= HEADER_HEIGHT + 1) {
          return setWindowLogicalSize(size.width, height);
        }
      })
      .catch(() => undefined);
    // `height` is read only when expanding; saving it must not resize the window again.
  }, [collapsed]);

  const stats = useMemo(() => {
    let recent = 0;
    let errors = 0;
    for (const record of feed.ipc) {
      if (record.kind !== "invoke") continue;
      if (!record.ok) errors += 1;
      if (record.startedAt >= now - RATE_WINDOW_MS) recent += 1;
    }
    return { perSecond: recent / (RATE_WINDOW_MS / 1000), errors };
  }, [feed.ipc, now]);

  return (
    <main id="debug-hud" className="relative flex h-screen flex-col overflow-hidden bg-zinc-900 text-zinc-100">
      <HudHeader
        panel={panel}
        onPanel={setPanel}
        collapsed={collapsed}
        onCollapse={setCollapsed}
        onClose={() => void debugToggleHud()}
        invokesPerSecond={stats.perSecond}
        errors={stats.errors}
        webviews={webviews?.length ?? null}
      />
      {collapsed ? null : (
        <>
          {feed.error ? (
            <p id="debug-hud-error" className="border-b border-zinc-800 px-3 py-1 text-[11px] text-red-300">
              {feed.error}
            </p>
          ) : null}
          {panel === "ipc" ? <IpcPanel records={feed.ipc} onClear={() => feed.clear("ipc")} /> : null}
          {panel === "webviews" ? <WebviewsPanel webviews={webviews} error={webviewsError} onRefresh={refreshWebviews} /> : null}
          {panel === "state" ? <StatePanel view={stateView} onView={setStateView} /> : null}
          {panel === "logs" ? <LogsPanel lines={feed.logs} onClear={() => feed.clear("logs")} /> : null}
          <div
            id="debug-hud-resize"
            title="Resize"
            className="absolute bottom-0 right-0 size-3 cursor-se-resize"
            onMouseDown={(event) => {
              if (event.button === 0) {
                event.preventDefault();
                void startWindowResize();
              }
            }}
          />
        </>
      )}
    </main>
  );
}
