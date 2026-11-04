import { Button } from "@/components/Button";
import { Toggle } from "@/components/Toggle";
import { openDefaultApps } from "@/lib/browser-client";
import { SETTINGS_SECTIONS } from "../settings-sections";
import { inputClass, matchesQuery, Row, Section, useReportMatch, type SettingsSectionProps } from "../SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-system")!;

const HOTKEY_LABEL = "Summon shortcut";
const HOTKEY_HINT = "Restores the window and focuses the address bar. Default is ctrl+alt+space.";
const DEFAULT_LABEL = "Default browser";
const DEFAULT_HINT = "Registers ce-browser for http, https and HTML, then opens Windows settings to confirm.";
const BATTERY_LABEL = "Battery saver";
const BATTERY_HINT = "On battery, background tabs sleep sooner and the visible page stays near 30 frames a second.";

export function SystemSection({ settings, query, onUpdate, onMatch }: SettingsSectionProps) {
  const visible = matchesQuery(
    query,
    section.title,
    HOTKEY_LABEL,
    HOTKEY_HINT,
    DEFAULT_LABEL,
    DEFAULT_HINT,
    BATTERY_LABEL,
    BATTERY_HINT,
  );
  useReportMatch(section.id, visible, onMatch);
  if (!visible) {
    return null;
  }

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
    </Section>
  );
}
