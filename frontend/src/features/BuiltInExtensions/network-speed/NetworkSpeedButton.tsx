import { Gauge } from "lucide-react";
import { cn } from "@/lib/cn";
import { ToolButton } from "../ToolButton";
import { formatRate, kindLabel, strength, type Strength, type StrengthTone } from "./format";
import { useNetworkSpeed } from "./use-network-speed";

const TONE_BAR: Record<StrengthTone, string> = {
  off: "bg-zinc-600",
  weak: "bg-red-400",
  fair: "bg-amber-400",
  good: "bg-sky-400",
  strong: "bg-emerald-400",
};

const TONE_ICON: Record<StrengthTone, string> = {
  off: "text-zinc-600",
  weak: "text-red-300",
  fair: "text-amber-300",
  good: "text-sky-300",
  strong: "text-emerald-300",
};

const BADGE_HEIGHT = ["h-[3px]", "h-[5px]", "h-[7px]", "h-[9px]"];
const DETAIL_HEIGHT = ["h-1.5", "h-2.5", "h-3.5", "h-[18px]"];

export function StrengthBars({ id, level, size }: { id: string; level: Strength; size: "badge" | "detail" }) {
  const heights = size === "badge" ? BADGE_HEIGHT : DETAIL_HEIGHT;
  return (
    <span
      id={id}
      className={cn("pointer-events-none flex items-end gap-px", size === "badge" && "absolute -top-0.5 -right-0.5")}
      aria-hidden
    >
      {heights.map((height, index) => (
        <span
          id={`${id}-bar-${index + 1}`}
          key={height}
          className={cn(
            "rounded-[1px]",
            size === "badge" ? "w-0.5" : "w-1",
            height,
            index < level.bars ? TONE_BAR[level.tone] : "bg-zinc-700",
          )}
        />
      ))}
    </span>
  );
}

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
          <StrengthBars id="address-network-speed-badge" level={level} size="badge" />
        </>
      }
    />
  );
}
