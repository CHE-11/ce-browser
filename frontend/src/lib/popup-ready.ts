import { useEffect, useRef, createContext } from "react";
import { popupHostReady } from "./browser-client";

/** False while the popup page is still applying the saved theme. */
export const PopupAppearanceContext = createContext(true);

/** Tells the backend the parked host can take a menu. Sends once per mount. */
export function usePopupHostReady(ready: boolean) {
  const sent = useRef(false);
  useEffect(() => {
    if (!ready || sent.current) {
      return;
    }
    sent.current = true;
    void popupHostReady();
  }, [ready]);
}
