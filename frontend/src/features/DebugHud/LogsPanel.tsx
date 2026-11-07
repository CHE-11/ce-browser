import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Trash2 } from "lucide-react";
import type { DebugLogLine } from "@type/browser";
import { cn } from "@/lib/cn";
import { formatClockMs } from "@/lib/debug-hud";
import { HudButton, HudEmpty, HudLiveToggle, HudSearch, HudSelect, HudToggle, HudToolbar } from "./HudControls";

/** Lines drawn at once. Filters still search the whole feed. */
const SHOWN_LINES = 1000;

const STREAM_OPTIONS = [
  { value: "", label: "All streams" },
  { value: "stderr", label: "stderr" },
  { value: "stdout", label: "stdout" },
];

type LogsPanelProps = {
  lines: DebugLogLine[];
  onClear: () => void;
};

export function LogsPanel({ lines, onClear }: LogsPanelProps) {
  const [query, setQuery] = useState("");
  const [area, setArea] = useState("");
  const [stream, setStream] = useState("");
  const [follow, setFollow] = useState(true);
  const [frozen, setFrozen] = useState<DebugLogLine[] | null>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const source = frozen ?? lines;

  const areaOptions = useMemo(() => {
    const areas = [...new Set(source.map((line) => line.area).filter(Boolean))].sort();
    return [{ value: "", label: "All areas" }, ...areas.map((item) => ({ value: item, label: item }))];
  }, [source]);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const matches = source.filter((line) => {
      if (stream && line.stream !== stream) return false;
      if (area && line.area !== area) return false;
      return !needle || line.text.toLowerCase().includes(needle);
    });
    return matches.slice(-SHOWN_LINES);
  }, [area, query, source, stream]);

  useLayoutEffect(() => {
    const element = scroller.current;
    if (follow && element) {
      element.scrollTop = element.scrollHeight;
    }
  }, [filtered, follow]);

  // Scrolling up stops following; scrolling back to the bottom resumes it.
  useEffect(() => {
    const element = scroller.current;
    if (!element) {
      return;
    }
    const onScroll = () => {
      const atBottom = element.scrollHeight - element.scrollTop - element.clientHeight < 8;
      setFollow(atBottom);
    };
    element.addEventListener("scroll", onScroll, { passive: true });
    return () => element.removeEventListener("scroll", onScroll);
  }, []);

  return (
    <section id="debug-logs" className="flex min-h-0 flex-1 flex-col">
      <HudToolbar id="debug-logs-toolbar">
        <HudSearch id="debug-logs-search" value={query} placeholder="Filter log text" onChange={setQuery} />
        <HudSelect id="debug-logs-area" value={area} options={areaOptions} onChange={setArea} title="Area" />
        <HudSelect id="debug-logs-stream" value={stream} options={STREAM_OPTIONS} onChange={setStream} title="Stream" />
        <HudToggle id="debug-logs-follow" pressed={follow} onChange={setFollow} title="Keep the newest line in view">
          Follow
        </HudToggle>
        <HudLiveToggle id="debug-logs-live" live={frozen === null} onChange={(live) => setFrozen(live ? null : lines)} />
        <HudButton id="debug-logs-clear" title="Clear the log" onClick={onClear}>
          <Trash2 id="debug-logs-clear-icon" aria-hidden className="size-3" />
          Clear
        </HudButton>
      </HudToolbar>
      <div id="debug-logs-scroller" ref={scroller} className="min-h-0 flex-1 overflow-auto py-1">
        {filtered.length === 0 ? (
          <HudEmpty id="debug-logs-empty">
            {source.length === 0 ? "Nothing logged yet. println! and eprintln! output from the backend shows up here." : "Nothing matches this filter."}
          </HudEmpty>
        ) : (
          <ol id="debug-logs-list" className="font-mono text-[11px] leading-[1.15rem]">
            {filtered.map((line) => (
              <li id={`debug-log-${line.id}`} key={line.id} className="flex gap-2 px-2 hover:bg-zinc-800/50">
                <span id={`debug-log-${line.id}-time`} className="shrink-0 tabular-nums text-zinc-600">
                  {formatClockMs(line.at)}
                </span>
                <span id={`debug-log-${line.id}-stream`} className={cn("w-10 shrink-0", line.stream === "stderr" ? "text-red-300/80" : "text-zinc-500")}>
                  {line.stream === "stderr" ? "err" : "out"}
                </span>
                <span id={`debug-log-${line.id}-text`} className={cn("min-w-0 flex-1 whitespace-pre-wrap break-all", line.stream === "stderr" ? "text-zinc-100" : "text-zinc-300")}>
                  {line.area ? (
                    <button
                      id={`debug-log-${line.id}-area`}
                      type="button"
                      title="Show only this area"
                      className="mr-1 rounded bg-zinc-800 px-1 text-sky-300 hover:bg-zinc-700"
                      onClick={() => setArea(line.area)}
                    >
                      {line.area}
                    </button>
                  ) : null}
                  {line.area ? line.text.slice(line.text.indexOf(": ") + 2) : line.text}
                </span>
              </li>
            ))}
          </ol>
        )}
      </div>
    </section>
  );
}
