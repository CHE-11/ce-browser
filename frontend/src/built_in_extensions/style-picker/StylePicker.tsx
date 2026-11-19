import { useEffect, useRef } from "react";
import { Check, ExternalLink } from "lucide-react";
import { Menu, MenuSeparator } from "@/components/Menu";
import { PopupFrame } from "@/components/PopupFrame";
import { closeThisPopup, createTab, getSettings, saveSettings } from "@/lib/browser-client";
import { menuIconClass, menuRowClass } from "@/lib/chrome-metrics";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import { activeStylePreset, presetPatch, STYLE_PRESETS, type StylePresetId } from "@/features/Styles/style-presets";
import { useSettingsState } from "@/lib/use-browser";

export function StylePicker() {
  const { settings, loaded } = useSettingsState();
  const focused = useRef(false);
  const busy = useRef(false);
  const active = settings ? activeStylePreset(settings) : null;

  useEffect(() => {
    if (!loaded || focused.current) {
      return;
    }
    const row = document.getElementById(active ? `style-picker-${active}` : "style-picker-open");
    if (!row) {
      return;
    }
    focused.current = true;
    row.focus();
  }, [loaded, active]);

  const choose = (id: StylePresetId) => {
    if (busy.current) {
      return;
    }
    const preset = STYLE_PRESETS.find((item) => item.id === id);
    if (!preset) {
      return;
    }
    busy.current = true;
    void getSettings()
      .then((current) => saveSettings({ ...current, ...presetPatch(preset.factor) }))
      .then(() => closeThisPopup())
      .catch(() => {
        busy.current = false;
      });
  };

  const openPage = () => {
    void createTab("ce://styles").finally(() => void closeThisPopup());
  };

  return (
    <PopupFrame id="style-picker" width={248} maxHeight={720} ready={loaded}>
      <Menu id="style-picker-menu" label="Styles">
        {STYLE_PRESETS.map((preset) => {
          const selected = active === preset.id;
          return (
            <button
              key={preset.id}
              id={`style-picker-${preset.id}`}
              type="button"
              role="menuitem"
              data-label={preset.label}
              title={preset.hint}
              aria-current={selected ? "true" : undefined}
              className={cn(
                "flex w-full items-center gap-2.5 rounded-lg text-left text-zinc-100 hover:bg-zinc-800",
                menuRowClass,
                focusRow,
                selected && "bg-zinc-800",
              )}
              onClick={() => choose(preset.id)}
            >
              <PresetMark id={`style-picker-${preset.id}-mark`} factor={preset.factor} />
              <span id={`style-picker-${preset.id}-name`} className="min-w-0 flex-1 truncate">
                {preset.label}
              </span>
              {selected ? <Check id={`style-picker-${preset.id}-check`} className="size-3.5 shrink-0 text-sky-400" aria-hidden /> : null}
            </button>
          );
        })}
      </Menu>
      <MenuSeparator id="style-picker-separator" />
      <div id="style-picker-footer" className="px-1.5 pb-1.5">
        <button
          id="style-picker-open"
          type="button"
          className={cn(
            "flex w-full items-center gap-2.5 rounded-lg text-left text-zinc-100 hover:bg-zinc-800",
            menuRowClass,
            focusRow,
          )}
          onClick={openPage}
        >
          <ExternalLink id="style-picker-open-icon" className={cn(menuIconClass, "text-zinc-400")} aria-hidden />
          <span id="style-picker-open-label" className="min-w-0 flex-1 truncate">
            Open Styles
          </span>
        </button>
      </div>
    </PopupFrame>
  );
}

function PresetMark({ id, factor }: { id: string; factor: number }) {
  const bars = [0.55, 0.8, 1];
  return (
    <span id={id} className="flex h-5 w-8 shrink-0 items-end justify-center gap-0.5" aria-hidden>
      {bars.map((bar, index) => (
        <span
          key={bar}
          id={`${id}-${index}`}
          className="w-1.5 rounded-sm bg-zinc-400"
          style={{ height: `${Math.max(0.35, bar * factor) * 0.7}rem` }}
        />
      ))}
    </span>
  );
}
