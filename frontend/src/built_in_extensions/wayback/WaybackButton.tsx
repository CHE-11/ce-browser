import { IconButton } from "@/components/IconButton";
import { createTab } from "@/lib/browser-client";
import { TabInfo } from "@type/browser";
import { Landmark } from "lucide-react";

type WaybackButtonProps = {
  activeTab: TabInfo | undefined;
};

/** Wayback Machine URL for an http(s) page. Internal pages are not archived. */
export function waybackUrl(url: string | undefined) {
  if (!url) {
    return null;
  }
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return null;
    }
    return `https://web.archive.org/web/*/${parsed.href}`;
  } catch {
    return null;
  }
}

export function WaybackButton({ activeTab }: WaybackButtonProps) {
  const archiveUrl = waybackUrl(activeTab?.url);

  const onOpen = () => {
    if (archiveUrl !== null) {
      void createTab(archiveUrl);
    }
  };

  return (
    <IconButton
      id="address-wayback"
      label={archiveUrl ? "Open in the Internet Archive" : "This page can't be archived"}
      disabled={archiveUrl === null}
      onClick={onOpen}
    >
      <Landmark id="address-wayback-icon" aria-hidden />
    </IconButton>
  );
}
