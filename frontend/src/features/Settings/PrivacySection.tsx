import { useState } from "react";
import { X } from "lucide-react";
import type { BrowserSettings } from "@type/browser";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { Toggle } from "@/components/Toggle";
import { SETTINGS_SECTIONS } from "./settings-sections";
import { inputClass, matchesQuery, Row, Section, useReportMatch, useSettingVisible, type SettingsSectionProps } from "./SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-privacy")!;

const TRACKING_LABEL = "Strip tracking parameters";
const TRACKING_HINT = "Removes utm, fbclid, gclid and similar from addresses and copied links.";
const FINGERPRINT_LABEL = "Fingerprint shield";
const FINGERPRINT_HINT = "Adds stable noise to canvas, WebGL and audio reads. The noise resets when the browser exits.";
const HTTPS_LABEL = "HTTPS-only";
const HTTPS_HINT = "http pages load over https. Localhost is left alone.";
const EXCEPTIONS_LABEL = "Sites that may use http";
const EXCEPTIONS_EMPTY = "No exceptions.";
const INDEX_LABEL = "Index page text";
const INDEX_HINT = "Saves page text on this computer so history search can match words from the page.";

export function PrivacySection({ settings, query, onUpdate, onMatch }: SettingsSectionProps) {
  const [httpsDraft, setHttpsDraft] = useState("");
  const visible = matchesQuery(
    query,
    section.title,
    TRACKING_LABEL,
    TRACKING_HINT,
    FINGERPRINT_LABEL,
    FINGERPRINT_HINT,
    HTTPS_LABEL,
    HTTPS_HINT,
    EXCEPTIONS_LABEL,
    EXCEPTIONS_EMPTY,
    INDEX_LABEL,
    INDEX_HINT,
    "Add",
    httpsDraft,
    ...settings.httpsExceptions,
  );
  useReportMatch(section.id, visible, onMatch);

  if (!visible) {
    return null;
  }

  return (
    <Section id={section.id} title={section.title}>
      <Row id="settings-strip-tracking" setting="stripTracking" label={TRACKING_LABEL} hint={TRACKING_HINT}>
        <Toggle id="settings-strip-tracking-toggle" checked={settings.stripTracking} onChange={(stripTracking) => onUpdate({ stripTracking })} />
      </Row>
      <Row id="settings-fingerprint" setting="fingerprintShield" label={FINGERPRINT_LABEL} hint={FINGERPRINT_HINT}>
        <Toggle
          id="settings-fingerprint-toggle"
          checked={settings.fingerprintShield}
          onChange={(fingerprintShield) => onUpdate({ fingerprintShield })}
        />
      </Row>
      <Row id="settings-https-only" setting="httpsOnly" label={HTTPS_LABEL} hint={HTTPS_HINT}>
        <Toggle id="settings-https-only-toggle" checked={settings.httpsOnly} onChange={(httpsOnly) => onUpdate({ httpsOnly })} />
      </Row>
      <Row id="settings-index-page-text" setting="indexPageText" label={INDEX_LABEL} hint={INDEX_HINT}>
        <Toggle
          id="settings-index-page-text-toggle"
          checked={settings.indexPageText}
          onChange={(indexPageText) => onUpdate({ indexPageText })}
        />
      </Row>
      <HttpsExceptions settings={settings} draft={httpsDraft} setDraft={setHttpsDraft} onUpdate={onUpdate} />
    </Section>
  );
}

function HttpsExceptions({
  settings,
  draft,
  setDraft,
  onUpdate,
}: {
  settings: BrowserSettings;
  draft: string;
  setDraft: (value: string) => void;
  onUpdate: SettingsSectionProps["onUpdate"];
}) {
  const sites = settings.httpsExceptions;
  const visible = useSettingVisible(EXCEPTIONS_LABEL, EXCEPTIONS_EMPTY, "Add", draft, ...sites);
  if (!visible) {
    return null;
  }
  const add = () => {
    const site = draft.trim();
    if (!site) {
      return;
    }
    onUpdate({ httpsExceptions: [...sites, site] });
    setDraft("");
  };
  return (
    <div id="settings-https-exceptions" className="px-5 py-3">
      <div id="settings-https-exceptions-label" className="text-sm text-zinc-100">
        {EXCEPTIONS_LABEL}
      </div>
      <div id="settings-https-exceptions-add" className="mb-2 mt-2 flex gap-2">
        <input
          id="settings-https-exceptions-input"
          value={draft}
          placeholder="example.com"
          className={inputClass}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && draft.trim()) {
              event.preventDefault();
              add();
            }
          }}
        />
        <Button id="settings-https-exceptions-add-button" size="sm" disabled={!draft.trim()} onClick={add}>
          Add
        </Button>
      </div>
      {sites.length === 0 ? (
        <div id="settings-https-exceptions-empty" className="text-xs text-zinc-500">
          {EXCEPTIONS_EMPTY}
        </div>
      ) : (
        <ul id="settings-https-exceptions-list" className="flex flex-col gap-1">
          {sites.map((site) => (
            <li
              id={`settings-https-exception-${site}`}
              key={site}
              className="flex h-8 items-center justify-between rounded-md bg-zinc-950 pl-3 pr-1 text-sm"
            >
              <span id={`settings-https-exception-${site}-host`} className="truncate">
                {site}
              </span>
              <IconButton
                id={`settings-https-exception-${site}-remove`}
                label={`Require https on ${site}`}
                onClick={() => onUpdate({ httpsExceptions: sites.filter((item) => item !== site) })}
              >
                <X id={`settings-https-exception-${site}-remove-icon`} aria-hidden />
              </IconButton>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
