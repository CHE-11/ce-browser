import { isComponentExtension } from "@/components/ExtensionIcon";
import { OpenAs } from "@/components/OpenAs";
import { PanelSide } from "@/components/PanelSide";
import { PinPlacement } from "@/components/PinPlacement";
import { PopupFrame, PopupMenu, PopupMenuItem } from "@/components/PopupFrame";
import { builtinTool } from "@/built_in_extensions/builtin";
import { RemoveExtensionDialog } from "@/features/Extensions/RemoveExtensionDialog";
import { closePopup, removeExtension, removeHostExtension } from "@/lib/browser-client";
import { extensionOpenAs, extensionPanelSide, saveExtensionOpenAs, saveExtensionPanelSide } from "@/lib/extension-open";
import { saveToolbarPlacement, toolbarPlacement } from "@/lib/toolbar-pin";
import { useSettingsState, useExtensions, useHostExtensions } from "@/lib/use-browser";
import type { ExtensionOpenAs, HostPanelSide, ToolbarPlacement } from "@type/browser";
import { Trash2 } from "lucide-react";
import { useState } from "react";

type ExtensionPinContextMenuProps = {
  extensionId: string;
};

const HOST_PREFIX = "host:";

export function AddressBarExtensionPinContextMenu({ extensionId }: ExtensionPinContextMenuProps) {
  const { settings, loaded: settingsLoaded } = useSettingsState();
  const { extensions, loaded: extensionsLoaded } = useExtensions();
  const { extensions: hostExtensions, loaded: hostLoaded } = useHostExtensions();
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const installed = extensions.find((extension) => extension.id === extensionId);
  const hostId = extensionId.startsWith(HOST_PREFIX) ? extensionId.slice(HOST_PREFIX.length) : null;
  const host = hostId ? hostExtensions.find((extension) => extension.id === hostId) : undefined;
  const name = installed?.name ?? host?.name ?? builtinTool(extensionId)?.name ?? "Extension";
  const place = toolbarPlacement(settings, extensionId);
  const canOpenAs = Boolean(installed?.popup);
  const sideOnly = Boolean(installed?.sidePanel) && !installed?.popup;
  const openAs = extensionOpenAs(settings, extensionId);
  const removable = Boolean(host) || (installed !== undefined && !isComponentExtension(installed.id, installed.name));

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

  const remove = () => {
    if (!removable) {
      return;
    }
    setConfirmRemove(false);
    setError(null);
    (host ? removeHostExtension(host.id) : removeExtension(extensionId))
      .then(() => void closePopup())
      .catch((reason: unknown) => setError(String(reason)));
  };

  return (
    <PopupFrame id="extension-pin-menu" width="menu" ready={settingsLoaded && extensionsLoaded && hostLoaded}>
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
      {removable ? (
        <PopupMenu id="extension-pin-menu-actions" label={`${name} actions`} className="border-t border-zinc-800">
          <PopupMenuItem
            id="extension-pin-menu-remove"
            icon={<Trash2 />}
            label="Remove from browser"
            danger
            onClick={() => setConfirmRemove(true)}
          />
        </PopupMenu>
      ) : null}
      {error ? (
        <p
          id="extension-pin-menu-error"
          className="mx-2 mb-2 rounded-md border border-red-900 bg-red-950 px-2 py-1.5 text-xs text-red-300"
        >
          {error}
        </p>
      ) : null}

      <div id="extension-pin-menu-confirm-room" className={confirmRemove ? "min-h-56 w-80" : undefined} />
      <RemoveExtensionDialog
        id="extension-pin-menu-remove-dialog"
        name={removable ? name : null}
        open={confirmRemove}
        onCancel={() => setConfirmRemove(false)}
        onConfirm={remove}
      />
    </PopupFrame>
  );
}
