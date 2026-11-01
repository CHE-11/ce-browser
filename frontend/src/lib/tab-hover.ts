import { closePopup, onPopupChanged, openPopup } from "./browser-client";

const FIRST_DELAY_MS = 550;
/** Moving from one tab to the next while a card is up swaps the card quickly. */
const NEXT_DELAY_MS = 80;
const CLOSE_DELAY_MS = 150;
const WARM_MS = 600;
const CARD_WIDTH = 280;
const CARD_HEIGHT = 240;

let openTimer: number | null = null;
let closeTimer: number | null = null;
let shownFor: number | null = null;
let closedAt = 0;
let popupKind: string | null = null;
let listening = false;

function listenForPopups() {
  if (listening) {
    return;
  }
  listening = true;
  void onPopupChanged((change) => {
    popupKind = change.kind;
    if (change.kind !== "tab-hover" && shownFor !== null) {
      shownFor = null;
      closedAt = Date.now();
    }
  });
}

function clearTimers() {
  if (openTimer !== null) {
    window.clearTimeout(openTimer);
    openTimer = null;
  }
  if (closeTimer !== null) {
    window.clearTimeout(closeTimer);
    closeTimer = null;
  }
}

/** The pointer rests on a tab. After a moment its hover card opens below it, or beside it in the sidebar. */
export function hoverTab(tabId: number, element: HTMLElement, vertical: boolean) {
  listenForPopups();
  clearTimers();
  if (popupKind !== null && popupKind !== "tab-hover") {
    return;
  }
  if (shownFor === tabId) {
    return;
  }
  const warm = shownFor !== null || Date.now() - closedAt < WARM_MS;
  openTimer = window.setTimeout(() => {
    openTimer = null;
    if (popupKind !== null && popupKind !== "tab-hover") {
      return;
    }
    const rect = element.getBoundingClientRect();
    shownFor = tabId;
    void openPopup({
      kind: "tab-hover",
      params: `id=${tabId}`,
      focus: false,
      x: vertical ? rect.right + 6 : rect.left,
      y: vertical ? rect.top : rect.bottom + 4,
      width: CARD_WIDTH,
      height: CARD_HEIGHT,
    }).catch(() => {
      shownFor = null;
    });
  }, warm ? NEXT_DELAY_MS : FIRST_DELAY_MS);
}

/** The pointer left the tab, or the tab was pressed. */
export function leaveTab(immediate = false) {
  clearTimers();
  if (shownFor === null) {
    return;
  }
  const close = () => {
    closeTimer = null;
    if (shownFor === null || (popupKind !== null && popupKind !== "tab-hover")) {
      shownFor = null;
      return;
    }
    shownFor = null;
    closedAt = Date.now();
    void closePopup();
  };
  if (immediate) {
    close();
  } else {
    closeTimer = window.setTimeout(close, CLOSE_DELAY_MS);
  }
}
