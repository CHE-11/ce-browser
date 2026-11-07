import { useEffect, useRef } from "react";
import { Check, Moon, Sun } from "lucide-react";
import { Menu } from "@/components/Menu";
import { PopupFrame } from "@/components/PopupFrame";
import { closeThisPopup, getSettings, saveSettings, sendThemePreview } from "@/lib/browser-client";
import { THEMES, normalizeTheme, resolveMode, themeSurfaceStyle, type ThemeId } from "@/lib/appearance";
import { menuRowClass } from "@/lib/chrome-metrics";
import { cn } from "@/lib/cn";
import { focusRing, focusRow } from "@/lib/focus-ring";
import { useSettingsState } from "@/lib/use-browser";

export function ThemePicker() {
  const { settings, loaded } = useSettingsState();
  const committed = useRef(false);
  const focused = useRef(false);
  const savedId = normalizeTheme(settings?.theme);
  const mode = resolveMode(settings?.colorMode ?? "dark");

  useEffect(() => {
    if (!settings || focused.current) {
      return;
    }
    const row = document.getElementById(`theme-picker-${savedId}`);
    if (!row) {
      return;
    }
    focused.current = true;
    row.focus();
  }, [settings, savedId]);

  useEffect(() => {
    return () => {
      if (!committed.current) {
        void sendThemePreview(null);
      }
    };
  }, []);

  const restore = () => {
    if (!committed.current) {
      void sendThemePreview(null);
    }
  };

  const preview = (id: ThemeId) => {
    if (!committed.current) {
      void sendThemePreview(id);
    }
  };

  const setMode = (colorMode: "light" | "dark") => {
    if (!settings || settings.colorMode === colorMode) {
      return;
    }
    void getSettings()
      .then((current) => saveSettings({ ...current, colorMode }))
      .catch(() => undefined);
  };

  const choose = (id: ThemeId) => {
    if (!settings || committed.current) {
      return;
    }
    committed.current = true;
    const previous = settings.theme;
    void sendThemePreview(id, true)
      .then(() => getSettings())
      .then((current) => saveSettings({ ...current, theme: id }))
      .catch(() => {
        committed.current = false;
        return sendThemePreview(previous, true);
      })
      .finally(() => void closeThisPopup());
  };

  return (
    <PopupFrame id="theme-picker" width={248} maxHeight={720} ready={loaded}>
      <div id="theme-picker-body" onMouseLeave={restore}>
        <div id="theme-picker-mode" role="radiogroup" aria-label="Mode" className="flex gap-0.5 px-2 pt-2">
          <button
            id="theme-picker-mode-light"
            type="button"
            role="radio"
            aria-checked={mode === "light"}
            disabled={!settings}
            className={cn(
              "inline-flex h-7 flex-1 items-center justify-center gap-1.5 rounded-md text-xs text-zinc-400 hover:text-zinc-100 disabled:opacity-40",
              focusRing,
              mode === "light" && "bg-zinc-700 text-zinc-100",
            )}
            onClick={() => setMode("light")}
          >
            <Sun id="theme-picker-mode-light-icon" className="size-3.5" aria-hidden />
            Light
          </button>
          <button
            id="theme-picker-mode-dark"
            type="button"
            role="radio"
            aria-checked={mode === "dark"}
            disabled={!settings}
            className={cn(
              "inline-flex h-7 flex-1 items-center justify-center gap-1.5 rounded-md text-xs text-zinc-400 hover:text-zinc-100 disabled:opacity-40",
              focusRing,
              mode === "dark" && "bg-zinc-700 text-zinc-100",
            )}
            onClick={() => setMode("dark")}
          >
            <Moon id="theme-picker-mode-dark-icon" className="size-3.5" aria-hidden />
            Dark
          </button>
        </div>
        <Menu id="theme-picker-menu" label="Themes">
          {THEMES.map((theme) => {
            const swatch = theme.swatch[mode];
            const active = settings?.theme === theme.id;
            return (
              <button
                key={theme.id}
                id={`theme-picker-${theme.id}`}
                type="button"
                role="menuitem"
                data-label={theme.label}
                aria-current={active ? "true" : undefined}
                className={cn(
                  "flex w-full items-center gap-2.5 rounded-lg text-left text-zinc-100 hover:bg-zinc-800",
                  menuRowClass,
                  focusRow,
                  active && "bg-zinc-800",
                )}
                onMouseEnter={() => preview(theme.id)}
                onFocus={() => preview(theme.id)}
                onClick={() => choose(theme.id)}
              >
                <span
                  id={`theme-picker-${theme.id}-swatch`}
                  className="flex h-5 w-8 shrink-0 items-center gap-0.5 rounded-sm px-0.5"
                  style={{ background: swatch.background }}
                >
                  <span
                    id={`theme-picker-${theme.id}-surface`}
                    className="h-3 flex-1 rounded-[2px]"
                    style={themeSurfaceStyle(swatch)}
                  />
                  <span
                    id={`theme-picker-${theme.id}-accent`}
                    className="size-2 shrink-0 rounded-full"
                    style={{ background: swatch.accent }}
                  />
                </span>
                <span id={`theme-picker-${theme.id}-name`} className="min-w-0 flex-1 truncate">
                  {theme.label}
                </span>
                {active ? (
                  <Check id={`theme-picker-${theme.id}-check`} className="size-3.5 shrink-0 text-sky-400" aria-hidden />
                ) : null}
              </button>
            );
          })}
        </Menu>
      </div>
    </PopupFrame>
  );
}
