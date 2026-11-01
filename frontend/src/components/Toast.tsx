import { useEffect, useRef, useState } from "react";
import { Undo2, X } from "lucide-react";
import { dismissToast, undoAction } from "../lib/browser-client";
import { Button } from "./Button";
import { IconButton } from "./IconButton";

const VISIBLE_MS = 8000;

type ToastProps = {
  params: URLSearchParams;
};

/** A message over the bottom of the page with an Undo button. It closes itself after a few seconds
 * unless the pointer rests on it. */
export function Toast({ params }: ToastProps) {
  const id = Number(params.get("id"));
  const message = params.get("message") ?? "";
  const canUndo = params.get("undo") === "1";
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hovered, setHovered] = useState(false);
  const left = useRef(VISIBLE_MS);

  useEffect(() => {
    document.documentElement.dataset.toast = "true";
  }, []);

  useEffect(() => {
    if (hovered || busy) {
      return;
    }
    const started = Date.now();
    const timer = window.setTimeout(() => void dismissToast(id), left.current);
    return () => {
      window.clearTimeout(timer);
      left.current = Math.max(1500, left.current - (Date.now() - started));
    };
  }, [hovered, busy, id]);

  const undo = () => {
    setBusy(true);
    setError(null);
    undoAction(id).catch((reason: unknown) => {
      setBusy(false);
      setError(String(reason));
    });
  };

  return (
    <div
      id="toast"
      role="status"
      aria-live="polite"
      className="flex h-full items-center gap-2 rounded-[10px] bg-zinc-900 pl-4 pr-1.5 text-sm text-zinc-100 ring-1 ring-inset ring-zinc-700 motion-safe:animate-[toast-in_160ms_ease-out]"
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onContextMenu={(event) => event.preventDefault()}
    >
      <p id="toast-message" className="min-w-0 flex-1 truncate" title={error ?? message}>
        {error ?? message}
      </p>
      {canUndo && !error ? (
        <Button id="toast-undo" variant="ghost" size="sm" disabled={busy} className="text-sky-300" onClick={undo}>
          <Undo2 id="toast-undo-icon" className="size-3.5" aria-hidden />
          Undo
        </Button>
      ) : null}
      <IconButton id="toast-close" label="Close" className="size-7" onClick={() => void dismissToast(id)}>
        <X id="toast-close-icon" aria-hidden />
      </IconButton>
    </div>
  );
}
