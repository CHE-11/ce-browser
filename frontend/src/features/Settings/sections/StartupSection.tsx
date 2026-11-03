import { useEffect, useState } from "react";
import type { BrowserSettings } from "@type/browser";
import { SETTINGS_SECTIONS } from "../settings-sections";
import { inputClass, matchesQuery, Row, Section, useReportMatch, useSettingVisible, type SettingsSectionProps } from "../SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-startup")!;

const STARTUP_LABEL = "When ce-browser starts";
const RESTORE_LABEL = "Continue where you left off";
const HOME_OPTION_LABEL = "Open pinned tabs and the home page";
const HOME_LABEL = "Home page";
const HOME_HINT = "Leave empty for the bookmarks page.";
const PINNED_LABEL = "Pinned tabs";
const PINNED_HINT = "One URL per line. This profile pins them every time it starts, and the list is saved in this profile's settings.";

export function StartupSection({ settings, query, onUpdate, onMatch }: SettingsSectionProps) {
  const [homeDraft, setHomeDraft] = useState("");
  const [pinnedDraft, setPinnedDraft] = useState("");

  useEffect(() => {
    setHomeDraft(settings.homePage);
  }, [settings.homePage]);

  const pinnedText = settings.pinnedTabs.join("\n");

  useEffect(() => {
    setPinnedDraft(pinnedText);
  }, [pinnedText]);

  const visible = matchesQuery(
    query,
    section.title,
    STARTUP_LABEL,
    RESTORE_LABEL,
    HOME_OPTION_LABEL,
    HOME_LABEL,
    HOME_HINT,
    homeDraft,
    "ce://bookmarks",
    PINNED_LABEL,
    PINNED_HINT,
    pinnedDraft,
    ...settings.pinnedTabs,
  );
  useReportMatch(section.id, visible, onMatch);

  if (!visible) {
    return null;
  }

  return (
    <Section id={section.id} title={section.title}>
      <Row id="settings-startup-mode" setting="startup" label={STARTUP_LABEL} search={[RESTORE_LABEL, HOME_OPTION_LABEL]}>
        <select
          id="settings-startup-mode-select"
          value={settings.startup}
          className={inputClass}
          onChange={(event) => onUpdate({ startup: event.target.value as BrowserSettings["startup"] })}
        >
          <option id="settings-startup-restore" value="restore">
            {RESTORE_LABEL}
          </option>
          <option id="settings-startup-home" value="home">
            {HOME_OPTION_LABEL}
          </option>
        </select>
      </Row>
      <Row id="settings-home-page" setting="homePage" label={HOME_LABEL} hint={HOME_HINT} search={[homeDraft, "ce://bookmarks"]}>
        <input
          id="settings-home-page-input"
          value={homeDraft}
          spellCheck={false}
          placeholder="ce://bookmarks"
          className={inputClass}
          onChange={(event) => setHomeDraft(event.target.value)}
          onBlur={() => homeDraft !== settings.homePage && onUpdate({ homePage: homeDraft.trim() })}
          onKeyDown={(event) => event.key === "Enter" && event.currentTarget.blur()}
        />
      </Row>
      <PinnedTabs settings={settings} draft={pinnedDraft} setDraft={setPinnedDraft} onUpdate={onUpdate} />
    </Section>
  );
}

function PinnedTabs({
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
  const visible = useSettingVisible(PINNED_LABEL, PINNED_HINT, draft, ...settings.pinnedTabs);
  if (!visible) {
    return null;
  }
  return (
    <div id="settings-pinned-tabs" className="px-5 py-3">
      <div id="settings-pinned-tabs-label" className="text-sm text-zinc-100">
        {PINNED_LABEL}
      </div>
      <div id="settings-pinned-tabs-hint" className="mb-2 text-xs text-zinc-500">
        {PINNED_HINT}
      </div>
      <textarea
        id="settings-pinned-tabs-input"
        value={draft}
        rows={6}
        spellCheck={false}
        placeholder={"https://mail.example.com\nhttps://calendar.example.com"}
        className="min-h-28 w-full resize-y rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-sm text-zinc-100 outline-none focus:border-sky-500"
        onChange={(event) => setDraft(event.target.value)}
        onBlur={() => {
          const next = draft
            .split(/\r?\n/)
            .map((line) => line.trim())
            .filter((line) => line.length > 0);
          const current = settings.pinnedTabs;
          const same = next.length === current.length && next.every((url, index) => url === current[index]);
          if (!same) {
            onUpdate({ pinnedTabs: next });
          }
        }}
      />
    </div>
  );
}
