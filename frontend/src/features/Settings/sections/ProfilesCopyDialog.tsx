import { useState } from "react";
import type { BrowserProfile, ProfileCopyPart } from "@type/browser";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { Toggle } from "@/components/Toggle";
import { cn } from "@/lib/cn";
import { copyProfile, listProfiles } from "@/lib/browser-client";
import { inputClass } from "../SettingsControls";

export const PROFILE_COPY_CHOICES: {
  id: ProfileCopyPart;
  label: string;
  hint: string;
  /** The source profile has to be closed. WebView2 keeps these files locked while it is open. */
  needsClosed?: boolean;
}[] = [
  {
    id: "search",
    label: "Search",
    hint: "Search engine, custom search URL, and keywords.",
  },
  {
    id: "history",
    label: "History",
    hint: "Visited pages and search history, including indexed page text.",
  },
  {
    id: "appearance",
    label: "Appearance",
    hint: "Theme, colors, zoom, backdrop, toasts, the status bar, and custom styles.",
  },
  {
    id: "downloads",
    label: "Downloads",
    hint: "Download folder, save prompts, and the download list.",
  },
  {
    id: "bookmarks",
    label: "Bookmarks",
    hint: "Saved bookmarks and the bookmarks bar.",
  },
  {
    id: "extensions",
    label: "Extensions",
    hint: "Installed extensions and which ones stay on the toolbar.",
  },
  {
    id: "cookies",
    label: "Cookies",
    hint: "Site cookies. Close this profile first.",
    needsClosed: true,
  },
  {
    id: "logins",
    label: "Saved logins",
    hint: "Passwords saved on websites. Close this profile first.",
    needsClosed: true,
  },
  {
    id: "siteData",
    label: "Site data",
    hint: "Local storage and other data sites saved here. Close this profile first.",
    needsClosed: true,
  },
  {
    id: "tabs",
    label: "Tabs",
    hint: "Layout, workspaces, sleep, and previews.",
  },
  {
    id: "startup",
    label: "Startup",
    hint: "Home page, what opens at startup, and pinned tabs.",
  },
  {
    id: "openTabs",
    label: "Open tabs",
    hint: "Tabs that were open. They return when startup restores the session.",
  },
  {
    id: "privacy",
    label: "Privacy",
    hint: "Tracking, fingerprint shield, HTTPS-only, and site permissions.",
  },
  {
    id: "adblock",
    label: "Ad blocker",
    hint: "Blocking and the sites you paused.",
  },
  {
    id: "ai",
    label: "AI",
    hint: "Provider, endpoint, and model, plus the saved API key.",
  },
  {
    id: "shopping",
    label: "Shopping",
    hint: "Region, model, competitors, and tracked prices.",
  },
  {
    id: "translate",
    label: "Translate",
    hint: "Language and provider.",
  },
  {
    id: "system",
    label: "System",
    hint: "Summon shortcut, battery saver, and automatic updates.",
  },
  {
    id: "shortcuts",
    label: "Shortcuts",
    hint: "Keyboard shortcuts.",
  },
  {
    id: "rewrites",
    label: "Address rewrites",
    hint: "Rules that change addresses as you open them.",
  },
  {
    id: "boards",
    label: "Boards",
    hint: "Pin boards and their pictures.",
  },
  {
    id: "passwords",
    label: "Password generator",
    hint: "Length and character choices.",
  },
  {
    id: "print",
    label: "Print and capture",
    hint: "Last choices in Print to PDF and Page to PNG.",
  },
  {
    id: "window",
    label: "Window",
    hint: "Size and position.",
  },
  {
    id: "icon",
    label: "Profile icon",
    hint: "The icon on the window, Start menu, and taskbar.",
  },
];

const PINNED = new Set<ProfileCopyPart>(["appearance", "system"]);
const DEFAULT_ON = new Set<ProfileCopyPart>(["appearance", "system", "search"]);

const ORDERED_COPY_CHOICES = [
  ...["appearance", "system"].map((id) => PROFILE_COPY_CHOICES.find((choice) => choice.id === id)!),
  ...PROFILE_COPY_CHOICES.filter((choice) => !PINNED.has(choice.id)).sort((a, b) =>
    a.label.localeCompare(b.label, undefined, { sensitivity: "base" }),
  ),
];

function defaultParts(): Record<ProfileCopyPart, boolean> {
  return Object.fromEntries(PROFILE_COPY_CHOICES.map((choice) => [choice.id, DEFAULT_ON.has(choice.id)])) as Record<
    ProfileCopyPart,
    boolean
  >;
}

export type ProfileCopyRequest = {
  id: string;
  sourceName: string;
  name: string;
  current: boolean;
};

export function ProfilesCopyDialog({
  request,
  onClose,
  onProfiles,
  onError,
}: {
  request: ProfileCopyRequest | null;
  onClose: () => void;
  onProfiles: (profiles: BrowserProfile[]) => void;
  onError: (message: string | null) => void;
}) {
  if (!request) {
    return null;
  }
  return <CopyForm key={request.id} request={request} onClose={onClose} onProfiles={onProfiles} onError={onError} />;
}

function CopyForm({
  request,
  onClose,
  onProfiles,
  onError,
}: {
  request: ProfileCopyRequest;
  onClose: () => void;
  onProfiles: (profiles: BrowserProfile[]) => void;
  onError: (message: string | null) => void;
}) {
  const [name, setName] = useState(request.name);
  const [parts, setParts] = useState(defaultParts);
  const [error, setError] = useState<string | null>(null);
  const [working, setWorking] = useState(false);
  const selected = PROFILE_COPY_CHOICES.filter((choice) => parts[choice.id] && !(choice.needsClosed && request.current)).map(
    (choice) => choice.id,
  );

  const setAll = (on: boolean) => {
    setParts((current) => {
      const next = { ...current };
      for (const choice of PROFILE_COPY_CHOICES) {
        if (choice.needsClosed && request.current) {
          next[choice.id] = false;
          continue;
        }
        next[choice.id] = on;
      }
      return next;
    });
    setError(null);
  };

  const submit = () => {
    if (working) {
      return;
    }
    const nextName = name.trim();
    if (!nextName) {
      setError("A profile needs a name.");
      return;
    }
    if (selected.length === 0) {
      setError("Choose at least one thing to copy.");
      return;
    }
    setWorking(true);
    onError(null);
    void copyProfile(request.id, nextName, selected)
      .then((next) => {
        onProfiles(next);
        onClose();
      })
      .catch((reason: unknown) => {
        setError(String(reason));
        setWorking(false);
        void listProfiles().then(onProfiles).catch(() => undefined);
      });
  };

  return (
    <Dialog
      id="settings-profile-copy"
      title="Copy profile"
      open
      onClose={() => {
        if (!working) {
          onClose();
        }
      }}
      panelClassName="h-[80vh] max-h-[90vh] max-w-4xl"
      bodyClassName="flex flex-col overflow-hidden"
    >
      <form
        id="settings-profile-copy-form"
        className="flex min-h-0 flex-1 flex-col gap-3"
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <p id="settings-profile-copy-source" className="text-sm text-zinc-300">
          Copy {request.sourceName} into a new profile. Turn on each part you want.
        </p>
        <label id="settings-profile-copy-name-label" className="flex flex-col gap-1 text-sm text-zinc-300">
          Name
          <input
            id="settings-profile-copy-name"
            autoFocus
            value={name}
            className={inputClass}
            onChange={(event) => {
              setName(event.target.value);
              setError(null);
            }}
          />
        </label>
        <fieldset id="settings-profile-copy-parts-field" aria-labelledby="settings-profile-copy-parts-legend" className="flex min-h-0 flex-1 flex-col gap-2">
          <div id="settings-profile-copy-parts-heading" className="flex items-center justify-between gap-2">
            <span id="settings-profile-copy-parts-legend" className="text-sm text-zinc-300">
              What to copy
            </span>
            <div id="settings-profile-copy-parts-bulk" className="flex gap-1">
              <Button id="settings-profile-copy-all" size="xs" variant="ghost" disabled={working} onClick={() => setAll(true)}>
                All
              </Button>
              <Button id="settings-profile-copy-none" size="xs" variant="ghost" disabled={working} onClick={() => setAll(false)}>
                None
              </Button>
            </div>
          </div>
          <div id="settings-profile-copy-parts" className="min-h-0 flex-1 overflow-auto">
            <table id="settings-profile-copy-table" className="w-full border-collapse text-left text-sm">
              <thead id="settings-profile-copy-table-head" className="sticky top-0 z-10 bg-zinc-900">
                <tr id="settings-profile-copy-table-head-row" className="text-xs text-zinc-500">
                  <th id="settings-profile-copy-table-name" className="w-44 border-b border-zinc-800 px-2 py-2 font-medium">
                    Name
                  </th>
                  <th id="settings-profile-copy-table-description" className="border-b border-zinc-800 px-2 py-2 font-medium">
                    Description
                  </th>
                  <th id="settings-profile-copy-table-toggle" className="w-16 border-b border-zinc-800 px-2 py-2 text-right font-medium">
                    Toggle
                  </th>
                </tr>
              </thead>
              <tbody id="settings-profile-copy-table-body">
                {ORDERED_COPY_CHOICES.map((choice) => {
                  const locked = Boolean(choice.needsClosed && request.current);
                  const checked = locked ? false : parts[choice.id];
                  return (
                    <tr id={`settings-profile-copy-part-${choice.id}`} key={choice.id} className="border-b border-zinc-800/80 last:border-b-0">
                      <td id={`settings-profile-copy-part-${choice.id}-name`} className="px-2 py-2 align-middle">
                        <label
                          id={`settings-profile-copy-part-${choice.id}-label`}
                          htmlFor={`settings-profile-copy-part-${choice.id}-toggle`}
                          className={cn("block", locked ? "text-zinc-500" : "cursor-pointer text-zinc-200")}
                        >
                          {choice.label}
                        </label>
                      </td>
                      <td id={`settings-profile-copy-part-${choice.id}-description`} className="px-2 py-2 align-middle text-xs text-zinc-500">
                        {locked ? "Close this profile before copying this." : choice.hint}
                      </td>
                      <td id={`settings-profile-copy-part-${choice.id}-switch`} className="px-2 py-2 text-right align-middle">
                        <Toggle
                          id={`settings-profile-copy-part-${choice.id}-toggle`}
                          size="sm"
                          checked={checked}
                          disabled={locked || working}
                          onChange={(value) => {
                            setParts((current) => ({ ...current, [choice.id]: value }));
                            setError(null);
                          }}
                        />
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </fieldset>
        {error ? (
          <p id="settings-profile-copy-error" className="text-sm text-red-300">
            {error}
          </p>
        ) : null}
        <div id="settings-profile-copy-actions" className="flex justify-end gap-2">
          <Button id="settings-profile-copy-cancel" size="sm" variant="ghost" disabled={working} onClick={onClose}>
            Cancel
          </Button>
          <Button id="settings-profile-copy-save" size="sm" type="submit" disabled={working || !name.trim() || selected.length === 0}>
            Copy
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
