import { useEffect, useState } from "react";
import { ImagePlus } from "lucide-react";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { BrowserProfile, ProfileCopyScope, ProfileIconChoice } from "@type/browser";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { cn } from "@/lib/cn";
import {
  copyProfile,
  createProfile,
  deleteProfile,
  exportProfileArchive,
  importProfileArchive,
  installProfileShortcut,
  listProfileIcons,
  listProfiles,
  openProfile,
  pickProfileIcon,
  pinProfileTaskbar,
  setProfileIcon,
  unpinProfileTaskbar,
} from "@/lib/browser-client";
import { SETTINGS_SECTIONS } from "./settings-sections";
import { inputClass, matchesQuery, Row, Section, useReportMatch, useSettingVisible, type SettingsSectionProps } from "./SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-profiles")!;

const NEW_PROFILE_HINT =
  "Each profile has its own bookmarks, history, extensions, and settings, kept in a folder named after the profile (Default for the first one) in the sync folder or in app data. Its icon is used for a separate Start menu item and taskbar button.";
const ARCHIVE_LABEL = "Profile file";
const ARCHIVE_HINT =
  "Saves this profile's bookmarks, history, extensions, and settings in one encrypted file. Cookies stay on this computer.";

type ArchivePrompt = {
  mode: "export" | "import";
  path: string;
  password: string;
  confirm: string;
  error: string | null;
};

type CopyPrompt = {
  id: string;
  sourceName: string;
  name: string;
  scope: ProfileCopyScope;
  current: boolean;
  error: string | null;
};

const COPY_CHOICES: { id: ProfileCopyScope; label: string; hint: string }[] = [
  {
    id: "settings",
    label: "App settings",
    hint: "Search, startup, downloads, appearance, and the other Settings values.",
  },
  {
    id: "data",
    label: "Profile data",
    hint: "App settings plus bookmarks, history, extensions, and the other files in the profile folder. Cookies stay on this computer.",
  },
  {
    id: "full",
    label: "Everything",
    hint: "Profile data plus cookies and saved logins.",
  },
];

function profileTerms(profile: BrowserProfile, icons: ProfileIconChoice[], iconOpen: boolean) {
  const terms = [
    profile.name,
    profile.current ? "This window" : "",
    profile.taskbar
      ? "Pinned to the taskbar."
      : profile.startMenu
        ? "Start menu item installed. Pin it for a taskbar button."
        : "No Start menu item yet.",
    "Change icon",
  ];
  if (!profile.current) {
    terms.push("Open");
  }
  terms.push("Copy");
  terms.push(profile.taskbar ? "Unpin" : "Pin to taskbar");
  if (!profile.startMenu) {
    terms.push("Add to Start");
  }
  if (profile.id !== "default" && !profile.current) {
    terms.push("Remove");
  }
  if (iconOpen) {
    terms.push(
      profile.current
        ? "This window, the Start menu, and the taskbar button use this icon."
        : "The Start menu and taskbar shortcut update now. That profile's window uses the icon the next time it opens.",
      "Choose an image",
      "Image",
      ...icons.flatMap((choice) => [choice.label, choice.id]),
    );
  }
  return terms;
}

export function ProfilesSection({ settings, query, onError, onMatch, revision }: SettingsSectionProps & { revision: number }) {
  const [profiles, setProfiles] = useState<BrowserProfile[]>([]);
  const [profileIcons, setProfileIcons] = useState<ProfileIconChoice[]>([]);
  const [profileName, setProfileName] = useState("");
  const [profileWorking, setProfileWorking] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<string | null>(null);
  const [iconProfile, setIconProfile] = useState<string | null>(null);
  const [archive, setArchive] = useState<ArchivePrompt | null>(null);
  const [archiveNote, setArchiveNote] = useState<string | null>(null);
  const [archiveWorking, setArchiveWorking] = useState(false);
  const [copying, setCopying] = useState<CopyPrompt | null>(null);

  useEffect(() => {
    let disposed = false;
    void listProfiles()
      .then((next) => {
        if (!disposed) {
          setProfiles(next);
        }
      })
      .catch((reason: unknown) => {
        if (!disposed) {
          onError(String(reason));
        }
      });
    void listProfileIcons()
      .then((next) => {
        if (!disposed) {
          setProfileIcons(next);
        }
      })
      .catch((reason: unknown) => {
        if (!disposed) {
          onError(String(reason));
        }
      });
    return () => {
      disposed = true;
    };
  }, [onError, revision, settings.syncFolder]);

  const pending = profiles.find((profile) => profile.id === pendingDelete);
  const confirmText = pendingDelete
    ? `Remove ${pending?.name ?? "this profile"}? Its bookmarks, history, extensions, settings, and Start menu item are deleted.`
    : "";
  const visible = matchesQuery(
    query,
    section.title,
    "New profile",
    NEW_PROFILE_HINT,
    "Create",
    profileName,
    ARCHIVE_LABEL,
    ARCHIVE_HINT,
    "Export",
    "Import",
    archiveNote ?? "",
    "Copy profile",
    ...COPY_CHOICES.flatMap((choice) => [choice.label, choice.hint]),
    ...profiles.flatMap((profile) => profileTerms(profile, profileIcons, iconProfile === profile.id)),
    confirmText,
    pendingDelete ? "Cancel" : "",
  );
  useReportMatch(section.id, visible, onMatch);

  const currentName = profiles.find((profile) => profile.current)?.name || "profile";

  const chooseArchive = (mode: "export" | "import") => {
    onError(null);
    setArchiveNote(null);
    const dialog =
      mode === "export"
        ? save({
            title: "Export profile",
            defaultPath: `${currentName}.ceprofile`,
            filters: [{ name: "ce-browser profile", extensions: ["ceprofile"] }],
          })
        : open({
            title: "Import profile",
            multiple: false,
            directory: false,
            filters: [{ name: "ce-browser profile", extensions: ["ceprofile"] }],
          });
    void Promise.resolve(dialog)
      .then((path) => {
        if (typeof path === "string" && path) {
          setArchive({ mode, path, password: "", confirm: "", error: null });
        }
      })
      .catch((reason: unknown) => onError(String(reason)));
  };

  const submitArchive = () => {
    if (!archive || archiveWorking) {
      return;
    }
    if (!archive.password) {
      setArchive({ ...archive, error: "Enter a password." });
      return;
    }
    if (archive.mode === "export" && archive.password !== archive.confirm) {
      setArchive({ ...archive, error: "Those passwords do not match." });
      return;
    }
    setArchiveWorking(true);
    const prompt = archive;
    const work =
      prompt.mode === "export"
        ? exportProfileArchive(prompt.path, prompt.password).then(() => "Profile exported.")
        : importProfileArchive(prompt.path, prompt.password).then((imported) => {
            return listProfiles().then((next) => {
              setProfiles(next);
              return `Imported as ${imported.name}. Open it from the list.`;
            });
          });
    void work
      .then((note) => {
        setArchive(null);
        setArchiveNote(note);
      })
      .catch((reason: unknown) => {
        setArchive((current) => (current ? { ...current, error: String(reason) } : current));
      })
      .finally(() => setArchiveWorking(false));
  };

  const submitCopy = () => {
    if (!copying || profileWorking) {
      return;
    }
    const name = copying.name.trim();
    if (!name) {
      setCopying({ ...copying, error: "A profile needs a name." });
      return;
    }
    if (copying.scope === "full" && copying.current) {
      setCopying({ ...copying, error: "Close that profile before copying its cookies and logins." });
      return;
    }
    setProfileWorking(true);
    onError(null);
    void copyProfile(copying.id, name, copying.scope)
      .then((next) => {
        setProfiles(next);
        setCopying(null);
      })
      .catch((reason: unknown) => {
        setCopying((current) => (current ? { ...current, error: String(reason) } : current));
        void listProfiles().then(setProfiles).catch(() => undefined);
      })
      .finally(() => setProfileWorking(false));
  };

  const runProfiles = (work: () => Promise<BrowserProfile[] | void>) => {
    setProfileWorking(true);
    onError(null);
    void work()
      .then((next) => {
        if (next) {
          setProfiles(next);
        }
      })
      .catch((reason: unknown) => {
        onError(String(reason));
        void listProfiles().then(setProfiles).catch(() => undefined);
      })
      .finally(() => setProfileWorking(false));
  };

  if (!visible) {
    return null;
  }

  return (
    <Section id={section.id} title={section.title}>
      <Row id="settings-profiles-new" label="New profile" hint={NEW_PROFILE_HINT} search={["Create", profileName]}>
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
      <Row id="settings-profiles-archive" label={ARCHIVE_LABEL} hint={ARCHIVE_HINT} search={["Export", "Import", archiveNote ?? ""]}>
        <Button id="settings-profiles-export" size="sm" disabled={profileWorking || archiveWorking} onClick={() => chooseArchive("export")}>
          Export
        </Button>
        <Button id="settings-profiles-import" size="sm" disabled={profileWorking || archiveWorking} onClick={() => chooseArchive("import")}>
          Import
        </Button>
      </Row>
      {archiveNote ? (
        <div id="settings-profiles-archive-note" className="px-5 pb-3 text-xs text-zinc-400">
          {archiveNote}
        </div>
      ) : null}
      {profiles.map((profile) => (
        <ProfileCard
          key={profile.id}
          profile={profile}
          icons={profileIcons}
          iconOpen={iconProfile === profile.id}
          working={profileWorking}
          onToggleIcons={() => setIconProfile((current) => (current === profile.id ? null : profile.id))}
          onOpen={() => runProfiles(() => openProfile(profile.id))}
          onCopy={() =>
            setCopying({
              id: profile.id,
              sourceName: profile.name,
              name: `${profile.name} copy`,
              scope: "settings",
              current: profile.current,
              error: null,
            })
          }
          onPin={() => runProfiles(() => pinProfileTaskbar(profile.id))}
          onUnpin={() => runProfiles(() => unpinProfileTaskbar(profile.id))}
          onShortcut={() => runProfiles(() => installProfileShortcut(profile.id))}
          onRemove={() => setPendingDelete(profile.id)}
          onChooseIcon={(iconId, path) => runProfiles(() => setProfileIcon(profile.id, iconId, path))}
          onPickFile={() =>
            void pickProfileIcon().then((path) => {
              if (path) {
                runProfiles(() => setProfileIcon(profile.id, "custom", path));
              }
            })
          }
        />
      ))}
      {pendingDelete ? (
        <DeleteConfirm
          text={confirmText}
          onCancel={() => setPendingDelete(null)}
          onRemove={() => {
            const id = pendingDelete;
            setPendingDelete(null);
            if (id) {
              runProfiles(() => deleteProfile(id));
            }
          }}
        />
      ) : null}
      <Dialog
        id="settings-profile-archive"
        title={archive?.mode === "import" ? "Import profile" : "Export profile"}
        open={archive !== null}
        onClose={() => {
          if (!archiveWorking) {
            setArchive(null);
          }
        }}
        panelClassName="max-w-md"
      >
        <form
          id="settings-profile-archive-form"
          className="flex flex-col gap-3"
          onSubmit={(event) => {
            event.preventDefault();
            submitArchive();
          }}
        >
          <p id="settings-profile-archive-path" className="break-all text-xs text-zinc-500">
            {archive?.path}
          </p>
          <label id="settings-profile-archive-password-label" className="flex flex-col gap-1 text-sm text-zinc-300">
            Password
            <input
              id="settings-profile-archive-password"
              type="password"
              autoFocus
              value={archive?.password ?? ""}
              className={inputClass}
              onChange={(event) => setArchive((current) => (current ? { ...current, password: event.target.value, error: null } : current))}
            />
          </label>
          {archive?.mode === "export" ? (
            <label id="settings-profile-archive-confirm-label" className="flex flex-col gap-1 text-sm text-zinc-300">
              Confirm password
              <input
                id="settings-profile-archive-confirm"
                type="password"
                value={archive.confirm}
                className={inputClass}
                onChange={(event) => setArchive((current) => (current ? { ...current, confirm: event.target.value, error: null } : current))}
              />
            </label>
          ) : null}
          {archive?.error ? (
            <p id="settings-profile-archive-error" className="text-sm text-red-300">
              {archive.error}
            </p>
          ) : null}
          <div id="settings-profile-archive-actions" className="flex justify-end gap-2">
            <Button id="settings-profile-archive-cancel" size="sm" variant="ghost" disabled={archiveWorking} onClick={() => setArchive(null)}>
              Cancel
            </Button>
            <Button id="settings-profile-archive-save" size="sm" type="submit" disabled={archiveWorking}>
              {archive?.mode === "import" ? "Import" : "Export"}
            </Button>
          </div>
        </form>
      </Dialog>
      <Dialog
        id="settings-profile-copy"
        title="Copy profile"
        open={copying !== null}
        onClose={() => {
          if (!profileWorking) {
            setCopying(null);
          }
        }}
        panelClassName="max-w-md"
      >
        <form
          id="settings-profile-copy-form"
          className="flex flex-col gap-3"
          onSubmit={(event) => {
            event.preventDefault();
            submitCopy();
          }}
        >
          <p id="settings-profile-copy-source" className="text-sm text-zinc-300">
            Copy {copying?.sourceName ?? "this profile"} into a new profile.
          </p>
          <label id="settings-profile-copy-name-label" className="flex flex-col gap-1 text-sm text-zinc-300">
            Name
            <input
              id="settings-profile-copy-name"
              autoFocus
              value={copying?.name ?? ""}
              className={inputClass}
              onChange={(event) => setCopying((current) => (current ? { ...current, name: event.target.value, error: null } : current))}
            />
          </label>
          <fieldset id="settings-profile-copy-scope" className="flex flex-col gap-2">
            <legend id="settings-profile-copy-scope-legend" className="mb-1 text-sm text-zinc-300">
              What to copy
            </legend>
            {COPY_CHOICES.map((choice) => {
              const disabled = choice.id === "full" && Boolean(copying?.current);
              return (
                <label
                  key={choice.id}
                  id={`settings-profile-copy-scope-${choice.id}-label`}
                  className={cn("flex items-start gap-2 text-sm", disabled ? "text-zinc-500" : "text-zinc-200")}
                >
                  <input
                    id={`settings-profile-copy-scope-${choice.id}`}
                    type="radio"
                    name="settings-profile-copy-scope"
                    className="mt-1"
                    checked={copying?.scope === choice.id}
                    disabled={disabled || profileWorking}
                    onChange={() => setCopying((current) => (current ? { ...current, scope: choice.id, error: null } : current))}
                  />
                  <span id={`settings-profile-copy-scope-${choice.id}-text`} className="flex flex-col">
                    <span id={`settings-profile-copy-scope-${choice.id}-title`}>{choice.label}</span>
                    <span id={`settings-profile-copy-scope-${choice.id}-hint`} className="text-xs text-zinc-500">
                      {disabled ? "Close this profile before copying cookies and saved logins." : choice.hint}
                    </span>
                  </span>
                </label>
              );
            })}
          </fieldset>
          {copying?.error ? (
            <p id="settings-profile-copy-error" className="text-sm text-red-300">
              {copying.error}
            </p>
          ) : null}
          <div id="settings-profile-copy-actions" className="flex justify-end gap-2">
            <Button id="settings-profile-copy-cancel" size="sm" variant="ghost" disabled={profileWorking} onClick={() => setCopying(null)}>
              Cancel
            </Button>
            <Button id="settings-profile-copy-save" size="sm" type="submit" disabled={profileWorking || !copying?.name.trim()}>
              Copy
            </Button>
          </div>
        </form>
      </Dialog>
    </Section>
  );
}

function ProfileCard({
  profile,
  icons,
  iconOpen,
  working,
  onToggleIcons,
  onOpen,
  onCopy,
  onPin,
  onUnpin,
  onShortcut,
  onRemove,
  onChooseIcon,
  onPickFile,
}: {
  profile: BrowserProfile;
  icons: ProfileIconChoice[];
  iconOpen: boolean;
  working: boolean;
  onToggleIcons: () => void;
  onOpen: () => void;
  onCopy: () => void;
  onPin: () => void;
  onUnpin: () => void;
  onShortcut: () => void;
  onRemove: () => void;
  onChooseIcon: (iconId: string, path?: string) => void;
  onPickFile: () => void;
}) {
  const visible = useSettingVisible(...profileTerms(profile, icons, iconOpen));
  if (!visible) {
    return null;
  }
  return (
    <div id={`settings-profile-${profile.id}`} className="flex flex-col gap-3 px-5 py-3">
      <div id={`settings-profile-${profile.id}-row`} className="flex items-center justify-between gap-6">
        <div id={`settings-profile-${profile.id}-text`} className="flex min-w-0 items-center gap-3">
          <button
            id={`settings-profile-${profile.id}-icon`}
            type="button"
            title="Change icon"
            aria-label={`Change icon for ${profile.name}`}
            className={cn(
              "size-9 shrink-0 overflow-hidden rounded-md border",
              iconOpen ? "border-sky-500 ring-1 ring-sky-500" : "border-zinc-700 hover:border-zinc-500",
            )}
            onClick={onToggleIcons}
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
            <Button id={`settings-profile-${profile.id}-open`} size="sm" disabled={working} onClick={onOpen}>
              Open
            </Button>
          ) : null}
          <Button id={`settings-profile-${profile.id}-copy-button`} size="sm" disabled={working} onClick={onCopy}>
            Copy
          </Button>
          {profile.taskbar ? (
            <Button id={`settings-profile-${profile.id}-unpin`} size="sm" disabled={working} onClick={onUnpin}>
              Unpin
            </Button>
          ) : (
            <Button id={`settings-profile-${profile.id}-pin`} size="sm" disabled={working} onClick={onPin}>
              Pin to taskbar
            </Button>
          )}
          {!profile.startMenu ? (
            <Button id={`settings-profile-${profile.id}-shortcut`} size="sm" disabled={working} onClick={onShortcut}>
              Add to Start
            </Button>
          ) : null}
          {profile.id !== "default" && !profile.current ? (
            <Button id={`settings-profile-${profile.id}-remove`} size="sm" variant="ghost" disabled={working} onClick={onRemove}>
              Remove
            </Button>
          ) : null}
        </div>
      </div>
      {iconOpen ? (
        <div id={`settings-profile-${profile.id}-icons`} className="flex flex-col gap-2 pl-12">
          <p id={`settings-profile-${profile.id}-icons-hint`} className="text-xs text-zinc-500">
            {profile.current
              ? "This window, the Start menu, and the taskbar button use this icon."
              : "The Start menu and taskbar shortcut update now. That profile's window uses the icon the next time it opens."}
          </p>
          <div id={`settings-profile-${profile.id}-icon-grid`} className="grid grid-cols-4 gap-2 sm:grid-cols-8">
            {icons.map((choice) => {
              const selected = profile.iconId === choice.id;
              return (
                <button
                  key={choice.id}
                  id={`settings-profile-${profile.id}-icon-${choice.id}`}
                  type="button"
                  title={choice.label}
                  disabled={working}
                  className={cn(
                    "flex flex-col items-center gap-1 rounded-md border p-1.5",
                    selected ? "border-sky-500 ring-1 ring-sky-500" : "border-zinc-700 hover:border-zinc-500",
                  )}
                  onClick={() => onChooseIcon(choice.id)}
                >
                  <img id={`settings-profile-${profile.id}-icon-${choice.id}-image`} src={choice.icon} alt="" className="size-10 rounded" />
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
              disabled={working}
              className={cn(
                "flex flex-col items-center gap-1 rounded-md border p-1.5",
                profile.iconId === "custom" ? "border-sky-500 ring-1 ring-sky-500" : "border-zinc-700 hover:border-zinc-500",
              )}
              onClick={onPickFile}
            >
              {profile.iconId === "custom" ? (
                <img id={`settings-profile-${profile.id}-icon-file-image`} src={profile.icon || "/icon.png"} alt="" className="size-10 rounded" />
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
  );
}

function DeleteConfirm({ text, onCancel, onRemove }: { text: string; onCancel: () => void; onRemove: () => void }) {
  const visible = useSettingVisible("Remove", "Cancel", text);
  if (!visible) {
    return null;
  }
  return (
    <div id="settings-profiles-confirm" className="flex flex-col gap-3 px-5 py-3">
      <p id="settings-profiles-confirm-text" className="text-sm text-zinc-200">
        {text}
      </p>
      <div id="settings-profiles-confirm-actions" className="flex justify-end gap-2">
        <Button id="settings-profiles-confirm-cancel" size="sm" variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <Button id="settings-profiles-confirm-remove" size="sm" variant="destructive" onClick={onRemove}>
          Remove
        </Button>
      </div>
    </div>
  );
}
