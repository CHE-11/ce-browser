import { Terminal } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { toggleConsolePanel } from "@/lib/browser-client";
import { useTabs } from "@/lib/use-browser";

/** Opens or closes the console beside the pages. */
export function ConsoleButton() {
  const { consolePanel } = useTabs();
  return (
    <IconButton
      id="address-console"
      label="Console"
      active={consolePanel}
      aria-pressed={consolePanel}
      onClick={() => void toggleConsolePanel()}
    >
      <Terminal id="address-console-icon" aria-hidden />
    </IconButton>
  );
}
