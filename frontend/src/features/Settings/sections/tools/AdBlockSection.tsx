import { X } from "lucide-react";
import type { BrowserSettings } from "@type/browser";
import { IconButton } from "@/components/IconButton";
import { Toggle } from "@/components/Toggle";
import { SETTINGS_SECTIONS } from "../../settings-sections";
import { matchesQuery, Row, Section, useReportMatch, useSettingVisible, type SettingsSectionProps } from "../../SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-adblock")!;

const ENABLED_LABEL = "Block ads and trackers";
const ENABLED_HINT = "Open pages pick up the change when they next load.";
const PAUSED_LABEL = "Paused sites";
const PAUSED_HINT = "Ads are allowed on these sites and their subdomains. Pause a site from the shield on the address bar.";
const PAUSED_EMPTY = "No paused sites.";

export function AdBlockSection({ settings, query, onUpdate, onMatch }: SettingsSectionProps) {
  const sites = settings.adblock.allowedSites;
  const visible = matchesQuery(query, section.title, ENABLED_LABEL, ENABLED_HINT, PAUSED_LABEL, PAUSED_HINT, PAUSED_EMPTY, ...sites);
  useReportMatch(section.id, visible, onMatch);

  if (!visible) {
    return null;
  }

  return (
    <Section id={section.id} title={section.title}>
      <Row id="settings-adblock-enabled" setting="adblock.enabled" label={ENABLED_LABEL} hint={ENABLED_HINT}>
        <Toggle
          id="settings-adblock-enabled-toggle"
          checked={settings.adblock.enabled}
          onChange={(enabled) => onUpdate({ adblock: { ...settings.adblock, enabled } })}
        />
      </Row>
      <PausedSites settings={settings} onUpdate={onUpdate} />
    </Section>
  );
}

function PausedSites({ settings, onUpdate }: { settings: BrowserSettings; onUpdate: SettingsSectionProps["onUpdate"] }) {
  const sites = settings.adblock.allowedSites;
  const visible = useSettingVisible(PAUSED_LABEL, PAUSED_HINT, PAUSED_EMPTY, ...sites);
  if (!visible) {
    return null;
  }
  return (
    <div id="settings-adblock-paused" className="px-5 py-3">
      <div id="settings-adblock-paused-label" className="text-sm text-zinc-100">
        {PAUSED_LABEL}
      </div>
      <div id="settings-adblock-paused-hint" className="mb-2 text-xs text-zinc-500">
        {PAUSED_HINT}
      </div>
      {sites.length === 0 ? (
        <div id="settings-adblock-paused-empty" className="text-xs text-zinc-500">
          {PAUSED_EMPTY}
        </div>
      ) : (
        <ul id="settings-adblock-paused-list" className="flex flex-col gap-1">
          {sites.map((site) => (
            <li
              id={`settings-adblock-paused-${site}`}
              key={site}
              className="flex h-8 items-center justify-between rounded-md bg-zinc-950 pl-3 pr-1 text-sm text-zinc-200"
            >
              <span id={`settings-adblock-paused-${site}-host`} className="truncate">
                {site}
              </span>
              <IconButton
                id={`settings-adblock-paused-${site}-remove`}
                label={`Block ads on ${site} again`}
                onClick={() =>
                  onUpdate({
                    adblock: {
                      ...settings.adblock,
                      allowedSites: sites.filter((item) => item !== site),
                    },
                  })
                }
              >
                <X id={`settings-adblock-paused-${site}-remove-icon`} aria-hidden />
              </IconButton>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
