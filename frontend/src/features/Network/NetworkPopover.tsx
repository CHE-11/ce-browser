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
    <PopupFrame id="network-popover" width={440} maxHeight={640} ready={entries !== null}>
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
          [...(entries ?? [])].reverse().map((entry, index) => (
            <li
              id={`network-row-${index}`}
              key={`${entry.url}-${index}`}
              className="flex items-baseline gap-2 border-t border-zinc-800/80 px-2 py-1.5 first:border-t-0"
              title={entry.url}
            >
              <span id={`network-row-${index}-status`} className={`w-8 shrink-0 text-xs tabular-nums ${statusClass(entry.status)}`}>
                {entry.status ?? "–"}
              </span>
              <span id={`network-row-${index}-method`} className="w-10 shrink-0 text-[11px] uppercase text-zinc-500">
                {entry.method}
              </span>
              <span id={`network-row-${index}-url`} className="min-w-0 flex-1 truncate text-xs text-zinc-200">
                {entry.url}
              </span>
            </li>
          ))
        )}
      </ul>
    </PopupFrame>
  );
}
