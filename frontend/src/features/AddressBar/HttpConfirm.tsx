import { useState } from "react";
import { TriangleAlert } from "lucide-react";
import { Button } from "@/components/Button";
import { PopupFrame } from "@/components/PopupFrame";
import { closeThisPopup, createTab, navigate, noteTyped } from "@/lib/browser-client";

type HttpConfirmProps = {
  params: URLSearchParams;
};

/** Asks before an address-bar `http` visit that HTTPS-only would otherwise rewrite. */
export function HttpConfirm({ params }: HttpConfirmProps) {
  const url = params.get("url") ?? "";
  const id = Number(params.get("id"));
  const fresh = params.get("new") === "1";
  const typed = params.get("typed") === "1";
  const [busy, setBusy] = useState(false);

  const stay = () => {
    if (!busy) {
      void closeThisPopup();
    }
  };

  const go = () => {
    if (busy || !url) {
      return;
    }
    setBusy(true);
    if (typed) {
      void noteTyped(url);
    }
    const work = fresh || !Number.isFinite(id) ? createTab(url, false, undefined, true) : navigate(id, url, true);
    void work.finally(() => closeThisPopup());
  };

  return (
    <PopupFrame id="http-confirm" width={380} role="dialog" labelledBy="http-confirm-title">
      <div id="http-confirm-body" className="flex flex-col gap-3 p-4">
        <div id="http-confirm-heading" className="flex items-center gap-2">
          <TriangleAlert id="http-confirm-icon" className="size-4 shrink-0 text-amber-400" aria-hidden />
          <h2 id="http-confirm-title" className="text-sm font-medium text-zinc-100">
            Open this HTTP site?
          </h2>
        </div>
        <p id="http-confirm-url" className="break-all text-sm font-medium text-zinc-100">
          {url || "This address"}
        </p>
        <p id="http-confirm-detail" className="text-sm leading-relaxed text-zinc-400">
          This connection is not encrypted. Continue to open this site over HTTP. Other pages on it stay on HTTP until you quit.
        </p>
        <div id="http-confirm-actions" className="flex justify-end gap-2">
          <Button id="http-confirm-cancel" size="sm" variant="ghost" disabled={busy} onClick={stay}>
            Cancel
          </Button>
          <Button id="http-confirm-continue" size="sm" variant="primary" disabled={busy || !url} autoFocus onClick={go}>
            Continue
          </Button>
        </div>
      </div>
    </PopupFrame>
  );
}
