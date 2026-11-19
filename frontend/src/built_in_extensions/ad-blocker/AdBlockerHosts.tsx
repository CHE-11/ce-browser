import type { AdblockHost, AdblockKind } from "@type/browser";

const KIND_LABELS: Record<string, [string, string]> = {
  document: ["document", "documents"],
  stylesheet: ["stylesheet", "stylesheets"],
  image: ["image", "images"],
  media: ["media file", "media files"],
  font: ["font", "fonts"],
  script: ["script", "scripts"],
  xhr: ["XHR", "XHR"],
  fetch: ["fetch", "fetches"],
  track: ["text track", "text tracks"],
  event: ["event stream", "event streams"],
  websocket: ["socket", "sockets"],
  manifest: ["manifest", "manifests"],
  exchange: ["exchange", "exchanges"],
  ping: ["ping", "pings"],
  report: ["report", "reports"],
  other: ["other", "other"],
};

function kindPhrase(kind: AdblockKind) {
  const [one, many] = KIND_LABELS[kind.kind] ?? [kind.kind, kind.kind];
  return `${kind.count} ${kind.count === 1 ? one : many}`;
}

function hostDetail(hit: AdblockHost) {
  const kinds = hit.kinds.map(kindPhrase).join(", ");
  if (hit.rule === hit.host) {
    return kinds;
  }
  return kinds ? `${kinds} · ${hit.rule}` : hit.rule;
}

type AdBlockerHostsProps = {
  hosts: AdblockHost[];
};

/** Hosts blocked on the current page, busiest first. */
export function AdBlockerHosts({ hosts }: AdBlockerHostsProps) {
  if (hosts.length === 0) {
    return (
      <p id="ad-blocker-hosts-empty" className="px-2.5 text-[11px] text-zinc-500">
        Nothing blocked on this page yet.
      </p>
    );
  }

  return (
    <div id="ad-blocker-hosts" className="flex flex-col gap-1">
      <p id="ad-blocker-hosts-label" className="px-2.5 text-[11px] text-zinc-500">
        {hosts.length === 1 ? "1 host" : `${hosts.length} hosts`}
      </p>
      <ul id="ad-blocker-host-list" className="max-h-56 overflow-y-auto rounded-lg bg-zinc-800/40">
        {hosts.map((hit, index) => {
          const detail = hostDetail(hit);
          return (
            <li
              id={`ad-blocker-host-${index}`}
              key={hit.host}
              title={hit.rule === hit.host ? hit.host : `${hit.host} matched ${hit.rule}`}
              className="flex items-baseline gap-2 px-2.5 py-1.5"
            >
              <span
                id={`ad-blocker-host-${index}-count`}
                className="w-8 shrink-0 text-right text-xs tabular-nums text-sky-300"
              >
                {hit.count}
              </span>
              <span id={`ad-blocker-host-${index}-name`} className="min-w-0 flex-1">
                <span id={`ad-blocker-host-${index}-host`} className="block truncate text-xs text-zinc-200">
                  {hit.host}
                </span>
                {detail ? (
                  <span id={`ad-blocker-host-${index}-detail`} className="block truncate text-[11px] text-zinc-500">
                    {detail}
                  </span>
                ) : null}
              </span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
