import type { BrowserSettings, ExtensionOpenAs, HostPanelSide } from "@type/browser";
import { placeExtensionPanel, saveSettings } from "./browser-client";

/** CSS pixels. A missing width uses `fallback`. */
export const STORE_PANEL_WIDTH = { min: 240, max: 720, fallback: 380 };

/** A missing id opens as a popover, including an older settings file. */
export function extensionOpenAs(settings: BrowserSettings | null, id: string): ExtensionOpenAs {
  return settings?.extensionOpenAs?.[id] === "panel" ? "panel" : "popover";
}

export function saveExtensionOpenAs(settings: BrowserSettings, id: string, mode: ExtensionOpenAs) {
  const extensionOpenAs = { ...(settings.extensionOpenAs ?? {}) };
  if (mode === "popover") {
    delete extensionOpenAs[id];
  } else {
    extensionOpenAs[id] = mode;
  }
  return saveSettings({ ...settings, extensionOpenAs });
}

/** A missing id docks on the right, including an older settings file. */
export function extensionPanelSide(settings: BrowserSettings | null, id: string): HostPanelSide {
  return settings?.extensionPanelSide?.[id] === "left" ? "left" : "right";
}

export function saveExtensionPanelSide(settings: BrowserSettings, id: string, side: HostPanelSide) {
  const extensionPanelSide = { ...(settings.extensionPanelSide ?? {}) };
  if (side === "right") {
    delete extensionPanelSide[id];
  } else {
    extensionPanelSide[id] = side;
  }
  return saveSettings({ ...settings, extensionPanelSide }).then((saved) =>
    placeExtensionPanel(id).then(
      () => saved,
      () => saved,
    ),
  );
}

/** A missing id opens at 380 CSS pixels. */
export function extensionPanelWidth(settings: BrowserSettings | null, id: string) {
  const saved = settings?.extensionPanelWidth?.[id];
  if (typeof saved !== "number" || !Number.isFinite(saved)) {
    return STORE_PANEL_WIDTH.fallback;
  }
  return Math.min(STORE_PANEL_WIDTH.max, Math.max(STORE_PANEL_WIDTH.min, Math.round(saved)));
}

export function saveExtensionPanelWidth(settings: BrowserSettings, id: string, width: number) {
  const next = Math.min(STORE_PANEL_WIDTH.max, Math.max(STORE_PANEL_WIDTH.min, Math.round(width)));
  const extensionPanelWidth = { ...(settings.extensionPanelWidth ?? {}) };
  if (next === STORE_PANEL_WIDTH.fallback) {
    delete extensionPanelWidth[id];
  } else {
    extensionPanelWidth[id] = next;
  }
  return saveSettings({ ...settings, extensionPanelWidth }).then((saved) =>
    placeExtensionPanel(id).then(
      () => saved,
      () => saved,
    ),
  );
}

/** A missing id can be dragged. */
export function extensionPanelResizable(settings: BrowserSettings | null, id: string) {
  return settings?.extensionPanelResizable?.[id] !== false;
}

export function saveExtensionPanelResizable(settings: BrowserSettings, id: string, resizable: boolean) {
  const extensionPanelResizable = { ...(settings.extensionPanelResizable ?? {}) };
  if (resizable) {
    delete extensionPanelResizable[id];
  } else {
    extensionPanelResizable[id] = false;
  }
  return saveSettings({ ...settings, extensionPanelResizable }).then((saved) =>
    placeExtensionPanel(id).then(
      () => saved,
      () => saved,
    ),
  );
}
