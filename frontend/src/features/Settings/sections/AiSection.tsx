import { useEffect, useState } from "react";
import { Button } from "@/components/Button";
import { aiKeyStatus, clearAiKey, listAiModels, setAiKey, testAiConnection } from "@/lib/browser-client";
import type { AiKeyStatus, AiProviderId, AiProviderSlot } from "@type/browser";
import { AI_PROVIDERS, remoteProvider } from "../../../lib/ai/ai-providers";
import { modelAcceptsThinking } from "../../../lib/ai/model-label";
import { SETTINGS_SECTIONS } from "../settings-sections";
import { errorText, inputClass, matchesQuery, Row, Section, useReportMatch, type SettingsSectionProps } from "../SettingsControls";
import { useAiModels } from "../../../lib/ai/useAiModels";
import { AiModelPicker } from "./AiModelPicker";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-ai")!;

const PROVIDER_LABEL = "Provider";
const PROVIDER_HINT = "Used for Shopping summaries. Price tracking works with no provider.";
const ENDPOINT_LABEL = "Endpoint";
const ENDPOINT_HINT = "An OpenAI-compatible base URL. A local server may omit the key.";
const MODEL_LABEL = "Model";
const THINKING_LABEL = "Thinking";
const THINKING_HINT = "How long the model thinks before it answers. Higher costs more.";
const THINKING_LEVELS = [
  { value: "", label: "Default" },
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
];
const KEY_LABEL = "API key";

function keyLabel(provider: AiProviderId) {
  if (provider === "openai") {
    return "OPENAI_API_KEY";
  }
  if (provider === "claude") {
    return "ANTHROPIC_API_KEY";
  }
  return "API key";
}

function slotOf(settings: SettingsSectionProps["settings"], provider: AiProviderId): AiProviderSlot {
  return settings.aiProviders[provider];
}

export function AiSection({ settings, query, onUpdate, onError, onMatch }: SettingsSectionProps) {
  const provider = settings.aiProvider;
  const slot = slotOf(settings, provider);
  const remote = remoteProvider(provider);
  const showEndpoint = provider === "custom" || provider === "local";
  const fetched = useAiModels(provider, remote);
  const [localModels, setLocalModels] = useState<string[]>([]);
  const modelHint = !remote
    ? "Model name on this server."
    : fetched.loading
      ? "Loading text models…"
      : fetched.error
        ? fetched.error
        : fetched.models.length === 0
          ? "No text models were returned."
          : "The six newest text models. $ is cheaper and $$$$ costs more.";
  const visible = matchesQuery(
    query,
    section.title,
    "tools",
    PROVIDER_LABEL,
    PROVIDER_HINT,
    ENDPOINT_LABEL,
    ENDPOINT_HINT,
    MODEL_LABEL,
    modelHint,
    THINKING_LABEL,
    THINKING_HINT,
    KEY_LABEL,
    keyLabel(provider),
    "openai",
    "claude",
    "ollama",
    "assistant",
    "dpapi",
  );
  useReportMatch(section.id, visible, onMatch);
  const [draftKey, setDraftKey] = useState("");
  const [status, setStatus] = useState<AiKeyStatus | null>(null);
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

  const refreshLocal = () => {
    void listAiModels()
      .then(setLocalModels)
      .catch(() => setLocalModels([]));
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
        if (remote) {
          fetched.refresh();
        }
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
        if (remote) {
          fetched.refresh();
        } else {
          refreshLocal();
        }
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
          {AI_PROVIDERS.map((item) => (
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
      <Row id="settings-ai-model" setting="aiProviders" label={MODEL_LABEL} hint={modelHint}>
        <AiModelPicker
          id="settings-ai-model"
          value={slot.model}
          models={remote ? fetched.models : localModels}
          loading={remote && fetched.loading}
          remote={remote}
          onChange={(model) => updateSlot({ model })}
          onRefresh={remote ? fetched.refresh : refreshLocal}
          onFocus={remote ? undefined : refreshLocal}
        />
      </Row>
      {modelAcceptsThinking(provider, slot.model) ? (
        <Row id="settings-ai-thinking" setting="aiProviders" label={THINKING_LABEL} hint={THINKING_HINT} search={["reasoning", "effort"]}>
          <select
            id="settings-ai-thinking-input"
            className={inputClass}
            value={THINKING_LEVELS.some((level) => level.value === slot.thinking) ? slot.thinking : ""}
            onChange={(event) => updateSlot({ thinking: event.target.value })}
          >
            {THINKING_LEVELS.map((level) => (
              <option id={`settings-ai-thinking-${level.value || "default"}`} key={level.label} value={level.value}>
                {level.label}
              </option>
            ))}
          </select>
        </Row>
      ) : null}
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
    </Section>
  );
}
