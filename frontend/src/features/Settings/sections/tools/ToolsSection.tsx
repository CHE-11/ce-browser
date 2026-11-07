import type { SettingsSectionProps } from "../../SettingsControls";
import { AdBlockSection } from "./AdBlockSection";
import { AiSection } from "./AiSection";
import { AutomationSection, McpSection } from "./AutomationSection";
import { ShoppingSection } from "./ShoppingSection";
import { TranslateSection } from "./TranslateSection";
import { PrintToPDfSsection } from "./PrintToPDfSsection";

export function ToolsSection(props: SettingsSectionProps) {
  return (
    <div id="settings-tools" className="flex flex-col gap-6 pt-10">
      <AdBlockSection {...props} />
      <AiSection {...props} />
      <AutomationSection {...props} />
      <McpSection {...props} />
      <ShoppingSection {...props} />
      <TranslateSection {...props} />
      <PrintToPDfSsection {...props} />
    </div>
  );
}
