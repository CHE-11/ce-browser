import { useEffect } from "react";
import { ShoppingPanel } from "./ShoppingPanel";

/** The docked column beside the pages for price tracking and deals on the active tab. */
export function ShoppingPanelPage() {
  useEffect(() => {
    document.title = "Shopping";
  }, []);

  return <ShoppingPanel />;
}
