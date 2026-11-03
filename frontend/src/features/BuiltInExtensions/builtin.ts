import { BookOpen, Clapperboard, Eraser, ImageDown, KeyRound, Languages, Layers, LayoutGrid, Printer, Shield, ShoppingCart, SlidersHorizontal, type LucideIcon } from "lucide-react";

export type BuiltinTool = {
  id: string;
  name: string;
  description: string;
  icon: LucideIcon;
  /** Drawn in a fixed address-bar slot instead of the tool strip. */
  address?: boolean;
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
    id: "boards",
    name: "Boards",
    description: "Save pages, regions, sections, links and images to boards.",
    icon: LayoutGrid,
  },
  {
    id: "media-hub",
    name: "Media",
    description: "Play, pause and seek audio and video in open tabs.",
    icon: Clapperboard,
    address: true,
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
  {
    id: "page-controls",
    name: "Page controls",
    description: "Change dark mode, scripts, media and other behavior for this site.",
    icon: SlidersHorizontal,
  },
  {
    id: "translate",
    name: "Translate",
    description: "Translate the page in place.",
    icon: Languages,
  },
  {
    id: "password",
    name: "Password",
    description: "Generate a password on this computer.",
    icon: KeyRound,
  },
  {
    id: "shopping",
    name: "Shopping",
    description: "Track prices and find coupons and better prices for this product.",
    icon: ShoppingCart,
  },
  {
    id: "reading",
    name: "Reading",
    description: "Word count, reading time and reading ease.",
    icon: BookOpen,
  },
];

export function builtinTool(id: string) {
  return BUILTIN_TOOLS.find((tool) => tool.id === id);
}
