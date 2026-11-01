import { useEffect, useState, type ReactNode } from "react";
import { Check, FolderOpen, ImagePlus, Minus, Plus, RotateCcw, X } from "lucide-react";
import type { BrowserProfile, BrowserSettings, ProfileIconChoice, SearchEngine, SyncStatus } from "../../../types/browser";
import { Button } from "../components/Button";
import { IconButton } from "../components/IconButton";
import { PageShell } from "../components/PageShell";
import { Toggle } from "../components/Toggle";
import { COLOR_MODE_OPTIONS, THEMES, resolveMode } from "../lib/appearance";
import {
  chooseFolder,
  clearSyncFolder,
  createProfile,
  createSyncFolder,
  defaultDownloadDir,
  getUserStyles,
  showSyncFolder,
  showUserStyles,
  deleteProfile,
  installProfileShortcut,
  listProfileIcons,
  listProfiles,
  pickProfileIcon,
  onSyncStatus,
  openProfile,
  pinProfileTaskbar,
  pickSyncFolder,
  saveSettings,
  selectSyncFolder,
  setProfileIcon,
  syncStatus,
  unpinProfileTaskbar,
} from "../lib/browser-client";
import { cn } from "../lib/cn";
import { useSettings } from "../lib/use-browser";
import { clampZoom, DEFAULT_ZOOM, MAX_ZOOM, MIN_ZOOM, PAGE_ZOOM_LEVELS, stepPageZoom, ZOOM_STEP } from "../lib/zoom";

const SEARCH_ENGINES: { id: SearchEngine; label: string }[] = [
  { id: "google", label: "Google" },
  { id: "bing", label: "Bing" },
  { id: "duckduckgo", label: "DuckDuckGo" },
  { id: "brave", label: "Brave Search" },
  { id: "startpage", label: "Startpage" },
  { id: "kagi", label: "Kagi (no ads)" },
  { id: "mojeek", label: "Mojeek (no ads)" },
  { id: "custom", label: "Custom" },
];

const inputClass =
  "h-8 w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 text-sm text-zinc-100 outline-none focus:border-sky-500";

function errorText(reason: unknown) {
  if (typeof reason === "string" && reason.trim()) {
    return reason;
  }
  if (reason instanceof Error && reason.message.trim()) {
    return reason.message;
  }
  if (reason && typeof reason === "object" && "message" in reason) {
    const message = reason.message;
    if (typeof message === "string" && message.trim()) {
      return message;
    }
  }
  try {
    const text = JSON.stringify(reason);
    if (text && text !== "null" && text !== "{}") {
      return text;
    }
  } catch {
    // A circular value still needs a readable fallback below.
  }
  return "Unknown error";
}

function Section({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  return (
    <section id={id} className="flex flex-col gap-1 rounded-lg border border-zinc-800 bg-zinc-900/60">
      <h2 id={`${id}-title`} className="px-5 pt-4 text-sm font-semibold text-zinc-200">
        {title}
      </h2>
      <div id={`${id}-rows`} className="flex flex-col divide-y divide-zinc-800">
        {children}
      </div>
    </section>
  );
}

function Row({ id, label, hint, children }: { id: string; label: string; hint?: string; children: ReactNode }) {
  return (
    <div id={id} className="flex items-center justify-between gap-6 px-5 py-3">
      <div id={`${id}-text`} className="min-w-0">
        <div id={`${id}-label`} className="text-sm text-zinc-100">
          {label}
        </div>
        {hint ? (
          <div id={`${id}-hint`} className="text-xs text-zinc-500">
            {hint}
          </div>
        ) : null}
      </div>
      <div id={`${id}-control`} className="flex w-80 shrink-0 items-center justify-end gap-2">
        {children}
      </div>
    </div>
  );
}

export function SettingsPage() {
  const settings = useSettings();
  const [homeDraft, setHomeDraft] = useState("");
  const [pinnedDraft, setPinnedDraft] = useState("");
  const [customDraft, setCustomDraft] = useState("");
  const [pendingCustom, setPendingCustom] = useState(false);
  const [systemDownloads, setSystemDownloads] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [syncState, setSyncState] = useState<SyncStatus | null>(null);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [syncWorking, setSyncWorking] = useState(false);
  const [profiles, setProfiles] = useState<BrowserProfile[]>([]);
  const [profileIcons, setProfileIcons] = useState<ProfileIconChoice[]>([]);
  const [profileName, setProfileName] = useState("");
  const [profileWorking, setProfileWorking] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<string | null>(null);
  const [iconProfile, setIconProfile] = useState<string | null>(null);
  const [userCssPath, setUserCssPath] = useState("");

  useEffect(() => {
    if (settings) {
      setHomeDraft(settings.homePage);
      setPinnedDraft(settings.pinnedTabs.join("\n"));
      setCustomDraft(settings.customSearchUrl);
    }
  }, [settings]);

  useEffect(() => {
    void defaultDownloadDir().then(setSystemDownloads).catch(() => undefined);
    void listProfiles().then(setProfiles).catch((reason: unknown) => setError(String(reason)));
    void listProfileIcons().then(setProfileIcons).catch((reason: unknown) => setError(String(reason)));
    void getUserStyles()
      .then((styles) => setUserCssPath(styles.path))
      .catch((reason: unknown) => setError(String(reason)));
  }, []);

  useEffect(() => {
    let disposed = false;
    void syncStatus()
      .then((status) => {
        if (!disposed) {
          setSyncState(status);
        }
      })
      .catch(() => undefined);
    const unlisten = onSyncStatus((status) => {
      if (!disposed) {
        setSyncState(status);
      }
    });
    return () => {
      disposed = true;
      void unlisten.then((stop) => stop());
    };
  }, []);

  if (!settings) {
    return (
      <PageShell id="settings-page" page="settings" title="Settings">
        <div id="settings-page-loading" className="text-sm text-zinc-500">
          Loading settings
        </div>
      </PageShell>
    );
  }

  const update = (patch: Partial<BrowserSettings>) => {
    setError(null);
    void saveSettings({ ...settings, ...patch }).catch((reason: unknown) => setError(String(reason)));
  };

  const engine = pendingCustom ? "custom" : settings.searchEngine;
  const pageZoom = settings.pageZoom || DEFAULT_ZOOM;
  const resolved = resolveMode(settings.colorMode);
  const syncBusy = syncWorking;

  const reportSyncError = (reason: unknown) => {
    const message = errorText(reason);
    setError(message);
    setSyncError(message);
  };

  const runSync = (work: () => Promise<SyncStatus>) => {
    setSyncWorking(true);
    setError(null);
    setSyncError(null);
    void work()
      .then((status) => {
        setSyncState(status);
        if (status.error) {
          setError(status.error);
          setSyncError(status.error);
        }
      })
      .catch(reportSyncError)
      .finally(() => {
        setSyncWorking(false);
        void listProfiles().then(setProfiles).catch(() => undefined);
      });
  };

  const pickAndSync = (title: string, apply: (path: string) => Promise<SyncStatus>) => {
    setError(null);
    setSyncError(null);
    void pickSyncFolder(title)
      .then((path) => {
        if (path) {
          runSync(() => apply(path));
        }
      })
      .catch(reportSyncError);
  };


  const runProfiles = (work: () => Promise<BrowserProfile[] | void>) => {
    setProfileWorking(true);
    setError(null);
    void work()
      .then((next) => {
        if (next) {
          setProfiles(next);
        }
      })
      .catch((reason: unknown) => {
        setError(String(reason));
        void listProfiles().then(setProfiles).catch(() => undefined);
      })
      .finally(() => setProfileWorking(false));
  };

  return (
    <PageShell id="settings-page" page="settings" title="Settings">
      <div id="settings-page-sections" className="mx-auto flex max-w-3xl flex-col gap-6 pb-10">
        {error ? (
          <div id="settings-page-error" className="rounded-md border border-red-900 bg-red-950 px-3 py-2 text-sm text-red-300">
            {error}
          </div>
        ) : null}

        <Section id="settings-profiles" title="Profiles">
          <Row
            id="settings-profiles-new"
            label="New profile"
            hint="Each profile has its own bookmarks, history, extensions, and settings, kept in a folder named after the profile (Default for the first one) in the sync folder or in app data. Its icon is used for a separate Start menu item and taskbar button."
          >
            <input
              id="settings-profiles-name"
              value={profileName}
              placeholder="Work"
              className={`${inputClass} min-w-0 flex-1`}
              onChange={(event) => setProfileName(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && profileName.trim() && !profileWorking) {
                  const name = profileName.trim();
                  setProfileName("");
                  runProfiles(() => createProfile(name));
                }
              }}
            />
            <Button
              id="settings-profiles-create"
              size="sm"
              disabled={profileWorking || !profileName.trim()}
              onClick={() => {
                const name = profileName.trim();
                setProfileName("");
                runProfiles(() => createProfile(name));
              }}
            >
              Create
            </Button>
          </Row>
          {profiles.map((profile) => (
            <div id={`settings-profile-${profile.id}`} key={profile.id} className="flex flex-col gap-3 px-5 py-3">
              <div id={`settings-profile-${profile.id}-row`} className="flex items-center justify-between gap-6">
                <div id={`settings-profile-${profile.id}-text`} className="flex min-w-0 items-center gap-3">
                  <button
                    id={`settings-profile-${profile.id}-icon`}
                    type="button"
                    title="Change icon"
                    aria-label={`Change icon for ${profile.name}`}
                    className={cn(
                      "size-9 shrink-0 overflow-hidden rounded-md border",
                      iconProfile === profile.id ? "border-sky-500 ring-1 ring-sky-500" : "border-zinc-700 hover:border-zinc-500",
                    )}
                    onClick={() => setIconProfile((current) => (current === profile.id ? null : profile.id))}
                  >
                    <img id={`settings-profile-${profile.id}-icon-image`} src={profile.icon || "/icon.png"} alt="" className="size-full" />
                  </button>
                  <div id={`settings-profile-${profile.id}-copy`} className="min-w-0">
                    <div id={`settings-profile-${profile.id}-name`} className="truncate text-sm text-zinc-100">
                      {profile.name}
                      {profile.current ? " · This window" : ""}
                    </div>
                    <div id={`settings-profile-${profile.id}-hint`} className="text-xs text-zinc-500">
                      {profile.taskbar
                        ? "Pinned to the taskbar."
                        : profile.startMenu
                          ? "Start menu item installed. Pin it for a taskbar button."
                          : "No Start menu item yet."}
                    </div>
                  </div>
                </div>
                <div id={`settings-profile-${profile.id}-actions`} className="flex shrink-0 flex-wrap items-center justify-end gap-2">
                  {!profile.current ? (
                    <Button
                      id={`settings-profile-${profile.id}-open`}
                      size="sm"
                      disabled={profileWorking}
                      onClick={() => runProfiles(() => openProfile(profile.id))}
                    >
                      Open
                    </Button>
                  ) : null}
                  {profile.taskbar ? (
                    <Button
                      id={`settings-profile-${profile.id}-unpin`}
                      size="sm"
                      disabled={profileWorking}
                      onClick={() => runProfiles(() => unpinProfileTaskbar(profile.id))}
                    >
                      Unpin
                    </Button>
                  ) : (
                    <Button
                      id={`settings-profile-${profile.id}-pin`}
                      size="sm"
                      disabled={profileWorking}
                      onClick={() => runProfiles(() => pinProfileTaskbar(profile.id))}
                    >
                      Pin to taskbar
                    </Button>
                  )}
                  {!profile.startMenu ? (
                    <Button
                      id={`settings-profile-${profile.id}-shortcut`}
                      size="sm"
                      disabled={profileWorking}
                      onClick={() => runProfiles(() => installProfileShortcut(profile.id))}
                    >
                      Add to Start
                    </Button>
                  ) : null}
                  {profile.id !== "default" && !profile.current ? (
                    <Button
                      id={`settings-profile-${profile.id}-remove`}
                      size="sm"
                      variant="ghost"
                      disabled={profileWorking}
                      onClick={() => setPendingDelete(profile.id)}
                    >
                      Remove
                    </Button>
                  ) : null}
                </div>
              </div>
              {iconProfile === profile.id ? (
                <div id={`settings-profile-${profile.id}-icons`} className="flex flex-col gap-2 pl-12">
                  <p id={`settings-profile-${profile.id}-icons-hint`} className="text-xs text-zinc-500">
                    {profile.current
                      ? "This window, the Start menu, and the taskbar button use this icon."
                      : "The Start menu and taskbar shortcut update now. That profile's window uses the icon the next time it opens."}
                  </p>
                  <div id={`settings-profile-${profile.id}-icon-grid`} className="grid grid-cols-4 gap-2 sm:grid-cols-8">
                    {profileIcons.map((choice) => {
                      const selected = profile.iconId === choice.id;
                      return (
                        <button
                          key={choice.id}
                          id={`settings-profile-${profile.id}-icon-${choice.id}`}
                          type="button"
                          title={choice.label}
                          disabled={profileWorking}
                          className={cn(
                            "flex flex-col items-center gap-1 rounded-md border p-1.5",
                            selected ? "border-sky-500 ring-1 ring-sky-500" : "border-zinc-700 hover:border-zinc-500",
                          )}
                          onClick={() => runProfiles(() => setProfileIcon(profile.id, choice.id))}
                        >
                          <img
                            id={`settings-profile-${profile.id}-icon-${choice.id}-image`}
                            src={choice.icon}
                            alt=""
                            className="size-10 rounded"
                          />
                          <span id={`settings-profile-${profile.id}-icon-${choice.id}-label`} className="truncate text-[11px] text-zinc-400">
                            {choice.label}
                          </span>
                        </button>
                      );
                    })}
                    <button
                      id={`settings-profile-${profile.id}-icon-file`}
                      type="button"
                      title="Choose an image"
                      disabled={profileWorking}
                      className={cn(
                        "flex flex-col items-center gap-1 rounded-md border p-1.5",
                        profile.iconId === "custom" ? "border-sky-500 ring-1 ring-sky-500" : "border-zinc-700 hover:border-zinc-500",
                      )}
                      onClick={() =>
                        void pickProfileIcon().then((path) => {
                          if (path) {
                            runProfiles(() => setProfileIcon(profile.id, "custom", path));
                          }
                        })
                      }
                    >
                      {profile.iconId === "custom" ? (
                        <img
                          id={`settings-profile-${profile.id}-icon-file-image`}
                          src={profile.icon || "/icon.png"}
                          alt=""
                          className="size-10 rounded"
                        />
                      ) : (
                        <ImagePlus id={`settings-profile-${profile.id}-icon-file-image`} className="size-10 p-2 text-zinc-300" aria-hidden />
                      )}
                      <span id={`settings-profile-${profile.id}-icon-file-label`} className="truncate text-[11px] text-zinc-400">
                        Image
                      </span>
                    </button>
                  </div>
                </div>
              ) : null}
            </div>
          ))}
          {pendingDelete ? (
            <div id="settings-profiles-confirm" className="flex flex-col gap-3 px-5 py-3">
              <p id="settings-profiles-confirm-text" className="text-sm text-zinc-200">
                Remove {profiles.find((profile) => profile.id === pendingDelete)?.name ?? "this profile"}? Its bookmarks, history, extensions, settings, and Start menu item are deleted.
              </p>
              <div id="settings-profiles-confirm-actions" className="flex justify-end gap-2">
                <Button id="settings-profiles-confirm-cancel" size="sm" variant="ghost" onClick={() => setPendingDelete(null)}>
                  Cancel
                </Button>
                <Button
                  id="settings-profiles-confirm-remove"
                  size="sm"
                  variant="destructive"
                  onClick={() => {
                    const id = pendingDelete;
                    setPendingDelete(null);
                    if (id) {
                      runProfiles(() => deleteProfile(id));
                    }
                  }}
                >
                  Remove
                </Button>
              </div>
            </div>
          ) : null}
        </Section>

        <Section id="settings-search" title="Search">
          <Row
            id="settings-search-engine"
            label="Search engine"
            hint="Used when the address bar text is not a URL. Kagi and Mojeek do not show ads. Kagi needs an account."
          >
            <select
              id="settings-search-engine-select"
              value={engine}
              className={inputClass}
              onChange={(event) => {
                const value = event.target.value as SearchEngine;
                if (value === "custom" && !settings.customSearchUrl) {
                  setPendingCustom(true);
                  return;
                }
                setPendingCustom(false);
                update({ searchEngine: value });
              }}
            >
              {SEARCH_ENGINES.map((option) => (
                <option key={option.id} id={`settings-search-engine-${option.id}`} value={option.id}>
                  {option.label}
                </option>
              ))}
            </select>
          </Row>
          {engine === "custom" ? (
            <Row id="settings-search-custom" label="Custom search URL" hint="Use %s where the query goes.">
              <input
                id="settings-search-custom-input"
                value={customDraft}
                spellCheck={false}
                placeholder="https://example.com/search?q=%s"
                className={inputClass}
                onChange={(event) => setCustomDraft(event.target.value)}
                onBlur={() => {
                  if (customDraft.trim()) {
                    setPendingCustom(false);
                    update({ searchEngine: "custom", customSearchUrl: customDraft.trim() });
                  }
                }}
                onKeyDown={(event) => event.key === "Enter" && event.currentTarget.blur()}
              />
            </Row>
          ) : null}
        </Section>

        <Section id="settings-startup" title="On startup">
          <Row id="settings-startup-mode" label="When ce-browser starts">
            <select
              id="settings-startup-mode-select"
              value={settings.startup}
              className={inputClass}
              onChange={(event) => update({ startup: event.target.value as BrowserSettings["startup"] })}
            >
              <option id="settings-startup-restore" value="restore">
                Continue where you left off
              </option>
              <option id="settings-startup-home" value="home">
                Open pinned tabs and the home page
              </option>
            </select>
          </Row>
          <Row id="settings-home-page" label="Home page" hint="Leave empty for the bookmarks page.">
            <input
              id="settings-home-page-input"
              value={homeDraft}
              spellCheck={false}
              placeholder="ce://bookmarks"
              className={inputClass}
              onChange={(event) => setHomeDraft(event.target.value)}
              onBlur={() => homeDraft !== settings.homePage && update({ homePage: homeDraft.trim() })}
              onKeyDown={(event) => event.key === "Enter" && event.currentTarget.blur()}
            />
          </Row>
          <div id="settings-pinned-tabs" className="px-5 py-3">
            <div id="settings-pinned-tabs-label" className="text-sm text-zinc-100">
              Pinned tabs
            </div>
            <div id="settings-pinned-tabs-hint" className="mb-2 text-xs text-zinc-500">
              One URL per line. This profile pins them every time it starts, and the list is saved in this profile's settings.
            </div>
            <textarea
              id="settings-pinned-tabs-input"
              value={pinnedDraft}
              rows={6}
              spellCheck={false}
              placeholder={"https://mail.example.com\nhttps://calendar.example.com"}
              className="min-h-28 w-full resize-y rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-sm text-zinc-100 outline-none focus:border-sky-500"
              onChange={(event) => setPinnedDraft(event.target.value)}
              onBlur={() => {
                const next = pinnedDraft
                  .split(/\r?\n/)
                  .map((line) => line.trim())
                  .filter((line) => line.length > 0);
                const current = settings.pinnedTabs;
                const same = next.length === current.length && next.every((url, index) => url === current[index]);
                if (!same) {
                  update({ pinnedTabs: next });
                }
              }}
            />
          </div>
        </Section>

        <Section id="settings-downloads" title="Downloads">
          <Row id="settings-download-dir" label="Location" hint={settings.downloadDir || systemDownloads || "System Downloads folder"}>
            {settings.downloadDir ? (
              <Button id="settings-download-dir-reset" size="sm" variant="ghost" onClick={() => update({ downloadDir: "" })}>
                <RotateCcw id="settings-download-dir-reset-icon" className="size-3.5" aria-hidden />
                Default
              </Button>
            ) : null}
            <Button
              id="settings-download-dir-change"
              size="sm"
              onClick={() =>
                void chooseFolder("Choose the download folder", settings.downloadDir || systemDownloads).then(
                  (folder) => folder && update({ downloadDir: folder }),
                )
              }
            >
              <FolderOpen id="settings-download-dir-change-icon" className="size-3.5" aria-hidden />
              Change
            </Button>
          </Row>
          <Row id="settings-ask-save" label="Ask where to save each file">
            <Toggle id="settings-ask-save-toggle" checked={settings.askWhereToSave} onChange={(value) => update({ askWhereToSave: value })} />
          </Row>
        </Section>

        <Section id="settings-adblock" title="Ad blocker">
          <Row id="settings-adblock-enabled" label="Block ads and trackers" hint="Open pages pick up the change when they next load.">
            <Toggle
              id="settings-adblock-enabled-toggle"
              checked={settings.adblock.enabled}
              onChange={(enabled) => update({ adblock: { ...settings.adblock, enabled } })}
            />
          </Row>
          <div id="settings-adblock-paused" className="px-5 py-3">
            <div id="settings-adblock-paused-label" className="text-sm text-zinc-100">
              Paused sites
            </div>
            <div id="settings-adblock-paused-hint" className="mb-2 text-xs text-zinc-500">
              Ads are allowed on these sites and their subdomains. Pause a site from the shield on the address bar.
            </div>
            {settings.adblock.allowedSites.length === 0 ? (
              <div id="settings-adblock-paused-empty" className="text-xs text-zinc-500">
                No paused sites.
              </div>
            ) : (
              <ul id="settings-adblock-paused-list" className="flex flex-col gap-1">
                {settings.adblock.allowedSites.map((site) => (
                  <li
                    id={`settings-adblock-paused-${site}`}
                    key={site}
                    className="flex h-8 items-center justify-between rounded-md bg-zinc-950 pl-3 pr-1 text-sm text-zinc-200"
                  >
                    <span id={`settings-adblock-paused-${site}-host`} className="truncate">
                      {site}
                    </span>
                    <IconButton
                      id={`settings-adblock-paused-${site}-remove`}
                      label={`Block ads on ${site} again`}
                      onClick={() =>
                        update({
                          adblock: {
                            ...settings.adblock,
                            allowedSites: settings.adblock.allowedSites.filter((item) => item !== site),
                          },
                        })
                      }
                    >
                      <X id={`settings-adblock-paused-${site}-remove-icon`} aria-hidden />
                    </IconButton>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </Section>

        <Section id="settings-appearance" title="Appearance">
          <Row id="settings-bookmarks-bar" label="Show bookmarks bar" hint="Ctrl+Shift+B">
            <Toggle
              id="settings-bookmarks-bar-toggle"
              checked={settings.showBookmarksBar}
              onChange={(value) => update({ showBookmarksBar: value })}
            />
          </Row>
          <Row id="settings-color-mode" label="Mode">
            <div id="settings-color-mode-options" className="flex rounded-md border border-zinc-700 p-0.5">
              {COLOR_MODE_OPTIONS.map((option) => (
                <button
                  key={option.id}
                  id={`settings-color-mode-${option.id}`}
                  type="button"
                  className={cn(
                    "h-7 rounded px-3 text-xs text-zinc-300 hover:text-zinc-100",
                    settings.colorMode === option.id && "bg-zinc-700 text-zinc-100",
                  )}
                  onClick={() => update({ colorMode: option.id })}
                >
                  {option.label}
                </button>
              ))}
            </div>
          </Row>
          <div id="settings-theme" className="px-5 py-3">
            <div id="settings-theme-label" className="mb-3 text-sm text-zinc-100">
              Theme
            </div>
            <div id="settings-theme-grid" className="grid grid-cols-5 gap-2">
              {THEMES.map((theme) => {
                const swatch = theme.swatch[resolved];
                const active = settings.theme === theme.id;
                return (
                  <button
                    key={theme.id}
                    id={`settings-theme-${theme.id}`}
                    type="button"
                    title={theme.label}
                    className={cn(
                      "relative flex flex-col overflow-hidden rounded-md border text-left",
                      active ? "border-sky-500 ring-1 ring-sky-500" : "border-zinc-700 hover:border-zinc-500",
                    )}
                    onClick={() => update({ theme: theme.id })}
                  >
                    <span id={`settings-theme-${theme.id}-preview`} className="flex h-10 items-end gap-1 p-1.5" style={{ background: swatch.background }}>
                      <span id={`settings-theme-${theme.id}-surface`} className="h-5 flex-1 rounded-sm" style={{ background: swatch.surface }} />
                      <span id={`settings-theme-${theme.id}-accent`} className="size-3 rounded-full" style={{ background: swatch.accent }} />
                    </span>
                    <span id={`settings-theme-${theme.id}-name`} className="truncate bg-zinc-900 px-1.5 py-1 text-[11px] text-zinc-300">
                      {theme.label}
                    </span>
                    {active ? (
                      <Check id={`settings-theme-${theme.id}-check`} className="absolute right-1 top-1 size-3.5 rounded-full bg-sky-500 p-0.5 text-white" />
                    ) : null}
                  </button>
                );
              })}
            </div>
          </div>
          <div id="settings-user-styles" className="flex flex-col gap-3 px-5 py-3">
            <div id="settings-user-styles-heading">
              <div id="settings-user-styles-label" className="text-sm text-zinc-100">
                Custom styles files
              </div>
              <p id="settings-user-styles-hint" className="mt-1 text-xs text-zinc-500">
                styles.css for this profile. The starter rules are commented out, so uncomment them to change the theme. The file loads when the browser opens.
              </p>
            </div>
            {userCssPath ? (
              <button
                id="settings-user-styles-path"
                type="button"
                title="Show in File Explorer"
                className="break-all text-left text-xs text-zinc-500 underline-offset-2 hover:text-sky-400 hover:underline"
                onClick={() => void showUserStyles().catch((reason: unknown) => setError(String(reason)))}
              >
                {userCssPath}
              </button>
            ) : null}
          </div>
          <Row id="settings-ui-zoom" label="Browser UI size" hint="Scales tabs, toolbars and ce:// pages.">
            {settings.uiZoom !== DEFAULT_ZOOM ? (
              <Button id="settings-ui-zoom-reset" size="xs" variant="outline" onClick={() => update({ uiZoom: DEFAULT_ZOOM })}>
                <RotateCcw id="settings-ui-zoom-reset-icon" className="size-3.5" aria-hidden />
                <span id="settings-ui-zoom-reset-label" className="text-xs text-zinc-400">Reset</span>
              </Button>
            ) : null}

            <IconButton
              id="settings-ui-zoom-out"
              label="Decrease browser UI size"
              disabled={settings.uiZoom <= MIN_ZOOM}
              onClick={() => update({ uiZoom: clampZoom(settings.uiZoom - ZOOM_STEP) })}
            >
              <Minus id="settings-ui-zoom-out-icon" aria-hidden />
            </IconButton>
            <span id="settings-ui-zoom-value" className="w-10 text-center text-xs text-zinc-400">
              {settings.uiZoom}%
            </span>
            <IconButton
              id="settings-ui-zoom-in"
              label="Increase browser UI size"
              disabled={settings.uiZoom >= MAX_ZOOM}
              onClick={() => update({ uiZoom: clampZoom(settings.uiZoom + ZOOM_STEP) })}
            >
              <Plus id="settings-ui-zoom-in-icon" aria-hidden />
            </IconButton>
          </Row>
          <Row
            id="settings-page-zoom"
            label="Web page zoom"
            hint="Default size for websites. Ctrl +/- changes the page you are on. Ctrl 0 returns to this size."
          >
            {pageZoom !== DEFAULT_ZOOM ? (
              <Button id="settings-page-zoom-reset" size="xs" variant="outline" onClick={() => update({ pageZoom: DEFAULT_ZOOM })} >
                <RotateCcw id="settings-page-zoom-reset-icon" className="size-3.5" aria-hidden />
                <span id="settings-page-zoom-reset-label" className="text-xs text-zinc-400">Reset</span>
              </Button>
            ) : null}
            <IconButton
              id="settings-page-zoom-out"
              label="Decrease web page zoom"
              disabled={pageZoom <= PAGE_ZOOM_LEVELS[0]}
              onClick={() => update({ pageZoom: stepPageZoom(pageZoom, -1) })}
            >
              <Minus id="settings-page-zoom-out-icon" aria-hidden />
            </IconButton>
            <span id="settings-page-zoom-value" className="w-10 text-center text-xs text-zinc-400">
              {pageZoom}%
            </span>
            <IconButton
              id="settings-page-zoom-in"
              label="Increase web page zoom"
              disabled={pageZoom >= PAGE_ZOOM_LEVELS[PAGE_ZOOM_LEVELS.length - 1]}
              onClick={() => update({ pageZoom: stepPageZoom(pageZoom, 1) })}
            >
              <Plus id="settings-page-zoom-in-icon" aria-hidden />
            </IconButton>
   
          </Row>
        </Section>

        <Section id="settings-sync" title="Sync">
          <Row
            id="settings-sync-folder"
            label="Folder location"
            hint="Profiles are stored in this folder. Selecting a folder uses the profiles already in it. Cookies and the download folder stay on this computer."
          >
            <Button
              id="settings-sync-create"
              size="sm"
              disabled={syncBusy}
              onClick={() => pickAndSync("Create a sync folder", createSyncFolder)}
            >
              Create
            </Button>
            <Button
              id="settings-sync-select"
              size="sm"
              disabled={syncBusy}
              onClick={() => pickAndSync("Select a sync folder", selectSyncFolder)}
            >
              Select
            </Button>
          </Row>
          {settings.syncFolder ? (
            <div id="settings-sync-path" className="flex items-center justify-between gap-6 px-5 py-3">
              <div id="settings-sync-path-text" className="min-w-0">
                <button
                  id="settings-sync-path-folder"
                  type="button"
                  title="Show in File Explorer"
                  className="break-all text-left text-xs text-zinc-400 underline-offset-2 hover:text-sky-400 hover:underline"
                  onClick={() => void showSyncFolder().catch((reason: unknown) => setError(String(reason)))}
                >
                  {settings.syncFolder}
                </button>
                <div id="settings-sync-path-files" className="mt-1 text-xs text-zinc-500">
                  profiles.json and a folder for each profile (Default, then one per profile name)
                </div>
              </div>
              <Button id="settings-sync-clear" size="sm" variant="ghost" disabled={syncBusy} onClick={() => runSync(() => clearSyncFolder())}>
                Clear
              </Button>
            </div>
          ) : null}
          {syncError || syncState?.error ? (
            <div id="settings-sync-error" className="px-5 py-3 text-sm text-red-300">
              {syncError ?? syncState?.error}
            </div>
          ) : null}
        </Section>
      </div>
    </PageShell>
  );
}
