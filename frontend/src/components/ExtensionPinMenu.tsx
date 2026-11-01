import { PinOff } from "lucide-react";
import { builtinTool } from "../extensions/builtin";
import { closePopup } from "../lib/browser-client";
import { saveToolbarPin } from "../lib/toolbar-pin";
import { useExtensions, useSettings } from "../lib/use-browser";
import { PopupFrame, PopupMenu, PopupMenuItem } from "./PopupFrame";

type ExtensionPinMenuProps = {
  extensionId: string;
};

export function ExtensionPinMenu({ extensionId }: ExtensionPinMenuProps) {
  const settings = useSettings();
  const { extensions } = useExtensions();
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
    <PopupFrame id="extension-pin-menu">
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
