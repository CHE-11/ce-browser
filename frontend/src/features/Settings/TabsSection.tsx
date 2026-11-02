import { useEffect, useState } from "react";
import { X } from "lucide-react";
import type { BrowserSettings } from "@type/browser";
import { Button } from "@/components/Button";
import { IconButton } from "@/components/IconButton";
import { Toggle } from "@/components/Toggle";
import { cn } from "@/lib/cn";
import { SETTINGS_SECTIONS } from "./settings-sections";
import { inputClass, matchesQuery, Row, Section, Segmented, useReportMatch, useSettingVisible, type SettingsSectionProps } from "./SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-tabs")!;

const WORKSPACES_LABEL = "Workspaces";
const WORKSPACES_HINT = "The button on the left of the tab strip. Off hides switching, and the tabs you are looking at stay where they are.";
const LAYOUT_LABEL = "Tab layout";
const LAYOUT_HINT = "Vertical tabs sit in a sidebar left of the page.";
const CYCLE_LABEL = "Ctrl+Tab order";
const CYCLE_HINT = "Recently used goes to the tab you used last, then the one before it.";
const SWITCHER_LABEL = "Show the tab switcher";
const SWITCHER_HINT = "With recently used order, Ctrl+Tab shows thumbnails until you let go of Ctrl.";
const HOVER_LABEL = "Show a preview when hovering a tab";
const HOVER_HINT = "After a moment, a small card shows the page, its title, and its address.";
const PEEK_LABEL = "Shift+click opens links in a peek";
const PEEK_HINT = "A peek shows the page over the current tab. Esc closes it.";
const MUTE_LABEL = "Mute tabs in the background";
const MUTE_HINT = "Only the tab on screen plays sound.";
const SLEEP_LABEL = "Put background tabs to sleep";
const SLEEP_HINT = "Sleeping tabs free memory and CPU, and wake when you select them. Pinned tabs stay loaded.";
const MINUTES_LABEL = "Sleep after";
const MINUTES_HINT = "Minutes a tab stays in the background first.";
const NEVER_LABEL = "Never sleep on these sites";
const NEVER_HINT = "Tabs on these sites and their subdomains stay awake. Pinned tabs and tabs playing sound never sleep.";
const NEVER_EMPTY = "No sites yet. You can also add one from a tab's right-click menu.";

export function TabsSection({ settings, query, onUpdate, onMatch }: SettingsSectionProps) {
  const [sleepMinutesDraft, setSleepMinutesDraft] = useState("30");
  const [neverSleepDraft, setNeverSleepDraft] = useState("");

  useEffect(() => {
    setSleepMinutesDraft(String(settings.tabSleep.minutes));
  }, [settings.tabSleep.minutes]);

  const visible = matchesQuery(
    query,
    section.title,
    WORKSPACES_LABEL,
    WORKSPACES_HINT,
    LAYOUT_LABEL,
    LAYOUT_HINT,
    "horizontal",
    "vertical",
    CYCLE_LABEL,
    CYCLE_HINT,
    SWITCHER_LABEL,
    SWITCHER_HINT,
    "ctrl+tab",
    HOVER_LABEL,
    HOVER_HINT,
    "thumbnail",
    PEEK_LABEL,
    PEEK_HINT,
    MUTE_LABEL,
    MUTE_HINT,
    SLEEP_LABEL,
    SLEEP_HINT,
    MINUTES_LABEL,
    MINUTES_HINT,
    "minutes",
    sleepMinutesDraft,
    NEVER_LABEL,
    NEVER_HINT,
    NEVER_EMPTY,
    "Add",
    neverSleepDraft,
    ...settings.tabSleep.neverSleepSites,
  );
  useReportMatch(section.id, visible, onMatch);

  const addNeverSleep = () => {
    const site = neverSleepDraft.trim();
    if (!site) {
      return;
    }
    setNeverSleepDraft("");
    onUpdate({ tabSleep: { ...settings.tabSleep, neverSleepSites: [...settings.tabSleep.neverSleepSites, site] } });
  };

  if (!visible) {
    return null;
  }

  return (
    <Section id={section.id} title={section.title}>
      <Row id="settings-workspaces" setting="workspacesEnabled" label={WORKSPACES_LABEL} hint={WORKSPACES_HINT} search={["switcher", "tab strip"]}>
        <Toggle
          id="settings-workspaces-toggle"
          checked={settings.workspacesEnabled}
          onChange={(workspacesEnabled) => onUpdate({ workspacesEnabled })}
        />
      </Row>
      <Row id="settings-tab-layout" setting="tabLayout" label={LAYOUT_LABEL} hint={LAYOUT_HINT} search={["horizontal", "vertical"]}>
        <Segmented
          id="settings-tab-layout-options"
          label={LAYOUT_LABEL}
          value={settings.tabLayout}
          options={[
            { value: "horizontal", label: "Horizontal" },
            { value: "vertical", label: "Vertical" },
          ]}
          onChange={(tabLayout) => onUpdate({ tabLayout })}
        />
      </Row>
      <Row id="settings-tab-cycle" setting="tabCycleOrder" label={CYCLE_LABEL} hint={CYCLE_HINT} search={["ctrl+tab", "recent", "strip order"]}>
        <Segmented
          id="settings-tab-cycle-options"
          label={CYCLE_LABEL}
          value={settings.tabCycleOrder}
          options={[
            { value: "strip", label: "Strip order" },
            { value: "recent", label: "Recently used" },
          ]}
          onChange={(tabCycleOrder) => onUpdate({ tabCycleOrder })}
        />
      </Row>
      <Row id="settings-tab-switcher" setting="tabSwitcher" label={SWITCHER_LABEL} hint={SWITCHER_HINT} search={["thumbnails", "overlay"]}>
        <Toggle
          id="settings-tab-switcher-toggle"
          checked={settings.tabSwitcher}
          disabled={settings.tabCycleOrder !== "recent"}
          onChange={(tabSwitcher) => onUpdate({ tabSwitcher })}
        />
      </Row>
      <Row id="settings-tab-hover" label={HOVER_LABEL} hint={HOVER_HINT} search={["thumbnail", "preview", "card"]}>
        <Toggle
          id="settings-tab-hover-toggle"
          checked={settings.tabHoverPreview}
          onChange={(tabHoverPreview) => onUpdate({ tabHoverPreview })}
        />
      </Row>
      <Row id="settings-peek" setting="peekOnShiftClick" label={PEEK_LABEL} hint={PEEK_HINT}>
        <Toggle id="settings-peek-toggle" checked={settings.peekOnShiftClick} onChange={(value) => onUpdate({ peekOnShiftClick: value })} />
      </Row>
      <Row id="settings-mute-background" setting="muteBackgroundTabs" label={MUTE_LABEL} hint={MUTE_HINT}>
        <Toggle
          id="settings-mute-background-toggle"
          checked={settings.muteBackgroundTabs}
          onChange={(value) => onUpdate({ muteBackgroundTabs: value })}
        />
      </Row>
      <Row id="settings-sleep" setting="tabSleep.enabled" label={SLEEP_LABEL} hint={SLEEP_HINT}>
        <Toggle
          id="settings-sleep-toggle"
          checked={settings.tabSleep.enabled}
          onChange={(enabled) => onUpdate({ tabSleep: { ...settings.tabSleep, enabled } })}
        />
      </Row>
      <Row id="settings-sleep-minutes" setting="tabSleep.minutes" label={MINUTES_LABEL} hint={MINUTES_HINT} search={["minutes", sleepMinutesDraft]}>
        <input
          id="settings-sleep-minutes-input"
          type="number"
          min={5}
          max={720}
          step={5}
          disabled={!settings.tabSleep.enabled}
          value={sleepMinutesDraft}
          className={cn(inputClass, "w-24 text-right disabled:opacity-40")}
          onChange={(event) => setSleepMinutesDraft(event.target.value)}
          onBlur={() => {
            const minutes = Math.min(720, Math.max(5, Math.round(Number(sleepMinutesDraft) || 30)));
            setSleepMinutesDraft(String(minutes));
            if (minutes !== settings.tabSleep.minutes) {
              onUpdate({ tabSleep: { ...settings.tabSleep, minutes } });
            }
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.currentTarget.blur();
            }
          }}
        />
        <span id="settings-sleep-minutes-unit" className="text-xs text-zinc-500">
          minutes
        </span>
      </Row>
      <NeverSleep settings={settings} draft={neverSleepDraft} setDraft={setNeverSleepDraft} onAdd={addNeverSleep} onUpdate={onUpdate} />
    </Section>
  );
}

function NeverSleep({
  settings,
  draft,
  setDraft,
  onAdd,
  onUpdate,
}: {
  settings: BrowserSettings;
  draft: string;
  setDraft: (value: string) => void;
  onAdd: () => void;
  onUpdate: SettingsSectionProps["onUpdate"];
}) {
  const sites = settings.tabSleep.neverSleepSites;
  const visible = useSettingVisible(NEVER_LABEL, NEVER_HINT, NEVER_EMPTY, "Add", draft, ...sites);
  if (!visible) {
    return null;
  }
  return (
    <div id="settings-never-sleep" className="px-5 py-3">
      <div id="settings-never-sleep-label" className="text-sm text-zinc-100">
        {NEVER_LABEL}
      </div>
      <div id="settings-never-sleep-hint" className="mb-2 text-xs text-zinc-500">
        {NEVER_HINT}
      </div>
      <div id="settings-never-sleep-add" className="mb-2 flex gap-2">
        <input
          id="settings-never-sleep-input"
          value={draft}
          placeholder="example.com"
          className={inputClass}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              onAdd();
            }
          }}
        />
        <Button id="settings-never-sleep-add-button" size="sm" disabled={!draft.trim()} onClick={onAdd}>
          Add
        </Button>
      </div>
      {sites.length === 0 ? (
        <div id="settings-never-sleep-empty" className="text-xs text-zinc-500">
          {NEVER_EMPTY}
        </div>
      ) : (
        <ul id="settings-never-sleep-list" className="flex flex-col gap-1">
          {sites.map((site) => (
            <li
              id={`settings-never-sleep-${site}`}
              key={site}
              className="flex h-8 items-center justify-between rounded-md bg-zinc-950 pl-3 pr-1 text-sm text-zinc-200"
            >
              <span id={`settings-never-sleep-${site}-host`} className="truncate">
                {site}
              </span>
              <IconButton
                id={`settings-never-sleep-${site}-remove`}
                label={`Let tabs on ${site} sleep again`}
                onClick={() =>
                  onUpdate({
                    tabSleep: {
                      ...settings.tabSleep,
                      neverSleepSites: sites.filter((item) => item !== site),
                    },
                  })
                }
              >
                <X id={`settings-never-sleep-${site}-remove-icon`} aria-hidden />
              </IconButton>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
