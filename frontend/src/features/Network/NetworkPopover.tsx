import { useEffect, useState } from "react";
import { ListTree } from "lucide-react";
import { PopoverHeader } from "@features/BuiltInExtensions/PopoverParts";
import { PopupFrame } from "@/components/PopupFrame";
import { clearNetworkLog, networkLog } from "@/lib/browser-client";
import type { NetworkEntry } from "@type/browser";

type NetworkPopoverProps = {
  tabId: number;
};

function statusClass(status: number | null) {
  if (status === null) {
    return "text-zinc-500";
  }
  if (status >= 400) {
    return "text-red-300";
  }
  if (status >= 300) {
    return "text-amber-300";
  }
  return "text-emerald-300";
}

function formatWhen(time: number) {
  return new Date(time).toLocaleString(undefined, {
    month: "numeric",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
    second: "2-digit",
  });
}

function formatDuration(duration: number | null) {
  if (duration === null) {
    return "–";
  }
  if (duration < 1000) {
    return `${duration} ms`;
  }
  const seconds = duration / 1000;
  if (seconds < 10) {
    return `${seconds.toFixed(1)} s`;
  }
  if (seconds < 60) {
    return `${Math.round(seconds)} s`;
  }
  const minutes = seconds / 60;
  if (minutes < 10) {
    return `${minutes.toFixed(1)} m`;
  }
  return `${Math.round(minutes)} m`;
}

export function NetworkPopover({ tabId }: NetworkPopoverProps) {
  const [entries, setEntries] = useState<NetworkEntry[] | null>(null);

  useEffect(() => {
    let disposed = false;
    const load = () => {
      void networkLog(tabId).then((next) => {
        if (!disposed) {
          setEntries(next);
        }
      });
    };
    load();
    const timer = window.setInterval(load, 1000);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [tabId]);

  return (
    <PopupFrame id="network-popover" width={760} maxHeight={640} ready={entries !== null}>
      <PopoverHeader id="network-header" icon={<ListTree aria-hidden />} title="Network" subtitle={`${entries?.length ?? 0} recent requests`} />
      <div id="network-toolbar" className="flex justify-end px-3 pb-1">
        <button
          id="network-clear"
          type="button"
          className="h-7 rounded-md px-2 text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
          onClick={() => {
            void clearNetworkLog(tabId).then(() => setEntries([]));
          }}
        >
          Clear
        </button>
      </div>
      <ul id="network-list" className="flex flex-col px-2 pb-2">
        {(entries ?? []).length === 0 ? (
          <li id="network-empty" className="px-2 py-3 text-sm text-zinc-500">
            No requests yet.
          </li>
        ) : (
          <>
            <li id="network-columns" className="flex items-baseline gap-2 px-2 pb-1 text-[10px] uppercase tracking-wide text-zinc-500">
              <span id="network-column-status" className="w-8 shrink-0">
                Status
              </span>
              <span id="network-column-method" className="w-12 shrink-0">
                Method
              </span>
              <span id="network-column-when" className="w-40 shrink-0">
                Date time
              </span>
              <span id="network-column-duration" className="w-16 shrink-0 text-right">
                Request
              </span>
              <span id="network-column-url" className="min-w-0 flex-1">
                URL
              </span>
            </li>
            {[...(entries ?? [])].reverse().map((entry, index) => (
              <li
                id={`network-row-${index}`}
                key={`${entry.time}-${entry.url}-${index}`}
                className="flex items-baseline gap-2 border-t border-zinc-800/80 px-2 py-1.5"
                title={entry.url}
              >
                <span id={`network-row-${index}-status`} className={`w-8 shrink-0 text-xs tabular-nums ${statusClass(entry.status)}`}>
                  {entry.status ?? "–"}
                </span>
                <span id={`network-row-${index}-method`} className="w-12 shrink-0 text-[11px] uppercase text-zinc-500">
                  {entry.method}
                </span>
                <span id={`network-row-${index}-when`} className="w-40 shrink-0 truncate text-[11px] tabular-nums text-zinc-400" title={formatWhen(entry.time)}>
                  {formatWhen(entry.time)}
                </span>
                <span id={`network-row-${index}-duration`} className="w-16 shrink-0 text-right text-[11px] tabular-nums text-zinc-300" title="Request time">
                  {formatDuration(entry.durationMs)}
                </span>
                <span id={`network-row-${index}-url`} className="min-w-0 flex-1 truncate text-xs text-zinc-200">
                  {entry.url}
                </span>
              </li>
            ))}
          </>
        )}
      </ul>
    </PopupFrame>
  );
}
