import { useEffect, useState } from "react";
import { Bookmark, Code, Database, Download, Folder, Globe, History, ImageDown, Puzzle, Settings, Sparkles } from "lucide-react";
import { cn } from "../lib/cn";
import { faviconFor } from "../lib/format";

const INTERNAL_ICONS: Record<string, typeof Globe> = {
  "ce://bookmarks": Bookmark,
  "ce://downloads": Download,
  "ce://files": Folder,
  "ce://history": History,
  "ce://extensions": Puzzle,
  "ce://settings": Settings,
  "ce://newtab": Sparkles,
  "ce://capture-editor": ImageDown,
  "ce://rewrites": Code,
  "ce://storage": Database,
};

type FavIconProps = {
  id: string;
  url: string;
  src?: string | null;
  className?: string;
};

export function FavIcon({ id, url, src, className }: FavIconProps) {
  const candidate = src ?? faviconFor(url);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setFailed(false);
  }, [candidate]);

  const Internal = INTERNAL_ICONS[url];
  if (Internal) {
    return <Internal id={id} className={cn("size-4 shrink-0 text-sky-400", className)} aria-hidden />;
  }
  if (!candidate || failed) {
    return <Globe id={id} className={cn("size-4 shrink-0 text-zinc-400", className)} aria-hidden />;
  }
  return (
    <img
      id={id}
      src={candidate}
      alt=""
      draggable={false}
      className={cn("size-4 shrink-0 rounded-sm object-contain", className)}
      onError={() => setFailed(true)}
    />
  );
}
