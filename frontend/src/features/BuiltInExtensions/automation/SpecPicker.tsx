import { useState } from "react";
import { Crosshair } from "lucide-react";
import type { ExtractSpec } from "@type/browser";
import { Button } from "@/components/Button";
import { extractPick } from "@/lib/browser-client";

type SpecPickerProps = {
  id: string;
  spec: ExtractSpec | null;
  /** Tab to pick the list in. */
  tabId: number | null;
  onPick: (spec: ExtractSpec) => void;
};

export function SpecPicker({ id, spec, tabId, onPick }: SpecPickerProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const pick = () => {
    if (tabId === null) {
      return;
    }
    setBusy(true);
    setError(null);
    void extractPick(tabId)
      .then((picked) => {
        if (picked) {
          onPick(picked.spec);
        }
      })
      .catch((reason: unknown) => setError(String(reason)))
      .finally(() => setBusy(false));
  };

  return (
    <div id={id} className="flex flex-wrap items-center gap-2">
      {spec ? (
        <p id={`${id}-summary`} className="min-w-0 flex-1 text-xs text-zinc-300">
          Reads <span className="font-mono text-sky-300">{spec.item}</span> with {spec.fields.map((field) => field.name).join(", ")}.
        </p>
      ) : (
        <p id={`${id}-none`} className="min-w-0 flex-1 text-xs text-zinc-500">
          Nothing picked yet.
        </p>
      )}
      <Button id={`${id}-pick`} size="xs" variant="outline" disabled={tabId === null || busy} onClick={pick} title={tabId === null ? "Open the page in a tab first" : undefined}>
        <Crosshair id={`${id}-pick-icon`} className="size-3.5" aria-hidden />
        {busy ? "Pick on the page…" : spec ? "Pick again" : "Pick on page"}
      </Button>
      {error ? (
        <p id={`${id}-error`} className="w-full text-xs text-red-400">
          {error}
        </p>
      ) : null}
    </div>
  );
}
