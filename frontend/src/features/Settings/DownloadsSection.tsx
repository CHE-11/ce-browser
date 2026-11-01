import { useEffect, useState } from "react";
import { FolderOpen, RotateCcw } from "lucide-react";
import { Button } from "@/components/Button";
import { Toggle } from "@/components/Toggle";
import { chooseFolder, defaultDownloadDir } from "@/lib/browser-client";
import { SETTINGS_SECTIONS } from "./settings-sections";
import { matchesQuery, Row, Section, useReportMatch, type SettingsSectionProps } from "./SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-downloads")!;

const LOCATION_LABEL = "Location";
const ASK_LABEL = "Ask where to save each file";
const ACCEL_LABEL = "Accelerate large downloads";
const ACCEL_HINT = "Files of 8 MB or more use 4 connections when the server allows it.";
const NAME_LABEL = "File name";
const NAME_HINT = "Empty keeps the site's name. Tokens: {site} {date} {name} {ext}";

export function DownloadsSection({ settings, query, onUpdate, onMatch }: SettingsSectionProps) {
  const [systemDownloads, setSystemDownloads] = useState("");

  useEffect(() => {
    void defaultDownloadDir().then(setSystemDownloads).catch(() => undefined);
  }, []);

  const locationHint = settings.downloadDir || systemDownloads || "System Downloads folder";
  const visible = matchesQuery(
    query,
    section.title,
    LOCATION_LABEL,
    locationHint,
    settings.downloadDir ? "Default" : "",
    "Change",
    ASK_LABEL,
    ACCEL_LABEL,
    ACCEL_HINT,
    NAME_LABEL,
    NAME_HINT,
    settings.downloadNameTemplate,
    "{site}-{date}-{name}",
  );
  useReportMatch(section.id, visible, onMatch);

  if (!visible) {
    return null;
  }

  return (
    <Section id={section.id} title={section.title}>
      <Row id="settings-download-dir" setting="downloadDir" label={LOCATION_LABEL} hint={locationHint} search={[settings.downloadDir ? "Default" : "", "Change"]}>
        {settings.downloadDir ? (
          <Button id="settings-download-dir-reset" size="sm" variant="ghost" onClick={() => onUpdate({ downloadDir: "" })}>
            <RotateCcw id="settings-download-dir-reset-icon" className="size-3.5" aria-hidden />
            Default
          </Button>
        ) : null}
        <Button
          id="settings-download-dir-change"
          size="sm"
          onClick={() =>
            void chooseFolder("Choose the download folder", settings.downloadDir || systemDownloads).then(
              (folder) => folder && onUpdate({ downloadDir: folder }),
            )
          }
        >
          <FolderOpen id="settings-download-dir-change-icon" className="size-3.5" aria-hidden />
          Change
        </Button>
      </Row>
      <Row id="settings-ask-save" setting="askWhereToSave" label={ASK_LABEL}>
        <Toggle id="settings-ask-save-toggle" checked={settings.askWhereToSave} onChange={(value) => onUpdate({ askWhereToSave: value })} />
      </Row>
      <Row id="settings-accelerate" setting="accelerateDownloads" label={ACCEL_LABEL} hint={ACCEL_HINT}>
        <Toggle
          id="settings-accelerate-toggle"
          checked={settings.accelerateDownloads}
          onChange={(value) => onUpdate({ accelerateDownloads: value })}
        />
      </Row>
      <Row id="settings-download-name" setting="downloadNameTemplate" label={NAME_LABEL} hint={NAME_HINT} search={[settings.downloadNameTemplate, "{site}-{date}-{name}"]}>
        <input
          id="settings-download-name-input"
          defaultValue={settings.downloadNameTemplate}
          key={settings.downloadNameTemplate}
          placeholder="{site}-{date}-{name}"
          spellCheck={false}
          className="h-8 w-56 rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm outline-none focus:border-sky-500"
          onBlur={(event) => {
            const next = event.target.value.trim();
            if (next !== settings.downloadNameTemplate) {
              onUpdate({ downloadNameTemplate: next });
            }
          }}
        />
      </Row>
    </Section>
  );
}
