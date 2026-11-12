import { SwatchBook } from "lucide-react";
import { ToolButton } from "../ToolButton";

export function StylePickerButton() {
  return (
    <ToolButton
      id="style-picker"
      label="Styles"
      kind="style-picker"
      icon={<SwatchBook id="address-style-picker-icon" aria-hidden />}
      activeTab={undefined}
      needsPage={false}
    />
  );
}
