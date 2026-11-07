import { useEffect, useRef, useState } from "react";
import { ChevronDown, Plus, RotateCcw, Trash2 } from "lucide-react";
import type { PrintEditorSettings, PrintPaper, PrintPreset } from "@type/browser";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import {
  CLEANUP_OPTIONS,
  FONT_OPTIONS,
  IMAGE_OPTIONS,
  MARGIN_OPTIONS,
  MAX_PRINT_PRESETS,
  MODE_OPTIONS,
  PAPER_GROUPS,
  customPaperReady,
  newPrintPreset,
  paperOptionLabel,
  papersInGroup,
  presetHint,
} from "@features/BuiltInExtensions/print-pdf/print-options";
import { cn } from "@/lib/cn";
import { focusRing } from "@/lib/focus-ring";
import { inputClass, Segmented, useSettingChanged } from "../../SettingsControls";

export const PRESETS_LABEL = "Quick cleanup presets";
export const PRESETS_HINT = "Named recipes in the Print to PDF menu. Each one saves a cleanup without opening the editor.";

const selectClass = cn(inputClass, "max-w-80");

function patchPreset(presets: PrintPreset[], id: string, change: Partial<PrintPreset>) {
  return presets.map((preset) => (preset.id === id ? { ...preset, ...change } : preset));
}

function NameField({ id, value, onCommit }: { id: string; value: string; onCommit: (name: string) => void }) {
  const [draft, setDraft] = useState<string | null>(null);
  const committed = useRef(value);
  const shown = draft ?? value;

  useEffect(() => {
    if (value !== committed.current) {
      committed.current = value;
      setDraft(null);
    }
  }, [value]);

  return (
    <label id={`${id}-label`} className="flex flex-col gap-1">
      <span id={`${id}-caption`} className="text-xs text-zinc-400">
        Name
      </span>
      <input
        id={id}
        className={selectClass}
        maxLength={40}
        value={shown}
        onChange={(event) => {
          const next = event.target.value.slice(0, 40);
          setDraft(next);
          const trimmed = next.trim().slice(0, 40);
          if (trimmed && trimmed !== value) {
            committed.current = trimmed;
            onCommit(trimmed);
          }
        }}
        onBlur={() => {
          const name = (draft ?? value).trim().slice(0, 40) || "Preset";
          committed.current = name;
          setDraft(null);
          if (name !== value) {
            onCommit(name);
          }
        }}
      />
    </label>
  );
}

function PresetFields({
  preset,
  print,
  onChange,
}: {
  preset: PrintPreset;
  print: PrintEditorSettings;
  onChange: (change: Partial<PrintPreset>) => void;
}) {
  const showCustom = preset.paper === "custom" || customPaperReady(print);

  return (
    <div id={`settings-print-preset-${preset.id}-fields`} className="flex flex-col gap-4 px-5 pb-4">
      <NameField
        id={`settings-print-preset-${preset.id}-name`}
        value={preset.name}
        onCommit={(name) => onChange({ name })}
      />
      <fieldset id={`settings-print-preset-${preset.id}-content`} className="flex flex-col gap-2">
        <legend id={`settings-print-preset-${preset.id}-content-legend`} className="text-[11px] font-semibold uppercase tracking-wider text-zinc-500">
          Content
        </legend>
        <Segmented
          id={`settings-print-preset-${preset.id}-mode`}
          label="Content"
          value={preset.mode}
          options={MODE_OPTIONS}
          onChange={(mode) => onChange({ mode })}
        />
        <label id={`settings-print-preset-${preset.id}-header`} className="flex items-center gap-2 text-sm text-zinc-200">
          <input
            id={`settings-print-preset-${preset.id}-header-input`}
            type="checkbox"
            className="size-3.5 accent-sky-500"
            checked={preset.showHeader}
            onChange={(event) => onChange({ showHeader: event.target.checked })}
          />
          Title, source and date
        </label>
      </fieldset>
      <fieldset id={`settings-print-preset-${preset.id}-cleanup`} className="flex flex-col gap-2">
        <legend id={`settings-print-preset-${preset.id}-cleanup-legend`} className="text-[11px] font-semibold uppercase tracking-wider text-zinc-500">
          Clean up
        </legend>
        <div id={`settings-print-preset-${preset.id}-cleanup-options`} className="grid grid-cols-1 gap-1.5 sm:grid-cols-2">
          {CLEANUP_OPTIONS.map((option) => (
            <label id={`settings-print-preset-${preset.id}-${option.flag}`} key={option.flag} className="flex items-center gap-2 text-sm text-zinc-200">
              <input
                id={`settings-print-preset-${preset.id}-${option.flag}-input`}
                type="checkbox"
                className="size-3.5 accent-sky-500"
                checked={preset[option.flag]}
                onChange={(event) => onChange({ [option.flag]: event.target.checked })}
              />
              {option.label}
            </label>
          ))}
        </div>
      </fieldset>
      <fieldset id={`settings-print-preset-${preset.id}-text`} className="flex flex-col gap-2">
        <legend id={`settings-print-preset-${preset.id}-text-legend`} className="text-[11px] font-semibold uppercase tracking-wider text-zinc-500">
          Text
        </legend>
        <label id={`settings-print-preset-${preset.id}-text-size`} className="flex max-w-80 flex-col gap-1.5">
          <span id={`settings-print-preset-${preset.id}-text-size-row`} className="flex items-center justify-between text-sm text-zinc-200">
            <span id={`settings-print-preset-${preset.id}-text-size-label`}>Text size</span>
            <span id={`settings-print-preset-${preset.id}-text-size-value`} className="text-xs text-zinc-400">
              {preset.textSize}%
            </span>
          </span>
          <input
            id={`settings-print-preset-${preset.id}-text-size-input`}
            type="range"
            min={70}
            max={150}
            step={5}
            value={preset.textSize}
            className="w-full accent-sky-500"
            onChange={(event) => onChange({ textSize: Number(event.target.value) })}
          />
        </label>
        <Segmented
          id={`settings-print-preset-${preset.id}-font`}
          label="Font"
          value={preset.font}
          options={FONT_OPTIONS}
          onChange={(font) => onChange({ font })}
        />
      </fieldset>
      <fieldset id={`settings-print-preset-${preset.id}-images`} className="flex flex-col gap-2">
        <legend id={`settings-print-preset-${preset.id}-images-legend`} className="text-[11px] font-semibold uppercase tracking-wider text-zinc-500">
          Images and color
        </legend>
        <label id={`settings-print-preset-${preset.id}-image-size-label`} className="flex flex-col gap-1">
          <span id={`settings-print-preset-${preset.id}-image-size-caption`} className="text-xs text-zinc-400">
            Image size
          </span>
          <select
            id={`settings-print-preset-${preset.id}-image-size`}
            className={selectClass}
            value={preset.imageSize}
            onChange={(event) => onChange({ imageSize: event.target.value as PrintPreset["imageSize"] })}
          >
            {IMAGE_OPTIONS.map((option) => (
              <option id={`settings-print-preset-${preset.id}-image-size-${option.value}`} key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <label id={`settings-print-preset-${preset.id}-backgrounds`} className="flex items-center gap-2 text-sm text-zinc-200">
          <input
            id={`settings-print-preset-${preset.id}-backgrounds-input`}
            type="checkbox"
            className="size-3.5 accent-sky-500"
            checked={preset.removeBackgrounds}
            onChange={(event) => onChange({ removeBackgrounds: event.target.checked })}
          />
          Remove background colors
        </label>
      </fieldset>
      <fieldset id={`settings-print-preset-${preset.id}-page`} className="flex flex-col gap-2">
        <legend id={`settings-print-preset-${preset.id}-page-legend`} className="text-[11px] font-semibold uppercase tracking-wider text-zinc-500">
          Page
        </legend>
        <label id={`settings-print-preset-${preset.id}-paper-label`} className="flex flex-col gap-1">
          <span id={`settings-print-preset-${preset.id}-paper-caption`} className="text-xs text-zinc-400">
            Paper size
          </span>
          <select
            id={`settings-print-preset-${preset.id}-paper`}
            className={selectClass}
            value={preset.paper}
            onChange={(event) => onChange({ paper: event.target.value as PrintPaper })}
          >
            {PAPER_GROUPS.map((group) => (
              <optgroup id={`settings-print-preset-${preset.id}-paper-${group.id}`} key={group.id} label={group.label}>
                {papersInGroup(group.id).map((paper) => (
                  <option id={`settings-print-preset-${preset.id}-paper-${paper}`} key={paper} value={paper}>
                    {paperOptionLabel(paper)}
                  </option>
                ))}
              </optgroup>
            ))}
            {showCustom ? (
              <optgroup id={`settings-print-preset-${preset.id}-paper-custom`} label="Custom">
                <option id={`settings-print-preset-${preset.id}-paper-custom-option`} value="custom">
                  {paperOptionLabel("custom", print)}
                </option>
              </optgroup>
            ) : null}
          </select>
        </label>
        {preset.paper === "custom" ? (
          <p id={`settings-print-preset-${preset.id}-custom-hint`} className="text-xs text-zinc-500">
            {customPaperReady(print)
              ? "Uses the custom size under Page sizes."
              : "Set a custom width and height under Page sizes. Until then this recipe uses Letter."}
          </p>
        ) : null}
        <Segmented
          id={`settings-print-preset-${preset.id}-orientation`}
          label="Orientation"
          value={preset.landscape ? "landscape" : "portrait"}
          options={[
            { value: "portrait", label: "Portrait" },
            { value: "landscape", label: "Landscape" },
          ]}
          onChange={(value) => onChange({ landscape: value === "landscape" })}
        />
        <Segmented
          id={`settings-print-preset-${preset.id}-margins`}
          label="Margins"
          value={preset.margins}
          options={MARGIN_OPTIONS}
          onChange={(margins) => onChange({ margins })}
        />
        <label id={`settings-print-preset-${preset.id}-print-backgrounds`} className="flex items-center gap-2 text-sm text-zinc-200">
          <input
            id={`settings-print-preset-${preset.id}-print-backgrounds-input`}
            type="checkbox"
            className="size-3.5 accent-sky-500"
            checked={preset.printBackgrounds}
            onChange={(event) => onChange({ printBackgrounds: event.target.checked })}
          />
          Print backgrounds
        </label>
        <label id={`settings-print-preset-${preset.id}-header-footer`} className="flex items-center gap-2 text-sm text-zinc-200">
          <input
            id={`settings-print-preset-${preset.id}-header-footer-input`}
            type="checkbox"
            className="size-3.5 accent-sky-500"
            checked={preset.headerFooter}
            onChange={(event) => onChange({ headerFooter: event.target.checked })}
          />
          Page headers and footers
        </label>
      </fieldset>
    </div>
  );
}

export function PrintPresets({
  print,
  onChange,
}: {
  print: PrintEditorSettings;
  onChange: (next: PrintEditorSettings) => void;
}) {
  const [openId, setOpenId] = useState<string | null>(null);
  const { changed, reset } = useSettingChanged(["printEditor.presets"]);
  const presets = print.presets ?? [];
  const atLimit = presets.length >= MAX_PRINT_PRESETS;

  const update = (next: PrintPreset[]) => onChange({ ...print, presets: next });

  return (
    <div id="settings-print-presets" className="flex flex-col gap-1 py-1">
      <div id="settings-print-presets-heading" className="flex items-start justify-between gap-3 px-5 py-3">
        <div id="settings-print-presets-text" className="min-w-0">
          <div id="settings-print-presets-label" className="text-sm text-zinc-100">
            {PRESETS_LABEL}
          </div>
          <p id="settings-print-presets-hint" className="text-xs text-zinc-500">
            {PRESETS_HINT}
          </p>
        </div>
        {changed && reset ? (
          <IconButton id="settings-print-presets-reset" label="Reset quick cleanup presets to default" className="size-7" onClick={reset}>
            <RotateCcw id="settings-print-presets-reset-icon" aria-hidden />
          </IconButton>
        ) : null}
      </div>
      <div id="settings-print-presets-list" className="flex flex-col">
        {presets.length === 0 ? (
          <p id="settings-print-presets-empty" className="px-5 pb-3 text-xs text-zinc-500">
            No presets. Quick clean PDF uses the last choices from the print editor.
          </p>
        ) : (
          presets.map((preset) => {
            const open = openId === preset.id;
            return (
              <div id={`settings-print-preset-${preset.id}`} key={preset.id} className="border-t border-zinc-800">
                <div id={`settings-print-preset-${preset.id}-row`} className="flex items-center gap-2 px-3 py-2">
                  <button
                    id={`settings-print-preset-${preset.id}-toggle`}
                    type="button"
                    aria-expanded={open}
                    className={cn("flex min-w-0 flex-1 items-center gap-2 rounded-md px-2 py-1 text-left hover:bg-zinc-800/60", focusRing)}
                    onClick={() => setOpenId(open ? null : preset.id)}
                  >
                    <ChevronDown
                      id={`settings-print-preset-${preset.id}-chevron`}
                      aria-hidden
                      className={cn("size-4 shrink-0 text-zinc-500 transition-transform", !open && "-rotate-90")}
                    />
                    <span id={`settings-print-preset-${preset.id}-summary`} className="min-w-0">
                      <span id={`settings-print-preset-${preset.id}-title`} className="block truncate text-sm text-zinc-100">
                        {preset.name}
                      </span>
                      <span id={`settings-print-preset-${preset.id}-hint`} className="block truncate text-xs text-zinc-500">
                        {presetHint(preset)}
                      </span>
                    </span>
                  </button>
                  <IconButton
                    id={`settings-print-preset-${preset.id}-delete`}
                    label={`Delete ${preset.name}`}
                    className="size-7"
                    onClick={() => {
                      update(presets.filter((item) => item.id !== preset.id));
                      if (open) {
                        setOpenId(null);
                      }
                    }}
                  >
                    <Trash2 id={`settings-print-preset-${preset.id}-delete-icon`} aria-hidden />
                  </IconButton>
                </div>
                {open ? (
                  <PresetFields
                    preset={preset}
                    print={print}
                    onChange={(change) => update(patchPreset(presets, preset.id, change))}
                  />
                ) : null}
              </div>
            );
          })
        )}
      </div>
      <div id="settings-print-presets-add-row" className="flex items-center gap-3 px-5 pb-3">
        <Button
          id="settings-print-presets-add"
          size="sm"
          disabled={atLimit}
          onClick={() => {
            const preset = newPrintPreset(presets);
            update([...presets, preset]);
            setOpenId(preset.id);
          }}
        >
          <Plus id="settings-print-presets-add-icon" className="size-3.5" aria-hidden />
          Add preset
        </Button>
        {atLimit ? (
          <p id="settings-print-presets-limit" className="text-xs text-zinc-500">
            {MAX_PRINT_PRESETS} presets is the limit.
          </p>
        ) : null}
      </div>
    </div>
  );
}
