import { Type } from "lucide-react";
import { ToolButton } from "../ToolButton";

export function FontPickerButton() {
  return (
    <ToolButton
      id="font-picker"
      label="Font"
      kind="font-picker"
      icon={<Type id="address-font-picker-icon" aria-hidden />}
      activeTab={undefined}
      needsPage={false}
    />
  );
}
