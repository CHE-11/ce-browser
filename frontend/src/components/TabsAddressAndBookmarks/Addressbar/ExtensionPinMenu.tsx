import { PopupFrame, PopupMenu, PopupMenuItem } from "@/components/PopupFrame";
import { builtinTool } from "@/features/BuiltInExtensions/builtin";
import { closePopup } from "@/lib/browser-client";
import { saveToolbarPin } from "@/lib/toolbar-pin";
import { useSettingsState, useExtensions } from "@/lib/use-browser";
import { PinOff } from "lucide-react";


type ExtensionPinMenuProps = {
  extensionId: string;
};

export function ExtensionPinMenu({ extensionId }: ExtensionPinMenuProps) {
  const { settings, loaded: settingsLoaded } = useSettingsState();
  const { extensions, loaded: extensionsLoaded } = useExtensions();
  const name =
    extensions.find((extension) => extension.id === extensionId)?.name ??
    builtinTool(extensionId)?.name ??
    "Extension";

  const unpin = () => {
    if (!settings) {
      return;
    }
    void saveToolbarPin(settings, extensionId, false).finally(() => void closePopup());
  };

  return (
    <PopupFrame id="extension-pin-menu" ready={settingsLoaded && extensionsLoaded}>
      <PopupMenu id="extension-pin-menu-items" label={name}>
        <PopupMenuItem
          id="extension-pin-menu-unpin"
          icon={<PinOff />}
          label="Unpin from toolbar"
          disabled={!settings || !extensionId}
          onClick={unpin}
        />
      </PopupMenu>
    </PopupFrame>
  );
}
