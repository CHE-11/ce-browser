import { useEffect, useState } from "react";
import { AppWindow, Download, MessageSquare } from "lucide-react";
import type { AgentRun } from "@type/browser";
import { Button } from "@/components/Button";
import { inputClassName } from "./agent-styles";

type Props = {
  run: AgentRun;
  onDialog: (accept: boolean, text: string) => void;
  onShowTab: (tabId: number) => void;
};

/** Dialog, download and new-tab results sitting with the approval card. */
export function AgentPageStatus({ run, onDialog, onShowTab }: Props) {
  const [prompt, setPrompt] = useState(run.dialog?.detail ?? "");
  useEffect(() => {
    setPrompt(run.dialog?.detail ?? "");
  }, [run.dialog?.detail, run.dialog?.text, run.dialog?.kind]);
  const dialog = run.dialog;
  const download = run.download;
  const opened = run.openedTab;

  return (
    <div id="agent-page-status" className="flex flex-col gap-2">
      {dialog ? (
        <div id="agent-dialog" role="alertdialog" aria-labelledby="agent-dialog-message" className="rounded-lg border border-sky-800 bg-sky-950/40 p-3">
          <div id="agent-dialog-header" className="mb-2 flex items-start gap-2">
            <MessageSquare id="agent-dialog-icon" className="mt-0.5 size-4 shrink-0 text-sky-300" aria-hidden />
            <div id="agent-dialog-body" className="min-w-0">
              <span id="agent-dialog-kind" className="mb-0.5 inline-block rounded bg-sky-900/70 px-1 text-[10px] font-medium uppercase tracking-wide text-sky-200">
                {dialog.kind || "dialog"}
              </span>
              <p id="agent-dialog-message" className="whitespace-pre-wrap break-words text-sm text-zinc-100">
                {dialog.text}
              </p>
            </div>
          </div>
          {dialog.kind === "prompt" ? (
            <input
              id="agent-dialog-input"
              className={inputClassName}
              value={prompt}
              placeholder="Reply"
              onChange={(event) => setPrompt(event.target.value)}
            />
          ) : null}
          <div id="agent-dialog-actions" className="mt-2 flex justify-end gap-2">
            <Button id="agent-dialog-dismiss" size="sm" variant="ghost" onClick={() => onDialog(false, "")}>
              Dismiss
            </Button>
            <Button id="agent-dialog-accept" size="sm" variant="primary" onClick={() => onDialog(true, dialog.kind === "prompt" ? prompt : "")}>
              Accept
            </Button>
          </div>
        </div>
      ) : null}
      {download ? (
        <p id="agent-download" className="flex items-start gap-2 rounded-md bg-zinc-900 px-3 py-2 text-xs text-zinc-300">
          <Download id="agent-download-icon" className="mt-0.5 size-3.5 shrink-0 text-zinc-500" aria-hidden />
          <span id="agent-download-text" className="min-w-0 break-all">
            {download.text}
            {download.detail ? <span id="agent-download-path" className="block text-zinc-500">{download.detail}</span> : null}
          </span>
        </p>
      ) : null}
      {opened ? (
        <div id="agent-opened" className="flex items-center gap-2 rounded-md bg-zinc-900 px-3 py-2">
          <AppWindow id="agent-opened-icon" className="size-3.5 shrink-0 text-zinc-500" aria-hidden />
          <span id="agent-opened-text" className="min-w-0 flex-1 truncate text-xs text-zinc-300">
            {opened.kind === "window" ? `Opened a window (${opened.detail})` : opened.text || "New tab"}
          </span>
          {opened.tabId ? (
            <Button id="agent-opened-switch" size="sm" variant="outline" onClick={() => onShowTab(opened.tabId)}>
              Switch
            </Button>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
