const EXTENSION_ID = /^[a-p]{32}$/;

export type ExtensionStore = "chrome" | "edge";

export type StoreListing = {
  store: ExtensionStore;
  id: string;
};

/** A Chrome Web Store or Edge Add-ons listing, or a bare extension id (treated as Chrome). */
export function storeListing(input: string): StoreListing | null {
  const text = input.trim();
  if (EXTENSION_ID.test(text)) {
    return { store: "chrome", id: text };
  }
  let url: URL;
  try {
    url = new URL(text);
  } catch {
    return null;
  }
  const host = url.hostname.toLowerCase().replace(/^www\./, "");
  let store: ExtensionStore;
  if (host === "chromewebstore.google.com" || host === "chrome.google.com") {
    store = "chrome";
  } else if (host === "microsoftedge.microsoft.com") {
    store = "edge";
  } else {
    return null;
  }
  const path = url.pathname.toLowerCase();
  if (store === "chrome" && !path.includes("/detail/")) {
    return null;
  }
  if (store === "edge" && !path.includes("/addons/detail/")) {
    return null;
  }
  const id = url.pathname.split("/").find((part) => EXTENSION_ID.test(part));
  if (!id) {
    return null;
  }
  return { store, id };
}

export function storeLabel(store: ExtensionStore | null): string {
  if (store === "chrome") {
    return "Chrome Web Store";
  }
  if (store === "edge") {
    return "Edge Add-ons";
  }
  return "";
}
