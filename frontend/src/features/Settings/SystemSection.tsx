import { useState } from "react";
import { Button } from "@/components/Button";
import { Toggle } from "@/components/Toggle";
import { checkForUpdate, createTab, installUpdate, openDefaultApps } from "@/lib/browser-client";
import { relativeTime } from "@/lib/time";
import { useUpdateStatus } from "@/lib/use-browser";
import { SETTINGS_SECTIONS } from "./settings-sections";
import { inputClass, matchesQuery, Row, Section, useReportMatch, type SettingsSectionProps } from "./SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-system")!;

const HOTKEY_LABEL = "Summon shortcut";
const HOTKEY_HINT = "Restores the window and focuses the address bar. Default is ctrl+alt+space.";
const DEFAULT_LABEL = "Default browser";
const DEFAULT_HINT = "Registers ce-browser for http, https and HTML, then opens Windows settings to confirm.";
const BATTERY_LABEL = "Battery saver";
const BATTERY_HINT = "On battery, background tabs sleep sooner and the visible page stays near 30 frames a second.";
const AUTO_UPDATE_LABEL = "Update automatically";
const AUTO_UPDATE_HINT = "Checks for a new version when ce-browser starts and once a day, then asks before installing.";
const VERSION_LABEL = "Version";

export function SystemSection({ settings, query, onUpdate, onMatch }: SettingsSectionProps) {
  const update = useUpdateStatus();
  const [error, setError] = useState<string | null>(null);
  const visible = matchesQuery(
    query,
    section.title,
    HOTKEY_LABEL,
    HOTKEY_HINT,
    DEFAULT_LABEL,
    DEFAULT_HINT,
    BATTERY_LABEL,
    BATTERY_HINT,
    AUTO_UPDATE_LABEL,
    AUTO_UPDATE_HINT,
    VERSION_LABEL,
    "check for updates",
    "what's new",
  );
  useReportMatch(section.id, visible, onMatch);
  if (!visible) {
    return null;
  }

  const versionHint = error
    ? error
    : update?.error
      ? update.error
      : update?.available
        ? `Version ${update.available} is ready to install.`
        : update?.checking
          ? "Checking for updates…"
          : update?.checkedAt
            ? `Up to date. Last checked ${relativeTime(new Date(update.checkedAt).toISOString())}.`
            : "Updates come from the ce-browser releases on GitHub.";

  const check = () => {
    setError(null);
    void checkForUpdate().catch((reason: unknown) => setError(String(reason)));
  };

  return (
    <Section id={section.id} title={section.title}>
      <Row id="settings-summon" setting="summonHotkey" label={HOTKEY_LABEL} hint={HOTKEY_HINT}>
        <input
          id="settings-summon-input"
          className={inputClass}
          value={settings.summonHotkey}
          spellCheck={false}
          onChange={(event) => onUpdate({ summonHotkey: event.target.value })}
        />
      </Row>
      <Row id="settings-default-browser" label={DEFAULT_LABEL} hint={DEFAULT_HINT}>
        <Button id="settings-default-browser-open" size="sm" onClick={() => void openDefaultApps()}>
          Make default
        </Button>
      </Row>
      <Row id="settings-battery" setting="batterySaver" label={BATTERY_LABEL} hint={BATTERY_HINT}>
        <Toggle id="settings-battery-toggle" checked={settings.batterySaver} onChange={(value) => onUpdate({ batterySaver: value })} />
      </Row>
      <Row id="settings-auto-update" setting="autoUpdate" label={AUTO_UPDATE_LABEL} hint={AUTO_UPDATE_HINT}>
        <Toggle id="settings-auto-update-toggle" checked={settings.autoUpdate} onChange={(autoUpdate) => onUpdate({ autoUpdate })} />
      </Row>
      <Row
        id="settings-version"
        label={update ? `${VERSION_LABEL} ${update.current}` : VERSION_LABEL}
        hint={versionHint}
        search={["check for updates", "what's new"]}
      >
        <Button
          id="settings-version-notes"
          size="sm"
          variant="ghost"
          onClick={() => void createTab(`ce://whats-new${update ? `?version=${encodeURIComponent(update.current)}` : ""}`)}
        >
          What's new
        </Button>
        {update?.available ? (
          <Button id="settings-version-install" size="sm" variant="primary" disabled={update.installing} onClick={() => void installUpdate().catch((reason: unknown) => setError(String(reason)))}>
            {update.installing ? "Downloading…" : "Restart to update"}
          </Button>
        ) : (
          <Button id="settings-version-check" size="sm" disabled={update?.checking} onClick={check}>
            Check for updates
          </Button>
        )}
      </Row>
    </Section>
  );
}
