import { Clapperboard, Eraser, ImageDown, Layers, Printer, Shield, type LucideIcon } from "lucide-react";

export type BuiltinTool = {
  id: string;
  name: string;
  description: string;
  icon: LucideIcon;
};

/** Tools that ship on the address bar. Their ids are stored when they are unpinned. */
export const BUILTIN_TOOLS: BuiltinTool[] = [
  {
    id: "print-pdf",
    name: "Print to PDF",
    description: "Save the current page as a PDF.",
    icon: Printer,
  },
  {
    id: "page-capture",
    name: "Save page as PNG",
    description: "Save or copy the page as an image.",
    icon: ImageDown,
  },
  {
    id: "media-hub",
    name: "Media",
    description: "Play, pause and seek audio and video in open tabs.",
    icon: Clapperboard,
  },
  {
    id: "clear-cache",
    name: "Clear site cache",
    description: "Clear cached files for the site in the current tab.",
    icon: Eraser,
  },
  {
    id: "what-runs",
    name: "What runs",
    description: "See the frameworks, languages and services a site uses.",
    icon: Layers,
  },
  {
    id: "ad-blocker",
    name: "Ad blocker",
    description: "Block ads and trackers on websites.",
    icon: Shield,
  },
];

export function builtinTool(id: string) {
  return BUILTIN_TOOLS.find((tool) => tool.id === id);
}
