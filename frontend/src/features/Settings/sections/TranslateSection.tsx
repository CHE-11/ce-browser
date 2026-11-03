import { SETTINGS_SECTIONS } from "../settings-sections";
import { inputClass, matchesQuery, Row, Section, useReportMatch, type SettingsSectionProps } from "../SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-translate")!;

const LANG_LABEL = "Language";
const LANG_HINT = "Language code, for example en or es.";
const URL_LABEL = "Provider";
const URL_HINT = "%s is the text and %t is the language.";

export function TranslateSection({ settings, query, onUpdate, onMatch }: SettingsSectionProps) {
  const visible = matchesQuery(query, section.title, "tools", LANG_LABEL, LANG_HINT, URL_LABEL, URL_HINT, "translate");
  useReportMatch(section.id, visible, onMatch);

  if (!visible) {
    return null;
  }

  return (
    <Section id={section.id} title={section.title}>
      <Row id="settings-translate-language" setting="translateLanguage" label={LANG_LABEL} hint={LANG_HINT}>
        <input
          id="settings-translate-language-input"
          className={inputClass}
          value={settings.translateLanguage}
          onChange={(event) => onUpdate({ translateLanguage: event.target.value })}
        />
      </Row>
      <Row id="settings-translate-url" setting="translateUrl" label={URL_LABEL} hint={URL_HINT}>
        <input
          id="settings-translate-url-input"
          className={inputClass}
          value={settings.translateUrl}
          spellCheck={false}
          onChange={(event) => onUpdate({ translateUrl: event.target.value })}
        />
      </Row>
    </Section>
  );
}
