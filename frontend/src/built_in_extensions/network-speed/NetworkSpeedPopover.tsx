import { Gauge } from "lucide-react";
import { PopupFrame } from "@/components/PopupFrame";
import { copyText, showNotice } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { PopoverHeader } from "../PopoverParts";

import {
  externalIpHint,
  formatBits,
  formatRate,
  ipAddressHint,
  jitterHint,
  kindLabel,
  latencyHint,
  linkHint,
  pingHint,
  Strength,
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

function copyIpAddress(address: string) {
  void copyText(address)
    .then(() => showNotice(`Copied ${address} to the clipboard.`))
    .catch(() => undefined);
}

const BADGE_HEIGHT = ["h-[3px]", "h-[5px]", "h-[7px]", "h-[9px]"];
const DETAIL_HEIGHT = ["h-1.5", "h-2.5", "h-3.5", "h-[18px]"];


const TONE_BAR: Record<StrengthTone, string> = {
  off: "bg-zinc-600",
  weak: "bg-red-400",
  fair: "bg-amber-400",
  good: "bg-sky-400",
  strong: "bg-emerald-400",
};


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


export function NetworkSpeedPopover() {
  const speed = useNetworkSpeed();
  const level = strength(speed);
  const subtitle = speed.online ? speed.name || kindLabel(speed.kind) : "Offline";
  const address = speed.ipAddress;
  const external = speed.externalIp;

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

        <Row
          id="network-speed-ping"
          label="Ping"
          value={speed.pingMs == null ? "—" : `${speed.pingMs} ms`}
          hint={pingHint(speed.pingMs)}
        />
        <Row
          id="network-speed-jitter"
          label="Jitter"
          value={speed.jitterMs == null ? "—" : `${speed.jitterMs} ms`}
          hint={jitterHint(speed.jitterMs)}
        />
        <Row
          id="network-speed-local-ip"
          label="Local IP"
          value={address ?? "—"}
          hint={ipAddressHint(address)}
          onActivate={address ? () => copyIpAddress(address) : undefined}
        />
        <Row
          id="network-speed-external-ip"
          label="External IP"
          value={external ?? "—"}
          hint={externalIpHint(external)}
          onActivate={external ? () => copyIpAddress(external) : undefined}
        />

      </div>
    </PopupFrame>
  );
}

function Row({
  id,
  label,
  value,
  hint,
  onActivate,
}: {
  id: string;
  label: string;
  value: string;
  hint?: string;
  onActivate?: () => void;
}) {
  return (
    <div id={id} className="flex items-baseline justify-between gap-3 py-1">
      <span id={`${id}-label`} className="text-xs text-zinc-400">
        {label}
      </span>
      <span
        id={`${id}-value`}
        className={cn(
          "truncate text-xs font-medium tabular-nums text-zinc-100",
          onActivate ? "cursor-pointer" : "cursor-help",
        )}
        title={hint}
        onClick={onActivate}
        onContextMenu={
          onActivate
            ? (event) => {
                event.preventDefault();
                onActivate();
              }
            : undefined
        }
      >
        {value}
      </span>
    </div>
  );
}
