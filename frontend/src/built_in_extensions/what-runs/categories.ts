import { TechnologyCategory } from "@type/browser";
import {
  AppWindow,
  Boxes,
  ChartLine,
  CircleHelp,
  Cloud,
  Code,
  FileText,
  Hammer,
  Package,
  Palette,
  Server,
  type LucideIcon,
} from "lucide-react";

export type CategoryInfo = {
  id: TechnologyCategory;
  label: string;
  icon: LucideIcon;
};

/** Display order of the groups in the What runs popover. */
export const CATEGORIES: CategoryInfo[] = [
  { id: "platform", label: "Software", icon: AppWindow },
  { id: "framework", label: "Frameworks", icon: Boxes },
  { id: "language", label: "Languages", icon: Code },
  { id: "ui", label: "UI and CSS", icon: Palette },
  { id: "library", label: "Libraries", icon: Package },
  { id: "cms", label: "CMS and site builders", icon: FileText },
  { id: "analytics", label: "Analytics and tags", icon: ChartLine },
  { id: "server", label: "Server", icon: Server },
  { id: "hosting", label: "Hosting and CDN", icon: Cloud },
  { id: "build", label: "Build tools", icon: Hammer },
  { id: "other", label: "Other", icon: CircleHelp },
];
