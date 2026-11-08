export function percent(item) {
  if (!item.totalBytes) return item.state === "completed" ? 100 : 0;
  return Math.max(0, Math.min(100, Math.round((item.receivedBytes / item.totalBytes) * 100)));
}
