import { getSettings, getWindowsColorScheme, saveSettings, setWindowsColorScheme } from "@/lib/browser-client";
import { noteWindowsScheme, publishAppearance, type ResolvedMode } from "@/lib/appearance";

let inflight: Promise<ResolvedMode> | null = null;

/**
 * Flips Windows light and dark mode. Websites that read `prefers-color-scheme` follow.
 * When the browser is set to light or dark, it switches with Windows. Match system already follows.
 * `from` is the mode the button is showing. Without it, the saved browser mode or Windows is used.
 */
export function toggleSystemColorScheme(from?: ResolvedMode) {
  if (inflight) {
    return inflight;
  }
  inflight = flip(from).finally(() => {
    inflight = null;
  });
  return inflight;
}

async function flip(from?: ResolvedMode): Promise<ResolvedMode> {
  const settings = await getSettings();
  const current: ResolvedMode =
    from ??
    (settings.colorMode === "light" || settings.colorMode === "dark" ? settings.colorMode : await getWindowsColorScheme());
  const next: ResolvedMode = current === "dark" ? "light" : "dark";
  await setWindowsColorScheme(next);
  noteWindowsScheme(next);
  if (settings.colorMode !== "system") {
    publishAppearance(settings.theme, next);
    await saveSettings({ ...settings, colorMode: next });
  }
  return next;
}
