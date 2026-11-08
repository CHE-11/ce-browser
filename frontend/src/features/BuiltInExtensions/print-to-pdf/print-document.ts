import { ExtractedPage, PrintEditorSettings, PrintMode } from "@type/browser";
import { type CleanupKind, type CleanupStats, extractArticle, removeClutter } from "./cleanup";
import { printFontStack } from "./print-options";

/** Every element of the source page gets a stable id, so edits survive a rebuild. */
export const ID_ATTR = "data-ce-id";
export const FORMAT_STYLE_ID = "ce-print-format";
export const EDITOR_STYLE_ID = "ce-print-editor";
export const GUIDES_ID = "ce-page-guides";

export type EditOp = { type: "delete"; ids: string[] } | { type: "text"; id: string; html: string };

/** Everything that decides the document's content. Formatting and paper live elsewhere. */
export type ContentState = {
  mode: PrintMode;
  removeAds: boolean;
  removeNavigation: boolean;
  removePopups: boolean;
  removeSocial: boolean;
  removeMedia: boolean;
  ops: EditOp[];
};

export type BuiltDocument = {
  html: string;
  stats: CleanupStats;
  /** False when article mode found no article and fell back to the full page. */
  articleFound: boolean;
};

export function contentStateFrom(settings: PrintEditorSettings): ContentState {
  return {
    mode: settings.mode,
    removeAds: settings.removeAds,
    removeNavigation: settings.removeNavigation,
    removePopups: settings.removePopups,
    removeSocial: settings.removeSocial,
    removeMedia: settings.removeMedia,
    ops: [],
  };
}

/** No page script may run: the preview shares the browser UI's origin. */
const CSP =
  "default-src * data: blob: 'unsafe-inline'; script-src 'none'; object-src 'none'; base-uri *; form-action 'none'";

/** Parses the extracted page once and tags every element with an id. */
export function parsePage(page: ExtractedPage): Document {
  const doc = new DOMParser().parseFromString(page.html, "text/html");
  doc.querySelectorAll("script, noscript").forEach((node) => node.remove());
  let next = 0;
  doc.body?.querySelectorAll("*").forEach((element) => {
    element.setAttribute(ID_ATTR, `e${(next++).toString(36)}`);
  });
  return doc;
}

const READER_CSS = `
html { background: #fff; }
body {
  margin: 0 auto;
  max-width: 46rem;
  padding: 0;
  color: #1a1a1a;
  font-family: system-ui, -apple-system, "Segoe UI", Roboto, Arial, sans-serif;
  font-size: calc(17px * var(--ce-text-scale, 1));
  line-height: 1.62;
  word-wrap: break-word;
  overflow-wrap: break-word;
}
article[data-ce-root] > * { max-width: 100%; }
h1, h2, h3, h4, h5, h6 { line-height: 1.25; margin: 1.4em 0 0.5em; font-weight: 700; color: #111; break-after: avoid; }
h1 { font-size: 1.9em; } h2 { font-size: 1.45em; } h3 { font-size: 1.2em; } h4 { font-size: 1.05em; }
p, ul, ol, dl, blockquote, pre, table, figure { margin: 0 0 1em; }
a { color: #0b57d0; text-decoration: underline; }
img, svg, video, picture, canvas { max-width: 100%; height: auto; }
figure { margin: 1.2em 0; break-inside: avoid; }
figcaption { font-size: 0.85em; color: #555; margin-top: 0.4em; }
blockquote { border-left: 3px solid #ccc; padding-left: 1em; color: #444; margin-left: 0; }
pre, code { font-family: Consolas, "Cascadia Mono", "Courier New", monospace; font-size: 0.88em; }
pre { background: #f5f5f5; padding: 0.8em 1em; border-radius: 4px; white-space: pre-wrap; }
table { border-collapse: collapse; width: 100%; font-size: 0.92em; }
th, td { border: 1px solid #ddd; padding: 0.35em 0.6em; text-align: left; vertical-align: top; }
hr { border: 0; border-top: 1px solid #ddd; margin: 2em 0; }
`;

const HEADER_CSS = `
[data-ce-header] { margin: 0 0 1.6em; padding: 0 0 1em; border-bottom: 1px solid #ddd; font-family: system-ui, "Segoe UI", Arial, sans-serif; color: #222; break-inside: avoid; }
[data-ce-header] [data-ce-site] { font-size: 12px; text-transform: uppercase; letter-spacing: 0.06em; color: #666; margin: 0 0 6px; }
[data-ce-header] h1 { font-size: 26px; line-height: 1.25; margin: 0 0 8px; color: #111; }
[data-ce-header] [data-ce-meta] { font-size: 13px; color: #555; margin: 0 0 2px; }
[data-ce-header] [data-ce-url] { display: block; font-size: 11px; color: #0b57d0; word-break: break-all; margin: 0; text-decoration: underline; }
`;

const PRINT_CSS = `
img, figure, svg, picture, table, pre, blockquote { break-inside: avoid; }
h1, h2, h3, h4 { break-after: avoid; }
@media print { body { -webkit-print-color-adjust: exact; print-color-adjust: exact; } }
`;

function zeroStats(): CleanupStats {
  return { ads: 0, navigation: 0, popups: 0, social: 0, media: 0 };
}

function cleanupKinds(state: ContentState): CleanupKind[] {
  const kinds: CleanupKind[] = [];
  if (state.removeAds) kinds.push("ads");
  if (state.removePopups) kinds.push("popups");
  if (state.removeSocial) kinds.push("social");
  if (state.removeNavigation) kinds.push("navigation");
  if (state.removeMedia) kinds.push("media");
  return kinds;
}

function formatDate(value: string) {
  if (!value) {
    return "";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" });
}

function buildHeader(doc: Document, page: ExtractedPage) {
  const header = doc.createElement("header");
  header.setAttribute("data-ce-header", "");
  header.setAttribute("data-ce-keep", "");
  header.setAttribute(ID_ATTR, "ce-header");
  const add = (tag: string, marker: string, id: string, text: string) => {
    if (!text) {
      return;
    }
    const element = doc.createElement(tag);
    if (marker) {
      element.setAttribute(marker, "");
    }
    element.setAttribute(ID_ATTR, id);
    element.textContent = text;
    header.appendChild(element);
  };
  add("p", "data-ce-site", "ce-header-site", page.siteName);
  add("h1", "", "ce-header-title", page.title);
  const meta = [page.byline, formatDate(page.published)].filter(Boolean).join(" · ");
  add("p", "data-ce-meta", "ce-header-meta", meta);
  if (page.url && linkable(page.url)) {
    const link = doc.createElement("a");
    link.setAttribute("data-ce-url", "");
    link.setAttribute(ID_ATTR, "ce-header-url");
    link.setAttribute("href", page.url);
    link.textContent = page.url;
    header.appendChild(link);
  } else {
    add("p", "data-ce-url", "ce-header-url", page.url);
  }
  return header;
}

/** Addresses the PDF viewer can open. Anything else stays plain text. */
function linkable(url: string) {
  try {
    const protocol = new URL(url).protocol;
    return protocol === "http:" || protocol === "https:" || protocol === "file:";
  } catch {
    return false;
  }
}

function ensureHead(doc: Document, page: ExtractedPage) {
  const head = doc.head ?? doc.documentElement.insertBefore(doc.createElement("head"), doc.body);
  head.querySelectorAll("meta[http-equiv], base, meta[charset]").forEach((node) => node.remove());
  const charset = doc.createElement("meta");
  charset.setAttribute("charset", "utf-8");
  const csp = doc.createElement("meta");
  csp.setAttribute("http-equiv", "Content-Security-Policy");
  csp.setAttribute("content", CSP);
  const base = doc.createElement("base");
  base.setAttribute("href", page.url);
  head.prepend(charset, csp, base);
  if (!head.querySelector("title")) {
    const title = doc.createElement("title");
    title.textContent = page.title;
    head.appendChild(title);
  }
  return head;
}

function addStyle(doc: Document, id: string, css: string) {
  const style = doc.createElement("style");
  style.id = id;
  style.textContent = css;
  (doc.head ?? doc.documentElement).appendChild(style);
}

function sameText(a: string, b: string) {
  const clean = (value: string) => value.replace(/\s+/g, " ").trim().toLowerCase();
  return clean(a) === clean(b);
}

function applyOps(doc: Document, ops: EditOp[]) {
  for (const op of ops) {
    if (op.type === "delete") {
      for (const id of op.ids) {
        doc.querySelector(`[${ID_ATTR}="${CSS.escape(id)}"]`)?.remove();
      }
    } else {
      const element = doc.querySelector(`[${ID_ATTR}="${CSS.escape(op.id)}"]`);
      if (element) {
        element.innerHTML = op.html;
      }
    }
  }
}

/** Formatting that can change without rebuilding the document. */
export function formatCss(settings: PrintEditorSettings, mode: PrintMode) {
  const scale = Math.min(150, Math.max(70, settings.textSize)) / 100;
  const rules: string[] = [`:root { --ce-text-scale: ${scale}; }`];
  if (mode === "full" && scale !== 1) {
    rules.push(`body { zoom: ${scale}; }`);
  }
  switch (settings.imageSize) {
    case "large":
      rules.push("img, picture, svg:not([width='1em']), video, canvas { max-width: 85% !important; height: auto !important; }");
      break;
    case "medium":
      rules.push("img, picture, video, canvas { max-width: 55% !important; height: auto !important; }");
      break;
    case "small":
      rules.push("img, picture, video, canvas { max-width: 30% !important; height: auto !important; }");
      break;
    case "none":
      rules.push("img, picture, figure, video, canvas, svg:not([data-ce-icon]) { display: none !important; }");
      break;
    default:
      break;
  }
  if (settings.removeBackgrounds) {
    rules.push(
      "*, *::before, *::after { background-color: transparent !important; background-image: none !important; box-shadow: none !important; }",
      "html, body { background: #fff !important; }",
      "body, body * { color: #111 !important; }",
      "a, a * { color: #0b57d0 !important; }",
    );
  }
  const stack = printFontStack(settings.font);
  if (stack) {
    rules.push(
      `body, body *:not(pre):not(code):not(kbd):not(samp) { font-family: ${stack} !important; }`,
    );
  }
  return rules.join("\n");
}

/** Builds the printable document from the parsed page, the content choices and the edits. */
export function buildDocument(
  source: Document,
  page: ExtractedPage,
  state: ContentState,
  format: PrintEditorSettings,
): BuiltDocument {
  const stats = zeroStats();
  const kinds = cleanupKinds(state);
  let doc = source.cloneNode(true) as Document;
  let articleFound = false;

  if (state.mode === "article") {
    // Clutter goes first so it cannot win the article scoring.
    for (const kind of kinds) {
      stats[kind] += removeClutter(doc, kind);
    }
    const article = extractArticle(doc);
    if (article) {
      articleFound = true;
      const reader = document.implementation.createHTMLDocument(page.title);
      if (page.lang) {
        reader.documentElement.lang = page.lang;
      }
      const root = reader.createElement("article");
      root.setAttribute("data-ce-root", "");
      root.setAttribute(ID_ATTR, "ce-root");
      root.append(...Array.from(article.childNodes).map((node) => reader.importNode(node, true)));
      reader.body.appendChild(root);
      for (const kind of kinds) {
        stats[kind] += removeClutter(reader, kind, root);
      }
      const firstHeading = root.querySelector("h1");
      if (format.showHeader && firstHeading && sameText(firstHeading.textContent ?? "", page.title)) {
        firstHeading.remove();
      }
      doc = reader;
      ensureHead(doc, page);
      addStyle(doc, "ce-print-reader", READER_CSS);
    }
  }

  if (!articleFound) {
    for (const kind of kinds) {
      stats[kind] += removeClutter(doc, kind);
    }
    ensureHead(doc, page);
  }

  if (format.showHeader && doc.body) {
    doc.body.prepend(buildHeader(doc, page));
  }
  applyOps(doc, state.ops);
  addStyle(doc, "ce-print-header", HEADER_CSS);
  addStyle(doc, "ce-print-base", PRINT_CSS);
  addStyle(doc, FORMAT_STYLE_ID, formatCss(format, articleFound ? "article" : "full"));
  return {
    html: `<!DOCTYPE html>\n${doc.documentElement.outerHTML}`,
    stats,
    articleFound,
  };
}

/**
 * Fixes that need layout, so they run once the document is rendered: fixed overlays are
 * removed and sticky bars stop following the reader down every printed page.
 */
export function finalizeRendered(doc: Document, state: ContentState) {
  const view = doc.defaultView;
  if (!view || !doc.body) {
    return 0;
  }
  let removed = 0;
  const viewportArea = Math.max(1, view.innerWidth * view.innerHeight);
  doc.body.querySelectorAll<HTMLElement>("*").forEach((element) => {
    if (!element.isConnected || element.closest("[data-ce-keep]")) {
      return;
    }
    const style = view.getComputedStyle(element);
    if (style.position === "fixed") {
      const rect = element.getBoundingClientRect();
      const covers = (rect.width * rect.height) / viewportArea;
      if (state.removePopups || covers > 0.3) {
        element.remove();
        removed += 1;
        return;
      }
      element.style.setProperty("position", "static", "important");
    } else if (style.position === "sticky") {
      element.style.setProperty("position", "static", "important");
    }
  });
  if (state.removePopups) {
    for (const node of [doc.documentElement, doc.body]) {
      node.style.setProperty("overflow", "visible", "important");
      node.style.setProperty("height", "auto", "important");
      node.style.removeProperty("position");
    }
    doc.body.classList.remove("modal-open", "no-scroll", "noscroll", "overflow-hidden", "is-locked");
  }
  doc.querySelectorAll<HTMLElement>("[style*='blur(']").forEach((element) => {
    element.style.setProperty("filter", "none", "important");
  });
  return removed;
}

/** The rendered document as standalone HTML, without the editor's own highlight styles. */
export function serializeForPrint(doc: Document) {
  const clone = doc.documentElement.cloneNode(true) as HTMLElement;
  clone.querySelector(`#${EDITOR_STYLE_ID}`)?.remove();
  clone.querySelector(`#${GUIDES_ID}`)?.remove();
  clone.classList.remove("ce-editing-text");
  clone.querySelectorAll("[contenteditable]").forEach((element) => element.removeAttribute("contenteditable"));
  clone.querySelectorAll(".ce-hover, .ce-selected").forEach((element) => {
    element.classList.remove("ce-hover", "ce-selected");
    if (!element.getAttribute("class")) {
      element.removeAttribute("class");
    }
  });
  return `<!DOCTYPE html>\n${clone.outerHTML}`;
}

/**
 * Renders a document in a hidden frame of this webview, applies the layout fixes and returns
 * the final HTML. Used by the quick PDF, which has no editor to render into.
 */
export function renderOffscreen(html: string, state: ContentState, width: number): Promise<string> {
  return new Promise((resolve) => {
    const frame = document.createElement("iframe");
    frame.id = "ce-print-offscreen";
    frame.setAttribute("aria-hidden", "true");
    frame.style.cssText = `position:fixed;left:-20000px;top:0;width:${width}px;height:1200px;border:0;visibility:hidden;`;
    let done = false;
    const finish = () => {
      if (done) {
        return;
      }
      done = true;
      const doc = frame.contentDocument;
      let result = html;
      if (doc?.documentElement) {
        finalizeRendered(doc, state);
        result = serializeForPrint(doc);
      }
      frame.remove();
      resolve(result);
    };
    frame.addEventListener("load", () => window.setTimeout(finish, 400), { once: true });
    window.setTimeout(finish, 15000);
    frame.srcdoc = html;
    document.body.appendChild(frame);
  });
}
