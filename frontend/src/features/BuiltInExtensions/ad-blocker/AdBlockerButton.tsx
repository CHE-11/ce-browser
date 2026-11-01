import { useRef } from "react";
import { ShieldCheck, ShieldOff } from "lucide-react";

import { tabParams } from "../shared";
import { sitePaused, useAdblockStats } from "./use-adblock-stats";
import { IconButton } from "@/components/IconButton";
import { openPopupBelow } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { TabInfo, BrowserSettings } from "@type/browser";

type AdBlockerButtonProps = {
  activeTab: TabInfo | undefined;
  settings: BrowserSettings | null;
};

const POPOVER_WIDTH = 320;
/** Placeholder only. The popover stays hidden until it reports its real height. */
const POPOVER_HEIGHT = 260;

/** Host name of an http(s) page, without a port. Other pages load no ads. */
export function pageHostname(url: string | undefined) {
  if (!url) {
    return null;
  }
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return null;
    }
    return parsed.hostname || null;
  } catch {
    return null;
  }
}

function badgeText(count: number) {
  return count > 99 ? "99+" : String(count);
}

export function AdBlockerButton({ activeTab, settings }: AdBlockerButtonProps) {
  const buttonRef = useRef<HTMLButtonElement>(null);
  const host = pageHostname(activeTab?.url);
  const blocked = useAdblockStats(activeTab?.id ?? null);
  const enabled = settings?.adblock?.enabled ?? true;
  const paused = host !== null && sitePaused(settings?.adblock?.allowedSites ?? [], host);
  const active = enabled && !paused;

  const label = !enabled
    ? "Ad blocker is off"
    : paused
      ? `Ad blocker paused on ${host}`
      : host
        ? `${blocked} ${blocked === 1 ? "ad" : "ads"} blocked on ${host}`
        : "Ad blocker";

  const open = () => {
    const anchor = buttonRef.current;
    if (!activeTab || !anchor || !host) {
      return;
    }
    const params = new URLSearchParams(tabParams(activeTab));
    params.set("host", host);
    void openPopupBelow(anchor, {
      kind: "ad-blocker",
      params: params.toString(),
      width: POPOVER_WIDTH,
      height: POPOVER_HEIGHT,
    });
  };

  return (
    <IconButton
      id="address-ad-blocker"
      ref={buttonRef}
      label={label}
      disabled={host === null}
      className={cn("relative", active && blocked > 0 && "text-sky-300")}
      onClick={open}
    >
      {active ? (
        <ShieldCheck id="address-ad-blocker-icon" aria-hidden />
      ) : (
        <ShieldOff id="address-ad-blocker-icon" aria-hidden />
      )}
      {active && host && blocked > 0 ? (
        <span
          id="address-ad-blocker-badge"
          className="pointer-events-none absolute -right-0.5 -top-0.5 min-w-4 rounded-full bg-sky-600 px-1 text-center text-[9px] font-semibold leading-4 text-white"
          aria-hidden
        >
          {badgeText(blocked)}
        </span>
      ) : null}
    </IconButton>
  );
}
