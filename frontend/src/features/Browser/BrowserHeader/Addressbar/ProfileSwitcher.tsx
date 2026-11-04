import { useEffect, useState } from "react";
import { Check, Plus, Settings } from "lucide-react";
import type { BrowserProfile } from "@type/browser";
import { Menu, MenuItem, MenuSeparator } from "@/components/Menu";
import { PopupFrame } from "@/components/PopupFrame";
import { closeThisPopup, createTab, listProfiles, openProfile } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";

/** Profile list hung from the address-bar button. Another profile opens in its own window. */
export function ProfileSwitcher() {
  const [profiles, setProfiles] = useState<BrowserProfile[] | null>(null);
  const [opening, setOpening] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

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
          setProfiles([]);
          setError(String(reason));
        }
      });
    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    if (!profiles || profiles.length === 0) {
      return;
    }
    const current = profiles.find((profile) => profile.current) ?? profiles[0];
    document.getElementById(`profile-switcher-${current.id}`)?.focus();
  }, [profiles]);

  const open = (id: string) => {
    if (opening) {
      return;
    }
    setOpening(id);
    setError(null);
    void openProfile(id)
      .then(() => closeThisPopup())
      .catch((reason: unknown) => {
        setOpening(null);
        setError(String(reason));
      });
  };

  const openSettings = (focus: string) => {
    void createTab(`ce://settings?focus=${encodeURIComponent(focus)}`).finally(() => void closeThisPopup());
  };

  return (
    <PopupFrame id="profile-switcher" width="menu" ready={profiles !== null}>
      <Menu id="profile-switcher-menu" label="Profiles">
        {(profiles ?? []).map((profile) => (
          <button
            key={profile.id}
            id={`profile-switcher-${profile.id}`}
            type="button"
            role="menuitem"
            data-label={profile.name}
            aria-current={profile.current ? "true" : undefined}
            disabled={opening !== null}
            title={profile.current ? "This window" : `Open ${profile.name}`}
            className={cn(
              "flex h-9 w-full items-center gap-2.5 rounded-lg px-2.5 text-left text-sm text-zinc-100 hover:bg-zinc-800 disabled:opacity-40",
              focusRow,
              profile.current && "bg-zinc-800",
            )}
            onClick={() => (profile.current ? void closeThisPopup() : open(profile.id))}
          >
            <img
              id={`profile-switcher-${profile.id}-icon`}
              src={profile.icon || "/icon.png"}
              alt=""
              className="size-5 shrink-0 rounded"
            />
            <span id={`profile-switcher-${profile.id}-name`} className="min-w-0 flex-1 truncate">
              {profile.name}
            </span>
            {profile.current ? (
              <Check id={`profile-switcher-${profile.id}-check`} className="size-3.5 shrink-0 text-sky-400" aria-hidden />
            ) : null}
          </button>
        ))}
        <MenuSeparator id="profile-switcher-separator" />
        <MenuItem
          id="profile-switcher-new"
          icon={<Plus id="profile-switcher-new-glyph" aria-hidden />}
          label="New profile"
          disabled={opening !== null}
          onClick={() => openSettings("settings-profiles-name")}
        />
        <MenuItem
          id="profile-switcher-manage"
          icon={<Settings id="profile-switcher-manage-glyph" aria-hidden />}
          label="Manage profiles"
          disabled={opening !== null}
          onClick={() => openSettings("settings-profiles")}
        />
      </Menu>
      {error ? (
        <p id="profile-switcher-error" className="px-3 pb-2 text-xs text-red-300">
          {error}
        </p>
      ) : null}
    </PopupFrame>
  );
}
