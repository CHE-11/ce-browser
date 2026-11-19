import { useEffect, useState } from "react";
import { ChevronRight, Copy } from "lucide-react";
import type { NetworkEntry, NetworkHeader } from "@type/browser";
import { copyText } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import { authBadge, authFacts, formatDuration, formatWhen, networkColumnClass } from "@/lib/network-log";

type NetworkRowProps = {
  entry: NetworkEntry;
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

function HeaderList({
  id,
  title,
  headers,
}: {
  id: string;
  title: string;
  headers: NetworkHeader[];
}) {
  return (
    <section id={id} className="mt-2">
      <h4 id={`${id}-title`} className="text-[10px] uppercase tracking-wide text-zinc-500">
        {title} ({headers.length})
      </h4>
      {headers.length === 0 ? (
        <p id={`${id}-empty`} className="mt-1 text-xs text-zinc-500">
          None reported.
        </p>
      ) : (
        <dl id={`${id}-list`} className="mt-1 flex flex-col gap-1">
          {headers.map((header, index) => (
            <div id={`${id}-row-${index}`} key={`${header.name}-${index}`} className="grid grid-cols-[9rem_minmax(0,1fr)] gap-2">
              <dt id={`${id}-name-${index}`} className="truncate font-mono text-[11px] text-zinc-400" title={header.name}>
                {header.name}
              </dt>
              <dd id={`${id}-value-${index}`} className="min-w-0 break-all font-mono text-[11px] text-zinc-200">
                {header.value}
              </dd>
            </div>
          ))}
        </dl>
      )}
    </section>
  );
}

export function NetworkRow({ entry }: NetworkRowProps) {
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState<string | null>(null);
  const badge = authBadge(entry);
  const facts = open ? authFacts(entry) : [];
  const rowId = `network-row-${entry.id}`;

  useEffect(() => {
    if (!copied) {
      return;
    }
    const timer = window.setTimeout(() => setCopied(null), 1200);
    return () => window.clearTimeout(timer);
  }, [copied]);

  return (
    <li id={rowId} className="border-t border-zinc-800/80">
      <button
        id={`${rowId}-toggle`}
        type="button"
        className={cn("flex w-full items-center gap-2 px-2 py-1.5 text-left hover:bg-zinc-800/60", focusRow)}
        aria-expanded={open}
        title={entry.url}
        onClick={() => setOpen((current) => !current)}
      >
        <span id={`${rowId}-chevron-wrap`} className={cn(networkColumnClass.toggle, "inline-flex items-center")}>
          <ChevronRight
            id={`${rowId}-chevron`}
            aria-hidden
            className={cn("size-3.5 text-zinc-500 transition-transform", open && "rotate-90")}
          />
        </span>
        <span id={`${rowId}-status`} className={cn(networkColumnClass.status, "text-xs tabular-nums", statusClass(entry.status))}>
          {entry.status ?? "–"}
        </span>
        <span id={`${rowId}-method`} className={cn(networkColumnClass.method, "truncate text-[11px] uppercase text-zinc-400")}>
          {entry.method}
        </span>
        <span id={`${rowId}-when`} className={cn(networkColumnClass.when, "truncate text-[11px] tabular-nums text-zinc-400")} title={formatWhen(entry.time)}>
          {formatWhen(entry.time)}
        </span>
        <span id={`${rowId}-duration`} className={cn(networkColumnClass.duration, "text-[11px] tabular-nums text-zinc-300")}>
          {formatDuration(entry.durationMs)}
        </span>
        <span
          id={`${rowId}-auth`}
          className={cn(networkColumnClass.auth, "truncate text-[11px]", badge ? "text-sky-300" : "text-zinc-600")}
        >
          {badge ?? "–"}
        </span>
        <span id={`${rowId}-url`} className={cn(networkColumnClass.url, "truncate text-xs text-zinc-200")}>
          {entry.url}
        </span>
      </button>
      {open ? (
        <div id={`${rowId}-details`} className="border-t border-zinc-800/60 bg-zinc-950/50 px-3 py-2">
          <p id={`${rowId}-meta`} className="text-[11px] text-zinc-500">
            {entry.kind} · {formatDuration(entry.durationMs)}
          </p>
          <section id={`${rowId}-auth-section`} className="mt-2">
            <h4 id={`${rowId}-auth-title`} className="text-[10px] uppercase tracking-wide text-zinc-500">
              Auth
            </h4>
            {facts.length === 0 ? (
              <p id={`${rowId}-auth-empty`} className="mt-1 text-xs text-zinc-500">
                {badge
                  ? "The URL looks like a sign-in or token request. No credential header was visible."
                  : "No authorization, cookie, or authentication header was visible."}
              </p>
            ) : (
              <ul id={`${rowId}-auth-list`} className="mt-1 flex flex-col gap-1.5">
                {facts.map((fact) => (
                  <li id={`${rowId}-auth-${fact.key}`} key={fact.key} className="rounded-md border border-zinc-800 bg-zinc-900/80 px-2 py-1.5">
                    <div id={`${rowId}-auth-${fact.key}-bar`} className="flex items-center gap-2">
                      <span id={`${rowId}-auth-${fact.key}-label`} className="truncate font-mono text-[11px] text-zinc-200">
                        {fact.label}
                      </span>
                      <span id={`${rowId}-auth-${fact.key}-detail`} className="text-[10px] uppercase tracking-wide text-sky-300/80">
                        {fact.detail}
                      </span>
                      <button
                        id={`${rowId}-auth-${fact.key}-copy`}
                        type="button"
                        className="ml-auto h-6 shrink-0 rounded px-1.5 text-[11px] text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100"
                        onClick={() => {
                          void copyText(fact.value).then(() => setCopied(fact.key));
                        }}
                      >
                        <Copy id={`${rowId}-auth-${fact.key}-copy-icon`} aria-hidden className="mr-1 inline size-3" />
                        {copied === fact.key ? "Copied" : "Copy"}
                      </button>
                    </div>
                    <p id={`${rowId}-auth-${fact.key}-value`} className="mt-1 break-all font-mono text-[11px] text-zinc-300">
                      {fact.value}
                    </p>
                  </li>
                ))}
              </ul>
            )}
          </section>
          <HeaderList id={`${rowId}-request`} title="Request headers" headers={entry.requestHeaders} />
          <HeaderList id={`${rowId}-response`} title="Response headers" headers={entry.responseHeaders} />
        </div>
      ) : null}
    </li>
  );
}
