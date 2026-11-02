import { useEffect, useRef, useState, type ReactNode } from "react";
import { Activity } from "lucide-react";
import type { StatusSnapshot } from "@type/browser";
import { useAdblockStats } from "@/features/BuiltInExtensions/ad-blocker/use-adblock-stats";
import { PopupFrame } from "@/components/PopupFrame";
import { onStatusChanged, onStatusText } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { useDownloads, useSettings, useTabs } from "@/lib/use-browser";

type LiveStatus = StatusSnapshot & { receivedAt: number };

/** Browser status, opened from the address bar. */
export function StatusPopover() {
  const snapshot = useTabs();
  const downloads = useDownloads();
  const settings = useSettings();
  const activeTab = snapshot.tabs.find((tab) => tab.id === snapshot.activeId);
  const status = useStatus();
  const hoverText = useHoverText(activeTab?.id ?? null);
  const [now, setNow] = useState(() => Date.now());
  const blocked = useAdblockStats(activeTab?.id ?? null);
  const { downloading, rate } = useDownloadRate(downloads);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  const page = status?.pages.find((item) => item.id === activeTab?.id);
  const extra = status && page?.shown ? Math.max(0, now - status.receivedAt) : 0;
  const sleepingTabs = snapshot.windowTabs.filter((tab) => tab.sleeping);
  const asleep = sleepingTabs.length;
  const sleepSaved = sleepingTabs.reduce((sum, tab) => sum + (tab.sleepSaved || 0), 0);
  const accent = settings?.siteAccent && activeTab?.accent ? activeTab.accent : undefined;
  const version = status?.version ?? "";
  const sessionMs = status ? Math.max(0, now - status.startedAt) : null;
  const systemTotal = status?.systemTotal ?? 0;
  const systemUsed = status?.systemUsed ?? 0;
  const systemPct = systemTotal > 0 ? Math.min(100, (systemUsed / systemTotal) * 100) : 0;
  const free = Math.max(0, systemTotal - systemUsed);
  const tabUsed = page?.tabUsed ?? null;
  const tabHeap = page?.tabHeap ?? null;
  const tight = systemPct >= 85;
  const host = page?.siteHost || "";

  return (
    <PopupFrame id="status-popover" width={300}>
      <div id="status-popover-accent" className="h-1 shrink-0" style={{ backgroundColor: accent ?? "rgb(63 63 70)" }} />
      <header id="status-popover-header" className="flex items-center gap-3 px-4 pb-3 pt-3">
        <span id="status-popover-mark" className="inline-flex size-8 shrink-0 items-center justify-center rounded-lg bg-sky-500/15 text-sky-300">
          <Activity id="status-popover-mark-icon" className="size-4" aria-hidden />
        </span>
        <div id="status-popover-title-block" className="min-w-0 flex-1">
          <h2 id="status-popover-title" className="truncate text-sm font-semibold text-zinc-100">
            {version ? `ce-browser ${version}` : "ce-browser"}
          </h2>
          <p id="status-popover-host" className="truncate text-xs text-zinc-500" title={host || undefined}>
            {host || "No page"}
          </p>
        </div>
        {activeTab ? (
          <div id="status-popover-zoom" className="shrink-0 text-right" title="Page zoom">
            <p id="status-popover-zoom-value" className="text-lg font-semibold tabular-nums text-zinc-100">
              {Math.round(activeTab.zoom * 100)}%
            </p>
            <p id="status-popover-zoom-label" className="text-[10px] uppercase tracking-wide text-zinc-500">
              Zoom
            </p>
          </div>
        ) : null}
      </header>
      {hoverText ? (
        <p id="status-popover-link" title={hoverText} className="mx-4 mb-3 truncate rounded-md bg-sky-500/10 px-2 py-1 text-xs text-sky-300">
          {hoverText}
        </p>
      ) : null}
      <section id="status-popover-memory" className="mx-4 mb-3 rounded-lg bg-zinc-950/70 p-3">
        <div id="status-popover-memory-heading" className="mb-2 flex items-center justify-between gap-3">
          <h3 id="status-popover-memory-title" className="text-[11px] font-medium uppercase tracking-wide text-zinc-500">
            Memory
          </h3>
          {systemTotal > 0 ? (
            <span id="status-popover-memory-percent" className={cn("text-xs tabular-nums", tight ? "text-amber-300" : "text-zinc-300")}>
              {Math.round(systemPct)}%
            </span>
          ) : null}
        </div>
        <div id="status-popover-system-meter" className="mb-3 h-1.5 overflow-hidden rounded-full bg-zinc-800" title={systemTotal > 0 ? `${formatBytes(systemUsed)} used, ${formatBytes(free)} free of ${formatBytes(systemTotal)}` : "System memory"}>
          <div
            id="status-popover-system-meter-fill"
            className={cn("h-full rounded-full", tight ? "bg-amber-400" : "bg-sky-400")}
            style={{ width: `${systemPct}%` }}
          />
        </div>
        <StatRow id="status-popover-system" label="System" value={systemTotal > 0 ? formatPair(systemUsed, systemTotal) : "—"} />
        <StatRow id="status-popover-app" label="This app" value={status && status.appUsed > 0 ? formatBytes(status.appUsed) : "—"} hint="ce-browser and its WebView2 processes" />
        <StatRow
          id="status-popover-page"
          label="This page"
          value={tabUsed !== null ? formatBytes(tabUsed) : tabHeap !== null ? `${formatBytes(tabHeap)} heap` : "—"}
          hint={
            tabUsed !== null && page?.tabShared
              ? "Renderer working set. Other tabs on this site can share it."
              : tabHeap !== null
                ? "JavaScript heap. The renderer process was not matched."
                : "Renderer working set for this page."
          }
        />
        {asleep > 0 ? (
          <StatRow
            id="status-popover-sleep-saved"
            label={`Sleeping tabs (${asleep})`}
            value={sleepSaved > 0 ? `about ${formatBytes(sleepSaved)} saved` : "—"}
            hint="What those tabs used just before they went to sleep"
          />
        ) : null}
      </section>
      <section id="status-popover-time" className="mx-4 mb-3 rounded-lg bg-zinc-950/70 px-3 py-1">
        <h3 id="status-popover-time-title" className="pb-1 pt-2 text-[11px] font-medium uppercase tracking-wide text-zinc-500">
          Time
        </h3>
        <StatRow id="status-popover-clock" label="Clock" value={new Date(now).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })} />
        <StatRow id="status-popover-session" label="This session" value={sessionMs !== null ? formatDuration(sessionMs) : "—"} hint="Since ce-browser opened" />
        <StatRow id="status-popover-here" label="This tab" value={activeTab ? formatDuration((page?.tabSessionMs ?? 0) + extra) : "—"} hint="On screen this session" />
        <StatRow id="status-popover-all" label="All time" value={host ? formatDuration((page?.siteAllTimeMs ?? 0) + extra) : "—"} hint={host ? `On ${host}, across every session` : "Time on this site"} />
      </section>
      {blocked > 0 || asleep > 0 || downloading || status?.batterySaver ? (
        <div id="status-popover-flags" className="flex flex-wrap gap-1.5 px-4 pb-4">
          {blocked > 0 ? (
            <Flag id="status-popover-ads" title="Requests the ad blocker stopped on this page">
              {blocked} blocked
            </Flag>
          ) : null}
          {asleep > 0 ? (
            <Flag id="status-popover-sleeping" title="Tabs suspended in the background">
              {asleep} asleep{sleepSaved > 0 ? `, ${formatBytes(sleepSaved)} saved` : ""}
            </Flag>
          ) : null}
          {downloading ? (
            <Flag id="status-popover-download" title="Download speed">
              ↓ {rate > 0 ? formatRate(rate) : "…"}
            </Flag>
          ) : null}
          {status?.batterySaver ? (
            <Flag id="status-popover-battery" title="Battery saver is on. Background tabs sleep sooner." className="text-amber-300">
              Saver
            </Flag>
          ) : null}
        </div>
      ) : (
        <div id="status-popover-flags-gap" className="h-1" />
      )}
    </PopupFrame>
  );
}

function StatRow({ id, label, value, hint }: { id: string; label: string; value: string; hint?: string }) {
  return (
    <div id={id} className="flex items-baseline justify-between gap-3 py-1" title={hint}>
      <span id={`${id}-label`} className="text-xs text-zinc-400">
        {label}
      </span>
      <span id={`${id}-value`} className="truncate text-xs font-medium tabular-nums text-zinc-100">
        {value}
      </span>
    </div>
  );
}

function Flag({ id, title, className, children }: { id: string; title: string; className?: string; children: ReactNode }) {
  return (
    <span id={id} title={title} className={cn("rounded-full bg-zinc-800 px-2 py-0.5 text-[11px] text-zinc-300", className)}>
      {children}
    </span>
  );
}

function useStatus() {
  const [status, setStatus] = useState<LiveStatus | null>(null);
  useEffect(() => {
    const unlisten = onStatusChanged((next) => {
      setStatus({ ...next, receivedAt: Date.now() });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);
  return status;
}

function useHoverText(tabId: number | null) {
  const [hover, setHover] = useState<{ tabId: number; text: string } | null>(null);
  useEffect(() => {
    const unlisten = onStatusText((next) => setHover(next));
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);
  if (hover === null || hover.tabId !== tabId) {
    return "";
  }
  return hover.text.trim();
}

function useDownloadRate(downloads: { state: string; receivedBytes: number }[]) {
  const active = downloads.filter((item) => item.state === "inProgress");
  const bytes = active.reduce((sum, item) => sum + item.receivedBytes, 0);
  const [rate, setRate] = useState(0);
  const previous = useRef<{ at: number; bytes: number } | null>(null);

  useEffect(() => {
    const at = Date.now();
    const last = previous.current;
    previous.current = { at, bytes };
    if (!last || active.length === 0) {
      setRate(0);
      return;
    }
    const seconds = (at - last.at) / 1000;
    if (seconds < 0.05) {
      return;
    }
    setRate(Math.max(0, (bytes - last.bytes) / seconds));
  }, [bytes, active.length]);

  return { downloading: active.length > 0, rate };
}

function formatDuration(ms: number) {
  const total = Math.max(0, Math.floor(ms / 1000));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  if (hours > 0) {
    return `${hours}h ${minutes.toString().padStart(2, "0")}m`;
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds.toString().padStart(2, "0")}s`;
  }
  return `${seconds}s`;
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes < 0) {
    return "0 B";
  }
  if (bytes >= 1024 ** 3) {
    return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  }
  if (bytes >= 1024 ** 2) {
    return `${Math.round(bytes / 1024 ** 2)} MB`;
  }
  if (bytes >= 1024) {
    return `${Math.round(bytes / 1024)} KB`;
  }
  return `${Math.round(bytes)} B`;
}

function formatPair(used: number, total: number) {
  if (total >= 1024 ** 3) {
    const totalGb = total / 1024 ** 3;
    const whole = Math.abs(totalGb - Math.round(totalGb)) < 0.05;
    return `${(used / 1024 ** 3).toFixed(1)}/${(total / 1024 ** 3).toFixed(whole ? 0 : 1)} GB`;
  }
  return `${formatBytes(used)}/${formatBytes(total)}`;
}

function formatRate(bytesPerSecond: number) {
  if (bytesPerSecond >= 1024 * 1024) {
    return `${(bytesPerSecond / (1024 * 1024)).toFixed(1)} MB/s`;
  }
  if (bytesPerSecond >= 1024) {
    return `${Math.round(bytesPerSecond / 1024)} KB/s`;
  }
  return `${Math.round(bytesPerSecond)} B/s`;
}
