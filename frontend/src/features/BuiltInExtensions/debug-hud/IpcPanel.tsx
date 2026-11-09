import { useMemo, useState } from "react";
import { Trash2 } from "lucide-react";
import type { DebugIpcRecord } from "@type/browser";
import { SLOW_CALL_MS, groupIpcRows } from "@/lib/debug-hud";
import { HudButton, HudEmpty, HudLiveToggle, HudSearch, HudSelect, HudToggle, HudToolbar } from "./HudControls";
import { IpcRow } from "./IpcRow";

/** Rows drawn at once. Filters still search the whole feed. */
const SHOWN_ROWS = 400;

type IpcPanelProps = {
  records: DebugIpcRecord[];
  onClear: () => void;
};

const KIND_OPTIONS = [
  { value: "", label: "All" },
  { value: "invoke", label: "Invokes" },
  { value: "event", label: "Events" },
];

export function IpcPanel({ records, onClear }: IpcPanelProps) {
  const [query, setQuery] = useState("");
  const [webview, setWebview] = useState("");
  const [kind, setKind] = useState("");
  const [errorsOnly, setErrorsOnly] = useState(false);
  const [slowOnly, setSlowOnly] = useState(false);
  const [frozen, setFrozen] = useState<DebugIpcRecord[] | null>(null);
  const source = frozen ?? records;

  const rows = useMemo(() => groupIpcRows(source), [source]);
  const webviewOptions = useMemo(() => {
    const labels = [...new Set(source.map((record) => record.webview))].sort();
    return [{ value: "", label: "All webviews" }, ...labels.map((label) => ({ value: label, label }))];
  }, [source]);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const matches = rows.filter(({ record, webviews }) => {
      if (kind && record.kind !== kind) return false;
      if (errorsOnly && record.ok) return false;
      if (slowOnly && (record.durationMs === null || record.durationMs < SLOW_CALL_MS)) return false;
      if (webview && !webviews.includes(webview)) return false;
      return !needle || record.name.toLowerCase().includes(needle) || record.argsPreview.toLowerCase().includes(needle);
    });
    return matches.slice(-SHOWN_ROWS).reverse();
  }, [errorsOnly, kind, query, rows, slowOnly, webview]);

  return (
    <section id="debug-ipc" className="flex min-h-0 flex-1 flex-col">
      <HudToolbar id="debug-ipc-toolbar">
        <HudSearch id="debug-ipc-search" value={query} placeholder="Filter command, event or args" onChange={setQuery} />
        <HudSelect id="debug-ipc-kind" value={kind} options={KIND_OPTIONS} onChange={setKind} title="Kind" />
        <HudSelect id="debug-ipc-webview" value={webview} options={webviewOptions} onChange={setWebview} title="Webview" />
        <HudToggle id="debug-ipc-errors" pressed={errorsOnly} onChange={setErrorsOnly}>
          Errors
        </HudToggle>
        <HudToggle id="debug-ipc-slow" pressed={slowOnly} onChange={setSlowOnly} title={`Calls of ${SLOW_CALL_MS} ms or more`}>
          Slow
        </HudToggle>
        <HudLiveToggle id="debug-ipc-live" live={frozen === null} onChange={(live) => setFrozen(live ? null : records)} />
        <HudButton id="debug-ipc-clear" title="Clear the IPC feed" onClick={onClear}>
          <Trash2 id="debug-ipc-clear-icon" aria-hidden className="size-3" />
          Clear
        </HudButton>
      </HudToolbar>
      <div id="debug-ipc-scroller" className="min-h-0 flex-1 overflow-auto">
        {filtered.length === 0 ? (
          <HudEmpty id="debug-ipc-empty">{rows.length === 0 ? "No calls yet. Use the browser and they show up here." : "Nothing matches this filter."}</HudEmpty>
        ) : (
          <ul id="debug-ipc-list">
            {filtered.map((row) => (
              <IpcRow key={row.key} row={row} />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
