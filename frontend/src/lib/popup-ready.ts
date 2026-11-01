import { createContext, useContext, useEffect, useRef } from "react";
import { popupHostReady, popupPainted } from "./browser-client";

/** False while the popup page is still applying the saved theme. */
export const PopupAppearanceContext = createContext(true);

/** Generation captured when this menu mounted. A later menu has a newer one. */
export const PopupGenerationContext = createContext(0);

export function readPopupGeneration() {
  const value = (window as Window & { __cePopupGen?: number }).__cePopupGen;
  return typeof value === "number" ? value : 0;
}

function nextFrame() {
  return new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
}

/**
 * Tells the backend the popup's first complete frame is drawn once `ready` is true and the saved
 * theme is applied. The popup stays offscreen until the resize and shape reports finish, plus one
 * frame, so the frame that is shown already has its final height.
 */
export function usePopupPainted(ready: boolean, settle?: () => Promise<unknown>) {
  const appearanceReady = useContext(PopupAppearanceContext);
  const menuGeneration = useContext(PopupGenerationContext);
  const sent = useRef(false);
  const settleRef = useRef(settle);
  settleRef.current = settle;
  const show = ready && appearanceReady;

  useEffect(() => {
    if (!show || sent.current) {
      return;
    }
    const generation = menuGeneration || readPopupGeneration();
    let cancelled = false;
    void (async () => {
      await settleRef.current?.().catch(() => undefined);
      await nextFrame();
      if (cancelled || sent.current) {
        return;
      }
      sent.current = true;
      await popupPainted(generation).catch(() => undefined);
    })();
    return () => {
      cancelled = true;
    };
  }, [show, menuGeneration]);
}

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
