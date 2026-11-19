import { Workflow } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { toggleAutomationPanel } from "@/lib/browser-client";
import { useTabs } from "@/lib/use-browser";

/** Opens or closes the automations panel beside the pages. */
export function AutomationsButton() {
  const { automationPanel } = useTabs();
  return (
    <IconButton
      id="address-automations"
      label="Automations"
      active={automationPanel}
      aria-pressed={automationPanel}
      onClick={() => void toggleAutomationPanel()}
    >
      <Workflow id="address-automations-icon" aria-hidden />
    </IconButton>
  );
}
