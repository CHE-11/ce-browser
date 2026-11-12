import { Palette } from "lucide-react";
import { ToolButton } from "../ToolButton";

export function ThemePickerButton() {
  return (
    <ToolButton
      id="theme-picker"
      label="Theme"
      kind="theme-picker"
      icon={<Palette id="address-theme-picker-icon" aria-hidden />}
      activeTab={undefined}
      needsPage={false}
    />
  );
}
