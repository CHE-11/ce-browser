import { SETTINGS_SECTIONS } from "./settings-sections";
import { inputClass, matchesQuery, Row, Section, useReportMatch, type SettingsSectionProps } from "./SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-tools")!;

const LANG_LABEL = "Translate language";
const LANG_HINT = "Language code, for example en or es.";
const URL_LABEL = "Translate provider";
const URL_HINT = "%s is the text and %t is the language.";
const ENDPOINT_LABEL = "Assistant endpoint";
const ENDPOINT_HINT = "An OpenAI-compatible base URL. A local server may omit the key.";
const MODEL_LABEL = "Assistant model";
const KEY_LABEL = "Assistant API key";
const KEY_HINT = "Stays on this computer. Sync does not copy it.";

export function ToolsSection({ settings, query, onUpdate, onMatch }: SettingsSectionProps) {
  const visible = matchesQuery(
    query,
    section.title,
    LANG_LABEL,
    LANG_HINT,
    URL_LABEL,
    URL_HINT,
    ENDPOINT_LABEL,
    ENDPOINT_HINT,
    MODEL_LABEL,
    KEY_LABEL,
    KEY_HINT,
  );
  useReportMatch(section.id, visible, onMatch);
  if (!visible) {
    return null;
  }

  return (
    <Section id={section.id} title={section.title}>
      <Row id="settings-translate-language" label={LANG_LABEL} hint={LANG_HINT}>
        <input
          id="settings-translate-language-input"
          className={inputClass}
          value={settings.translateLanguage}
          onChange={(event) => onUpdate({ translateLanguage: event.target.value })}
        />
      </Row>
      <Row id="settings-translate-url" label={URL_LABEL} hint={URL_HINT}>
        <input
          id="settings-translate-url-input"
          className={inputClass}
          value={settings.translateUrl}
          spellCheck={false}
          onChange={(event) => onUpdate({ translateUrl: event.target.value })}
        />
      </Row>
      <Row id="settings-assistant-endpoint" label={ENDPOINT_LABEL} hint={ENDPOINT_HINT}>
        <input
          id="settings-assistant-endpoint-input"
          className={inputClass}
          value={settings.assistantEndpoint}
          spellCheck={false}
          onChange={(event) => onUpdate({ assistantEndpoint: event.target.value })}
        />
      </Row>
      <Row id="settings-assistant-model" label={MODEL_LABEL}>
        <input
          id="settings-assistant-model-input"
          className={inputClass}
          value={settings.assistantModel}
          spellCheck={false}
          onChange={(event) => onUpdate({ assistantModel: event.target.value })}
        />
      </Row>
      <Row id="settings-assistant-key" label={KEY_LABEL} hint={KEY_HINT}>
        <input
          id="settings-assistant-key-input"
          className={inputClass}
          type="password"
          value={settings.assistantKey}
          autoComplete="off"
          onChange={(event) => onUpdate({ assistantKey: event.target.value })}
        />
      </Row>
    </Section>
  );
}
