import { useEffect, useState } from "react";
import { CircleUser } from "lucide-react";
import type { BrowserProfile } from "@type/browser";
import { IconButton } from "@/components/IconButton";
import { listProfiles, onProfilesChanged, openPopup } from "@/lib/browser-client";

/** Current profile, shown just left of the address-bar menu. */
export function ProfileSwitcherButton() {
  const [profile, setProfile] = useState<BrowserProfile | null>(null);

  useEffect(() => {
    let disposed = false;
    const apply = (profiles: BrowserProfile[]) => {
      if (!disposed) {
        setProfile(profiles.find((item) => item.current) ?? null);
      }
    };
    void listProfiles().then(apply).catch(() => undefined);
    const listen = onProfilesChanged(apply);
    return () => {
      disposed = true;
      void listen.then((stop) => stop());
    };
  }, []);

  const name = profile?.name || "Profile";

  return (
    <IconButton
      id="address-profile"
      label={`Switch profile, ${name}`}
      aria-haspopup="menu"
      onClick={(event) => void openPopup("profile-switcher", event.currentTarget)}
    >
      {profile?.icon ? (
        <img id="address-profile-icon" src={profile.icon} alt="" className="size-5 rounded-md" />
      ) : (
        <CircleUser id="address-profile-icon" aria-hidden />
      )}
    </IconButton>
  );
}
