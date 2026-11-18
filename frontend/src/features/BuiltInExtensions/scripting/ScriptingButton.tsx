import { SquareCode } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { toggleScriptingPanel } from "@/lib/browser-client";
import { useTabs } from "@/lib/use-browser";

/** Opens or closes the scripting panel beside the pages. */
export function ScriptingButton() {
  const { scriptingPanel } = useTabs();
  return (
    <IconButton
      id="address-scripting"
      label="Scripting"
      active={scriptingPanel}
      aria-pressed={scriptingPanel}
      onClick={() => void toggleScriptingPanel()}
    >
      <SquareCode id="address-scripting-icon" aria-hidden />
    </IconButton>
  );
}
