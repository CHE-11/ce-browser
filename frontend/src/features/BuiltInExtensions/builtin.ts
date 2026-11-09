import {
  BookOpen,
  Bot,
  CircleDot,
  Clapperboard,
  Eraser,
  FileText,
  ImageDown,
  Video,
  KeyRound,
  Languages,
  Layers,
  LayoutGrid,
  Printer,
  Shield,
  ShoppingCart,
  SlidersHorizontal,
  SquareTerminal,
  TableProperties,
  Workflow,
  type LucideIcon,
} from "lucide-react";

export type BuiltinTool = {
  id: string;
  name: string;
  description: string;
  icon: LucideIcon;
  /** Defaults to the address bar. Other tools default to the tools toolbar. */
  address?: boolean;
  /** Developer tool. It is listed with the other built-in tools and runs in dev and release builds. */
  dev?: boolean;
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
    name: "Save as PNG",
    description: "Save or copy the page as an image.",
    icon: ImageDown,
  },
  {
    id: "recording",
    name: "Recording",
    description: "Record a tab, window, or screen.",
    icon: Video,
  },
  {
    id: "webpage-markdown",
    name: "To .md",
    description: "Convert the current page to Markdown.",
    icon: FileText,
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
    name: "Clear cache",
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
    description: "Generate a password.",
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
  {
    id: "agent",
    name: "Agent",
    description: "Let AI do a task in the current tab with real clicks and typing.",
    icon: Bot,
  },
  {
    id: "automations",
    name: "Automations",
    description: "View flows and watch steps as you record them, beside the page.",
    icon: Workflow,
  },
  {
    id: "record-flow",
    name: "Record flow",
    description: "Record what you do in a tab and replay it later or on a schedule.",
    icon: CircleDot,
  },
  {
    id: "extract",
    name: "Extract data",
    description: "Turn lists and tables on a page into a dataset you can export.",
    icon: TableProperties,
  },
  {
    id: "debug-hud",
    name: "Debug HUD",
    description: "IPC, logs, webviews and browser state in a window beside the page.",
    icon: SquareTerminal,
    dev: true,
  },
];

export function builtinTool(id: string) {
  return BUILTIN_TOOLS.find((tool) => tool.id === id);
}
