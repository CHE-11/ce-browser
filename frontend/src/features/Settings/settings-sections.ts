export const SETTINGS_SECTIONS = [
  { id: "settings-profiles", title: "Profiles" },
  { id: "settings-appearance", title: "Appearance" },
  { id: "settings-bookmarks", title: "Bookmarks" },
  { id: "settings-downloads", title: "Downloads" },
  { id: "settings-privacy", title: "Privacy" },
  { id: "settings-search", title: "Search" },

  { id: "settings-startup", title: "Startup" },
  { id: "settings-sync", title: "Sync" },
  { id: "settings-system", title: "System" },
  { id: "settings-tabs", title: "Tabs" },
  { id: "settings-update", title: "Update" },
  { id: "settings-adblock", title: "Ad blocker" },
  { id: "settings-ai", title: "AI" },
  { id: "settings-shopping", title: "Shopping" },
  { id: "settings-translate", title: "Translate" },
] as const;

export type SettingsSectionId = (typeof SETTINGS_SECTIONS)[number]["id"];
