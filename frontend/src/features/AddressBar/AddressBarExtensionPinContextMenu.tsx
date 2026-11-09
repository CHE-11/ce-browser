import { OpenAs } from "@/components/OpenAs";
import { PanelSide } from "@/components/PanelSide";
import { PinPlacement } from "@/components/PinPlacement";
import { PopupFrame } from "@/components/PopupFrame";
import { builtinTool } from "@/features/BuiltInExtensions/builtin";
import { closePopup } from "@/lib/browser-client";
import { extensionOpenAs, extensionPanelSide, saveExtensionOpenAs, saveExtensionPanelSide } from "@/lib/extension-open";
import { saveToolbarPlacement, toolbarPlacement } from "@/lib/toolbar-pin";
import { useSettingsState, useExtensions } from "@/lib/use-browser";
import type { ExtensionOpenAs, HostPanelSide, ToolbarPlacement } from "@type/browser";

type ExtensionPinContextMenuProps = {
  extensionId: string;
};

export function AddressBarExtensionPinContextMenu({ extensionId }: ExtensionPinContextMenuProps) {
  const { settings, loaded: settingsLoaded } = useSettingsState();
  const { extensions, loaded: extensionsLoaded } = useExtensions();
  const installed = extensions.find((extension) => extension.id === extensionId);
  const name = installed?.name ?? builtinTool(extensionId)?.name ?? "Extension";
  const place = toolbarPlacement(settings, extensionId);
  const canOpenAs = Boolean(installed?.popup);
  const sideOnly = Boolean(installed?.sidePanel) && !installed?.popup;
  const openAs = extensionOpenAs(settings, extensionId);

  const setPlace = (next: ToolbarPlacement) => {
    if (!settings) {
      return;
    }
    void saveToolbarPlacement(settings, extensionId, next).finally(() => void closePopup());
  };

  const setOpenAs = (mode: ExtensionOpenAs) => {
    if (!settings) {
      return;
    }
    const saved = saveExtensionOpenAs(settings, extensionId, mode);
    if (mode === "popover") {
      void saved.finally(() => void closePopup());
      return;
    }
    void saved;
  };

  const setSide = (side: HostPanelSide) => {
    if (!settings) {
      return;
    }
    void saveExtensionPanelSide(settings, extensionId, side);
  };

  return (
    <PopupFrame id="extension-pin-menu" width="menu" ready={settingsLoaded && extensionsLoaded}>
      <div id="extension-pin-menu-row" className="flex items-center gap-3 px-3 py-2">
        <span id="extension-pin-menu-name" className="min-w-0 flex-1 truncate text-sm text-zinc-100">
          {name}
        </span>
        <PinPlacement
          id="extension-pin-menu-pin"
          placement={place}
          disabled={!settings || !extensionId}
          onChange={setPlace}
        />
      </div>
      {canOpenAs || sideOnly ? (
        <div id="extension-pin-menu-open-as-row" className="flex items-center gap-3 border-t border-zinc-800 px-3 py-2">
          <span id="extension-pin-menu-open-as-label" className="min-w-0 flex-1 truncate text-sm text-zinc-400">
            Open as
          </span>
          {canOpenAs ? (
            <OpenAs
              id="extension-pin-menu-open-as"
              value={openAs}
              disabled={!settings}
              onChange={setOpenAs}
            />
          ) : (
            <span id="extension-pin-menu-side-only" className="text-sm text-zinc-300">
              Side panel
            </span>
          )}
          {openAs === "panel" || sideOnly ? (
            <PanelSide
              id="extension-pin-menu-side"
              side={extensionPanelSide(settings, extensionId)}
              disabled={!settings}
              onChange={setSide}
            />
          ) : null}
        </div>
      ) : null}
    </PopupFrame>
  );
}
