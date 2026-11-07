import type { AgentAccess } from "@type/browser";

export const DEFAULT_ACCESS: AgentAccess = {
  sites: "browser",
  site: "",
  tabs: true,
  browserData: true,
  files: true,
  clipboard: true,
  memory: true,
  helpers: true,
  scripts: false,
  notifications: true,
};

export type AccessToggle = Exclude<keyof AgentAccess, "sites" | "site">;

export const ACCESS_TOGGLES: { key: AccessToggle; label: string; hint: string }[] = [
  { key: "tabs", label: "Other tabs", hint: "Open, read, switch to, and close tabs in this window." },
  { key: "browserData", label: "Browser data", hint: "History, bookmarks, downloads, boards, and your saved flows." },
  { key: "files", label: "Filesystem", hint: "Save files to Downloads\\ce-agent and upload files to pages." },
  { key: "clipboard", label: "Clipboard", hint: "Put text on the clipboard." },
  { key: "memory", label: "Memory", hint: "Read and keep notes across runs." },
  { key: "helpers", label: "Helper agents", hint: "Start helpers in background tabs. Needs other tabs." },
  { key: "scripts", label: "Page scripts", hint: "Run JavaScript in pages. Settings must allow it too." },
  { key: "notifications", label: "Notifications", hint: "Show Windows notifications." },
];

/** Older saved choices miss newer keys; those get the default. */
export function withDefaults(access: Partial<AgentAccess> | null | undefined): AgentAccess {
  const merged = { ...DEFAULT_ACCESS, ...(access ?? {}) };
  return { ...merged, sites: merged.sites === "site" ? "site" : "browser" };
}

/** A few words for the lock button: where the run may go and how much is off. */
export function accessSummary(access: AgentAccess) {
  const where = access.sites === "site" ? access.site || "This site" : "Whole browser";
  const off = ACCESS_TOGGLES.filter((item) => item.key !== "scripts" && !access[item.key]).length;
  return off > 0 ? `${where} · ${off} off` : where;
}
