import type { TabInfo } from "../../../types/browser";

/** Pages the page tools can read: ordinary http(s) pages and local files. */
export function capturable(url: string | undefined) {
  return !!url && /^(https?|file):/i.test(url);
}

/** Query string handed to a built-in extension popover, so it renders without loading anything. */
export function tabParams(tab: TabInfo) {
  return new URLSearchParams({ tab: String(tab.id), title: tab.title || tab.url, url: tab.url }).toString();
}

/** Popover input decoded from `tabParams`. */
export type PopoverTab = {
  id: number;
  title: string;
  url: string;
};

export function readPopoverTab(params: URLSearchParams): PopoverTab {
  return {
    id: Number(params.get("tab")),
    title: params.get("title") ?? "",
    url: params.get("url") ?? "",
  };
}

/** A file name made from a page title, without characters Windows rejects. */
export function fileNameFor(title: string, url: string, extension: string) {
  let base = title.trim();
  if (!base) {
    try {
      base = new URL(url).hostname;
    } catch {
      base = "page";
    }
  }
  base = base
    .replace(/[<>:"/\\|?*\u0000-\u001f]/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/[. ]+$/, "")
    .slice(0, 120);
  return `${base || "page"}.${extension}`;
}

/** Joins a folder and a file name with a Windows separator. */
export function joinPath(folder: string, name: string) {
  if (!folder) {
    return name;
  }
  return /[\\/]$/.test(folder) ? `${folder}${name}` : `${folder}\\${name}`;
}
