import { fileNameFor } from "../shared";

/** Keep in sync with `suggested_file_name` in `src-tauri/src/settings.rs`. */
export const FILE_NAME_TEMPLATES = {
  article: "{date} - {author} - {article}",
  cleanPage: "{site} - {article}",
  reader: "{author} - {article}",
  fallback: "{date} - {author} - {article}",
} as const;

export const FILE_NAME_TEMPLATE_MAX = 160;

export type FileNameGroup = "page" | "address" | "date" | "file";

export const FILE_NAME_GROUPS: { id: FileNameGroup; label: string }[] = [
  { id: "page", label: "Page" },
  { id: "address", label: "Address" },
  { id: "date", label: "Date" },
  { id: "file", label: "File" },
];

export const FILE_NAME_TOKENS: { token: string; label: string; hint: string; group: FileNameGroup }[] = [
  { token: "title", label: "Title", hint: "The page title, as the tab shows it", group: "page" },
  {
    token: "article",
    label: "Article",
    hint: "The article name: the title with the site name taken off. {article_name} works too",
    group: "page",
  },
  { token: "author", label: "Author", hint: "The byline, when the page names one. {byline} works too", group: "page" },
  { token: "site", label: "Site", hint: "The site name, or the domain when the page does not say. {site_name} works too", group: "page" },
  { token: "host", label: "Host", hint: "The domain, without www. {domain} works too", group: "address" },
  { token: "section", label: "Section", hint: "The first folder in the address, such as news or blog", group: "address" },
  { token: "slug", label: "Slug", hint: "The article name taken from the end of the address", group: "address" },
  {
    token: "date",
    label: "Published",
    hint: "The published date, as YYYY-MM-DD. Skipped when the page does not name one. {published} works too",
    group: "date",
  },
  { token: "year", label: "Year", hint: "The year of the published date", group: "date" },
  { token: "month", label: "Month", hint: "The month of the published date", group: "date" },
  { token: "day", label: "Day", hint: "The day of the published date", group: "date" },
  { token: "saved", label: "Saved", hint: "The day you save the PDF, as YYYY-MM-DD", group: "date" },
  { token: "preset", label: "Preset", hint: "This preset's name", group: "file" },
  { token: "lang", label: "Language", hint: "The page language, such as en", group: "file" },
];

const ALIASES: Record<string, string> = {
  article_name: "article",
  articlename: "article",
  headline: "article",
  author_name: "author",
  byline: "author",
  site_name: "site",
  sitename: "site",
  publication: "site",
  domain: "host",
  hostname: "host",
  published: "date",
  published_date: "date",
  pubdate: "date",
  page_title: "title",
  pagetitle: "title",
  language: "lang",
  preset_name: "preset",
  path: "slug",
  article_slug: "slug",
};

/** What a preset can put in a file name. Empty strings are left out of the name. */
export type FileNameSource = {
  title: string;
  url: string;
  author: string;
  site: string;
  published: string;
  lang: string;
  preset: string;
  /** The day the PDF is saved. Defaults to now, and only fills `{saved}`. */
  saved?: Date;
};

/** A sample article, so the settings field can show the name a template would produce. */
export const FILE_NAME_PREVIEW: FileNameSource = {
  title: "Why the sky is blue | Example Times",
  url: "https://www.example.com/science/why-the-sky-is-blue",
  author: "Ada Lovelace",
  site: "Example Times",
  published: "2026-03-14T15:00:00Z",
  lang: "en",
  preset: "Article",
};

export function suggestedFileNameTemplate(id: string) {
  if (id === "clean-page") {
    return FILE_NAME_TEMPLATES.cleanPage;
  }
  if (id === "reader") {
    return FILE_NAME_TEMPLATES.reader;
  }
  if (id === "article") {
    return FILE_NAME_TEMPLATES.article;
  }
  return FILE_NAME_TEMPLATES.fallback;
}

/** The saved template, or the suggested one when an older preset has not stored one yet. A blank template stays blank. */
export function storedFileNameTemplate(preset: { id: string; fileNameTemplate?: string | null }) {
  if (typeof preset.fileNameTemplate === "string") {
    return preset.fileNameTemplate;
  }
  return suggestedFileNameTemplate(preset.id);
}

export function previewFileName(template: string, presetName: string) {
  return fileNameFromTemplate(template, { ...FILE_NAME_PREVIEW, preset: presetName.trim() || FILE_NAME_PREVIEW.preset });
}

/** The PDF name for a preset template. A blank template, or one that fills in to nothing, uses the page title. */
export function fileNameFromTemplate(template: string, source: FileNameSource, extension = "pdf") {
  const fallback = () => fileNameFor(source.title, source.url, extension);
  const trimmed = template.trim();
  if (!trimmed) {
    return fallback();
  }
  const saved = source.saved ?? new Date();
  const published = publishedParts(source.published);
  const site = source.site.trim() || hostOf(source.url);
  const title = source.title.trim();
  const parts = pathParts(source.url);
  const values: Record<string, string> = {
    title,
    article: articleName(title, site),
    author: source.author.trim(),
    site,
    host: hostOf(source.url),
    section: parts.length > 1 ? words(parts[0]) : "",
    slug: words(parts[parts.length - 1] ?? ""),
    date: published.date,
    year: published.year,
    month: published.month,
    day: published.day,
    saved: stamp(saved),
    preset: source.preset.trim(),
    lang: source.lang.trim(),
  };
  const base = sanitizeBase(tidy(joinPieces(piecesOf(trimmed, values))));
  if (!base) {
    return fallback();
  }
  return `${base}.${extension.replace(/^\./, "")}`;
}

function publishedParts(published: string) {
  const match = published.match(/(\d{4})-(\d{2})-(\d{2})/);
  if (!match) {
    return { date: "", year: "", month: "", day: "" };
  }
  return { date: `${match[1]}-${match[2]}-${match[3]}`, year: match[1], month: match[2], day: match[3] };
}

function stamp(date: Date) {
  const year = String(date.getFullYear());
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function hostOf(url: string) {
  try {
    return new URL(url).hostname.replace(/^www\./i, "");
  } catch {
    return "";
  }
}

function pathParts(url: string) {
  try {
    return new URL(url).pathname
      .split("/")
      .filter(Boolean)
      .map((part) => {
        try {
          return decodeURIComponent(part);
        } catch {
          return part;
        }
      });
  } catch {
    return [];
  }
}

function words(segment: string) {
  return segment
    .replace(/\.(html?|php|aspx?)$/i, "")
    .replace(/[-_]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

/** Title with a leading or trailing site name removed, so the file can use the article on its own. */
function articleName(title: string, site: string) {
  const name = title.trim();
  const siteName = site.trim();
  if (!name || !siteName) {
    return name;
  }
  const escaped = siteName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const separators = "[|–—·•:\\-]";
  const stripped = name
    .replace(new RegExp(`\\s*${separators}\\s*${escaped}\\s*$`, "i"), "")
    .replace(new RegExp(`^${escaped}\\s*${separators}\\s*`, "i"), "")
    .trim();
  return stripped || name;
}

type Piece = { text: string; token: boolean };

function piecesOf(template: string, values: Record<string, string>): Piece[] {
  const pieces: Piece[] = [];
  const pattern = /\{\s*([a-z0-9_]+)\s*\}/gi;
  let last = 0;
  for (const match of template.matchAll(pattern)) {
    const index = match.index ?? 0;
    if (index > last) {
      pieces.push({ text: template.slice(last, index), token: false });
    }
    const name = match[1].toLowerCase();
    const key = ALIASES[name] ?? name;
    pieces.push({ text: values[key] ?? "", token: true });
    last = index + match[0].length;
  }
  if (last < template.length) {
    pieces.push({ text: template.slice(last), token: false });
  }
  return pieces;
}

function isSeparator(text: string) {
  return text.length > 0 && /^[\s\-–—_|·•:,;/\\]+$/.test(text);
}

/** Drops placeholders the page left empty, and the dash or slash that only separated them. */
function joinPieces(pieces: Piece[]) {
  const drop = new Set<number>();
  for (let index = 0; index < pieces.length; index += 1) {
    const piece = pieces[index];
    if (!piece.token || piece.text.trim()) {
      continue;
    }
    drop.add(index);
    const next = pieces[index + 1];
    const previous = pieces[index - 1];
    if (next && !next.token && isSeparator(next.text)) {
      drop.add(index + 1);
    } else if (previous && !previous.token && isSeparator(previous.text) && !drop.has(index - 1)) {
      drop.add(index - 1);
    }
  }
  return pieces
    .filter((_, index) => !drop.has(index))
    .map((piece) => piece.text)
    .join("");
}

function tidy(value: string) {
  let text = value.replace(/\(\s*\)|\[\s*\]|\{\s*\}/g, " ");
  text = text.replace(/(?:\s*[-–—_|·•:,;/\\]+\s*){2,}/g, (match) => {
    const mark = match.match(/[-–—_|·•:,;/\\]/);
    return mark ? ` ${mark[0]} ` : " ";
  });
  text = text.replace(/\s+/g, " ").trim();
  text = text.replace(/^[\s\-–—_|·•:,;/\\]+|[\s\-–—_|·•:,;/\\]+$/g, "").trim();
  return text;
}

function sanitizeBase(value: string) {
  const cleaned = tidy(
    value
      .replace(/[\\/]+/g, "-")
      .replace(/[:|]/g, " - ")
      .replace(/[<>"?*\u0000-\u001f]/g, " "),
  )
    .replace(/\.pdf$/i, "")
    .replace(/[\s.]+$/g, "")
    .slice(0, 120)
    .replace(/[\s.\-–—_|·•:,;]+$/g, "")
    .trim();
  return cleaned;
}
