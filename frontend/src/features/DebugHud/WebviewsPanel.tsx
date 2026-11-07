import { useMemo, useState } from "react";
import { Bug, RotateCw, ScanSearch, SquareX } from "lucide-react";
import type { DebugWebview } from "@type/browser";
import { debugClearOutline, debugOpenDevtools, debugOutline, debugReloadWebview } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { HudButton, HudEmpty, HudSearch, HudToggle, HudToolbar } from "./HudControls";

type WebviewsPanelProps = {
  webviews: DebugWebview[] | null;
  error: string;
  /** Lists the webviews again now, so an outline change shows at once. */
  onRefresh: () => void;
};

const KIND_TONE: Record<string, string> = {
  ui: "bg-sky-500/15 text-sky-300",
  tab: "bg-emerald-500/15 text-emerald-300",
  popup: "bg-violet-500/15 text-violet-300",
  extpopup: "bg-violet-500/15 text-violet-300",
  devtools: "bg-amber-500/15 text-amber-300",
  agent: "bg-pink-500/15 text-pink-300",
  peek: "bg-cyan-500/15 text-cyan-300",
  debug: "bg-zinc-700/60 text-zinc-300",
};

function failureText(reason: unknown, fallback: string) {
  return typeof reason === "string" && reason.trim() ? reason : fallback;
}

export function WebviewsPanel({ webviews, error, onRefresh }: WebviewsPanelProps) {
  const [query, setQuery] = useState("");
  const [hideParked, setHideParked] = useState(false);
  const [actionError, setActionError] = useState("");

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return (webviews ?? []).filter((item) => {
      if (hideParked && item.parked) return false;
      return !needle || item.label.toLowerCase().includes(needle) || item.url.toLowerCase().includes(needle) || item.kind.includes(needle);
    });
  }, [hideParked, query, webviews]);

  const run = (action: Promise<unknown>, fallback: string) => {
    setActionError("");
    void action.catch((reason: unknown) => setActionError(failureText(reason, fallback))).finally(onRefresh);
  };

  const outlined = webviews?.find((item) => item.outlined) ?? null;
  const shownError = actionError || error;

  return (
    <section id="debug-webviews" className="flex min-h-0 flex-1 flex-col">
      <HudToolbar id="debug-webviews-toolbar">
        <HudSearch id="debug-webviews-search" value={query} placeholder="Filter label, kind or URL" onChange={setQuery} />
        <HudToggle id="debug-webviews-hide-parked" pressed={hideParked} onChange={setHideParked} title="Hide background tabs and idle popups">
          Hide parked
        </HudToggle>
        <HudButton
          id="debug-webviews-clear-outline"
          title={outlined ? `Clear the outline on ${outlined.label}` : "No webview is outlined"}
          disabled={!outlined}
          className={cn(outlined && "text-fuchsia-300")}
          onClick={() => run(debugClearOutline(), "The outline could not be cleared.")}
        >
          <SquareX id="debug-webviews-clear-outline-icon" aria-hidden className="size-3" />
          Clear outline
        </HudButton>
        <span id="debug-webviews-count" className="ml-auto text-[10px] text-zinc-500">
          {webviews ? `${filtered.length} of ${webviews.length}` : ""}
        </span>
      </HudToolbar>
      {shownError ? (
        <p id="debug-webviews-error" className="px-3 py-1 text-[11px] text-red-300">
          {shownError}
        </p>
      ) : null}
      <div id="debug-webviews-scroller" className="min-h-0 flex-1 overflow-auto">
        {webviews === null ? (
          <HudEmpty id="debug-webviews-loading">Loading webviews…</HudEmpty>
        ) : filtered.length === 0 ? (
          <HudEmpty id="debug-webviews-empty">Nothing matches this filter.</HudEmpty>
        ) : (
          <ul id="debug-webviews-list">
            {filtered.map((item) => {
              const rowId = `debug-webview-${item.label}`;
              return (
                <li
                  id={rowId}
                  key={item.label}
                  className={cn("border-t border-zinc-800/70 px-2 py-1.5", item.parked && "opacity-60", item.outlined && "bg-fuchsia-500/10")}
                >
                  <div id={`${rowId}-bar`} className="flex items-center gap-1.5">
                    <span id={`${rowId}-kind`} className={cn("shrink-0 rounded px-1 text-[9px] font-semibold uppercase", KIND_TONE[item.kind] ?? "bg-zinc-800 text-zinc-400")}>
                      {item.kind}
                    </span>
                    <span id={`${rowId}-label`} className="min-w-0 truncate font-mono text-[11px] text-zinc-100">
                      {item.label}
                    </span>
                    {item.window !== item.label ? (
                      <span id={`${rowId}-window`} className="shrink-0 text-[10px] text-zinc-500">
                        in {item.window}
                      </span>
                    ) : null}
                    {item.parked ? (
                      <span id={`${rowId}-parked`} className="shrink-0 rounded bg-zinc-800 px-1 text-[9px] uppercase text-zinc-400">
                        parked
                      </span>
                    ) : null}
                    {!item.windowVisible ? (
                      <span id={`${rowId}-hidden`} className="shrink-0 rounded bg-zinc-800 px-1 text-[9px] uppercase text-zinc-400">
                        hidden
                      </span>
                    ) : null}
                    <span id={`${rowId}-actions`} className="ml-auto flex shrink-0 items-center">
                      <HudButton
                        id={`${rowId}-outline`}
                        title={item.outlined ? "Clear the outline" : "Outline it on screen"}
                        disabled={item.parked && !item.outlined}
                        className={cn(item.outlined && "bg-fuchsia-500/20 text-fuchsia-200 hover:bg-fuchsia-500/30 hover:text-fuchsia-100")}
                        onClick={() => run(debugOutline(item.label), "The outline could not be drawn.")}
                      >
                        <ScanSearch id={`${rowId}-outline-icon`} aria-hidden className="size-3.5" />
                      </HudButton>
                      <HudButton id={`${rowId}-devtools`} title="Open DevTools" onClick={() => run(debugOpenDevtools(item.label), "DevTools did not open.")}>
                        <Bug id={`${rowId}-devtools-icon`} aria-hidden className="size-3.5" />
                      </HudButton>
                      <HudButton id={`${rowId}-reload`} title="Reload" onClick={() => run(debugReloadWebview(item.label), "The webview did not reload.")}>
                        <RotateCw id={`${rowId}-reload-icon`} aria-hidden className="size-3.5" />
                      </HudButton>
                    </span>
                  </div>
                  <p id={`${rowId}-geometry`} className="mt-0.5 font-mono text-[10px] tabular-nums text-zinc-500">
                    {item.width}×{item.height} at {item.x},{item.y} · screen {item.screenX},{item.screenY}
                    {item.tabId !== null ? ` · tab ${item.tabId}` : ""}
                  </p>
                  <p id={`${rowId}-url`} className="truncate font-mono text-[10px] text-zinc-400" title={item.url}>
                    {item.url || "–"}
                  </p>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </section>
  );
}
