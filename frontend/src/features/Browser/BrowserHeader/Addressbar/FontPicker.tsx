import { useEffect, useRef } from "react";
import { Check } from "lucide-react";
import { Menu } from "@/components/Menu";
import { PopupFrame } from "@/components/PopupFrame";
import { closeThisPopup, getSettings, saveSettings, sendFontPreview } from "@/lib/browser-client";
import { FONTS, normalizeFont, type FontId } from "@/lib/appearance";
import { menuRowClass } from "@/lib/chrome-metrics";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";
import { useSettingsState } from "@/lib/use-browser";

export function FontPicker() {
  const { settings, loaded } = useSettingsState();
  const committed = useRef(false);
  const focused = useRef(false);
  const savedId = normalizeFont(settings?.uiFont);

  useEffect(() => {
    if (!settings || focused.current) {
      return;
    }
    const row = document.getElementById(`font-picker-${savedId}`);
    if (!row) {
      return;
    }
    focused.current = true;
    row.focus();
  }, [settings, savedId]);

  useEffect(() => {
    return () => {
      if (!committed.current) {
        void sendFontPreview(null);
      }
    };
  }, []);

  const restore = () => {
    if (!committed.current) {
      void sendFontPreview(null);
    }
  };

  const preview = (id: FontId) => {
    if (!committed.current) {
      void sendFontPreview(id);
    }
  };

  const choose = (id: FontId) => {
    if (!settings || committed.current) {
      return;
    }
    committed.current = true;
    const previous = settings.uiFont;
    void sendFontPreview(id, true)
      .then(() => getSettings())
      .then((current) => saveSettings({ ...current, uiFont: id }))
      .catch(() => {
        committed.current = false;
        return sendFontPreview(previous, true);
      })
      .finally(() => void closeThisPopup());
  };

  return (
    <PopupFrame id="font-picker" width={280} maxHeight={720} ready={loaded}>
      <div id="font-picker-body" onMouseLeave={restore}>
        <Menu id="font-picker-menu" label="Fonts">
          {FONTS.map((font) => {
            const active = settings?.uiFont === font.id;
            return (
              <button
                key={font.id}
                id={`font-picker-${font.id}`}
                type="button"
                role="menuitem"
                data-label={font.label}
                aria-current={active ? "true" : undefined}
                className={cn(
                  "flex w-full items-center gap-2.5 rounded-lg text-left text-zinc-100 hover:bg-zinc-800",
                  menuRowClass,
                  focusRow,
                  active && "bg-zinc-800",
                )}
                style={{ fontFamily: font.stack }}
                onMouseEnter={() => preview(font.id)}
                onFocus={() => preview(font.id)}
                onClick={() => choose(font.id)}
              >
                <span id={`font-picker-${font.id}-name`} className="min-w-0 flex-1 truncate">
                  {font.label}
                </span>
                {active ? (
                  <Check id={`font-picker-${font.id}-check`} className="size-3.5 shrink-0 text-sky-400" aria-hidden />
                ) : null}
              </button>
            );
          })}
        </Menu>
      </div>
    </PopupFrame>
  );
}
