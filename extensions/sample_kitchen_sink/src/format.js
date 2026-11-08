export function studio() {
  const ce = window.ce;
  if (!ce || !ce.tabs || !ce.bookmarks || !ce.downloads || !ce.storage || !ce.ui || !ce.runtime) return null;
  return ce;
}

export function explain(reason) {
  if (!reason) return "Something went wrong.";
  if (typeof reason === "string") return reason;
  return reason.message || String(reason);
}

export function hostOf(url) {
  try {
    return new URL(url).host;
  } catch {
    return "";
  }
}

export function formatBytes(value) {
  const amount = Number(value);
  if (!Number.isFinite(amount) || amount < 0) return "";
  if (amount < 1024) return `${Math.round(amount)} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let size = amount / 1024;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  const digits = size >= 10 || Number.isInteger(size) ? 0 : 1;
  return `${size.toFixed(digits)} ${units[unit]}`;
}

export function formatWhen(ms) {
  const time = Number(ms);
  if (!Number.isFinite(time) || time <= 0) return "";
  const date = new Date(time);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export function linksIn(tree) {
  const links = [];
  const walk = (node) => {
    if (!node || node.separator) return;
    if (node.url) links.push(node);
    for (const child of node.children || []) walk(child);
  };
  walk(tree && tree.bar);
  walk(tree && tree.other);
  return links.sort((a, b) => (b.addedAt || 0) - (a.addedAt || 0));
}

export function downloadTone(state) {
  if (state === "inProgress") return "bg-sky-400/15 text-sky-200";
  if (state === "paused") return "bg-amber-400/15 text-amber-100";
  if (state === "completed") return "bg-emerald-400/15 text-emerald-200";
  if (state === "interrupted") return "bg-rose-400/15 text-rose-200";
  return "bg-zinc-800 text-zinc-300";
}

export function downloadLabel(state) {
  if (state === "inProgress") return "Downloading";
  if (state === "paused") return "Paused";
  if (state === "completed") return "Saved";
  if (state === "cancelled") return "Cancelled";
  if (state === "interrupted") return "Interrupted";
  return "Unknown";
}

export function downloadMeta(item) {
  const received = formatBytes(item.receivedBytes);
  const total = item.totalBytes ? formatBytes(item.totalBytes) : "";
  const size = total ? `${received} of ${total}` : received;
  const when = formatWhen(item.finishedAt || item.startedAt);
  return [size, hostOf(item.url), when].filter(Boolean).join(" · ");
}

export function downloadPercent(item) {
  const total = Number(item.totalBytes);
  const received = Number(item.receivedBytes);
  if (!Number.isFinite(total) || total <= 0 || !Number.isFinite(received)) return null;
  return Math.max(0, Math.min(100, Math.round((received / total) * 100)));
}
