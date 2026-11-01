import { useEffect, useState } from "react";
import { Check, Minus, Plus } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { Toggle } from "@/components/Toggle";
import { COLOR_MODE_OPTIONS, THEMES, resolveMode } from "@/lib/appearance";
import { backdropSupport, getUserStyles, showUserStyles } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { clampZoom, DEFAULT_ZOOM, MAX_ZOOM, MIN_ZOOM, PAGE_ZOOM_LEVELS, stepPageZoom, ZOOM_STEP } from "@/lib/zoom";
import type { BackdropSupport, ToastPosition } from "@type/browser";
import { SETTINGS_SECTIONS } from "./settings-sections";
import {
  inputClass,
  matchesQuery,
  Row,
  Section,
  Segmented,
  useReportMatch,
  useSectionMatches,
  useSettingVisible,
  useSettingsQuery,
  type SettingsSectionProps,
} from "./SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-appearance")!;

const ACCENT_LABEL = "Site accent";
const ACCENT_HINT = "Tints the active tab and address bar from the site's theme color or favicon.";
const MODE_LABEL = "Mode";
const BACKDROP_LABEL = "Window backdrop";
const BACKDROP_HINT = "Mica or Acrylic shows the desktop color through the tab strip and toolbars.";
const THEME_LABEL = "Theme";
const STYLES_LABEL = "Custom styles files";
const STYLES_HINT =
  "styles.css for this profile. The starter rules are commented out, so uncomment them to change the theme. The file loads when the browser opens.";
const UI_ZOOM_LABEL = "Browser UI size";
const UI_ZOOM_HINT = "Scales tabs, toolbars and ce:// pages.";
const PAGE_ZOOM_LABEL = "Web page zoom";
const PAGE_ZOOM_HINT = "Default size for websites. Ctrl +/- changes the page you are on. Ctrl 0 returns to this size.";
const TOAST_LABEL = "Toast position";
const TOAST_HINT = "Where messages such as Undo for closed tabs appear over the page.";
const TOAST_POSITIONS: { value: ToastPosition; label: string }[] = [
  { value: "bottom-right", label: "Bottom right" },
  { value: "bottom-center", label: "Bottom center" },
  { value: "bottom-left", label: "Bottom left" },
  { value: "top-right", label: "Top right" },
  { value: "top-center", label: "Top center" },
  { value: "top-left", label: "Top left" },
];

export function AppearanceSection({ settings, query, onUpdate, onError, onMatch }: SettingsSectionProps) {
  const [userCssPath, setUserCssPath] = useState("");

  useEffect(() => {
    void getUserStyles()
      .then((styles) => setUserCssPath(styles.path))
      .catch((reason: unknown) => onError(String(reason)));
  }, [onError]);

  const pageZoom = settings.pageZoom || DEFAULT_ZOOM;
  const visible = matchesQuery(
    query,
    section.title,
    ACCENT_LABEL,
    ACCENT_HINT,
    MODE_LABEL,
    ...COLOR_MODE_OPTIONS.map((option) => option.label),
    BACKDROP_LABEL,
    BACKDROP_HINT,
    "mica",
    "acrylic",
    THEME_LABEL,
    ...THEMES.map((theme) => theme.label),
    STYLES_LABEL,
    STYLES_HINT,
    userCssPath,
    "Show in File Explorer",
    UI_ZOOM_LABEL,
    UI_ZOOM_HINT,
    settings.uiZoom !== DEFAULT_ZOOM ? "Reset" : "",
    "Decrease browser UI size",
    "Increase browser UI size",
    `${settings.uiZoom}%`,
    PAGE_ZOOM_LABEL,
    PAGE_ZOOM_HINT,
    pageZoom !== DEFAULT_ZOOM ? "Reset" : "",
    "Decrease web page zoom",
    "Increase web page zoom",
    `${pageZoom}%`,
    TOAST_LABEL,
    TOAST_HINT,
    "notification",
    ...TOAST_POSITIONS.map((option) => option.label),
  );
  useReportMatch(section.id, visible, onMatch);

  if (!visible) {
    return null;
  }

  return (
    <Section id={section.id} title={section.title}>
      <Row id="settings-site-accent" setting="siteAccent" label={ACCENT_LABEL} hint={ACCENT_HINT}>
        <Toggle id="settings-site-accent-toggle" checked={settings.siteAccent} onChange={(value) => onUpdate({ siteAccent: value })} />
      </Row>
      <Row id="settings-color-mode" setting="colorMode" label={MODE_LABEL} search={COLOR_MODE_OPTIONS.map((option) => option.label)}>
        <div id="settings-color-mode-options" className="flex rounded-md border border-zinc-700 p-0.5">
          {COLOR_MODE_OPTIONS.map((option) => (
            <button
              key={option.id}
              id={`settings-color-mode-${option.id}`}
              type="button"
              className={cn(
                "h-7 rounded px-3 text-xs text-zinc-300 hover:text-zinc-100",
                settings.colorMode === option.id && "bg-zinc-700 text-zinc-100",
              )}
              onClick={() => onUpdate({ colorMode: option.id })}
            >
              {option.label}
            </button>
          ))}
        </div>
      </Row>
      <BackdropRow settings={settings} onUpdate={onUpdate} />
      <ThemePicker settings={settings} onUpdate={onUpdate} />
      <UserStyles path={userCssPath} onError={onError} />
      <Row
        id="settings-ui-zoom" setting="uiZoom"
        label={UI_ZOOM_LABEL}
        hint={UI_ZOOM_HINT}
        search={[
          settings.uiZoom !== DEFAULT_ZOOM ? "Reset" : "",
          "Decrease browser UI size",
          "Increase browser UI size",
          `${settings.uiZoom}%`,
        ]}
      >
        <IconButton
          id="settings-ui-zoom-out"
          label="Decrease browser UI size"
          disabled={settings.uiZoom <= MIN_ZOOM}
          onClick={() => onUpdate({ uiZoom: clampZoom(settings.uiZoom - ZOOM_STEP) })}
        >
          <Minus id="settings-ui-zoom-out-icon" aria-hidden />
        </IconButton>
        <span id="settings-ui-zoom-value" className="w-10 text-center text-xs text-zinc-400">
          {settings.uiZoom}%
        </span>
        <IconButton
          id="settings-ui-zoom-in"
          label="Increase browser UI size"
          disabled={settings.uiZoom >= MAX_ZOOM}
          onClick={() => onUpdate({ uiZoom: clampZoom(settings.uiZoom + ZOOM_STEP) })}
        >
          <Plus id="settings-ui-zoom-in-icon" aria-hidden />
        </IconButton>
      </Row>
      <Row
        id="settings-page-zoom" setting="pageZoom"
        label={PAGE_ZOOM_LABEL}
        hint={PAGE_ZOOM_HINT}
        search={[
          pageZoom !== DEFAULT_ZOOM ? "Reset" : "",
          "Decrease web page zoom",
          "Increase web page zoom",
          `${pageZoom}%`,
        ]}
      >
        <IconButton
          id="settings-page-zoom-out"
          label="Decrease web page zoom"
          disabled={pageZoom <= PAGE_ZOOM_LEVELS[0]}
          onClick={() => onUpdate({ pageZoom: stepPageZoom(pageZoom, -1) })}
        >
          <Minus id="settings-page-zoom-out-icon" aria-hidden />
        </IconButton>
        <span id="settings-page-zoom-value" className="w-10 text-center text-xs text-zinc-400">
          {pageZoom}%
        </span>
        <IconButton
          id="settings-page-zoom-in"
          label="Increase web page zoom"
          disabled={pageZoom >= PAGE_ZOOM_LEVELS[PAGE_ZOOM_LEVELS.length - 1]}
          onClick={() => onUpdate({ pageZoom: stepPageZoom(pageZoom, 1) })}
        >
          <Plus id="settings-page-zoom-in-icon" aria-hidden />
        </IconButton>
      </Row>
      <Row
        id="settings-toast-position"
        setting="toastPosition"
        label={TOAST_LABEL}
        hint={TOAST_HINT}
        search={["notification", ...TOAST_POSITIONS.map((option) => option.label)]}
      >
        <select
          id="settings-toast-position-select"
          value={settings.toastPosition ?? "bottom-right"}
          aria-label={TOAST_LABEL}
          className={inputClass}
          onChange={(event) => onUpdate({ toastPosition: event.target.value as ToastPosition })}
        >
          {TOAST_POSITIONS.map((option) => (
            <option key={option.value} id={`settings-toast-position-${option.value}`} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
      </Row>
    </Section>
  );
}

function BackdropRow({ settings, onUpdate }: { settings: SettingsSectionProps["settings"]; onUpdate: SettingsSectionProps["onUpdate"] }) {
  const [support, setSupport] = useState<BackdropSupport | null>(null);

  useEffect(() => {
    void backdropSupport()
      .then(setSupport)
      .catch(() => setSupport({ mica: false, acrylic: false, restartNeeded: false }));
  }, [settings.windowBackdrop]);

  const unavailable = support !== null && !support.mica && !support.acrylic;
  const hint = unavailable
    ? "This version of Windows can't draw a backdrop."
    : support?.restartNeeded
      ? "Restart ce-browser to show the backdrop."
      : support && !support.mica
        ? "Mica needs Windows 11. Acrylic works here."
        : BACKDROP_HINT;
  return (
    <Row id="settings-backdrop" setting="windowBackdrop" label={BACKDROP_LABEL} hint={hint} search={["mica", "acrylic", "transparency"]}>
      <Segmented
        id="settings-backdrop-options"
        label={BACKDROP_LABEL}
        value={settings.windowBackdrop}
        disabled={unavailable}
        options={[
          { value: "none", label: "None" },
          ...(support?.mica ?? true ? [{ value: "mica" as const, label: "Mica" }] : []),
          ...(support?.acrylic ?? true ? [{ value: "acrylic" as const, label: "Acrylic" }] : []),
        ]}
        onChange={(windowBackdrop) => onUpdate({ windowBackdrop })}
      />
    </Row>
  );
}

function ThemePicker({ settings, onUpdate }: { settings: SettingsSectionProps["settings"]; onUpdate: SettingsSectionProps["onUpdate"] }) {
  const query = useSettingsQuery();
  const sectionMatches = useSectionMatches();
  const labels = THEMES.map((theme) => theme.label);
  const visible = useSettingVisible(THEME_LABEL, ...labels);
  if (!visible) {
    return null;
  }
  const resolved = resolveMode(settings.colorMode);
  const themes = sectionMatches || matchesQuery(query, THEME_LABEL) ? THEMES : THEMES.filter((theme) => matchesQuery(query, theme.label));
  return (
    <div id="settings-theme" className="px-5 py-3">
      <div id="settings-theme-label" className="mb-3 text-sm text-zinc-100">
        {THEME_LABEL}
      </div>
      <div id="settings-theme-grid" className="grid grid-cols-5 gap-2">
        {themes.map((theme) => {
          const swatch = theme.swatch[resolved];
          const active = settings.theme === theme.id;
          return (
            <button
              key={theme.id}
              id={`settings-theme-${theme.id}`}
              type="button"
              title={theme.label}
              className={cn(
                "relative flex flex-col overflow-hidden rounded-md border text-left",
                active ? "border-sky-500 ring-1 ring-sky-500" : "border-zinc-700 hover:border-zinc-500",
              )}
              onClick={() => onUpdate({ theme: theme.id })}
            >
              <span id={`settings-theme-${theme.id}-preview`} className="flex h-10 items-end gap-1 p-1.5" style={{ background: swatch.background }}>
                <span id={`settings-theme-${theme.id}-surface`} className="h-5 flex-1 rounded-sm" style={{ background: swatch.surface }} />
                <span id={`settings-theme-${theme.id}-accent`} className="size-3 rounded-full" style={{ background: swatch.accent }} />
              </span>
              <span id={`settings-theme-${theme.id}-name`} className="truncate bg-zinc-900 px-1.5 py-1 text-[11px] text-zinc-300">
                {theme.label}
              </span>
              {active ? (
                <Check id={`settings-theme-${theme.id}-check`} className="absolute right-1 top-1 size-3.5 rounded-full bg-sky-500 p-0.5 text-white" />
              ) : null}
            </button>
          );
        })}
      </div>
    </div>
  );
}

function UserStyles({ path, onError }: { path: string; onError: (message: string | null) => void }) {
  const visible = useSettingVisible(STYLES_LABEL, STYLES_HINT, path, "Show in File Explorer");
  if (!visible) {
    return null;
  }
  return (
    <div id="settings-user-styles" className="flex flex-col gap-3 px-5 py-3">
      <div id="settings-user-styles-heading">
        <div id="settings-user-styles-label" className="text-sm text-zinc-100">
          {STYLES_LABEL}
        </div>
        <p id="settings-user-styles-hint" className="mt-1 text-xs text-zinc-500">
          {STYLES_HINT}
        </p>
      </div>
      {path ? (
        <button
          id="settings-user-styles-path"
          type="button"
          title="Show in File Explorer"
          className="break-all text-left text-xs text-zinc-500 underline-offset-2 hover:text-sky-400 hover:underline"
          onClick={() => void showUserStyles().catch((reason: unknown) => onError(String(reason)))}
        >
          {path}
        </button>
      ) : null}
    </div>
  );
}
