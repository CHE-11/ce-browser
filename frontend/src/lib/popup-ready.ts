import { createContext, useContext, useEffect, useRef } from "react";
import { popupPainted } from "./browser-client";

/** False while the popup page is still applying the saved theme. */
export const PopupAppearanceContext = createContext(true);

function nextFrame() {
  return new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
}

/**
 * Tells the backend the popup's first complete frame is drawn once `ready` is true and the saved
 * theme is applied. The popup stays offscreen until then. `settle` resolves when the last size
 * report has been applied, so the frame that is shown already has its final height.
 */
export function usePopupPainted(ready: boolean, settle?: () => Promise<unknown>) {
  const appearanceReady = useContext(PopupAppearanceContext);
  const sent = useRef(false);
  const settleRef = useRef(settle);
  settleRef.current = settle;
  const show = ready && appearanceReady;

  useEffect(() => {
    if (!show || sent.current) {
      return;
    }
    let cancelled = false;
    void (async () => {
      await nextFrame();
      await settleRef.current?.().catch(() => undefined);
      await nextFrame();
      await nextFrame();
      if (cancelled || sent.current) {
        return;
      }
      sent.current = true;
      await popupPainted().catch(() => undefined);
    })();
    return () => {
      cancelled = true;
    };
  }, [show]);
}
