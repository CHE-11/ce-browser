import type { AiProviderId } from "@type/browser";

export const AI_PROVIDERS: { id: AiProviderId; label: string }[] = [
  { id: "openai", label: "OpenAI" },
  { id: "claude", label: "Claude" },
  { id: "custom", label: "Custom" },
  { id: "local", label: "Local / Ollama" },
];

export function providerName(provider: AiProviderId) {
  return AI_PROVIDERS.find((item) => item.id === provider)?.label ?? "Local / Ollama";
}

export function remoteProvider(provider: AiProviderId) {
  return provider === "openai" || provider === "claude";
}
