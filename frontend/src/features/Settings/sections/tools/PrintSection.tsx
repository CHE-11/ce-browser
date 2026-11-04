import { useState } from "react";
import { RotateCcw } from "lucide-react";
import type { PrintEditorSettings } from "@type/browser";
import { IconButton } from "@/components/IconButton";
import {
  PAPER_GROUPS,
  PAPER_SIZES,
  customPaperReady,
  paperOptionLabel,
  papersInGroup,
} from "@features/BuiltInExtensions/print-pdf/print-options";
import { SETTINGS_SECTIONS } from "../../settings-sections";
import { inputClass, matchesQuery, Section, useReportMatch, useSettingChanged, type SettingsSectionProps } from "../../SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-print")!;

const SIZES_LABEL = "Page sizes";
const SIZES_HINT = "Checked sizes appear in the print editor. Letter, Tabloid, and Legal start on.";
const CUSTOM_LABEL = "Custom";
const CUSTOM_HINT = "Width and height in inches, from 1 to 50.";

function patchPrint(settings: PrintEditorSettings, change: Partial<PrintEditorSettings>): PrintEditorSettings {
  return { ...settings, ...change };
}

function InchesField({
  id,
  label,
  value,
  onChange,
}: {
  id: string;
  label: string;
  value: number;
  onChange: (inches: number) => void;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const shown = draft ?? (value > 0 ? String(value) : "");

  return (
    <label id={`${id}-label`} className="flex min-w-0 flex-1 flex-col gap-1">
      <span id={`${id}-caption`} className="text-xs text-zinc-400">
        {label}
      </span>
      <input
        id={id}
        className={inputClass}
        inputMode="decimal"
        placeholder="Inches"
        value={shown}
        onChange={(event) => {
          const next = event.target.value;
          if (next !== "" && !/^\d*\.?\d*$/.test(next)) {
            return;
          }
          setDraft(next);
          const parsed = next === "" || next === "." ? 0 : Number(next);
          if (Number.isFinite(parsed)) {
            onChange(parsed);
          }
        }}
        onBlur={() => setDraft(null)}
      />
    </label>
  );
}

export function PrintSection({ settings, query, onUpdate, onMatch }: SettingsSectionProps) {
  const labels = (Object.keys(PAPER_SIZES) as (keyof typeof PAPER_SIZES)[]).map((paper) => PAPER_SIZES[paper].label);
  const visible = matchesQuery(query, section.title, "tools", SIZES_LABEL, SIZES_HINT, CUSTOM_LABEL, CUSTOM_HINT, "print", "pdf", ...labels);
  useReportMatch(section.id, visible, onMatch);
  const { changed, reset } = useSettingChanged(["printEditor.paperChoices", "printEditor.customWidth", "printEditor.customHeight"]);
  const print = settings.printEditor;
  const customOn = print.paperChoices.includes("custom");

  if (!visible) {
    return null;
  }

  const toggle = (id: string) => {
    const selected = print.paperChoices.includes(id);
    if (selected && print.paperChoices.length === 1) {
      return;
    }
    const paperChoices = selected ? print.paperChoices.filter((item) => item !== id) : [...print.paperChoices, id];
    onUpdate({ printEditor: patchPrint(print, { paperChoices }) });
  };

  return (
    <Section id={section.id} title={section.title}>
      <div id="settings-print-papers" className="flex flex-col gap-4 px-5 py-3">
        <div id="settings-print-papers-heading" className="flex items-start justify-between gap-3">
          <div id="settings-print-papers-text" className="min-w-0">
            <div id="settings-print-papers-label" className="text-sm text-zinc-100">
              {SIZES_LABEL}
            </div>
            <p id="settings-print-papers-hint" className="text-xs text-zinc-500">
              {SIZES_HINT}
            </p>
          </div>
          {changed && reset ? (
            <IconButton id="settings-print-papers-reset" label="Reset page sizes to default" className="size-7" onClick={reset}>
              <RotateCcw id="settings-print-papers-reset-icon" aria-hidden />
            </IconButton>
          ) : null}
        </div>
        {PAPER_GROUPS.map((group) => (
          <fieldset id={`settings-print-${group.id}`} key={group.id} className="flex flex-col gap-2">
            <legend id={`settings-print-${group.id}-legend`} className="text-[11px] font-semibold uppercase tracking-wider text-zinc-500">
              {group.label}
            </legend>
            <div id={`settings-print-${group.id}-options`} className="grid grid-cols-2 gap-x-4 gap-y-1.5 sm:grid-cols-3">
              {papersInGroup(group.id).map((paper) => {
                const checked = print.paperChoices.includes(paper);
                return (
                  <label id={`settings-print-${paper}`} key={paper} className="flex items-center gap-2 text-sm text-zinc-200">
                    <input
                      id={`settings-print-${paper}-input`}
                      type="checkbox"
                      className="size-3.5 accent-sky-500"
                      checked={checked}
                      onChange={() => toggle(paper)}
                    />
                    {paperOptionLabel(paper)}
                  </label>
                );
              })}
            </div>
          </fieldset>
        ))}
        <fieldset id="settings-print-custom" className="flex flex-col gap-2">
          <legend id="settings-print-custom-legend" className="text-[11px] font-semibold uppercase tracking-wider text-zinc-500">
            {CUSTOM_LABEL}
          </legend>
          <label id="settings-print-custom-toggle" className="flex items-center gap-2 text-sm text-zinc-200">
            <input
              id="settings-print-custom-input"
              type="checkbox"
              className="size-3.5 accent-sky-500"
              checked={customOn}
              onChange={() => toggle("custom")}
            />
            Custom size
          </label>
          {customOn ? (
            <div id="settings-print-custom-fields" className="flex flex-col gap-2">
              <div id="settings-print-custom-size" className="flex gap-3">
                <InchesField
                  id="settings-print-custom-width"
                  label="Width"
                  value={print.customWidth}
                  onChange={(customWidth) => onUpdate({ printEditor: patchPrint(print, { customWidth }) })}
                />
                <InchesField
                  id="settings-print-custom-height"
                  label="Height"
                  value={print.customHeight}
                  onChange={(customHeight) => onUpdate({ printEditor: patchPrint(print, { customHeight }) })}
                />
              </div>
              <p id="settings-print-custom-hint" className="text-xs text-zinc-500">
                {customPaperReady(print) ? CUSTOM_HINT : `${CUSTOM_HINT} Enter both before it appears in the editor.`}
              </p>
            </div>
          ) : null}
        </fieldset>
      </div>
    </Section>
  );
}
