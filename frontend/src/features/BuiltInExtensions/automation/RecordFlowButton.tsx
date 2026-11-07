import { CircleDot, Square } from "lucide-react";
import type { TabInfo } from "@type/browser";
import { IconButton } from "@/components/IconButton";
import { openAutomations, showNotice, startFlowRecording, stopFlowRecording } from "@/lib/browser-client";
import { useAutomations } from "@/lib/use-browser";
import { capturable } from "../shared";

/** Records clicks and typing in the active tab as a flow. Red while recording. */
export function RecordFlowButton({ activeTab }: { activeTab: TabInfo | undefined }) {
  const { snapshot } = useAutomations();
  const recording = snapshot?.recording ?? null;

  const toggle = () => {
    if (recording) {
      void stopFlowRecording(true)
        .then((id) => {
          if (id) {
            void openAutomations(`flow=${id}`);
          }
        })
        .catch((reason: unknown) => void showNotice(String(reason)));
      return;
    }
    if (activeTab) {
      void startFlowRecording(activeTab.id).catch((reason: unknown) => void showNotice(String(reason)));
    }
  };

  return (
    <IconButton
      id="address-record-flow"
      label={recording ? `Stop recording (${recording.steps.length} steps)` : "Record a flow"}
      active={!!recording}
      aria-pressed={!!recording}
      disabled={!recording && !capturable(activeTab?.url)}
      className={recording ? "text-red-400 hover:text-red-300" : undefined}
      onClick={toggle}
    >
      {recording ? <Square id="address-record-flow-icon" aria-hidden className="fill-current" /> : <CircleDot id="address-record-flow-icon" aria-hidden />}
    </IconButton>
  );
}
