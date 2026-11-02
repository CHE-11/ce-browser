import { Clipboard, FileText, Puzzle, type LucideIcon } from "lucide-react";
import { cn } from "../lib/cn";

type Fallback = {
  ids: string[];
  names: string[];
  icon: LucideIcon;
  className: string;
};

/** WebView2 ships these component extensions without an icon, so they would all share the puzzle glyph. */
const COMPONENT_ICONS: Fallback[] = [
  {
    ids: ["dgiklkfkllikcanfonkcabmbdfmgleag"],
    names: ["microsoft clipboard extension"],
    icon: Clipboard,
    className: "text-sky-400",
  },
  {
    ids: ["mhjfbmdgcfjbbpaeojofohoefgiehjai"],
    names: ["microsoft edge pdf viewer"],
    icon: FileText,
    className: "text-rose-400",
  },
];

function componentIcon(extensionId: string, name: string) {
  const normalized = name.trim().toLowerCase();
  return COMPONENT_ICONS.find((item) => item.ids.includes(extensionId) || item.names.includes(normalized));
}

type ExtensionIconProps = {
  id: string;
  extensionId: string;
  name: string;
  src?: string | null;
  className?: string;
};

export function ExtensionIcon({ id, extensionId, name, src, className }: ExtensionIconProps) {
  if (src) {
    return <img id={id} src={src} alt="" draggable={false} className={cn("shrink-0 rounded object-contain", className)} />;
  }

  const fallback = componentIcon(extensionId, name);
  const Icon = fallback?.icon ?? Puzzle;
  return <Icon id={id} className={cn("shrink-0", fallback?.className ?? "text-zinc-500", className)} aria-hidden />;
}
