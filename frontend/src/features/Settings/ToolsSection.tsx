import { useEffect, useState } from "react";
import { Button } from "@/components/Button";
import { aiKeyStatus, clearAiKey, listAiModels, setAiKey, testAiConnection } from "@/lib/browser-client";
import type { AiKeyStatus, AiProviderId, AiProviderSlot } from "@type/browser";
import { SETTINGS_SECTIONS } from "./settings-sections";
import { errorText, inputClass, matchesQuery, Row, Section, useReportMatch, type SettingsSectionProps } from "./SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-tools")!;

const LANG_LABEL = "Translate language";
const LANG_HINT = "Language code, for example en or es.";
const URL_LABEL = "Translate provider";
const URL_HINT = "%s is the text and %t is the language.";
const PROVIDER_LABEL = "Shopping AI provider";
const PROVIDER_HINT = "Summarizes deal search results. Price tracking works with no provider.";
const ENDPOINT_LABEL = "Endpoint";
const ENDPOINT_HINT = "An OpenAI-compatible base URL. A local server may omit the key.";
const MODEL_LABEL = "Model";
const KEY_LABEL = "API key";
const REGION_LABEL = "Shopping region";
const REGION_HINT = "DuckDuckGo region for coupon and price searches, for example us-en or uk-en.";
const SHOPPING_MODEL_LABEL = "Shopping model";
const COMPETITORS_LABEL = "Competitor stores";
const COMPETITORS_HINT = "Optional hosts, separated by commas. Adds one extra deal search.";

const PROVIDERS: { id: AiProviderId; label: string }[] = [
  { id: "openai", label: "OpenAI" },
  { id: "claude", label: "Claude" },
  { id: "custom", label: "Custom" },
  { id: "local", label: "Local / Ollama" },
];

function keyLabel(provider: AiProviderId) {
  if (provider === "openai") {
    return "OPENAI_API_KEY";
  }
  if (provider === "claude") {
    return "ANTHROPIC_API_KEY";
  }
  return "API key";
}

function providerName(provider: AiProviderId) {
  return PROVIDERS.find((item) => item.id === provider)?.label ?? "Local / Ollama";
}

function slotOf(settings: SettingsSectionProps["settings"], provider: AiProviderId): AiProviderSlot {
  return settings.aiProviders[provider];
}

export function ToolsSection({ settings, query, onUpdate, onError, onMatch }: SettingsSectionProps) {
  const provider = settings.aiProvider;
  const slot = slotOf(settings, provider);
  const showEndpoint = provider === "custom" || provider === "local";
  const shoppingHint = `Leave blank to use the ${providerName(provider)} model.`;
  const visible = matchesQuery(
    query,
    section.title,
    LANG_LABEL,
    LANG_HINT,
    URL_LABEL,
    URL_HINT,
    PROVIDER_LABEL,
    PROVIDER_HINT,
    ENDPOINT_LABEL,
    ENDPOINT_HINT,
    MODEL_LABEL,
    KEY_LABEL,
    keyLabel(provider),
    REGION_LABEL,
    REGION_HINT,
    SHOPPING_MODEL_LABEL,
    shoppingHint,
    COMPETITORS_LABEL,
    COMPETITORS_HINT,
    "coupons",
    "deals",
    "dpapi",
  );
  useReportMatch(section.id, visible, onMatch);
  const [draftKey, setDraftKey] = useState("");
  const [status, setStatus] = useState<AiKeyStatus | null>(null);
  const [models, setModels] = useState<string[]>([]);
  const [testing, setTesting] = useState(false);
  const [testMessage, setTestMessage] = useState("");
  const [confirmClear, setConfirmClear] = useState(false);

  useEffect(() => {
    let active = true;
    void aiKeyStatus(provider)
      .then((next) => {
        if (active) {
          setStatus(next);
        }
      })
      .catch(() => {
        if (active) {
          setStatus(null);
        }
      });
    return () => {
      active = false;
    };
  }, [provider]);

  if (!visible) {
    return null;
  }

  const updateSlot = (patch: Partial<AiProviderSlot>) => {
    onUpdate({
      aiProviders: {
        ...settings.aiProviders,
        [provider]: { ...slot, ...patch },
      },
    });
  };

  const refreshModels = () => {
    void listAiModels()
      .then(setModels)
      .catch(() => setModels([]));
  };

  const saveKey = () => {
    const key = draftKey.trim();
    if (!key) {
      return;
    }
    void setAiKey(provider, key)
      .then((next) => {
        setStatus(next);
        setDraftKey("");
        setConfirmClear(false);
        setTestMessage("Saved on this computer.");
        onError(null);
      })
      .catch((reason: unknown) => onError(errorText(reason)));
  };

  const clearKey = () => {
    void clearAiKey(provider)
      .then((next) => {
        setStatus(next);
        setConfirmClear(false);
        setTestMessage("");
        onError(null);
      })
      .catch((reason: unknown) => onError(errorText(reason)));
  };

  const test = () => {
    setTesting(true);
    setTestMessage("");
    void testAiConnection()
      .then(() => {
        setTestMessage("Connected.");
        onError(null);
        refreshModels();
      })
      .catch((reason: unknown) => setTestMessage(errorText(reason)))
      .finally(() => setTesting(false));
  };

  const keyHint = status?.unreadable
    ? "The saved key could not be read on this computer. Clear it and enter the key again."
    : status?.hasKey
      ? `Saved on this computer${status.hint ? ` · ends in ${status.hint}` : ""}.`
      : status?.fromEnv
        ? `Using ${keyLabel(provider)} from this computer's environment.`
        : "Encrypted with Windows DPAPI for this Windows user. Never synced and never exported.";

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
      <Row id="settings-ai-provider" setting="aiProvider" label={PROVIDER_LABEL} hint={PROVIDER_HINT}>
        <select
          id="settings-ai-provider-input"
          className={inputClass}
          value={provider}
          onChange={(event) => {
            setTestMessage("");
            setConfirmClear(false);
            onUpdate({ aiProvider: event.target.value as AiProviderId });
          }}
        >
          {PROVIDERS.map((item) => (
            <option id={`settings-ai-provider-${item.id}`} key={item.id} value={item.id}>
              {item.label}
            </option>
          ))}
        </select>
      </Row>
      {showEndpoint ? (
        <Row id="settings-ai-endpoint" setting="aiProviders" label={ENDPOINT_LABEL} hint={ENDPOINT_HINT}>
          <input
            id="settings-ai-endpoint-input"
            className={inputClass}
            value={slot.endpoint}
            spellCheck={false}
            onChange={(event) => updateSlot({ endpoint: event.target.value })}
          />
        </Row>
      ) : null}
      <Row id="settings-ai-model" setting="aiProviders" label={MODEL_LABEL}>
        <input
          id="settings-ai-model-input"
          className={inputClass}
          list="settings-ai-model-list"
          value={slot.model}
          spellCheck={false}
          onChange={(event) => updateSlot({ model: event.target.value })}
          onFocus={refreshModels}
        />
        <datalist id="settings-ai-model-list">
          {models.map((model, index) => (
            <option id={`settings-ai-model-option-${index}`} key={model} value={model} />
          ))}
        </datalist>
      </Row>
      <Row id="settings-ai-key" setting="aiProvider" label={keyLabel(provider)} hint={keyHint}>
        <div id="settings-ai-key-row" className="flex flex-col gap-2">
          <input
            id="settings-ai-key-input"
            className={inputClass}
            type="password"
            value={draftKey}
            autoComplete="off"
            placeholder={status?.hasKey ? "Key saved" : ""}
            onChange={(event) => setDraftKey(event.target.value)}
          />
          <div id="settings-ai-key-actions" className="flex flex-wrap gap-2">
            <Button id="settings-ai-key-set" size="sm" variant="primary" disabled={!draftKey.trim()} onClick={saveKey}>
              Set
            </Button>
            {confirmClear ? (
              <>
                <Button id="settings-ai-key-clear-confirm" size="sm" variant="destructive" onClick={clearKey}>
                  Clear saved key
                </Button>
                <Button id="settings-ai-key-clear-cancel" size="sm" variant="ghost" onClick={() => setConfirmClear(false)}>
                  Cancel
                </Button>
              </>
            ) : (
              <Button id="settings-ai-key-clear" size="sm" variant="ghost" disabled={!status?.hasKey && !status?.unreadable} onClick={() => setConfirmClear(true)}>
                Clear
              </Button>
            )}
            <Button id="settings-ai-test" size="sm" variant="outline" disabled={testing} onClick={test}>
              {testing ? "Testing…" : "Test connection"}
            </Button>
          </div>
          {testMessage ? (
            <p id="settings-ai-test-message" className="text-xs text-zinc-300">
              {testMessage}
            </p>
          ) : null}
          <p id="settings-ai-key-privacy" className="text-xs text-zinc-500">
            Encrypted with Windows DPAPI for this Windows user, never synced, never exported.
          </p>
        </div>
      </Row>
      <Row id="settings-shopping-region" setting="shoppingRegion" label={REGION_LABEL} hint={REGION_HINT} search={["coupons", "deals"]}>
        <input
          id="settings-shopping-region-input"
          className={inputClass}
          value={settings.shoppingRegion}
          spellCheck={false}
          onChange={(event) => onUpdate({ shoppingRegion: event.target.value })}
        />
      </Row>
      <Row id="settings-shopping-model" setting="shoppingModel" label={SHOPPING_MODEL_LABEL} hint={shoppingHint} search={["coupons", "deals"]}>
        <input
          id="settings-shopping-model-input"
          className={inputClass}
          value={settings.shoppingModel}
          placeholder={slot.model}
          spellCheck={false}
          onChange={(event) => onUpdate({ shoppingModel: event.target.value })}
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
