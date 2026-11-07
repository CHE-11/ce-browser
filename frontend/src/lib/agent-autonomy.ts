import type { AgentAutonomy } from "@type/browser";

export const AUTONOMY_LEVELS: { value: AgentAutonomy; label: string; hint: string }[] = [
  {
    value: "ask",
    label: "Ask",
    hint: "Purchases, sends, deletes, uploads, and sites off the allow list wait for your approval.",
  },
  {
    value: "auto",
    label: "Auto",
    hint: "Sends, posts, and new sites go ahead. Deletes, payments, and uploads still ask.",
  },
  {
    value: "full",
    label: "Full auto",
    hint: "The agent works on its own, deletes and scripts included. Only payments and uploads ask. Esc always stops it.",
  },
];

export function autonomyLabel(level: string) {
  return AUTONOMY_LEVELS.find((item) => item.value === level)?.label ?? "Ask";
}
