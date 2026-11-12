import { Route } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { createTab } from "@/lib/browser-client";

export function RewritesButton() {
  return (
    <IconButton id="address-rewrites" label="Request rules" onClick={() => void createTab("ce://rewrites")}>
      <Route id="address-rewrites-icon" aria-hidden />
    </IconButton>
  );
}
