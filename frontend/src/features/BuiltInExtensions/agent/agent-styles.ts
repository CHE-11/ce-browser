import type { AgentRunStatus } from "@type/browser";

export const inputClassName =
  "w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-sm text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-sky-500";

export const STATUS_TEXT: Record<AgentRunStatus, string> = {
  running: "Working",
  waiting: "Waiting for you",
  paused: "Paused while you take over",
  done: "Done",
  failed: "Failed",
  stopped: "Stopped",
};

export const STATUS_TONE: Record<AgentRunStatus, string> = {
  running: "bg-sky-500",
  waiting: "bg-amber-400",
  paused: "bg-violet-400",
  done: "bg-emerald-500",
  failed: "bg-red-500",
  stopped: "bg-zinc-500",
};

export function formatTokens(count: number) {
  if (count >= 1_000_000) {
    return `${(count / 1_000_000).toFixed(1)}M`;
  }
  if (count >= 1000) {
    return `${(count / 1000).toFixed(count >= 10_000 ? 0 : 1)}k`;
  }
  return String(count);
}

export function formatElapsed(ms: number) {
  const seconds = Math.max(0, Math.round(ms / 1000));
  if (seconds < 60) {
    return `${seconds}s`;
  }
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) {
    return `${minutes}m ${seconds % 60}s`;
  }
  return `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
}

export function formatClock(at: number) {
  return new Date(at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}
