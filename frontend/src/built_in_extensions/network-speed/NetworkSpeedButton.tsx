import { Gauge } from "lucide-react";
import { ToolButton } from "../ToolButton";
import { connectionTone, useInternetProbeSettled, type ConnectionTone } from "../network/connection";
import { formatRate, kindLabel, strength, type StrengthTone } from "./format";
import { useNetworkSpeed } from "./use-network-speed";

const TONE_ICON: Record<StrengthTone, string> = {
  off: "text-zinc-600",
  weak: "text-red-300",
  fair: "text-amber-300",
  good: "text-sky-300",
  strong: "text-emerald-300",
};

const CONNECTION_ICON: Record<ConnectionTone, string> = {
  offline: "text-red-400",
  poor: "text-orange-400",
};

export function NetworkSpeedButton() {
  const speed = useNetworkSpeed();
  const level = strength(speed);
  const connection = connectionTone(speed, useInternetProbeSettled(speed));
  const label =
    connection === "offline"
      ? "Network speed, no internet"
      : `Network speed, ${kindLabel(speed.kind)}, ${formatRate(speed.downBps)} down${connection === "poor" ? ", poor connection" : ""}`;

  return (
    <ToolButton
      id="network-speed"
      label={label}
      kind="network-speed"
      needsPage={false}
      activeTab={undefined}
      icon={
        <Gauge
          id="address-network-speed-icon"
          className={connection ? CONNECTION_ICON[connection] : TONE_ICON[level.tone]}
          aria-hidden
        />
      }
    />
  );
}
