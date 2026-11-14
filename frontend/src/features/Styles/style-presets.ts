import type { BrowserSettings } from "@type/browser";
import { CHROME, type ChromeKey } from "@/lib/chrome-metrics";
import { STYLE_SURFACES } from "./surfaces";

const CONTROLS = new Map(STYLE_SURFACES.flatMap((surface) => surface.controls.map((control) => [control.key, control])));

export const STYLE_PRESETS = [
  { id: "compact", label: "Compact", hint: "Smaller tabs, menus, and toolbars", factor: 0.85 },
  { id: "default", label: "Default", hint: "The sizes ce-browser ships with", factor: 1 },
  { id: "comfortable", label: "Comfortable", hint: "A little more room", factor: 1.15 },
  { id: "large", label: "Large", hint: "Taller rows and larger icons", factor: 1.3 },
] as const;

export type StylePresetId = (typeof STYLE_PRESETS)[number]["id"];

/** Every fine-tune size, scaled from the shipped defaults and clamped. A tab menu height of 0 stays 0. */
export function presetPatch(factor: number): Pick<BrowserSettings, ChromeKey> {
  const patch = {} as Pick<BrowserSettings, ChromeKey>;
  for (const [key, control] of CONTROLS) {
    if (key === "tabMenuHeight") {
      patch[key] = 0;
      continue;
    }
    const scaled = Math.min(control.max, Math.max(control.min, CHROME[key] * factor));
    patch[key] = control.unit === "rem" ? Math.round(scaled * 10000) / 10000 : Math.round(scaled);
  }
  return patch;
}

export function activeStylePreset(settings: BrowserSettings): StylePresetId | null {
  for (const preset of STYLE_PRESETS) {
    const patch = presetPatch(preset.factor);
    const match = (Object.keys(patch) as ChromeKey[]).every((key) => Math.abs(settings[key] - patch[key]) < 0.00005);
    if (match) {
      return preset.id;
    }
  }
  return null;
}
