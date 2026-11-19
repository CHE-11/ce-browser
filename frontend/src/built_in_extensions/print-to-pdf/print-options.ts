import {
  PaperSize,
  PdfOptions,
  PrintEditorSettings,
  PrintFont,
  PrintImageSize,
  PrintMargins,
  PrintMode,
  PrintPageNumberLocation,
  PrintPaper,
  PrintPreset,
} from "@type/browser";
import { FILE_NAME_TEMPLATES } from "./file-name";


/** CSS pixels per inch. The preview and WebView2's PDF layout both use it. */
export const CSS_DPI = 96;

export type PaperGroup = "paper" | "phone" | "tablet";

type PaperSpec = { label: string; width: number; height: number; group: PaperGroup };

/** A device screen, stored in inches so the preview and the PDF share one page size. */
function screen(label: string, cssWidth: number, cssHeight: number, group: Exclude<PaperGroup, "paper">): PaperSpec {
  return { label, width: cssWidth / CSS_DPI, height: cssHeight / CSS_DPI, group };
}

export const PAPER_SIZES: Record<PaperSize, PaperSpec> = {
  letter: { label: "Letter", width: 8.5, height: 11, group: "paper" },
  legal: { label: "Legal", width: 8.5, height: 14, group: "paper" },
  tabloid: { label: "Tabloid", width: 11, height: 17, group: "paper" },
  statement: { label: "Statement", width: 5.5, height: 8.5, group: "paper" },
  executive: { label: "Executive", width: 7.25, height: 10.5, group: "paper" },
  folio: { label: "Folio", width: 8.5, height: 13, group: "paper" },
  a3: { label: "A3", width: 11.69, height: 16.54, group: "paper" },
  a4: { label: "A4", width: 8.27, height: 11.69, group: "paper" },
  a5: { label: "A5", width: 5.83, height: 8.27, group: "paper" },
  a6: { label: "A6", width: 4.13, height: 5.83, group: "paper" },
  b5: { label: "B5", width: 6.93, height: 9.84, group: "paper" },
  photo4x6: { label: "Photo 4 × 6", width: 4, height: 6, group: "paper" },
  photo5x7: { label: "Photo 5 × 7", width: 5, height: 7, group: "paper" },
  photo8x10: { label: "Photo 8 × 10", width: 8, height: 10, group: "paper" },
  iphone16: screen("iPhone 16", 393, 852, "phone"),
  iphone16Plus: screen("iPhone 16 Plus", 430, 932, "phone"),
  iphone16Pro: screen("iPhone 16 Pro", 402, 874, "phone"),
  iphone16ProMax: screen("iPhone 16 Pro Max", 440, 956, "phone"),
  iphoneSe: screen("iPhone SE", 375, 667, "phone"),
  pixel9: screen("Pixel 9", 412, 892, "phone"),
  pixel9ProXl: screen("Pixel 9 Pro XL", 448, 998, "phone"),
  galaxyS24: screen("Galaxy S24", 360, 780, "phone"),
  galaxyS24Ultra: screen("Galaxy S24 Ultra", 384, 824, "phone"),
  ipadMini: screen("iPad mini", 744, 1133, "tablet"),
  ipad: screen("iPad", 820, 1180, "tablet"),
  ipadAir13: screen('iPad Air 13"', 1024, 1366, "tablet"),
  ipadPro11: screen('iPad Pro 11"', 834, 1210, "tablet"),
  ipadPro13: screen('iPad Pro 13"', 1032, 1376, "tablet"),
  galaxyTabS9: screen("Galaxy Tab S9", 800, 1280, "tablet"),
};

/** Shown when a profile has not chosen any sizes yet. */
export const DEFAULT_PAPER_CHOICES = ["letter", "tabloid", "legal"];

const MIN_PAPER_INCHES = 1;
const MAX_PAPER_INCHES = 50;

export const PAPER_GROUPS: { id: PaperGroup; label: string }[] = [
  { id: "paper", label: "Paper" },
  { id: "phone", label: "Phones" },
  { id: "tablet", label: "Tablets" },
];

export function papersInGroup(group: PaperGroup): PaperSize[] {
  return (Object.keys(PAPER_SIZES) as PaperSize[]).filter((paper) => PAPER_SIZES[paper].group === group);
}

export function isPaperSize(value: string): value is PaperSize {
  return Object.prototype.hasOwnProperty.call(PAPER_SIZES, value);
}

/** Both edges are inside the PDF page limits. */
export function customPaperReady(settings: Pick<PrintEditorSettings, "customWidth" | "customHeight">) {
  return [settings.customWidth, settings.customHeight].every(
    (inches) => inches >= MIN_PAPER_INCHES && inches <= MAX_PAPER_INCHES,
  );
}

/** Checked catalog sizes, in catalog order. */
export function chosenPapers(settings: Pick<PrintEditorSettings, "paperChoices">): PaperSize[] {
  const allowed = new Set(settings.paperChoices);
  return (Object.keys(PAPER_SIZES) as PaperSize[]).filter((paper) => allowed.has(paper));
}

/** Sizes the print editor lists. Custom is included only when both edges are valid. */
export function visiblePapers(settings: PrintEditorSettings): PrintPaper[] {
  const papers: PrintPaper[] = chosenPapers(settings);
  if (settings.paperChoices.includes("custom") && customPaperReady(settings)) {
    papers.push("custom");
  }
  return papers.length > 0 ? papers : ["letter"];
}

function paperSpec(settings: PrintEditorSettings): PaperSpec {
  if (settings.paper === "custom" && customPaperReady(settings)) {
    return { label: "Custom", width: settings.customWidth, height: settings.customHeight, group: "paper" };
  }
  if (settings.paper !== "custom" && PAPER_SIZES[settings.paper]) {
    return PAPER_SIZES[settings.paper];
  }
  return PAPER_SIZES.letter;
}

/** Select label: inches for paper, CSS pixels for a device screen. */
export function paperOptionLabel(paper: PrintPaper, settings?: Pick<PrintEditorSettings, "customWidth" | "customHeight">) {
  if (paper === "custom") {
    const width = settings?.customWidth ?? 0;
    const height = settings?.customHeight ?? 0;
    return `Custom (${width} × ${height} in)`;
  }
  const size = PAPER_SIZES[paper] ?? PAPER_SIZES.letter;
  if (size.group === "paper") {
    return `${size.label} (${size.width} × ${size.height} in)`;
  }
  return `${size.label} (${Math.round(size.width * CSS_DPI)} × ${Math.round(size.height * CSS_DPI)})`;
}

export const MARGIN_INCHES: Record<PrintMargins, number> = {
  none: 0,
  narrow: 0.25,
  normal: 0.5,
  wide: 1,
};

export const MODE_OPTIONS: { value: PrintMode; label: string }[] = [
  { value: "article", label: "Article" },
  { value: "full", label: "Full page" },
];

export const IMAGE_OPTIONS: { value: PrintImageSize; label: string }[] = [
  { value: "original", label: "Original" },
  { value: "large", label: "Large" },
  { value: "medium", label: "Medium" },
  { value: "small", label: "Small" },
  { value: "none", label: "None" },
];

export type PrintFontGroup = "page" | "serif" | "sans" | "mono";

export const FONT_GROUPS: { id: PrintFontGroup; label: string }[] = [
  { id: "page", label: "Page" },
  { id: "serif", label: "Serif" },
  { id: "sans", label: "Sans" },
  { id: "mono", label: "Mono" },
];

/** Windows faces the PDF can request. `stack` is omitted for the page's own fonts. */
export const FONT_OPTIONS: { value: PrintFont; label: string; group: PrintFontGroup; stack?: string }[] = [
  { value: "original", label: "Original", group: "page" },
  { value: "serif", label: "Serif", group: "serif", stack: "Georgia, Cambria, 'Times New Roman', serif" },
  { value: "georgia", label: "Georgia", group: "serif", stack: "Georgia, serif" },
  { value: "cambria", label: "Cambria", group: "serif", stack: "Cambria, Georgia, serif" },
  { value: "constantia", label: "Constantia", group: "serif", stack: "Constantia, Georgia, serif" },
  { value: "palatino", label: "Palatino", group: "serif", stack: "'Palatino Linotype', Palatino, 'Book Antiqua', serif" },
  { value: "times", label: "Times New Roman", group: "serif", stack: "'Times New Roman', Times, serif" },
  { value: "sans", label: "Sans", group: "sans", stack: "system-ui, 'Segoe UI', Roboto, Arial, sans-serif" },
  { value: "segoe", label: "Segoe UI", group: "sans", stack: "'Segoe UI', sans-serif" },
  { value: "calibri", label: "Calibri", group: "sans", stack: "Calibri, 'Segoe UI', sans-serif" },
  { value: "aptos", label: "Aptos", group: "sans", stack: "Aptos, Calibri, 'Segoe UI', sans-serif" },
  { value: "arial", label: "Arial", group: "sans", stack: "Arial, Helvetica, sans-serif" },
  { value: "verdana", label: "Verdana", group: "sans", stack: "Verdana, Geneva, sans-serif" },
  { value: "candara", label: "Candara", group: "sans", stack: "Candara, 'Segoe UI', sans-serif" },
  { value: "trebuchet", label: "Trebuchet MS", group: "sans", stack: "'Trebuchet MS', sans-serif" },
  { value: "consolas", label: "Consolas", group: "mono", stack: "Consolas, 'Cascadia Mono', monospace" },
  { value: "cascadia", label: "Cascadia Mono", group: "mono", stack: "'Cascadia Mono', Consolas, monospace" },
  { value: "courier", label: "Courier New", group: "mono", stack: "'Courier New', Courier, monospace" },
];

export function printFontStack(font: PrintFont) {
  return FONT_OPTIONS.find((option) => option.value === font)?.stack;
}

export const MARGIN_OPTIONS: { value: PrintMargins; label: string }[] = [
  { value: "none", label: "None" },
  { value: "narrow", label: "Narrow" },
  { value: "normal", label: "Normal" },
  { value: "wide", label: "Wide" },
];

/** Where `2/12` sits along the bottom of each page. */
export const PAGE_NUMBER_LOCATIONS: { value: PrintPageNumberLocation; label: string }[] = [
  { value: "left", label: "Left" },
  { value: "center", label: "Center" },
  { value: "right", label: "Right" },
];

export const MAX_PRINT_PRESETS = 12;

type CleanupFlag = "removeAds" | "removeNavigation" | "removePopups" | "removeSocial" | "removeMedia";

export const CLEANUP_OPTIONS: { flag: CleanupFlag; label: string; short: string }[] = [
  { flag: "removeAds", label: "Remove ads", short: "ads" },
  { flag: "removePopups", label: "Remove pop-ups", short: "pop-ups" },
  { flag: "removeNavigation", label: "Remove navigation", short: "menus" },
  { flag: "removeSocial", label: "Remove social and comments", short: "social" },
  { flag: "removeMedia", label: "Remove videos and embeds", short: "media" },
];

function articlePreset(): PrintPreset {
  return {
    id: "article",
    name: "Article",
    mode: "article",
    removeAds: true,
    removeNavigation: true,
    removePopups: true,
    removeSocial: true,
    removeMedia: false,
    textSize: 100,
    imageSize: "original",
    removeBackgrounds: false,
    font: "original",
    showHeader: true,
    paper: "letter",
    customWidth: 0,
    customHeight: 0,
    landscape: false,
    margins: "normal",
    headerFooter: false,
    showPageNumbers: false,
    pageNumberLocation: "right",
    printBackgrounds: true,
    fileNameTemplate: FILE_NAME_TEMPLATES.article,
  };
}

/** Recipes a new profile starts with. An empty saved list stays empty. */
export const DEFAULT_PRINT_PRESETS: PrintPreset[] = [
  articlePreset(),
  {
    ...articlePreset(),
    id: "clean-page",
    name: "Clean page",
    mode: "full",
    removeNavigation: false,
    removeSocial: false,
    fileNameTemplate: FILE_NAME_TEMPLATES.cleanPage,
  },
  {
    ...articlePreset(),
    id: "reader",
    name: "Reader",
    removeMedia: true,
    removeBackgrounds: true,
    font: "serif",
    printBackgrounds: false,
    fileNameTemplate: FILE_NAME_TEMPLATES.reader,
  },
];

/** One line for the print menu: what the recipe keeps and what it strips. */
export function presetHint(preset: PrintPreset) {
  const mode = preset.mode === "full" ? "Full page" : "Article";
  const removed = CLEANUP_OPTIONS.filter((option) => preset[option.flag]).map((option) => option.short);
  if (removed.length === 0) {
    return `${mode} · keeps the page`;
  }
  if (removed.length === CLEANUP_OPTIONS.length) {
    return `${mode} · clutter removed`;
  }
  return `${mode} · removes ${removed.join(", ")}`;
}

/** The editor's page setup with this recipe's cleanup and layout applied. */
export function applyPrintPreset(settings: PrintEditorSettings, presetId: string): PrintEditorSettings {
  const preset = settings.presets.find((item) => item.id === presetId);
  if (!preset) {
    return settings;
  }
  return {
    ...settings,
    mode: preset.mode,
    removeAds: preset.removeAds,
    removeNavigation: preset.removeNavigation,
    removePopups: preset.removePopups,
    removeSocial: preset.removeSocial,
    removeMedia: preset.removeMedia,
    textSize: preset.textSize,
    imageSize: preset.imageSize,
    removeBackgrounds: preset.removeBackgrounds,
    font: preset.font,
    showHeader: preset.showHeader,
    paper: preset.paper,
    ...(preset.paper === "custom"
      ? { customWidth: preset.customWidth, customHeight: preset.customHeight }
      : {}),
    landscape: preset.landscape,
    margins: preset.margins,
    headerFooter: preset.headerFooter,
    showPageNumbers: preset.showPageNumbers,
    pageNumberLocation: preset.pageNumberLocation,
    printBackgrounds: preset.printBackgrounds,
  };
}

export function newPrintPreset(existing: PrintPreset[]): PrintPreset {
  const names = new Set(existing.map((preset) => preset.name));
  let number = existing.length + 1;
  let name = `Preset ${number}`;
  while (names.has(name)) {
    number += 1;
    name = `Preset ${number}`;
  }
  return { ...articlePreset(), id: crypto.randomUUID(), name };
}

export const DEFAULT_PRINT_SETTINGS: PrintEditorSettings = {
  mode: "article",
  removeAds: true,
  removeNavigation: true,
  removePopups: true,
  removeSocial: true,
  removeMedia: false,
  textSize: 100,
  imageSize: "original",
  removeBackgrounds: false,
  font: "original",
  showHeader: true,
  paper: "letter",
  paperChoices: [...DEFAULT_PAPER_CHOICES],
  customWidth: 0,
  customHeight: 0,
  landscape: false,
  margins: "normal",
  headerFooter: false,
  showPageNumbers: false,
  pageNumberLocation: "right",
  printBackgrounds: true,
  openAfterSave: false,
  presets: DEFAULT_PRINT_PRESETS.map((preset) => ({ ...preset })),
};

/** Page size in inches as it is laid out, after orientation. */
export function pageInches(settings: PrintEditorSettings) {
  const paper = paperSpec(settings);
  return settings.landscape
    ? { width: paper.height, height: paper.width }
    : { width: paper.width, height: paper.height };
}

/** Width and height of the printable area in CSS pixels. */
export function contentBox(settings: PrintEditorSettings) {
  const page = pageInches(settings);
  const margin = MARGIN_INCHES[settings.margins] ?? MARGIN_INCHES.normal;
  return {
    width: Math.round((page.width - margin * 2) * CSS_DPI),
    height: Math.round((page.height - margin * 2) * CSS_DPI),
    margin: Math.round(margin * CSS_DPI),
  };
}

export function pdfOptions(settings: PrintEditorSettings, title: string, url: string): PdfOptions {
  const paper = paperSpec(settings);
  return {
    pageWidth: paper.width,
    pageHeight: paper.height,
    landscape: settings.landscape,
    margin: MARGIN_INCHES[settings.margins] ?? MARGIN_INCHES.normal,
    backgrounds: settings.printBackgrounds,
    headerFooter: settings.headerFooter,
    pageNumbers: settings.showPageNumbers,
    pageNumberLocation: settings.pageNumberLocation,
    textSize: settings.textSize,
    scale: 1,
    title,
    url,
  };
}
