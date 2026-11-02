import { createContext, useContext } from "react";

export type PopupHostState = {
  /** Carried by every size report, so a late report for a replaced popup is ignored. */
  token: number;
  /** 0 for a popup opened from the browser UI, one more for each submenu. */
  level: number;
  /** False while the popup window is still applying the saved theme. */
  appearanceReady: boolean;
};

export const PopupHostContext = createContext<PopupHostState | null>(null);

/** The popup window this page is showing in, or null outside one (toasts, ce:// pages). */
export function usePopupHost() {
  return useContext(PopupHostContext);
}
