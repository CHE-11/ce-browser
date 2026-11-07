import { PopupFrame, PopupMenu, PopupMenuItem } from "@/components/PopupFrame";
import { builtinTool } from "@/features/BuiltInExtensions/builtin";
import { closePopup } from "@/lib/browser-client";
import { saveToolbarPlacement, toolbarPlacement } from "@/lib/toolbar-pin";
import { useSettingsState, useExtensions } from "@/lib/use-browser";
import type { ToolbarPlacement } from "@type/browser";
import { Check, PanelRight, Pin, PinOff } from "lucide-react";


type ExtensionPinContextMenuProps = {
  extensionId: string;
};

export function ExtensionPinContextMenu({ extensionId }: ExtensionPinContextMenuProps) {
  const { settings, loaded: settingsLoaded } = useSettingsState();
  const { extensions, loaded: extensionsLoaded } = useExtensions();
  const name =
    extensions.find((extension) => extension.id === extensionId)?.name ??
    builtinTool(extensionId)?.name ??
    "Extension";
  const place = toolbarPlacement(settings, extensionId);

  const setPlace = (next: ToolbarPlacement) => {
    if (!settings) {
      return;
    }
    void saveToolbarPlacement(settings, extensionId, next).finally(() => void closePopup());
  };

  return (
    <PopupFrame id="extension-pin-menu" width={240} ready={settingsLoaded && extensionsLoaded}>
      <PopupMenu id="extension-pin-menu-items" label={name}>
        <PopupMenuItem
          id="extension-pin-menu-primary"
          icon={place.primary ? <Check /> : <Pin />}
          label="Primary"
          title="Show next to the address bar"
          disabled={!settings || !extensionId}
          onClick={() => setPlace({ ...place, primary: !place.primary })}
        />
        <PopupMenuItem
          id="extension-pin-menu-secondary"
          icon={place.secondary ? <Check /> : <PanelRight />}
          label="Secondary"
          title="Show on the tools toolbar"
          disabled={!settings || !extensionId}
          onClick={() => setPlace({ ...place, secondary: !place.secondary })}
        />
        <PopupMenuItem
          id="extension-pin-menu-unpin"
          icon={<PinOff />}
          label="Unpinned"
          title="Hide from both bars"
          disabled={!settings || !extensionId || (!place.primary && !place.secondary)}
          onClick={() => setPlace({ primary: false, secondary: false })}
        />
      </PopupMenu>
    </PopupFrame>
  );
}
