import { CircleDot, Square } from "lucide-react";
import type { TabInfo } from "@type/browser";
import { IconButton } from "@/components/IconButton";
import { showNotice, startFlowRecording, toggleAutomationPanel } from "@/lib/browser-client";
import { useAutomations } from "@/lib/use-browser";
import { capturable } from "../shared";

/** Starts a recording in the active tab and opens the automations panel beside the page. */
export function RecordFlowButton({ activeTab }: { activeTab: TabInfo | undefined }) {
  const { snapshot } = useAutomations();
  const recording = snapshot?.recording ?? null;

  const toggle = () => {
    if (recording) {
      void toggleAutomationPanel(true);
      return;
    }
    if (activeTab) {
      void startFlowRecording(activeTab.id).catch((reason: unknown) => void showNotice(String(reason)));
    }
  };

  return (
    <IconButton
      id="address-record-flow"
      label={recording ? `Show the recording (${recording.steps.length} steps)` : "Record a flow"}
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
