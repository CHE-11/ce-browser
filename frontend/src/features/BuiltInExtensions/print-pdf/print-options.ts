import { PaperSize, PdfOptions, PrintEditorSettings, PrintFont, PrintImageSize, PrintMargins, PrintMode } from "@type/browser";


/** CSS pixels per inch. The preview and WebView2's PDF layout both use it. */
export const CSS_DPI = 96;

export const PAPER_SIZES: Record<PaperSize, { label: string; width: number; height: number }> = {
  letter: { label: "Letter", width: 8.5, height: 11 },
  a4: { label: "A4", width: 8.27, height: 11.69 },
  legal: { label: "Legal", width: 8.5, height: 14 },
  tabloid: { label: "Tabloid", width: 11, height: 17 },
};

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

export const FONT_OPTIONS: { value: PrintFont; label: string }[] = [
  { value: "original", label: "Original" },
  { value: "serif", label: "Serif" },
  { value: "sans", label: "Sans" },
];

export const MARGIN_OPTIONS: { value: PrintMargins; label: string }[] = [
  { value: "none", label: "None" },
  { value: "narrow", label: "Narrow" },
  { value: "normal", label: "Normal" },
  { value: "wide", label: "Wide" },
];

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
  landscape: false,
  margins: "normal",
  headerFooter: false,
  printBackgrounds: true,
  openAfterSave: true,
};

/** Page size in inches as it is laid out, after orientation. */
export function pageInches(settings: PrintEditorSettings) {
  const paper = PAPER_SIZES[settings.paper] ?? PAPER_SIZES.letter;
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
  const paper = PAPER_SIZES[settings.paper] ?? PAPER_SIZES.letter;
  return {
    pageWidth: paper.width,
    pageHeight: paper.height,
    landscape: settings.landscape,
    margin: MARGIN_INCHES[settings.margins] ?? MARGIN_INCHES.normal,
    backgrounds: settings.printBackgrounds,
    headerFooter: settings.headerFooter,
    scale: 1,
    title,
    url,
  };
}
