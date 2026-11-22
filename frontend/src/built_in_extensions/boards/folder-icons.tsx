import {
  Archive,
  Book,
  Bookmark,
  Box,
  Briefcase,
  Brush,
  Camera,
  Code,
  Film,
  Flag,
  Folder,
  FolderOpen,
  Gamepad2,
  Globe,
  Heart,
  House,
  Image,
  Lightbulb,
  Music,
  Palette,
  Rocket,
  Shirt,
  ShoppingBag,
  Sparkles,
  Star,
  Tag,
  Type,
  Users,
  type LucideIcon,
} from "lucide-react";
import type { BoardFolder } from "@type/browser";
import { cn } from "@/lib/cn";
import { folderColorText } from "./folder-tree";

/** Icons a folder can use, by the name stored in `BoardFolder.icon`. */
export const FOLDER_ICONS: Record<string, LucideIcon> = {
  folder: Folder,
  star: Star,
  heart: Heart,
  image: Image,
  camera: Camera,
  palette: Palette,
  brush: Brush,
  type: Type,
  code: Code,
  film: Film,
  music: Music,
  book: Book,
  bookmark: Bookmark,
  briefcase: Briefcase,
  globe: Globe,
  lightbulb: Lightbulb,
  rocket: Rocket,
  flag: Flag,
  tag: Tag,
  sparkles: Sparkles,
  "gamepad-2": Gamepad2,
  shirt: Shirt,
  "shopping-bag": ShoppingBag,
  house: House,
  users: Users,
  archive: Archive,
  box: Box,
};

export const FOLDER_ICON_NAMES = Object.keys(FOLDER_ICONS);

type FolderGlyphProps = {
  id: string;
  folder: Pick<BoardFolder, "icon" | "color"> | undefined;
  open?: boolean;
  className?: string;
};

/** The folder's chosen icon in its color. Without a color it takes the surrounding text color. */
export function FolderGlyph({ id, folder, open, className }: FolderGlyphProps) {
  const Icon = (folder?.icon && FOLDER_ICONS[folder.icon]) || (open ? FolderOpen : Folder);
  return <Icon id={id} aria-hidden className={cn("size-4 shrink-0", className, folderColorText(folder))} />;
}
