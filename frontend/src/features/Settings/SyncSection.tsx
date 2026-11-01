import { useEffect, useState } from "react";
import type { SyncStatus } from "@type/browser";
import { Button } from "@/components/Button";
import { clearSyncFolder, createSyncFolder, onSyncStatus, pickSyncFolder, selectSyncFolder, showSyncFolder, syncStatus } from "@/lib/browser-client";
import { SETTINGS_SECTIONS } from "./settings-sections";
import { errorText, matchesQuery, Row, Section, useReportMatch, useSettingVisible, type SettingsSectionProps } from "./SettingsControls";

const section = SETTINGS_SECTIONS.find((item) => item.id === "settings-sync")!;

const FOLDER_LABEL = "Folder location";
const FOLDER_HINT =
  "Profiles are stored in this folder. Selecting a folder uses the profiles already in it. Cookies and the download folder stay on this computer.";
const FILES_HINT = "profiles.json and a folder for each profile (Default, then one per profile name)";

export function SyncSection({
  settings,
  query,
  onError,
  onMatch,
  onProfilesChanged,
}: SettingsSectionProps & { onProfilesChanged: () => void }) {
  const [syncState, setSyncState] = useState<SyncStatus | null>(null);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [syncWorking, setSyncWorking] = useState(false);

  useEffect(() => {
    let disposed = false;
    void syncStatus()
      .then((status) => {
        if (!disposed) {
          setSyncState(status);
        }
      })
      .catch(() => undefined);
    const unlisten = onSyncStatus((status) => {
      if (!disposed) {
        setSyncState(status);
      }
    });
    return () => {
      disposed = true;
      void unlisten.then((stop) => stop());
    };
  }, []);

  const shownError = syncError ?? syncState?.error ?? "";
  const visible = matchesQuery(
    query,
    section.title,
    FOLDER_LABEL,
    FOLDER_HINT,
    "Create",
    "Select",
    settings.syncFolder,
    "Show in File Explorer",
    settings.syncFolder ? FILES_HINT : "",
    settings.syncFolder ? "Clear" : "",
    shownError,
  );
  useReportMatch(section.id, visible, onMatch);

  const reportSyncError = (reason: unknown) => {
    const message = errorText(reason);
    onError(message);
    setSyncError(message);
  };

  const runSync = (work: () => Promise<SyncStatus>) => {
    setSyncWorking(true);
    onError(null);
    setSyncError(null);
    void work()
      .then((status) => {
        setSyncState(status);
        if (status.error) {
          onError(status.error);
          setSyncError(status.error);
        }
      })
      .catch(reportSyncError)
      .finally(() => {
        setSyncWorking(false);
        onProfilesChanged();
      });
  };

  const pickAndSync = (title: string, apply: (path: string) => Promise<SyncStatus>) => {
    onError(null);
    setSyncError(null);
    void pickSyncFolder(title)
      .then((path) => {
        if (path) {
          runSync(() => apply(path));
        }
      })
      .catch(reportSyncError);
  };

  if (!visible) {
    return null;
  }

  return (
    <Section id={section.id} title={section.title}>
      <Row id="settings-sync-folder" label={FOLDER_LABEL} hint={FOLDER_HINT} search={["Create", "Select"]}>
        <Button id="settings-sync-create" size="sm" disabled={syncWorking} onClick={() => pickAndSync("Create a sync folder", createSyncFolder)}>
          Create
        </Button>
        <Button id="settings-sync-select" size="sm" disabled={syncWorking} onClick={() => pickAndSync("Select a sync folder", selectSyncFolder)}>
          Select
        </Button>
      </Row>
      {settings.syncFolder ? (
        <SyncPath folder={settings.syncFolder} busy={syncWorking} onError={onError} onClear={() => runSync(() => clearSyncFolder())} />
      ) : null}
      {shownError ? <SyncError message={shownError} /> : null}
    </Section>
  );
}

function SyncPath({
  folder,
  busy,
  onError,
  onClear,
}: {
  folder: string;
  busy: boolean;
  onError: (message: string | null) => void;
  onClear: () => void;
}) {
  const visible = useSettingVisible(folder, "Show in File Explorer", FILES_HINT, "Clear");
  if (!visible) {
    return null;
  }
  return (
    <div id="settings-sync-path" className="flex items-center justify-between gap-6 px-5 py-3">
      <div id="settings-sync-path-text" className="min-w-0">
        <button
          id="settings-sync-path-folder"
          type="button"
          title="Show in File Explorer"
          className="break-all text-left text-xs text-zinc-400 underline-offset-2 hover:text-sky-400 hover:underline"
          onClick={() => void showSyncFolder().catch((reason: unknown) => onError(String(reason)))}
        >
          {folder}
        </button>
        <div id="settings-sync-path-files" className="mt-1 text-xs text-zinc-500">
          {FILES_HINT}
        </div>
      </div>
      <Button id="settings-sync-clear" size="sm" variant="ghost" disabled={busy} onClick={onClear}>
        Clear
      </Button>
    </div>
  );
}

function SyncError({ message }: { message: string }) {
  const visible = useSettingVisible(message);
  if (!visible) {
    return null;
  }
  return (
    <div id="settings-sync-error" className="px-5 py-3 text-sm text-red-300">
      {message}
    </div>
  );
}
