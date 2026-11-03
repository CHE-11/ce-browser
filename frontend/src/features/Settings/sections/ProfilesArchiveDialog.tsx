import { useState } from "react";
import type { BrowserProfile } from "@type/browser";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { exportProfileArchive, importProfileArchive, listProfiles } from "@/lib/browser-client";
import { inputClass } from "../SettingsControls";

export type ProfileArchiveRequest = {
  mode: "export" | "import";
  path: string;
};

export function ProfilesArchiveDialog({
  request,
  onClose,
  onDone,
}: {
  request: ProfileArchiveRequest | null;
  onClose: () => void;
  onDone: (note: string, profiles?: BrowserProfile[]) => void;
}) {
  if (!request) {
    return null;
  }
  return <ArchiveForm key={`${request.mode}:${request.path}`} request={request} onClose={onClose} onDone={onDone} />;
}

function ArchiveForm({
  request,
  onClose,
  onDone,
}: {
  request: ProfileArchiveRequest;
  onClose: () => void;
  onDone: (note: string, profiles?: BrowserProfile[]) => void;
}) {
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [working, setWorking] = useState(false);

  const submit = () => {
    if (working) {
      return;
    }
    if (!password) {
      setError("Enter a password.");
      return;
    }
    if (request.mode === "export" && password !== confirm) {
      setError("Those passwords do not match.");
      return;
    }
    setWorking(true);
    const work =
      request.mode === "export"
        ? exportProfileArchive(request.path, password).then(() => {
            onDone("Profile exported.");
          })
        : importProfileArchive(request.path, password).then((imported) =>
            listProfiles().then((next) => {
              onDone(`Imported as ${imported.name}. Open it from the list.`, next);
            }),
          );
    void work.catch((reason: unknown) => {
      setError(String(reason));
      setWorking(false);
    });
  };

  return (
    <Dialog
      id="settings-profile-archive"
      title={request.mode === "import" ? "Import profile" : "Export profile"}
      open
      onClose={() => {
        if (!working) {
          onClose();
        }
      }}
      panelClassName="max-w-md"
    >
      <form
        id="settings-profile-archive-form"
        className="flex flex-col gap-3"
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <p id="settings-profile-archive-path" className="break-all text-xs text-zinc-500">
          {request.path}
        </p>
        <label id="settings-profile-archive-password-label" className="flex flex-col gap-1 text-sm text-zinc-300">
          Password
          <input
            id="settings-profile-archive-password"
            type="password"
            autoFocus
            value={password}
            className={inputClass}
            onChange={(event) => {
              setPassword(event.target.value);
              setError(null);
            }}
          />
        </label>
        {request.mode === "export" ? (
          <label id="settings-profile-archive-confirm-label" className="flex flex-col gap-1 text-sm text-zinc-300">
            Confirm password
            <input
              id="settings-profile-archive-confirm"
              type="password"
              value={confirm}
              className={inputClass}
              onChange={(event) => {
                setConfirm(event.target.value);
                setError(null);
              }}
            />
          </label>
        ) : null}
        {error ? (
          <p id="settings-profile-archive-error" className="text-sm text-red-300">
            {error}
          </p>
        ) : null}
        <div id="settings-profile-archive-actions" className="flex justify-end gap-2">
          <Button id="settings-profile-archive-cancel" size="sm" variant="ghost" disabled={working} onClick={onClose}>
            Cancel
          </Button>
          <Button id="settings-profile-archive-save" size="sm" type="submit" disabled={working}>
            {request.mode === "import" ? "Import" : "Export"}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
