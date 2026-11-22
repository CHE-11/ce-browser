import { useEffect, useState } from "react";
import type { NetworkSpeed } from "@type/browser";

/** A public ping or external address should have landed by now. */
const INTERNET_PROBE_MS = 6000;

export type ConnectionTone = "offline" | "poor";

/**
 * Red when nothing is connected or the internet cannot be reached.
 * Orange when the link is up but the same checks the speed panel calls poor are failing.
 * Null while the first probe is still out, and on a healthy link.
 */
export function connectionTone(speed: NetworkSpeed, probeSettled: boolean): ConnectionTone | null {
  if (!speed.online) {
    return "offline";
  }
  if (!internetReached(speed)) {
    return probeSettled ? "offline" : null;
  }
  if (isPoor(speed)) {
    return "poor";
  }
  return null;
}

export function connectionLabel(tone: ConnectionTone | null) {
  if (tone === "offline") {
    return "Network, no internet";
  }
  if (tone === "poor") {
    return "Network, poor connection";
  }
  return "Network";
}

/** True once a missing ping and external address mean the internet is down, not that the probe is still running. */
export function useInternetProbeSettled(speed: NetworkSpeed) {
  const [settled, setSettled] = useState(false);
  const reached = internetReached(speed);

  useEffect(() => {
    if (!speed.online || reached) {
      setSettled(false);
      return;
    }
    const timer = window.setTimeout(() => setSettled(true), INTERNET_PROBE_MS);
    return () => window.clearTimeout(timer);
  }, [speed.online, reached]);

  return settled;
}

function internetReached(speed: NetworkSpeed) {
  return speed.pingMs != null || speed.externalIp != null;
}

/** Cutoffs match the poor / weak / unsteady lines in the speed panel. */
function isPoor(speed: NetworkSpeed) {
  if (speed.signal != null && speed.signal <= 50) {
    return true;
  }
  if (speed.pingMs != null && speed.pingMs > 80) {
    return true;
  }
  if (speed.rttMs != null && speed.rttMs > 30) {
    return true;
  }
  if (speed.jitterMs != null && speed.jitterMs > 20) {
    return true;
  }
  return false;
}
