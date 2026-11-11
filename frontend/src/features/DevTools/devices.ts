import { setViewport } from "@/lib/browser-client";

export type Platform = "ios" | "android" | "desktop";

/** Hardware cutout drawn over the page. */
export type CutoutStyle = "none" | "notch" | "island" | "punch";

export type Insets = {
  top: number;
  right: number;
  bottom: number;
  left: number;
};

export type DeviceGroup = "Phones" | "Tablets" | "Computers";

export type DevicePreset = {
  id: string;
  label: string;
  group: DeviceGroup;
  /** Screen size in CSS pixels, portrait for phones and tablets. */
  width: number;
  height: number;
  /** Draws a cutout when the cutout toggle is on. */
  cutout: boolean;
  /** Draws a home indicator or gesture bar. */
  home: boolean;
  style: CutoutStyle;
  platform: Platform;
  /** Standalone safe area while the screen is taller than it is wide. */
  portrait: Insets;
  /** Standalone safe area while the screen is wider than it is tall. */
  landscape: Insets;
};

export const DEVICE_GROUPS: DeviceGroup[] = ["Phones", "Tablets", "Computers"];

const ZERO: Insets = { top: 0, right: 0, bottom: 0, left: 0 };

/**
 * Face ID phones. Landscape moves the top inset to both sides and shortens the
 * home indicator. A home-button phone keeps a 20px status bar in portrait only.
 */
function iphone(
  id: string,
  label: string,
  width: number,
  height: number,
  style: CutoutStyle,
  top: number,
  bottom: number,
): DevicePreset {
  const classic = style === "none";
  return {
    id,
    label,
    group: "Phones",
    width,
    height,
    cutout: style !== "none",
    home: bottom > 0,
    style,
    platform: "ios",
    portrait: { top, right: 0, bottom, left: 0 },
    landscape: classic ? ZERO : { top: 0, right: top, bottom: 21, left: top },
  };
}

function android(
  id: string,
  label: string,
  width: number,
  height: number,
  portrait: Insets,
  landscape: Insets,
): DevicePreset {
  return {
    id,
    label,
    group: "Phones",
    width,
    height,
    cutout: true,
    home: true,
    style: "punch",
    platform: "android",
    portrait,
    landscape,
  };
}

function tablet(id: string, label: string, width: number, height: number): DevicePreset {
  const insets = { top: 24, right: 0, bottom: 20, left: 0 };
  return {
    id,
    label,
    group: "Tablets",
    width,
    height,
    cutout: false,
    home: true,
    style: "none",
    platform: "ios",
    portrait: insets,
    landscape: insets,
  };
}

function computer(id: string, label: string, width: number, height: number): DevicePreset {
  return {
    id,
    label,
    group: "Computers",
    width,
    height,
    cutout: false,
    home: false,
    style: "none",
    platform: "desktop",
    portrait: ZERO,
    landscape: ZERO,
  };
}

/**
 * Screen CSS pixels and the standalone (installed) safe area.
 * Portrait insets are what `env(safe-area-inset-*)` returns with `viewport-fit=cover`.
 * Landscape insets are the same device rotated.
 */
export const DEVICES: DevicePreset[] = [
  iphone("iphone-se", "iPhone SE", 375, 667, "none", 20, 0),
  iphone("iphone-16e", "iPhone 16e", 390, 844, "notch", 47, 34),
  iphone("iphone-16", "iPhone 16", 393, 852, "island", 59, 34),
  iphone("iphone-16-plus", "iPhone 16 Plus", 430, 932, "island", 59, 34),
  iphone("iphone-16-pro", "iPhone 16 Pro", 402, 874, "island", 62, 34),
  iphone("iphone-16-pro-max", "iPhone 16 Pro Max", 440, 956, "island", 62, 34),
  android(
    "pixel-9",
    "Pixel 9",
    412,
    923,
    { top: 48, right: 0, bottom: 24, left: 0 },
    { top: 24, right: 0, bottom: 24, left: 48 },
  ),
  android(
    "galaxy-s24",
    "Galaxy S24",
    360,
    780,
    { top: 34, right: 0, bottom: 15, left: 0 },
    { top: 30, right: 0, bottom: 15, left: 34 },
  ),
  tablet("ipad-mini", "iPad mini", 744, 1133),
  tablet("ipad-air", "iPad Air", 820, 1180),
  tablet("ipad-pro", "iPad Pro 13", 1032, 1376),
  computer("laptop", "Laptop", 1280, 800),
  computer("desktop", "Desktop", 1440, 900),
];

export function deviceFor(width: number, height: number) {
  return DEVICES.find(
    (device) =>
      (device.width === width && device.height === height) ||
      (device.width === height && device.height === width),
  );
}

function maxInsets(portrait: Insets, landscape: Insets): Insets {
  return {
    top: Math.max(portrait.top, landscape.top),
    right: Math.max(portrait.right, landscape.right),
    bottom: Math.max(portrait.bottom, landscape.bottom),
    left: Math.max(portrait.left, landscape.left),
  };
}

/** Custom sizes that don't match a device. Phones get an island-class inset. */
function fallback(width: number, height: number, cutout: boolean, home: boolean) {
  const shortSide = Math.min(width, height);
  const phone = shortSide > 0 && shortSide < 700;
  if (!cutout && !home) {
    return {
      style: "none" as const,
      platform: (phone ? "ios" : "desktop") as Platform,
      portrait: phone ? { top: 20, right: 0, bottom: 0, left: 0 } : ZERO,
      landscape: ZERO,
    };
  }
  if (!cutout && home) {
    const insets = { top: 24, right: 0, bottom: 20, left: 0 };
    return { style: "none" as const, platform: "ios" as Platform, portrait: insets, landscape: insets };
  }
  const top = phone ? 59 : 24;
  const bottom = home ? (phone ? 34 : 20) : 0;
  const portrait = { top, right: 0, bottom, left: 0 };
  const landscape = phone ? { top: 0, right: top, bottom: home ? 21 : 0, left: top } : portrait;
  return { style: "island" as const, platform: "ios" as Platform, portrait, landscape };
}

export type ViewportEnvironment = {
  style: CutoutStyle;
  platform: Platform;
  insets: Insets;
  maxInsets: Insets;
};

/**
 * What the page should see for this screen.
 * A normal mobile site gets no safe area: the browser already cleared the notch.
 * An installed app gets the device's standalone insets and `display-mode: standalone`.
 */
export function viewportEnvironment(
  width: number,
  height: number,
  installed: boolean,
  cutout: boolean,
  home: boolean,
): ViewportEnvironment {
  const device = deviceFor(width, height);
  const preset = device ?? fallback(width, height, cutout, home);
  const portrait = height >= width;
  const style: CutoutStyle = !cutout ? "none" : preset.style === "none" ? "island" : preset.style;
  if (!installed) {
    return { style, platform: preset.platform, insets: ZERO, maxInsets: ZERO };
  }
  return {
    style,
    platform: preset.platform,
    insets: portrait ? preset.portrait : preset.landscape,
    maxInsets: maxInsets(preset.portrait, preset.landscape),
  };
}

export function pushViewport(
  width: number,
  height: number,
  cutout: boolean,
  home: boolean,
  installed: boolean,
) {
  const roundedWidth = Math.round(width);
  const roundedHeight = Math.round(height);
  const environment = viewportEnvironment(roundedWidth, roundedHeight, installed, cutout, home);
  return setViewport({
    width: roundedWidth,
    height: roundedHeight,
    cutout,
    homeBar: home,
    installed,
    insets: environment.insets,
    maxInsets: environment.maxInsets,
    cutoutStyle: environment.style,
    platform: environment.platform,
  });
}
