import { PopupFrame, PopupMenu, PopupMenuItem } from "@/components/PopupFrame";
import { activateTab, closePopup, createTab, getSettings, navigate, saveSettings } from "@/lib/browser-client";
import { useSettingsState, useTabs } from "@/lib/use-browser";
import { Pin, Settings } from "lucide-react";

type PinContextMenuProps = {
  url: string;
};

/** Same comparison the settings list uses: one trailing slash still matches. */
function pinKey(url: string) {
  const trimmed = url.trim();
  if (!trimmed.endsWith("/")) {
    return trimmed;
  }
  const scheme = trimmed.indexOf("://");
  if (scheme < 0 || trimmed.length <= scheme + 4) {
    return trimmed;
  }
  return trimmed.slice(0, -1);
}

/** Address stored for the page in front of the user. A website contributes its origin. */
function currentSitePin(raw: string) {
  const trimmed = raw.trim();
  if (!trimmed || trimmed.toLowerCase() === "ce://newtab") {
    return "";
  }
  if (trimmed.slice(0, 5).toLowerCase() === "ce://") {
    const page = trimmed.slice(5).split(/[?#]/)[0].replace(/^\/+|\/+$/g, "");
    return page ? `ce://${page}` : "";
  }
  try {
    const parsed = new URL(trimmed);
    if (parsed.protocol === "http:" || parsed.protocol === "https:") {
      return parsed.origin;
    }
    if (parsed.protocol === "file:") {
      return trimmed.split(/[?#]/)[0];
    }
  } catch {
    return "";
  }
  return "";
}

export function PinContextMenu({ url }: PinContextMenuProps) {
  const { settings, loaded } = useSettingsState();
  const { windowTabs } = useTabs();
  const site = currentSitePin(url);
  const alreadyPinned = !!site && (settings?.pinnedTabs ?? []).some((pin) => pinKey(pin) === pinKey(site));

  const addSite = async () => {
    if (!site || alreadyPinned) {
      return;
    }
    const base = settings ?? (await getSettings());
    if (base.pinnedTabs.some((pin) => pinKey(pin) === pinKey(site))) {
      return;
    }
    await saveSettings({ ...base, pinnedTabs: [...base.pinnedTabs, site] });
  };

  const openPinnedTabs = async () => {
    const target = "ce://settings?focus=settings-pinned-tabs";
    const open = windowTabs.find((tab) => tab.url.startsWith("ce://settings"));
    if (open) {
      await activateTab(open.id);
      await navigate(open.id, target);
      return;
    }
    await createTab(target);
  };

  return (
    <PopupFrame id="pin-menu" width="menu" ready={loaded}>
      <PopupMenu id="pin-menu-items" label="Pinned tabs">
        <PopupMenuItem
          id="pin-menu-add"
          icon={<Pin />}
          label="Add current site to Pinned tabs"
          disabled={!site || alreadyPinned}
          title={alreadyPinned ? "This site is already in Pinned tabs" : undefined}
          onClick={() => void addSite().finally(() => void closePopup())}
        />
        <PopupMenuItem
          id="pin-menu-open"
          icon={<Settings />}
          label="Open Pinned tabs"
          onClick={() => void openPinnedTabs().finally(() => void closePopup())}
        />
      </PopupMenu>
    </PopupFrame>
  );
}
