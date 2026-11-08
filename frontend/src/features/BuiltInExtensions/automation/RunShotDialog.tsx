import { useEffect, useState } from "react";
import { Dialog } from "@/components/Dialog";
import { flowRunShot } from "@/lib/browser-client";

export type RunShot = { runId: number; index: number; label?: string };

type RunShotDialogProps = {
  shot: RunShot | null;
  onClose: () => void;
};

/** The page as it looked when a run's step failed. */
export function RunShotDialog({ shot, onClose }: RunShotDialogProps) {
  const [image, setImage] = useState<string | null>(null);
  const [state, setState] = useState<"loading" | "ready" | "missing">("loading");
  const runId = shot?.runId;
  const index = shot?.index;

  useEffect(() => {
    if (runId === undefined || index === undefined) {
      return;
    }
    let live = true;
    setImage(null);
    setState("loading");
    void flowRunShot(runId, index)
      .then((url) => {
        if (live) {
          setImage(url);
          setState(url ? "ready" : "missing");
        }
      })
      .catch(() => live && setState("missing"));
    return () => {
      live = false;
    };
  }, [runId, index]);

  return (
    <Dialog
      id="run-shot"
      title={shot ? `Step ${shot.index + 1} failed${shot.label ? `: ${shot.label}` : ""}` : "Screenshot"}
      open={shot !== null}
      onClose={onClose}
      panelClassName="max-w-5xl"
    >
      {state === "loading" ? (
        <p id="run-shot-loading" className="text-sm text-zinc-500">
          Loading the screenshot…
        </p>
      ) : null}
      {state === "missing" ? (
        <p id="run-shot-missing" className="text-sm text-zinc-500">
          The screenshot is gone. It's removed when the run leaves the history.
        </p>
      ) : null}
      {image ? <img id="run-shot-image" src={image} alt="The page when the step failed" className="w-full rounded border border-zinc-800" /> : null}
    </Dialog>
  );
}
