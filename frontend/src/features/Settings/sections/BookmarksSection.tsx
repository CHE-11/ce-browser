import { Toggle } from "@/components/Toggle";
import { STYLES_LINK_HINT, STYLES_LINK_LABEL, STYLES_LINK_TEXT, StylesLink } from "@/features/Styles/StylesLink";
import { SETTINGS_SECTIONS } from "../settings-sections";
import { matchesQuery, Row, Section, useReportMatch, type SettingsSectionProps } from "../SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-bookmarks")!;

const BAR_LABEL = "Show bookmarks bar";
const BAR_HINT = "Ctrl+Shift+B";
const FOLDER_ICONS_LABEL = "Show folder icons on the bookmarks bar";
const FOLDER_ICONS_HINT = "Off shows folder names only. Site icons stay.";

export function BookmarksSection({ settings, query, onUpdate, onMatch }: SettingsSectionProps) {
  const visible = matchesQuery(
    query,
    section.title,
    BAR_LABEL,
    BAR_HINT,
    FOLDER_ICONS_LABEL,
    FOLDER_ICONS_HINT,
    STYLES_LINK_LABEL,
    STYLES_LINK_HINT,
    STYLES_LINK_TEXT,
    "styles",
    "editor",
    "text size",
    "padding",
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
      <StylesLink
        id="settings-bookmark-styles-link"
        surface="bookmarks-bar"
        search={["bookmark", "sizes", "height", "icon", "padding", "menu", "text size", "rem", "folder"]}
      />
    </Section>
  );
}
