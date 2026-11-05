import { Button } from "@/components/Button";
import { Row } from "@/features/Settings/SettingsControls";
import { goToPage } from "@/lib/route";

export const STYLES_LINK_LABEL = "Fine-tune styles";
export const STYLES_LINK_HINT = "In the Styles Editor.";
export const STYLES_LINK_TEXT = "Styles Editor";

/** A settings row that opens the Styles Editor, optionally on one surface. */
export function StylesLink({ id, surface, search }: { id: string; surface?: string; search?: string[] }) {
  return (
    <Row id={id} label={STYLES_LINK_LABEL} hint={STYLES_LINK_HINT} search={["styles", "editor", STYLES_LINK_TEXT, ...(search ?? [])]}>
      <Button
        id={`${id}-open`}
        variant="link"
        onClick={() => goToPage(surface ? `styles?surface=${encodeURIComponent(surface)}` : "styles")}
      >
        {STYLES_LINK_TEXT}
      </Button>
    </Row>
  );
}
