export const SETTINGS_SECTIONS = [
  { id: "settings-profiles", title: "Profiles" },
  { id: "settings-search", title: "Search" },
  { id: "settings-startup", title: "On startup" },
  { id: "settings-downloads", title: "Downloads" },
  { id: "settings-tabs", title: "Tabs" },
  { id: "settings-privacy", title: "Privacy" },
  { id: "settings-adblock", title: "Ad blocker" },
  { id: "settings-appearance", title: "Appearance" },
  { id: "settings-tools", title: "Tools" },
  { id: "settings-system", title: "System" },
  { id: "settings-sync", title: "Sync" },
] as const;

export type SettingsSectionId = (typeof SETTINGS_SECTIONS)[number]["id"];
