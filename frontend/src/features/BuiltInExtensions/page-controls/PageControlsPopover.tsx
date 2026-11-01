import { useEffect, useState } from "react";
import { SlidersHorizontal } from "lucide-react";

import { OptionRow, PopoverHeader } from "../PopoverParts";
import { readPopoverTab } from "../shared";
import { controlFor, siteKey } from "./site";
import { PopupFrame } from "@/components/PopupFrame";
import { cn } from "@/lib/cn";
import { focusRing, focusRow } from "@/lib/focus-ring";
import {
  closePopup,
  createTab,
  openPopupBelow,
  openReader,
  reload,
  saveSettings,
  setDevice,
  setTabRefresh,
  startInspector,
  startZapper,
  updateSiteControl,
} from "@/lib/browser-client";
import { usePageRules, useSettings, useTabs } from "@/lib/use-browser";
import type { ReferrerMode, SitePatch } from "@type/browser";

type PageControlsPopoverProps = {
  params: URLSearchParams;
};

const SPEEDS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4];
const REFRESH = [
  { secs: 0, label: "Off" },
  { secs: 15, label: "15s" },
  { secs: 30, label: "30s" },
  { secs: 60, label: "1m" },
  { secs: 300, label: "5m" },
  { secs: 900, label: "15m" },
];

const actionClass = cn("h-9 rounded-lg px-2.5 text-left text-sm hover:bg-zinc-800", focusRow);

const fieldClass =
  "h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500";

export function PageControlsPopover({ params }: PageControlsPopoverProps) {
  const tab = readPopoverTab(params);
  const host = params.get("host") || siteKey(tab.url);
  const rules = usePageRules();
  const settings = useSettings();
  const snapshot = useTabs();
  const live = snapshot.tabs.find((item) => item.id === tab.id);
  const windowLabel = live?.window || "main";
  const device = snapshot.windows.find((item) => item.label === windowLabel)?.device ?? "";
  const site = controlFor(rules, host);
  const [agent, setAgent] = useState(site.userAgent);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setAgent(site.userAgent);
  }, [site.userAgent]);

  const apply = (patch: SitePatch, reloadPage = false) => {
    setError(null);
    void updateSiteControl(host, patch)
      .then(() => {
        if (reloadPage) {
          return reload(tab.id);
        }
        return undefined;
      })
      .catch((reason: unknown) => setError(String(reason)));
  };

  return (
    <PopupFrame id="page-controls-popover" maxHeight={640} ready={rules !== null && snapshot.loaded}>
      <PopoverHeader id="page-controls-header" icon={<SlidersHorizontal aria-hidden />} title="Page controls" subtitle={host} />
      <div id="page-controls-body" className="flex flex-col gap-1 px-2 pb-3">
        <OptionRow
          id="page-controls-interaction"
          label="Allow copy, paste and selection"
          hint="Stops the site blocking the right-click menu too"
          checked={site.forceInteraction}
          onChange={(forceInteraction) => apply({ forceInteraction })}
        />
        <OptionRow
          id="page-controls-clean"
          label="Clean page"
          hint="Hides sticky bars, cookie notices and subscribe popups"
          checked={site.cleanPage}
          onChange={(cleanPage) => apply({ cleanPage })}
        />
        <OptionRow
          id="page-controls-dark"
          label="Force dark mode"
          checked={site.dark.enabled}
          onChange={(darkEnabled) => apply({ darkEnabled })}
        />
        {site.dark.enabled ? (
          <div id="page-controls-dark-sliders" className="flex flex-col gap-2 px-2.5 pb-1">
            <label id="page-controls-brightness" className="flex items-center gap-2 text-xs text-zinc-400">
              <span id="page-controls-brightness-label" className="w-16 shrink-0">
                Brightness
              </span>
              <input
                id="page-controls-brightness-input"
                type="range"
                min={50}
                max={150}
                step={5}
                value={site.dark.brightness}
                className="min-w-0 flex-1"
                onChange={(event) => apply({ brightness: Number(event.target.value) })}
              />
              <span id="page-controls-brightness-value" className="w-8 text-right tabular-nums">
                {site.dark.brightness}
              </span>
            </label>
            <label id="page-controls-contrast" className="flex items-center gap-2 text-xs text-zinc-400">
              <span id="page-controls-contrast-label" className="w-16 shrink-0">
                Contrast
              </span>
              <input
                id="page-controls-contrast-input"
                type="range"
                min={50}
                max={150}
                step={5}
                value={site.dark.contrast}
                className="min-w-0 flex-1"
                onChange={(event) => apply({ contrast: Number(event.target.value) })}
              />
              <span id="page-controls-contrast-value" className="w-8 text-right tabular-nums">
                {site.dark.contrast}
              </span>
            </label>
          </div>
        ) : null}
        <label id="page-controls-speed" className="flex items-center gap-3 px-2.5 py-1.5 text-sm">
          <span id="page-controls-speed-label" className="min-w-0 flex-1">
            Video speed
            <span id="page-controls-speed-hint" className="block text-[11px] text-zinc-500">
              [ and ] change it while a video plays
            </span>
          </span>
          <select
            id="page-controls-speed-select"
            className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm"
            value={site.videoSpeed}
            onChange={(event) => apply({ videoSpeed: Number(event.target.value) })}
          >
            {SPEEDS.map((speed) => (
              <option id={`page-controls-speed-${speed}`} key={speed} value={speed}>
                {speed === 1 ? "Normal" : `${speed}x`}
              </option>
            ))}
          </select>
        </label>
        <label id="page-controls-agent" className="flex flex-col gap-1 px-2.5 py-1.5">
          <span id="page-controls-agent-label" className="text-sm">
            User agent
          </span>
          <input
            id="page-controls-agent-input"
            className={fieldClass}
            value={agent}
            placeholder="Browser default"
            spellCheck={false}
            onChange={(event) => setAgent(event.target.value)}
            onBlur={() => {
              if (agent.trim() !== site.userAgent) {
                apply({ userAgent: agent.trim() }, true);
              }
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.currentTarget.blur();
              }
            }}
          />
        </label>
        <label id="page-controls-referrer" className="flex items-center gap-3 px-2.5 py-1.5 text-sm">
          <span id="page-controls-referrer-label" className="min-w-0 flex-1">
            Referrer
          </span>
          <select
            id="page-controls-referrer-select"
            className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm"
            value={site.referrer}
            onChange={(event) => apply({ referrer: event.target.value as ReferrerMode })}
          >
            <option id="page-controls-referrer-default" value="default">
              Browser default
            </option>
            <option id="page-controls-referrer-none" value="none">
              Never send
            </option>
            <option id="page-controls-referrer-origin" value="origin">
              Site only
            </option>
            <option id="page-controls-referrer-same" value="same-origin">
              Same site only
            </option>
          </select>
        </label>
        <OptionRow
          id="page-controls-https"
          label="Allow http on this site"
          hint={settings?.httpsOnly ? "HTTPS-only is on" : "Turn on HTTPS-only in Settings"}
          checked={settings?.httpsExceptions.includes(host) ?? false}
          disabled={!settings?.httpsOnly}
          onChange={(allow) => {
            if (!settings) {
              return;
            }
            const httpsExceptions = allow
              ? [...settings.httpsExceptions, host]
              : settings.httpsExceptions.filter((item) => item !== host);
            void saveSettings({ ...settings, httpsExceptions });
          }}
        />
        <OptionRow
          id="page-controls-block-script"
          label="Block JavaScript"
          hint="Reloads this page"
          checked={site.blockScript}
          onChange={(blockScript) => apply({ blockScript }, true)}
        />
        <OptionRow
          id="page-controls-block-images"
          label="Block images"
          hint="Reloads this page"
          checked={site.blockImages}
          onChange={(blockImages) => apply({ blockImages }, true)}
        />
        <OptionRow
          id="page-controls-block-fonts"
          label="Block fonts"
          hint="Reloads this page"
          checked={site.blockFonts}
          onChange={(blockFonts) => apply({ blockFonts }, true)}
        />
        <label id="page-controls-refresh" className="flex items-center gap-3 px-2.5 py-1.5 text-sm">
          <span id="page-controls-refresh-label" className="min-w-0 flex-1">
            Auto-refresh this tab
          </span>
          <select
            id="page-controls-refresh-select"
            className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm"
            value={live?.refreshSecs ?? 0}
            onChange={(event) => void setTabRefresh(tab.id, Number(event.target.value), live?.watchChanges ?? false)}
          >
            {REFRESH.map((choice) => (
              <option id={`page-controls-refresh-${choice.secs}`} key={choice.secs} value={choice.secs}>
                {choice.label}
              </option>
            ))}
          </select>
        </label>
        <OptionRow
          id="page-controls-watch"
          label="Notify when the page changes"
          hint="A dot appears on the tab"
          checked={live?.watchChanges ?? false}
          disabled={(live?.refreshSecs ?? 0) === 0}
          onChange={(watch) => void setTabRefresh(tab.id, live?.refreshSecs ?? 0, watch)}
        />
        <div id="page-controls-device" className="flex gap-1 px-1 py-1">
          {(
            [
              ["", "Off"],
              ["phone", "Phone"],
              ["tablet", "Tablet"],
              ["laptop", "Laptop"],
            ] as const
          ).map(([value, label]) => (
            <button
              id={`page-controls-device-${value || "off"}`}
              key={value || "off"}
              type="button"
              aria-pressed={device === value}
              className={cn("h-7 flex-1 rounded-md text-xs", focusRing, device === value ? "bg-sky-600 text-white" : "bg-zinc-800 text-zinc-300")}
              onClick={() => void setDevice(windowLabel, value)}
            >
              {label}
            </button>
          ))}
        </div>
        <div id="page-controls-actions" className="flex flex-col gap-1 px-1 pt-1">
          <button
            id="page-controls-reader"
            type="button"
            className={actionClass}
            onClick={() => {
              void openReader(tab.id).finally(() => closePopup());
            }}
          >
            Reader mode
          </button>
          <button
            id="page-controls-zap"
            type="button"
            className={actionClass}
            onClick={() => {
              void startZapper(tab.id).finally(() => closePopup());
            }}
          >
            Pick an element to hide
            {site.hidden.length > 0 ? ` (${site.hidden.length})` : ""}
          </button>
          {site.hidden.length > 0 ? (
            <button
              id="page-controls-unhide"
              type="button"
              className={cn(actionClass, "text-zinc-400")}
              onClick={() => apply({ clearHidden: true })}
            >
              Show hidden elements again
            </button>
          ) : null}
          <button
            id="page-controls-network"
            type="button"
            className={actionClass}
            onClick={(event) =>
              void openPopupBelow(event.currentTarget, {
                kind: "network",
                params: `id=${tab.id}`,
                width: 440,
                height: 480,
              })
            }
          >
            Network log
          </button>
          <button
            id="page-controls-permissions"
            type="button"
            className={actionClass}
            onClick={() => {
              void createTab("ce://permissions").finally(() => closePopup());
            }}
          >
            Site permissions
          </button>
          <button
            id="page-controls-inspect"
            type="button"
            className={actionClass}
            onClick={() => {
              void startInspector(tab.id).finally(() => closePopup());
            }}
          >
            Color and font
          </button>
          <button
            id="page-controls-storage"
            type="button"
            className={actionClass}
            onClick={() => {
              void createTab(`ce://storage?tab=${tab.id}`).finally(() => closePopup());
            }}
          >
            Cookies and storage
          </button>
          <button
            id="page-controls-rewrites"
            type="button"
            className={actionClass}
            onClick={() => {
              void createTab("ce://rewrites").finally(() => closePopup());
            }}
          >
            Request rules
          </button>
          <button
            id="page-controls-scripts"
            type="button"
            className={actionClass}
            onClick={() => {
              void createTab(`ce://scripts?host=${encodeURIComponent(host)}`).finally(() => closePopup());
            }}
          >
            Site scripts
          </button>
        </div>
        {error ? (
          <p id="page-controls-error" className="px-2.5 text-xs text-red-300">
            {error}
          </p>
        ) : null}
      </div>
    </PopupFrame>
  );
}
