import { useState } from "react";
import { Hourglass } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { closePopup, endPage } from "@/lib/browser-client";
import { hostOf } from "@/lib/format";
import { useTabs } from "@/lib/use-browser";

type UnresponsivePromptProps = {
  tabId: number;
};

/** Asked when the page on screen stops answering: wait for it, or end its process. */
export function UnresponsivePrompt({ tabId }: UnresponsivePromptProps) {
  const snapshot = useTabs();
  const tab = snapshot.windowTabs.find((item) => item.id === tabId);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const name = tab ? tab.title || hostOf(tab.url) : "This page";

  const end = () => {
    setBusy(true);
    setError(null);
    endPage(tabId)
      .then(() => closePopup())
      .catch((reason: unknown) => {
        setBusy(false);
        setError(String(reason));
      });
  };

  return (
    <PopupFrame id="unresponsive" ready={snapshot.loaded}>
      <div id="unresponsive-body" role="alertdialog" aria-labelledby="unresponsive-title" aria-describedby="unresponsive-text" className="flex flex-col gap-3 p-4">
        <div id="unresponsive-head" className="flex items-start gap-3">
          <Hourglass id="unresponsive-icon" className="mt-0.5 size-5 shrink-0 text-amber-400" aria-hidden />
          <div id="unresponsive-copy" className="min-w-0">
            <h2 id="unresponsive-title" className="text-sm font-semibold text-zinc-100">
              Page isn't responding
            </h2>
            <p id="unresponsive-text" className="mt-1 text-xs leading-relaxed text-zinc-400">
              <span id="unresponsive-name" className="font-medium text-zinc-200">
                {name}
              </span>{" "}
              is busy. You can wait for it, or end it and reload later.
            </p>
          </div>
        </div>
        {error ? (
          <p id="unresponsive-error" className="text-xs text-red-300">
            {error}
          </p>
        ) : null}
        <div id="unresponsive-actions" className="flex justify-end gap-2">
          <Button id="unresponsive-wait" autoFocus disabled={busy} onClick={() => void closePopup()}>
            Wait
          </Button>
          <Button id="unresponsive-end" variant="destructive" disabled={busy} onClick={end}>
            End page
          </Button>
        </div>
      </div>
    </PopupFrame>
  );
}
