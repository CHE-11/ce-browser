import { useState } from "react";
import { Monitor, MonitorPlay } from "lucide-react";
import type { AgentRun } from "@type/browser";
import { cn } from "@/lib/cn";

/** The newest picture of the page, or the desktop, with the last pointer marked. */
export function AgentLiveView({ run, onShowTab, screen }: { run: AgentRun; onShowTab: () => void; screen: boolean }) {
  const [mode, setMode] = useState<"page" | "screen">("page");
  const step = [...run.steps].reverse().find((item) => item.px != null && item.py != null);
  const thumb = mode === "screen" && run.screen ? run.screen : [...run.steps].reverse().find((item) => item.thumb)?.thumb;
  const showingScreen = mode === "screen" && !!run.screen;
  if (!thumb && !screen) {
    return null;
  }

  return (
    <div id="agent-live-wrap" className="flex flex-col gap-1.5">
      {screen ? (
        <div id="agent-live-switch" className="flex gap-1" role="tablist" aria-label="What the live view shows">
          <button
            id="agent-live-page"
            type="button"
            role="tab"
            aria-selected={mode === "page"}
            className={cn("rounded px-2 py-0.5 text-[10px] ring-1 ring-zinc-800", mode === "page" ? "bg-zinc-800 text-zinc-100" : "text-zinc-500")}
            onClick={() => setMode("page")}
          >
            Page
          </button>
          <button
            id="agent-live-screen"
            type="button"
            role="tab"
            aria-selected={mode === "screen"}
            className={cn("rounded px-2 py-0.5 text-[10px] ring-1 ring-zinc-800", mode === "screen" ? "bg-zinc-800 text-zinc-100" : "text-zinc-500")}
            onClick={() => setMode("screen")}
          >
            Screen
          </button>
        </div>
      ) : null}
      {thumb ? (
        <button
          id="agent-live"
          type="button"
          title={showingScreen ? "The desktop" : "Show this tab"}
          className="group relative block w-full overflow-hidden rounded-lg ring-1 ring-zinc-800 hover:ring-sky-700"
          onClick={showingScreen ? undefined : onShowTab}
        >
          <img id="agent-live-image" src={thumb} alt={showingScreen ? "The desktop" : "What the agent sees now"} className="block w-full" />
          {!showingScreen && step?.px != null && step.py != null ? (
            <>
              {step.px2 != null && step.py2 != null ? (
                <svg id="agent-live-drag" className="pointer-events-none absolute inset-0 h-full w-full" aria-hidden>
                  <line id="agent-live-drag-line" x1={`${step.px * 100}%`} y1={`${step.py * 100}%`} x2={`${step.px2 * 100}%`} y2={`${step.py2 * 100}%`} stroke="#38bdf8" strokeWidth="2" />
                </svg>
              ) : null}
              <span
                id="agent-live-marker"
                className="pointer-events-none absolute size-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white bg-sky-500 shadow"
                style={{ left: `${(step.px2 ?? step.px) * 100}%`, top: `${(step.py2 ?? step.py) * 100}%` }}
              />
            </>
          ) : null}
          <span id="agent-live-badge" className="absolute left-2 top-2 inline-flex items-center gap-1 rounded bg-zinc-950/80 px-1.5 py-0.5 text-[10px] font-medium text-sky-300 ring-1 ring-zinc-800">
            {showingScreen ? <Monitor id="agent-live-badge-icon" className="size-3" aria-hidden /> : <MonitorPlay id="agent-live-badge-icon" className="size-3" aria-hidden />}
            {showingScreen ? "Screen" : "Live"}
          </span>
        </button>
      ) : (
        <p id="agent-live-empty" className="text-xs text-zinc-500">
          {mode === "screen" ? "No desktop screenshot yet." : "No picture of the page yet."}
        </p>
      )}
    </div>
  );
}
