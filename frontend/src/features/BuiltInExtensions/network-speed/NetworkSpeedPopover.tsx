import { Gauge } from "lucide-react";
import { PopupFrame } from "@/components/PopupFrame";
import { cn } from "@/lib/cn";
import { PopoverHeader } from "../PopoverParts";
import { StrengthBars } from "./NetworkSpeedButton";
import {
  formatBits,
  formatRate,
  kindLabel,
  latencyHint,
  linkHint,
  strength,
  strengthHint,
  throughputHint,
  type StrengthTone,
} from "./format";
import { useNetworkSpeed } from "./use-network-speed";

const TONE_TEXT: Record<StrengthTone, string> = {
  off: "text-zinc-500",
  weak: "text-red-300",
  fair: "text-amber-300",
  good: "text-sky-300",
  strong: "text-emerald-300",
};

export function NetworkSpeedPopover() {
  const speed = useNetworkSpeed();
  const level = strength(speed);
  const subtitle = speed.online ? speed.name || kindLabel(speed.kind) : "Offline";

  return (
    <PopupFrame id="network-speed-popover" width={300} maxHeight={360}>
      <PopoverHeader
        id="network-speed-header"
        icon={<Gauge id="network-speed-header-icon" aria-hidden />}
        title="Network speed"
        subtitle={subtitle}
      />
      <div id="network-speed-body" className="flex flex-col px-4 pb-3">
        <Row id="network-speed-type" label="Type" value={kindLabel(speed.kind)} />
        <div id="network-speed-strength" className="flex items-center justify-between gap-3 py-1">
          <span id="network-speed-strength-label" className="text-xs text-zinc-400">
            Strength
          </span>
          <span
            id="network-speed-strength-value"
            className="flex cursor-help items-center gap-2"
            title={strengthHint(speed)}
          >
            <StrengthBars id="network-speed-strength-bars" level={level} size="detail" />
            <span id="network-speed-strength-text" className={cn("text-xs font-medium", TONE_TEXT[level.tone])}>
              {level.label}
              {speed.signal != null ? ` · ${speed.signal}%` : ""}
            </span>
          </span>
        </div>
        <Row
          id="network-speed-down"
          label="Download"
          value={formatRate(speed.downBps)}
          hint={throughputHint(speed.downBps, speed.linkBps, "down")}
        />
        <Row
          id="network-speed-up"
          label="Upload"
          value={formatRate(speed.upBps)}
          hint={throughputHint(speed.upBps, speed.linkBps, "up")}
        />
        <Row
          id="network-speed-link"
          label="Link"
          value={speed.linkBps > 0 ? formatBits(speed.linkBps) : "—"}
          hint={linkHint(speed.linkBps)}
        />
        <Row
          id="network-speed-latency"
          label="Latency"
          value={speed.rttMs == null ? "—" : `${speed.rttMs} ms`}
          hint={latencyHint(speed.rttMs)}
        />
      </div>
    </PopupFrame>
  );
}

function Row({ id, label, value, hint }: { id: string; label: string; value: string; hint?: string }) {
  return (
    <div id={id} className="flex items-baseline justify-between gap-3 py-1">
      <span id={`${id}-label`} className="text-xs text-zinc-400">
        {label}
      </span>
      <span id={`${id}-value`} className="cursor-help truncate text-xs font-medium tabular-nums text-zinc-100" title={hint}>
        {value}
      </span>
    </div>
  );
}
