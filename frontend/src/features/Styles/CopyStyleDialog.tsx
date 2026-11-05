import { useEffect, useState } from "react";
import type { BrowserProfile, BrowserSettings } from "@type/browser";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { importStyleMetrics, listProfiles } from "@/lib/browser-client";
import { cn } from "@/lib/cn";
import { focusRow } from "@/lib/focus-ring";

export function CopyStyleDialog({
  open,
  onClose,
  onCopied,
}: {
  open: boolean;
  onClose: () => void;
  onCopied: (settings: BrowserSettings) => void;
}) {
  const [profiles, setProfiles] = useState<BrowserProfile[]>([]);
  const [selected, setSelected] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!open) {
      return;
    }
    setError(null);
    setBusy(false);
    void listProfiles()
      .then((next) => {
        const others = next.filter((profile) => !profile.current);
        setProfiles(others);
        setSelected(others[0]?.id ?? "");
      })
      .catch((reason: unknown) => setError(String(reason)));
  }, [open]);

  const copy = () => {
    if (!selected || busy) {
      return;
    }
    setBusy(true);
    setError(null);
    void importStyleMetrics(selected)
      .then((settings) => {
        onCopied(settings);
        onClose();
      })
      .catch((reason: unknown) => {
        setBusy(false);
        setError(String(reason));
      });
  };

  return (
    <Dialog id="styles-copy-dialog" title="Copy from profile" open={open} onClose={onClose} onEnter={copy}>
      <p id="styles-copy-hint" className="text-sm text-zinc-400">
        Replace the sizes on this page with the ones from another profile. Theme, font, and zoom stay as they are.
      </p>
      {profiles.length === 0 ? (
        <p id="styles-copy-empty" className="mt-3 text-sm text-zinc-500">
          There isn't another profile to copy from.
        </p>
      ) : (
        <div id="styles-copy-list" className="mt-3 flex flex-col gap-1" role="listbox" aria-label="Profiles">
          {profiles.map((profile) => (
            <button
              key={profile.id}
              id={`styles-copy-${profile.id}`}
              type="button"
              role="option"
              aria-selected={selected === profile.id}
              className={cn(
                "flex h-9 items-center gap-2 rounded-md px-2 text-left text-sm text-zinc-100 hover:bg-zinc-800",
                focusRow,
                selected === profile.id && "bg-zinc-800",
              )}
              onClick={() => setSelected(profile.id)}
            >
              <img id={`styles-copy-${profile.id}-icon`} src={profile.icon || "/icon.png"} alt="" className="size-5 shrink-0 rounded" />
              <span id={`styles-copy-${profile.id}-name`} className="min-w-0 flex-1 truncate">
                {profile.name}
              </span>
            </button>
          ))}
        </div>
      )}
      {error ? (
        <p id="styles-copy-error" className="mt-3 text-sm text-red-300">
          {error}
        </p>
      ) : null}
      <div id="styles-copy-actions" className="mt-4 flex justify-end gap-2">
        <Button id="styles-copy-cancel" variant="ghost" onClick={onClose}>
          Cancel
        </Button>
        <Button id="styles-copy-confirm" disabled={!selected || busy} onClick={copy}>
          Copy sizes
        </Button>
      </div>
    </Dialog>
  );
}
