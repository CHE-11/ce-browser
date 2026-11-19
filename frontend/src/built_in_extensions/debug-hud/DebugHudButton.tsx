import { SquareTerminal } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { debugToggleHud } from "@/lib/browser-client";

/** Opens or closes the debug HUD beside this window. */
export function DebugHudButton() {
  return (
    <IconButton id="address-debug-hud" label="Debug HUD (Ctrl+Shift+F12)" onClick={() => void debugToggleHud()}>
      <SquareTerminal id="address-debug-hud-icon" aria-hidden />
    </IconButton>
  );
}
