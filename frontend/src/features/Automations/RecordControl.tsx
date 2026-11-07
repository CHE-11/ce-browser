import { useState } from "react";
import { Circle, Square } from "lucide-react";
import type { FlowRecording, TabInfo } from "@type/browser";
import { Button } from "@/components/Button";
import { PageTabSelect, pickedTab } from "./PageTabSelect";

type RecordControlProps = {
  recording: FlowRecording | null;
  /** Name of the flow the recording adds to, when it isn't a new flow. */
  targetName: string | null;
  tabs: TabInfo[];
  recordingTab: TabInfo | undefined;
  onRecord: (tabId: number) => void;
  onStop: (save: boolean) => void;
  onShow: () => void;
};

export function RecordControl({ recording, targetName, tabs, recordingTab, onRecord, onStop, onShow }: RecordControlProps) {
  const [chosen, setChosen] = useState<number | null>(null);
  const tab = pickedTab(tabs, chosen);

  if (recording) {
    return (
      <div id="automations-record" className="flex flex-col gap-2 border-t border-zinc-800 pt-3">
        <button id="automations-record-status" type="button" className="flex items-start gap-2 rounded px-1 text-left text-xs text-red-300 hover:bg-zinc-900" onClick={onShow}>
          <Circle id="automations-record-dot" className="mt-0.5 size-2.5 shrink-0 animate-pulse fill-red-500 text-red-500" aria-hidden />
          <span id="automations-record-status-text" className="min-w-0">
            Recording {recording.steps.length} {recording.steps.length === 1 ? "step" : "steps"}
            {targetName ? ` into ${targetName}` : ""}
            {recordingTab ? <span className="block truncate text-zinc-500">on {recordingTab.title || recordingTab.url}</span> : null}
          </span>
        </button>
        <div id="automations-record-actions" className="flex gap-2">
          <Button id="automations-record-save" size="sm" variant="destructive" className="flex-1" onClick={() => onStop(true)}>
            <Square id="automations-record-save-icon" className="size-3" aria-hidden />
            Stop and save
          </Button>
          <Button id="automations-record-discard" size="sm" variant="ghost" onClick={() => onStop(false)}>
            Discard
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div id="automations-record" className="flex flex-col gap-2 border-t border-zinc-800 pt-3">
      <PageTabSelect id="automations-record-tab" tabs={tabs} value={tab} onChange={setChosen} label="Page to record on" emptyText="Open a web page to record on" />
      <Button id="automations-record-start" size="sm" variant="outline" disabled={tab === null} onClick={() => tab !== null && onRecord(tab)}>
        <Circle id="automations-record-start-icon" className="size-3 fill-red-500 text-red-500" aria-hidden />
        Record a flow
      </Button>
    </div>
  );
}
