import type { MouseEvent } from "react";
import type { TabInfo } from "../../../types/browser";
import { activateTab, closeSelectedTabs, closeTab, openPopup } from "./browser-client";

/** Right-click on a tab. A tab outside the selection becomes the only selected tab first. */
export function openTabMenu(event: MouseEvent, tab: TabInfo, grouped: boolean) {
  event.preventDefault();
  if (!grouped) {
    void activateTab(tab.id);
  }
  void openPopup("tab-menu", { x: event.clientX, y: event.clientY }, { params: `id=${tab.id}` });
}

/** Closes the whole selection when the tab is part of it. */
export function closeTabOrSelection(id: number, grouped: boolean) {
  void (grouped ? closeSelectedTabs() : closeTab(id));
}
