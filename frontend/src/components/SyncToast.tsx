import { useEffect, useState } from "react";
import type { SyncStatus } from "../../../types/browser";
import { onSyncStatus, syncStatus } from "../lib/browser-client";
import { Button } from "./Button";

export function SyncToast() {
  const [error, setError] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    const apply = (status: SyncStatus) => {
      if (disposed) {
        return;
      }
      const next = status.saveError ? status.error : null;
      setError(next);
      if (!next) {
        setDismissed(null);
      }
    };
    void syncStatus().then(apply).catch(() => undefined);
    const unlisten = onSyncStatus(apply);
    return () => {
      disposed = true;
      void unlisten.then((stop) => stop());
    };
  }, []);

  if (!error || error === dismissed) {
    return null;
  }

  return (
    <div id="sync-toast" role="status" className="flex items-start gap-3 border-t border-red-900 bg-red-950 px-3 py-2 text-xs text-red-200">
      <p id="sync-toast-message" className="min-w-0 flex-1">
        Sync save error: {error}
      </p>
      <Button
        id="sync-toast-dismiss"
        variant="link"
        size="xs"
        aria-label="Dismiss sync error"
        className="shrink-0 text-red-300 hover:text-red-100"
        onClick={() => setDismissed(error)}
      >
        Dismiss
      </Button>
    </div>
  );
}
