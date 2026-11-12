import { useEffect, useState } from "react";
import { networkSpeed } from "@/lib/browser-client";
import type { NetworkSpeed } from "@type/browser";

const EMPTY: NetworkSpeed = {
  online: false,
  name: "",
  kind: "offline",
  downBps: 0,
  upBps: 0,
  linkBps: 0,
  signal: null,
  rttMs: null,
};

/** Polls the adapter about once a second. A sample already in flight is left to finish. */
export function useNetworkSpeed() {
  const [speed, setSpeed] = useState<NetworkSpeed>(EMPTY);

  useEffect(() => {
    let gone = false;
    let pending = false;
    const tick = () => {
      if (pending) {
        return;
      }
      pending = true;
      void networkSpeed()
        .then((next) => {
          if (!gone) {
            setSpeed(next);
          }
        })
        .catch(() => {})
        .finally(() => {
          pending = false;
        });
    };
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => {
      gone = true;
      window.clearInterval(timer);
    };
  }, []);

  return speed;
}
