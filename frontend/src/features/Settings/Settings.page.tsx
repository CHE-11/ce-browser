import { useCallback, useEffect, useState } from "react";
import { Search, SearchX } from "lucide-react";
import type { BrowserSettings } from "@type/browser";
import { EmptyState } from "@/components/EmptyState";
import { PageShell } from "@/components/PageShell";
import { Skeleton, SkeletonRows } from "@/components/Skeleton";
import { defaultSettings, saveSettings } from "@/lib/browser-client";
import { useRoute } from "@/lib/route";
import { useSettings } from "@/lib/use-browser";
import { SETTINGS_INDEX } from "./settings-index";
import { AppearanceSection } from "./sections/AppearanceSection";
import { BookmarksSection } from "./sections/BookmarksSection";
import { DownloadsSection } from "./sections/DownloadsSection";
import { PrivacySection } from "./sections/PrivacySection";
import { ProfilesSection } from "./sections/ProfilesSection";
import { SearchSection } from "./sections/SearchSection";
import { SettingsNavigation } from "./SettingsNavigation";
import { SettingsQueryProvider, SettingsSectionProps, SettingsValuesProvider } from "./SettingsControls";
import { SETTINGS_SECTIONS } from "./settings-sections";
import { StartupSection } from "./sections/StartupSection";
import { SyncSection } from "./sections/SyncSection";
import { SystemSection } from "./sections/SystemSection";
import { TabsSection } from "./sections/TabsSection";
import { UpdateSection } from "./sections/UpdateSection";
import { ToolsSection } from "./sections/tools/ToolsSection";

export function SettingsPage() {
  const settings = useSettings();
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [profileRevision, setProfileRevision] = useState(0);
  const [matched, setMatched] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(SETTINGS_SECTIONS.map((section) => [section.id, true])),
  );
  const [defaults, setDefaults] = useState<BrowserSettings | null>(null);

  useEffect(() => {
    void defaultSettings()
      .then(setDefaults)
      .catch(() => setDefaults(null));
  }, []);

  const onMatch = useCallback((id: string, matches: boolean) => {
    setMatched((current) => (current[id] === matches ? current : { ...current, [id]: matches }));
  }, []);
  const onError = useCallback((message: string | null) => setError(message), []);
  const onProfilesChanged = useCallback(() => setProfileRevision((current) => current + 1), []);

  const needle = query.trim();
  const loaded = settings !== null;
  const visibleSections = SETTINGS_SECTIONS.filter((section) => !needle || matched[section.id] !== false);
  const none = needle.length > 0 && SETTINGS_SECTIONS.every((section) => matched[section.id] === false);
  const navigation = <SettingsNavigation sections={visibleSections} enabled={loaded} />;

  useEffect(() => {
    document.getElementById("settings-page-body")?.scrollTo({ top: 0 });
  }, [needle]);

  const route = useRoute();
  const focusId = route.kind === "page" ? route.params.get("focus") : null;
  useEffect(() => {
    if (!loaded || !focusId) {
      return;
    }
    let timer = 0;
    const frame = window.requestAnimationFrame(() => {
      const entry = SETTINGS_INDEX.find((item) => item.id === focusId);
      const target = document.getElementById(focusId) ?? (entry ? document.getElementById(entry.section) : null);
      if (!target) {
        return;
      }
      target.scrollIntoView({ block: "center", behavior: "smooth" });
      target.dataset.settingFocus = "true";
      timer = window.setTimeout(() => delete target.dataset.settingFocus, 2200);
    });
    return () => {
      window.cancelAnimationFrame(frame);
      window.clearTimeout(timer);
    };
  }, [focusId, loaded]);

  if (!settings) {
    return (
      <PageShell id="settings-page" page="settings" title="Settings" nav={navigation} lockHorizontalScroll>
        <div id="settings-page-loading" role="status" aria-label="Loading settings" className="mx-auto flex max-w-3xl flex-col gap-6">
          {[0, 1, 2].map((index) => (
            <div id={`settings-page-loading-${index}`} key={index} className="rounded-lg border border-zinc-800 bg-zinc-900/60 p-5">
              <Skeleton id={`settings-page-loading-${index}-title`} className="mb-4 h-4 w-32" />
              <SkeletonRows id={`settings-page-loading-${index}-rows`} rows={3} />
            </div>
          ))}
        </div>
      </PageShell>
    );
  }

  const update: SettingsSectionProps["onUpdate"] = (patch: Partial<BrowserSettings>) => {
    setError(null);
    void saveSettings({ ...settings, ...patch }).catch((reason: unknown) => setError(String(reason)));
  };

  return (
    <PageShell
      id="settings-page"
      page="settings"
      title="Settings"
      nav={navigation}
      lockHorizontalScroll
      actions={
        <label id="settings-page-search" className="flex h-8 w-64 items-center gap-2 rounded-md border border-zinc-700 bg-zinc-900 px-2">
          <Search id="settings-page-search-icon" className="size-4 text-zinc-500" aria-hidden />
          <input
            id="settings-page-search-input"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search settings"
            className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-zinc-500"
          />
        </label>
      }
    >
      <SettingsQueryProvider query={query}>
        <SettingsValuesProvider value={{ settings, defaults, onUpdate: update }}>
        <div id="settings-page-sections" className="mx-auto flex max-w-3xl flex-col gap-6 pb-10">
          {error ? (
            <div id="settings-page-error" className="rounded-md border border-red-900 bg-red-950 px-3 py-2 text-sm text-red-300">
              {error}
            </div>
          ) : null}
          {none ? (
            <EmptyState id="settings-page-empty" icon={SearchX} title="No settings match that search" hint="Try a shorter word, such as zoom, tabs or privacy." />

          ) : null}
          <ProfilesSection settings={settings} query={query} onUpdate={update} onError={onError} onMatch={onMatch} revision={profileRevision} />

          <AppearanceSection settings={settings} query={query} onUpdate={update} onError={onError} onMatch={onMatch} />
          <BookmarksSection settings={settings} query={query} onUpdate={update} onError={onError} onMatch={onMatch} />
          <DownloadsSection settings={settings} query={query} onUpdate={update} onError={onError} onMatch={onMatch} />
          <PrivacySection settings={settings} query={query} onUpdate={update} onError={onError} onMatch={onMatch} />
          <SearchSection settings={settings} query={query} onUpdate={update} onError={onError} onMatch={onMatch} />
          <StartupSection settings={settings} query={query} onUpdate={update} onError={onError} onMatch={onMatch} />
          <SyncSection
            settings={settings}
            query={query}
            onUpdate={update}
            onError={onError}
            onMatch={onMatch}
            onProfilesChanged={onProfilesChanged}
          />
          <SystemSection settings={settings} query={query} onUpdate={update} onError={onError} onMatch={onMatch} />
          <TabsSection settings={settings} query={query} onUpdate={update} onError={onError} onMatch={onMatch} />
          <UpdateSection settings={settings} query={query} onUpdate={update} onError={onError} onMatch={onMatch} />
          <ToolsSection settings={settings} query={query} onUpdate={update} onError={onError} onMatch={onMatch} />
  
        </div>
        </SettingsValuesProvider>
      </SettingsQueryProvider>
    </PageShell>
  );
}
