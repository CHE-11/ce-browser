import { useEffect, useState } from "react";
import { ImagePlus, Trash2 } from "lucide-react";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { BrowserProfile, ProfileIconChoice } from "@type/browser";
import { Button } from "@/components/Button";
import { cn } from "@/lib/cn";
import {
  chooseSavePath,
  createProfile,
  defaultDownloadDir,
  deleteProfile,
  installProfileShortcut,
  listProfileIcons,
  listProfiles,
  openProfile,
  pickProfileIcon,
  pinProfileTaskbar,
  saveProfileShortcut,
  setProfileIcon,
  setProfileIconText,
  showSavedFile,
  unpinProfileTaskbar,
} from "@/lib/browser-client";
import { SETTINGS_SECTIONS } from "../settings-sections";
import { inputClass, matchesQuery, Row, Section, useReportMatch, useSettingVisible, type SettingsSectionProps } from "../SettingsControls";
import { ProfilesArchiveDialog, type ProfileArchiveRequest } from "./ProfilesArchiveDialog";
import { PROFILE_COPY_CHOICES, ProfilesCopyDialog, type ProfileCopyRequest } from "./ProfilesCopyDialog";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-profiles")!;

const NEW_PROFILE_HINT =
  "Each profile has its own bookmarks, history, extensions, and settings, kept in a folder named after the profile (Default for the first one) in the sync folder or in app data. A ce-browser shortcut in that folder opens the profile. Its icon is used for a separate Start menu item and taskbar button.";
const ARCHIVE_LABEL = "Profile file";
const ARCHIVE_HINT =
  "Saves this profile's bookmarks, history, extensions, and settings in one encrypted file. Cookies stay on this computer.";

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
  terms.push("Shortcut");
  terms.push(profile.taskbar ? "Unpin" : "Pin to taskbar");
  if (!profile.startMenu) {
    terms.push("Add to Start");
  }
  if (profile.id !== "default" && !profile.current) {
    terms.push("Delete");
  }
  if (iconOpen) {
    terms.push(
      profile.current
        ? "This window, the Start menu, and the taskbar button use this icon."
        : "The Start menu and taskbar shortcut update now. That profile's window uses the icon the next time it opens.",
      "Show Icon Text",
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
  const [archive, setArchive] = useState<ProfileArchiveRequest | null>(null);
  const [archiveNote, setArchiveNote] = useState<string | null>(null);
  const [copying, setCopying] = useState<ProfileCopyRequest | null>(null);

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
    "App settings",
    ...PROFILE_COPY_CHOICES.flatMap((choice) => [choice.label, choice.hint]),
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
          setArchive({ mode, path });
        }
      })
      .catch((reason: unknown) => onError(String(reason)));
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
        <Button id="settings-profiles-export" size="sm" disabled={profileWorking} onClick={() => chooseArchive("export")}>
          Export
        </Button>
        <Button id="settings-profiles-import" size="sm" disabled={profileWorking} onClick={() => chooseArchive("import")}>
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
              current: profile.current,
            })
          }
          onPin={() => runProfiles(() => pinProfileTaskbar(profile.id))}
          onUnpin={() => runProfiles(() => unpinProfileTaskbar(profile.id))}
          onDownloadShortcut={() => downloadProfileShortcut(profile, onError)}
          onShortcut={() => runProfiles(() => installProfileShortcut(profile.id))}
          onRemove={() => setPendingDelete(profile.id)}
          onIconText={(enabled) => runProfiles(() => setProfileIconText(profile.id, enabled))}
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
      <ProfilesArchiveDialog
        request={archive}
        onClose={() => setArchive(null)}
        onDone={(note, next) => {
          if (next) {
            setProfiles(next);
          }
          setArchive(null);
          setArchiveNote(note);
        }}
      />
      <ProfilesCopyDialog request={copying} onClose={() => setCopying(null)} onProfiles={setProfiles} onError={onError} />
    </Section>
  );
}

function shortcutFileName(name: string) {
  const cleaned = name.replace(/[<>:"/\\|?*]/g, " ").replace(/\s+/g, " ").trim();
  return `ce-browser ${cleaned || "profile"}.lnk`;
}

function downloadProfileShortcut(profile: BrowserProfile, onError: (message: string | null) => void) {
  onError(null);
  const fileName = shortcutFileName(profile.name);
  void defaultDownloadDir()
    .catch(() => "")
    .then((folder) => chooseSavePath(folder ? `${folder}\\${fileName}` : fileName, "Shortcut", ["lnk"]))
    .then((path) => {
      if (!path) {
        return;
      }
      return saveProfileShortcut(profile.id, path).then(() => showSavedFile(path));
    })
    .catch((reason: unknown) => onError(String(reason)));
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
  onDownloadShortcut,
  onShortcut,
  onRemove,
  onIconText,
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
  onDownloadShortcut: () => void;
  onShortcut: () => void;
  onRemove: () => void;
  onIconText: (enabled: boolean) => void;
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
          <Button id={`settings-profile-${profile.id}-download-shortcut`} size="sm" disabled={working} onClick={onDownloadShortcut}>
            Shortcut
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
            <Button
              id={`settings-profile-${profile.id}-remove`}
              size="sm"
              variant="destructiveOutline"
              disabled={working}
              title="Delete"
              aria-label={`Delete ${profile.name}`}
              className="size-7 px-0"
              onClick={onRemove}
            >
              <Trash2 id={`settings-profile-${profile.id}-remove-icon`} className="size-3.5" aria-hidden />
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
          <label id={`settings-profile-${profile.id}-icon-text`} className="flex items-center gap-2 text-xs text-zinc-300">
            <input
              id={`settings-profile-${profile.id}-icon-text-input`}
              type="checkbox"
              checked={profile.iconText}
              disabled={working}
              onChange={(event) => onIconText(event.target.checked)}
            />
            Show Icon Text
          </label>
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
