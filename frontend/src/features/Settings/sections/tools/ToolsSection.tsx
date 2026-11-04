import type { SettingsSectionProps } from "../../SettingsControls";
import { AdBlockSection } from "./AdBlockSection";
import { AiSection } from "./AiSection";
import { ShoppingSection } from "./ShoppingSection";
import { PrintSection } from "./PrintSection";
import { TranslateSection } from "./TranslateSection";

export function ToolsSection(props: SettingsSectionProps) {
  return (
    <div id="settings-tools" className="flex flex-col gap-6 pt-10">
      <AdBlockSection {...props} />
      <AiSection {...props} />
      <ShoppingSection {...props} />
      <TranslateSection {...props} />
      <PrintSection {...props} />
    </div>
  );
}
