import type { NetworkSpeed } from "@type/browser";

export type StrengthTone = "off" | "weak" | "fair" | "good" | "strong";

export type Strength = {
  bars: 0 | 1 | 2 | 3 | 4;
  label: string;
  tone: StrengthTone;
};

export function kindLabel(kind: NetworkSpeed["kind"]) {
  if (kind === "wifi") {
    return "Wi-Fi";
  }
  if (kind === "ethernet") {
    return "Ethernet";
  }
  if (kind === "other") {
    return "Other";
  }
  return "Offline";
}

/** Wi-Fi signal becomes bars. A connected link with no signal reads as full bars. */
export function strength(speed: NetworkSpeed): Strength {
  if (!speed.online) {
    return { bars: 0, label: "Offline", tone: "off" };
  }
  if (speed.signal == null) {
    return { bars: 4, label: "Connected", tone: "strong" };
  }
  if (speed.signal <= 0) {
    return { bars: 0, label: "Weak", tone: "weak" };
  }
  if (speed.signal <= 25) {
    return { bars: 1, label: "Weak", tone: "weak" };
  }
  if (speed.signal <= 50) {
    return { bars: 2, label: "Fair", tone: "fair" };
  }
  if (speed.signal <= 75) {
    return { bars: 3, label: "Good", tone: "good" };
  }
  return { bars: 4, label: "Strong", tone: "strong" };
}

/** Bytes per second, shown as bits per second. */
export function formatRate(bytesPerSec: number) {
  return formatBits(Math.max(0, bytesPerSec) * 8);
}

/** Bits per second, as b/s, Kb/s, Mb/s, or Gb/s. */
export function formatBits(bitsPerSec: number) {
  const bits = Math.max(0, bitsPerSec);
  if (bits >= 1_000_000_000) {
    return `${trim(bits / 1_000_000_000)} Gb/s`;
  }
  if (bits >= 1_000_000) {
    return `${trim(bits / 1_000_000)} Mb/s`;
  }
  if (bits >= 1_000) {
    return `${trim(bits / 1_000)} Kb/s`;
  }
  return `${Math.round(bits)} b/s`;
}

function trim(value: number) {
  if (value >= 100) {
    return String(Math.round(value));
  }
  const fixed = value.toFixed(1);
  return fixed.endsWith(".0") ? fixed.slice(0, -2) : fixed;
}

/** Live throughput. Idle is healthy. A rate near the link speed is congested. */
export function throughputHint(bytesPerSec: number, linkBps: number, direction: "down" | "up") {
  const bits = Math.max(0, bytesPerSec) * 8;
  const moving = direction === "down" ? "downloading" : "uploading";
  if (bits < 100_000) {
    return `Quiet. Little is ${moving}. An idle connection looks like this.`;
  }
  if (linkBps > 0 && bits / linkBps >= 0.8) {
    return "Congested. This is most of the link, so other transfers will slow down.";
  }
  return `Clear. Traffic is ${moving}, with room left on the link.`;
}

/** Negotiated adapter speed. */
export function linkHint(linkBps: number) {
  if (linkBps <= 0) {
    return "The adapter did not report a link speed.";
  }
  if (linkBps < 25_000_000) {
    return "Slow link. Browsing works. Video calls and large files will struggle.";
  }
  if (linkBps < 100_000_000) {
    return "Modest link. Comfortable for browsing. Several streams will crowd it.";
  }
  if (linkBps < 1_000_000_000) {
    return "Good link. Video, calls, and large downloads fit comfortably.";
  }
  return "Fast link. Ordinary browsing and streaming will not fill it.";
}

/** Round trip to the default gateway. Lower is better. */
export function latencyHint(rttMs: number | null) {
  if (rttMs == null) {
    return "No reply from the gateway yet.";
  }
  if (rttMs <= 10) {
    return "Good. The gateway answers quickly, so the local hop is healthy.";
  }
  if (rttMs <= 30) {
    return "Fair. The gateway is slower than usual, so pages can feel a beat late.";
  }
  return "Poor. The gateway is slow to answer, so pages and calls will feel delayed.";
}

/** Radio quality, or a wired link that has no signal to measure. */
export function strengthHint(speed: NetworkSpeed) {
  if (!speed.online) {
    return "Offline. No adapter is connected.";
  }
  if (speed.signal == null) {
    return speed.kind === "wifi"
      ? "The link is up. Signal quality is not available yet."
      : "Connected. A wired link stays at full bars while the cable is up.";
  }
  if (speed.signal <= 25) {
    return "Weak signal. The radio may drop or slow the connection.";
  }
  if (speed.signal <= 50) {
    return "Fair signal. Usable, but distance or walls are costing speed.";
  }
  if (speed.signal <= 75) {
    return "Good signal. Strong enough for calls and video.";
  }
  return "Strong signal. The radio is not holding the connection back.";
}
