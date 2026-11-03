import { providerName, remoteProvider } from "../../../../lib/ai/ai-providers";
import { AiModelPicker } from "./AiModelPicker";
import { SETTINGS_SECTIONS } from "../../settings-sections";
import { inputClass, matchesQuery, Row, Section, useReportMatch, type SettingsSectionProps } from "../../SettingsControls";
import { useAiModels } from "../../../../lib/ai/useAiModels";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-shopping")!;

const REGION_LABEL = "Region";
const REGION_HINT = "DuckDuckGo region for coupon and price searches, for example us-en or uk-en.";
const MODEL_LABEL = "Model";
const COMPETITORS_LABEL = "Competitor stores";
const COMPETITORS_HINT = "Optional hosts, separated by commas. Adds one extra deal search.";

export function ShoppingSection({ settings, query, onUpdate, onMatch }: SettingsSectionProps) {
  const provider = settings.aiProvider;
  const remote = remoteProvider(provider);
  const fetched = useAiModels(provider, remote);
  const blankLabel = `Use the ${providerName(provider)} model`;
  const modelHint = !remote
    ? `Leave blank to use the ${providerName(provider)} model.`
    : fetched.loading
      ? "Loading text models…"
      : fetched.error
        ? fetched.error
        : `Leave blank to use the ${providerName(provider)} model.`;
  const visible = matchesQuery(
    query,
    section.title,
    "tools",
    REGION_LABEL,
    REGION_HINT,
    MODEL_LABEL,
    modelHint,
    COMPETITORS_LABEL,
    COMPETITORS_HINT,
    "coupons",
    "deals",
    "stores",
  );
  useReportMatch(section.id, visible, onMatch);

  if (!visible) {
    return null;
  }

  return (
    <Section id={section.id} title={section.title}>
      <Row id="settings-shopping-region" setting="shoppingRegion" label={REGION_LABEL} hint={REGION_HINT} search={["coupons", "deals"]}>
        <input
          id="settings-shopping-region-input"
          className={inputClass}
          value={settings.shoppingRegion}
          spellCheck={false}
          onChange={(event) => onUpdate({ shoppingRegion: event.target.value })}
        />
      </Row>
      <Row id="settings-shopping-model" setting="shoppingModel" label={MODEL_LABEL} hint={modelHint} search={["coupons", "deals"]}>
        <AiModelPicker
          id="settings-shopping-model"
          value={settings.shoppingModel}
          models={fetched.models}
          loading={remote && fetched.loading}
          remote={remote}
          blankLabel={remote ? blankLabel : undefined}
          onChange={(shoppingModel) => onUpdate({ shoppingModel })}
          onRefresh={fetched.refresh}
        />
      </Row>
      <Row id="settings-shopping-competitors" setting="shoppingCompetitors" label={COMPETITORS_LABEL} hint={COMPETITORS_HINT} search={["coupons", "deals"]}>
        <input
          id="settings-shopping-competitors-input"
          className={inputClass}
          value={settings.shoppingCompetitors.join(", ")}
          spellCheck={false}
          onChange={(event) =>
            onUpdate({
              shoppingCompetitors: event.target.value
                .split(",")
                .map((item) => item.trim())
                .filter(Boolean),
            })
          }
        />
      </Row>
    </Section>
  );
}
