import { useEffect, useState } from "react";
import { Toggle } from "@/components/Toggle";
import { BOOKMARK_SIZE } from "@/lib/bookmark-size";
import { cn } from "@/lib/cn";
import { SETTINGS_SECTIONS } from "./settings-sections";
import { inputClass, matchesQuery, Row, Section, useReportMatch, type SettingsSectionProps } from "./SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-bookmarks")!;

const BAR_LABEL = "Show bookmarks bar";
const BAR_HINT = "Ctrl+Shift+B";
const FOLDER_ICONS_LABEL = "Show folder icons on the bookmarks bar";
const FOLDER_ICONS_HINT = "Off shows folder names only. Site icons stay.";
const TEXT_HINT = "In rem. 1rem is the browser's base text size.";
const PADDING_HINT = "Side padding in rem. Top and bottom get half.";

type RemSettingKey = "bookmarksBarTextSize" | "bookmarksBarPadding" | "bookmarkFolderTextSize" | "bookmarkFolderPadding";

const BOOKMARK_SIZE_ROWS: {
  key: RemSettingKey;
  id: string;
  label: string;
  hint: string;
  fallback: number;
  limits: { min: number; max: number };
}[] = [
  {
    key: "bookmarksBarTextSize",
    id: "settings-bookmarks-bar-text-size",
    label: "Bookmarks bar text size",
    hint: TEXT_HINT,
    fallback: BOOKMARK_SIZE.barText,
    limits: BOOKMARK_SIZE.text,
  },
  {
    key: "bookmarksBarPadding",
    id: "settings-bookmarks-bar-padding",
    label: "Bookmarks bar padding",
    hint: PADDING_HINT,
    fallback: BOOKMARK_SIZE.barPadding,
    limits: BOOKMARK_SIZE.padding,
  },
  {
    key: "bookmarkFolderTextSize",
    id: "settings-bookmark-folder-text-size",
    label: "Bookmark folder item text size",
    hint: TEXT_HINT,
    fallback: BOOKMARK_SIZE.folderText,
    limits: BOOKMARK_SIZE.text,
  },
  {
    key: "bookmarkFolderPadding",
    id: "settings-bookmark-folder-padding",
    label: "Bookmark folder item padding",
    hint: PADDING_HINT,
    fallback: BOOKMARK_SIZE.folderPadding,
    limits: BOOKMARK_SIZE.padding,
  },
];

export function BookmarksSection({ settings, query, onUpdate, onMatch }: SettingsSectionProps) {
  const visible = matchesQuery(
    query,
    section.title,
    BAR_LABEL,
    BAR_HINT,
    FOLDER_ICONS_LABEL,
    FOLDER_ICONS_HINT,
    ...BOOKMARK_SIZE_ROWS.flatMap((row) => [row.label, row.hint]),
    "rem",
  );
  useReportMatch(section.id, visible, onMatch);

  if (!visible) {
    return null;
  }

  return (
    <Section id={section.id} title={section.title}>
      <Row id="settings-bookmarks-bar" setting="showBookmarksBar" label={BAR_LABEL} hint={BAR_HINT}>
        <Toggle id="settings-bookmarks-bar-toggle" checked={settings.showBookmarksBar} onChange={(value) => onUpdate({ showBookmarksBar: value })} />
      </Row>
      <Row id="settings-bookmarks-bar-folder-icons" setting="bookmarksBarFolderIcons" label={FOLDER_ICONS_LABEL} hint={FOLDER_ICONS_HINT}>
        <Toggle
          id="settings-bookmarks-bar-folder-icons-toggle"
          checked={settings.bookmarksBarFolderIcons}
          onChange={(value) => onUpdate({ bookmarksBarFolderIcons: value })}
        />
      </Row>
      {BOOKMARK_SIZE_ROWS.map((row) => (
        <RemSetting
          key={row.key}
          id={row.id}
          setting={row.key}
          label={row.label}
          hint={row.hint}
          value={settings[row.key] ?? row.fallback}
          fallback={row.fallback}
          limits={row.limits}
          onChange={(value) => onUpdate({ [row.key]: value })}
        />
      ))}
    </Section>
  );
}

type RemSettingProps = {
  id: string;
  setting: RemSettingKey;
  label: string;
  hint: string;
  value: number;
  fallback: number;
  limits: { min: number; max: number };
  onChange: (value: number) => void;
};

/** An exact size in rem. Saves on blur or Enter. */
function RemSetting({ id, setting, label, hint, value, fallback, limits, onChange }: RemSettingProps) {
  const [draft, setDraft] = useState(String(value));

  useEffect(() => {
    setDraft(String(value));
  }, [value]);

  const commit = () => {
    const parsed = Number(draft);
    const next = Number.isFinite(parsed) && draft.trim() !== "" ? parsed : fallback;
    const clamped = Math.round(Math.min(limits.max, Math.max(limits.min, next)) * 10000) / 10000;
    setDraft(String(clamped));
    if (clamped !== value) {
      onChange(clamped);
    }
  };

  return (
    <Row id={id} label={label} hint={hint} search={["rem", String(value)]} setting={setting}>
      <input
        id={`${id}-input`}
        type="number"
        min={limits.min}
        max={limits.max}
        step={BOOKMARK_SIZE.step}
        value={draft}
        aria-label={label}
        className={cn(inputClass, "w-24 text-right")}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.currentTarget.blur();
          }
        }}
      />
      <span id={`${id}-unit`} className="text-xs text-zinc-500">
        rem
      </span>
    </Row>
  );
}
