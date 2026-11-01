import type { SettingsSectionId } from "./settings-sections";

/** One setting the command palette can jump to. `id` is the row's element id on ce://settings. */
export type SettingsEntry = {
  id: string;
  label: string;
  section: SettingsSectionId;
  keywords?: string;
};

export const SETTINGS_INDEX: SettingsEntry[] = [
  { id: "settings-profiles-new", label: "New profile", section: "settings-profiles", keywords: "profiles account" },
  { id: "settings-profiles-archive", label: "Profile file", section: "settings-profiles", keywords: "export import backup" },
  { id: "settings-search-engine", label: "Search engine", section: "settings-search", keywords: "google bing duckduckgo" },
  { id: "settings-search-custom", label: "Custom search URL", section: "settings-search" },
  { id: "settings-gestures", label: "Mouse gestures", section: "settings-search", keywords: "right drag" },
  { id: "settings-keywords", label: "Search keywords", section: "settings-search", keywords: "shortcuts gh yt" },
  { id: "settings-startup-mode", label: "When ce-browser starts", section: "settings-startup", keywords: "restore session" },
  { id: "settings-home-page", label: "Home page", section: "settings-startup" },
  { id: "settings-download-dir", label: "Download location", section: "settings-downloads", keywords: "folder" },
  { id: "settings-ask-save", label: "Ask where to save each file", section: "settings-downloads" },
  { id: "settings-accelerate", label: "Accelerate large downloads", section: "settings-downloads" },
  { id: "settings-download-name", label: "Download file name", section: "settings-downloads", keywords: "template rename" },
  { id: "settings-tab-layout", label: "Tab layout", section: "settings-tabs", keywords: "vertical horizontal sidebar" },
  { id: "settings-tab-cycle", label: "Ctrl+Tab order", section: "settings-tabs", keywords: "recent switcher" },
  { id: "settings-tab-switcher", label: "Show the tab switcher", section: "settings-tabs", keywords: "thumbnails ctrl+tab" },
  { id: "settings-tab-hover", label: "Show a preview when hovering a tab", section: "settings-tabs", keywords: "thumbnail hover card" },
  { id: "settings-peek", label: "Shift+click opens links in a peek", section: "settings-tabs" },
  { id: "settings-mute-background", label: "Mute tabs in the background", section: "settings-tabs", keywords: "sound audio" },
  { id: "settings-sleep", label: "Put background tabs to sleep", section: "settings-tabs", keywords: "memory suspend" },
  { id: "settings-sleep-minutes", label: "Sleep after", section: "settings-tabs", keywords: "minutes" },
  { id: "settings-never-sleep", label: "Never sleep on these sites", section: "settings-tabs" },
  { id: "settings-strip-tracking", label: "Strip tracking parameters", section: "settings-privacy", keywords: "utm fbclid" },
  { id: "settings-fingerprint", label: "Fingerprint shield", section: "settings-privacy", keywords: "canvas webgl" },
  { id: "settings-https-only", label: "HTTPS-only", section: "settings-privacy", keywords: "secure http" },
  { id: "settings-index-page-text", label: "Index page text", section: "settings-privacy", keywords: "history search" },
  { id: "settings-adblock-enabled", label: "Block ads and trackers", section: "settings-adblock", keywords: "ad blocker" },
  { id: "settings-site-accent", label: "Site accent", section: "settings-appearance", keywords: "color tint" },
  { id: "settings-bookmarks-bar", label: "Show bookmarks bar", section: "settings-appearance" },
  { id: "settings-bookmarks-bar-folder-icons", label: "Show folder icons on the bookmarks bar", section: "settings-appearance", keywords: "folders" },
  { id: "settings-color-mode", label: "Light or dark mode", section: "settings-appearance", keywords: "theme dark light system" },
  { id: "settings-backdrop", label: "Window backdrop", section: "settings-appearance", keywords: "mica acrylic transparency" },
  { id: "settings-theme", label: "Theme", section: "settings-appearance", keywords: "colors nord dracula" },
  { id: "settings-ui-zoom", label: "Browser UI size", section: "settings-appearance", keywords: "zoom scale" },
  { id: "settings-page-zoom", label: "Web page zoom", section: "settings-appearance", keywords: "default zoom" },
  { id: "settings-toast-position", label: "Toast position", section: "settings-appearance", keywords: "notification undo message corner" },
  { id: "settings-translate-language", label: "Translate language", section: "settings-tools" },
  { id: "settings-translate-url", label: "Translate provider", section: "settings-tools" },
  { id: "settings-assistant-endpoint", label: "Assistant endpoint", section: "settings-tools", keywords: "ai openai ollama" },
  { id: "settings-assistant-model", label: "Assistant model", section: "settings-tools", keywords: "ai" },
  { id: "settings-assistant-key", label: "Assistant API key", section: "settings-tools", keywords: "ai token" },
  { id: "settings-shopping-region", label: "Shopping region", section: "settings-tools" },
  { id: "settings-summon", label: "Summon shortcut", section: "settings-system", keywords: "hotkey global" },
  { id: "settings-default-browser", label: "Default browser", section: "settings-system" },
  { id: "settings-battery", label: "Battery saver", section: "settings-system", keywords: "power" },
  { id: "settings-auto-update", label: "Update automatically", section: "settings-system", keywords: "updates version" },
  { id: "settings-sync-folder", label: "Sync folder", section: "settings-sync" },
];
