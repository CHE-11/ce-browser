import { Bot } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { toggleAgentPanel } from "@/lib/browser-client";
import { useTabs } from "@/lib/use-browser";

/** Opens or closes the agent panel beside the pages. */
export function AgentButton() {
  const { agentPanel } = useTabs();
  return (
    <IconButton id="address-agent" label="Agent (Ctrl+Shift+Y)" active={agentPanel} aria-pressed={agentPanel} onClick={() => void toggleAgentPanel()}>
      <Bot id="address-agent-icon" aria-hidden />
    </IconButton>
  );
}
