import { Gauge } from "lucide-react";
import { ToolButton } from "../ToolButton";
import { formatRate, kindLabel, strength, type StrengthTone } from "./format";
import { useNetworkSpeed } from "./use-network-speed";

const TONE_ICON: Record<StrengthTone, string> = {
  off: "text-zinc-600",
  weak: "text-red-300",
  fair: "text-amber-300",
  good: "text-sky-300",
  strong: "text-emerald-300",
};



export function NetworkSpeedButton() {
  const speed = useNetworkSpeed();
  const level = strength(speed);
  const label = speed.online
    ? `Network speed, ${kindLabel(speed.kind)}, ${formatRate(speed.downBps)} down`
    : "Network speed, offline";

  return (
    <ToolButton
      id="network-speed"
      label={label}
      kind="network-speed"
      needsPage={false}
      activeTab={undefined}
      icon={
        <>
          <Gauge id="address-network-speed-icon" className={TONE_ICON[level.tone]} aria-hidden />
        </>
      }
    />
  );
}
