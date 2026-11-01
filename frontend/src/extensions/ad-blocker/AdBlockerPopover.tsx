import { useState } from "react";
import { ShieldCheck } from "lucide-react";
import type { AdblockSettings } from "../../../../types/browser";
import { PopupFrame } from "../../components/PopupFrame";
import { reload, saveSettings } from "../../lib/browser-client";
import { cn } from "../../lib/cn";
import { useSettings } from "../../lib/use-browser";
import { OptionRow, PopoverHeader } from "../PopoverParts";
import { readPopoverTab } from "../shared";
import { siteKey, sitePaused, useAdblockStats } from "./use-adblock-stats";

type AdBlockerPopoverProps = {
  params: URLSearchParams;
};

/** Paused-sites list with `host` added, or with `host` and every parent domain removed. */
function withSitePaused(allowedSites: string[], host: string, pause: boolean) {
  const key = siteKey(host);
  if (pause) {
    return allowedSites.includes(key) ? allowedSites : [...allowedSites, key];
  }
  return allowedSites.filter((site) => key !== site && !key.endsWith(`.${site}`));
}

export function AdBlockerPopover({ params }: AdBlockerPopoverProps) {
  const tab = readPopoverTab(params);
  const host = params.get("host") ?? "";
  const settings = useSettings();
  const blocked = useAdblockStats(tab.id);
  const [saving, setSaving] = useState(false);

  const adblock = settings?.adblock ?? { enabled: true, allowedSites: [] };
  const paused = sitePaused(adblock.allowedSites, host);
  const active = adblock.enabled && !paused;

  const apply = async (next: AdblockSettings) => {
    if (!settings || saving) {
      return;
    }
    setSaving(true);
    try {
      await saveSettings({ ...settings, adblock: next });
      await reload(tab.id);
    } catch {
      // The toggles snap back to the stored settings on the next settings event.
    } finally {
      setSaving(false);
    }
  };

  return (
    <PopupFrame id="ad-blocker-popover" maxHeight={420}>
      <PopoverHeader
        id="ad-blocker-header"
        icon={<ShieldCheck aria-hidden />}
        title="Ad blocker"
        subtitle={host || tab.title || tab.url}
      />
      <div id="ad-blocker-body" className="flex flex-col gap-2 px-3 pb-3">
        <div
          id="ad-blocker-count"
          className={cn("rounded-lg p-3", active ? "bg-sky-500/10 text-sky-100" : "bg-zinc-800/60 text-zinc-400")}
        >
          <p id="ad-blocker-count-number" className="text-2xl font-semibold tabular-nums">
            {active ? blocked : "–"}
          </p>
          <p id="ad-blocker-count-label" className="text-xs opacity-80">
            {!adblock.enabled
              ? "Blocking is off everywhere"
              : paused
                ? "Blocking is paused on this site"
                : `${blocked === 1 ? "Ad or tracker" : "Ads and trackers"} blocked on this page`}
          </p>
        </div>
        <OptionRow
          id="ad-blocker-enabled"
          label="Block ads and trackers"
          hint="On every site"
          checked={adblock.enabled}
          disabled={!settings || saving}
          onChange={(enabled) => void apply({ ...adblock, enabled })}
        />
        <OptionRow
          id="ad-blocker-site"
          label={`Block on ${host}`}
          hint={paused ? "Paused on this site" : "Turn off to pause on this site"}
          checked={!paused}
          disabled={!settings || saving || !adblock.enabled || !host}
          onChange={(block) =>
            void apply({ ...adblock, allowedSites: withSitePaused(adblock.allowedSites, host, !block) })
          }
        />
        <p id="ad-blocker-note" className="px-2.5 text-[11px] text-zinc-500">
          Changes reload the page.
        </p>
      </div>
    </PopupFrame>
  );
}
