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

/**
 * What the page's `navigator` and media queries report.
 * Strings and safe areas come from Chrome DevTools' emulated device list
 * (devtools-frontend `EmulatedDevices.ts`): Safari on iPhone and iPad, Chrome on Android.
 */
export type BrowserProfile = {
  /** Empty leaves the host browser alone. */
  userAgent: string;
  /** `navigator.platform`. */
  navigatorPlatform: string;
  maxTouchPoints: number;
  mobile: boolean;
  /** `navigator.userAgentData.platform`, such as `iOS` or `Android`. */
  clientPlatform: string;
  clientPlatformVersion: string;
  model: string;
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
  /** User agent, platform, and touch the page should see for this device. */
  browser: BrowserProfile;
  /** Standalone safe area while the screen is taller than it is wide. */
  portrait: Insets;
  /** Standalone safe area while the screen is wider than it is tall. */
  landscape: Insets;
};

export const DEVICE_GROUPS: DeviceGroup[] = ["Phones", "Tablets", "Computers"];

const ZERO: Insets = { top: 0, right: 0, bottom: 0, left: 0 };

const HOST: BrowserProfile = {
  userAgent: "",
  navigatorPlatform: "",
  maxTouchPoints: 0,
  mobile: false,
  clientPlatform: "",
  clientPlatformVersion: "",
  model: "",
};

/** Reduced Chrome token from the device list. Phone checks match Android and Mobile, not this number. */
const CHROME = "131.0.0.0";
const SAFARI_PHONE =
  "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1";
const SAFARI_PAD =
  "Mozilla/5.0 (iPad; CPU OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1";
const SAFARI_MAC =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Safari/605.1.15";

function iosPhone(): BrowserProfile {
  return {
    userAgent: SAFARI_PHONE,
    navigatorPlatform: "iPhone",
    maxTouchPoints: 5,
    mobile: true,
    clientPlatform: "iOS",
    clientPlatformVersion: "18.5",
    model: "iPhone",
  };
}

/** Smaller iPads still send an iPad user agent. Larger ones send a Mac user agent and report touch. */
function iosPad(desktop: boolean): BrowserProfile {
  return desktop
    ? {
        userAgent: SAFARI_MAC,
        navigatorPlatform: "MacIntel",
        maxTouchPoints: 5,
        mobile: true,
        clientPlatform: "iOS",
        clientPlatformVersion: "18.5",
        model: "iPad",
      }
    : {
        userAgent: SAFARI_PAD,
        navigatorPlatform: "iPad",
        maxTouchPoints: 5,
        mobile: true,
        clientPlatform: "iOS",
        clientPlatformVersion: "18.5",
        model: "iPad",
      };
}

function androidPhone(model: string, version: string): BrowserProfile {
  return {
    userAgent: `Mozilla/5.0 (Linux; Android ${version}; ${model}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${CHROME} Mobile Safari/537.36`,
    navigatorPlatform: "Linux armv8l",
    maxTouchPoints: 5,
    mobile: true,
    clientPlatform: "Android",
    clientPlatformVersion: version,
    model,
  };
}

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
    browser: iosPhone(),
    portrait: { top, right: 0, bottom, left: 0 },
    landscape: classic ? ZERO : { top: 0, right: top, bottom: 21, left: top },
  };
}

function android(
  id: string,
  label: string,
  width: number,
  height: number,
  model: string,
  version: string,
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
    browser: androidPhone(model, version),
    portrait,
    landscape,
  };
}

function tablet(id: string, label: string, width: number, height: number, desktop: boolean): DevicePreset {
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
    browser: iosPad(desktop),
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
    browser: HOST,
    portrait: ZERO,
    landscape: ZERO,
  };
}

/**
 * Screen CSS pixels and the standalone (installed) safe area.
 * Portrait insets are what `env(safe-area-inset-*)` returns with `viewport-fit=cover`.
 * Landscape insets are the same device rotated.
 * iPhone values match Chrome's device list. Pixel's top inset does too; its bottom inset is the
 * gesture bar, which that list still leaves at zero.
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
    "Pixel 9",
    "14",
    { top: 58, right: 0, bottom: 24, left: 0 },
    { top: 0, right: 0, bottom: 24, left: 58 },
  ),
  android(
    "galaxy-s24",
    "Galaxy S24",
    360,
    780,
    "SM-S921B",
    "14",
    { top: 34, right: 0, bottom: 15, left: 0 },
    { top: 30, right: 0, bottom: 15, left: 34 },
  ),
  tablet("ipad-mini", "iPad mini", 744, 1133, false),
  tablet("ipad-air", "iPad Air", 820, 1180, true),
  tablet("ipad-pro", "iPad Pro 13", 1032, 1376, true),
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
      browser: phone ? iosPhone() : HOST,
      portrait: phone ? { top: 20, right: 0, bottom: 0, left: 0 } : ZERO,
      landscape: ZERO,
    };
  }
  if (!cutout && home) {
    const insets = { top: 24, right: 0, bottom: 20, left: 0 };
    return {
      style: "none" as const,
      platform: "ios" as Platform,
      browser: iosPad(!phone),
      portrait: insets,
      landscape: insets,
    };
  }
  const top = phone ? 59 : 24;
  const bottom = home ? (phone ? 34 : 20) : 0;
  const portrait = { top, right: 0, bottom, left: 0 };
  const landscape = phone ? { top: 0, right: top, bottom: home ? 21 : 0, left: top } : portrait;
  return {
    style: "island" as const,
    platform: "ios" as Platform,
    browser: phone ? iosPhone() : iosPad(true),
    portrait,
    landscape,
  };
}

export type ViewportEnvironment = {
  style: CutoutStyle;
  platform: Platform;
  browser: BrowserProfile;
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
    return {
      style,
      platform: preset.platform,
      browser: preset.browser,
      insets: ZERO,
      maxInsets: ZERO,
    };
  }
  return {
    style,
    platform: preset.platform,
    browser: preset.browser,
    insets: portrait ? preset.portrait : preset.landscape,
    maxInsets: maxInsets(preset.portrait, preset.landscape),
  };
}

/** Cutout, home bar, installed, and keyboard apply only while a side is under 768 CSS pixels. */
export function viewportShowsMobileOptions(width: number, height: number) {
  return (width > 0 && width < 768) || (height > 0 && height < 768);
}

export function pushViewport(
  width: number,
  height: number,
  cutout: boolean,
  home: boolean,
  installed: boolean,
  keyboard: boolean,
) {
  const roundedWidth = Math.round(width);
  const roundedHeight = Math.round(height);
  const mobile = viewportShowsMobileOptions(roundedWidth, roundedHeight);
  const nextCutout = mobile && cutout;
  const nextHome = mobile && home;
  const nextInstalled = mobile && installed;
  const nextKeyboard = mobile && keyboard;
  const environment = viewportEnvironment(roundedWidth, roundedHeight, nextInstalled, nextCutout, nextHome);
  return setViewport({
    width: roundedWidth,
    height: roundedHeight,
    cutout: nextCutout,
    homeBar: nextHome,
    installed: nextInstalled,
    keyboard: nextKeyboard,
    insets: environment.insets,
    maxInsets: environment.maxInsets,
    cutoutStyle: environment.style,
    platform: environment.platform,
    browser: environment.browser,
  });
}
