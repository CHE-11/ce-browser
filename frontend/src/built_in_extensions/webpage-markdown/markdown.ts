import type { MarkdownScope, PageMarkdownSettings } from "@type/browser";

const SKIP = new Set([
  "script",
  "style",
  "noscript",
  "template",
  "svg",
  "canvas",
  "iframe",
  "object",
  "embed",
  "video",
  "audio",
  "button",
  "input",
  "select",
  "textarea",
]);

const ARTICLE_SKIP = new Set(["nav", "footer", "aside"]);

/** Turns extracted page HTML into Markdown using the popover's choices. */
export function htmlToMarkdown(html: string, title: string, url: string, options: PageMarkdownSettings) {
  const document = new DOMParser().parseFromString(html, "text/html");
  const root = contentRoot(document, options.scope);
  let body = root ? tidy(blocks(root, options)) : "";
  const heading = title.trim().replace(/\s+/g, " ");
  const firstLine = body.split("\n", 1)[0] ?? "";
  if (options.header && heading && firstLine === `# ${heading}`) {
    body = body.slice(firstLine.length).trim();
  }
  const parts: string[] = [];
  if (options.header) {
    if (heading) {
      parts.push(`# ${heading}`);
    }
    if (url.trim()) {
      parts.push(`Source: ${url.trim()}`);
    }
  }
  if (body) {
    parts.push(body);
  }
  return parts.join("\n\n").trim();
}

function contentRoot(document: Document, scope: MarkdownScope) {
  if (scope === "page") {
    return document.body;
  }
  return document.querySelector("article") ?? document.querySelector("main") ?? document.querySelector("[role='main']") ?? document.body;
}

function blocks(node: Node, options: PageMarkdownSettings): string {
  if (node.nodeType === Node.TEXT_NODE) {
    const text = collapse(node.textContent ?? "");
    return text ? `\n\n${text}\n\n` : "";
  }
  if (!(node instanceof Element) || skipped(node, options)) {
    return "";
  }
  const tag = node.tagName.toLowerCase();
  if (/^h[1-6]$/.test(tag)) {
    const text = collapse(inline(node, options));
    return text ? `\n\n${"#".repeat(Number(tag[1]))} ${text}\n\n` : "";
  }
  if (tag === "p" || tag === "figcaption" || tag === "address") {
    const text = collapse(inline(node, options));
    return text ? `\n\n${text}\n\n` : "";
  }
  if (tag === "pre") {
    return codeBlock(node);
  }
  if (tag === "blockquote") {
    const inner = tidy(children(node, options));
    if (!inner) {
      return "";
    }
    return `\n\n${inner
      .split("\n")
      .map((line) => (line ? `> ${line}` : ">"))
      .join("\n")}\n\n`;
  }
  if (tag === "ul" || tag === "ol") {
    return list(node, options, 0);
  }
  if (tag === "table") {
    return table(node, options);
  }
  if (tag === "hr") {
    return "\n\n---\n\n";
  }
  if (tag === "img") {
    const markdown = image(node, options);
    return markdown ? `\n\n${markdown}\n\n` : "";
  }
  return children(node, options);
}

function children(node: Element, options: PageMarkdownSettings) {
  return Array.from(node.childNodes)
    .map((child) => blocks(child, options))
    .join("");
}

function list(element: Element, options: PageMarkdownSettings, depth: number): string {
  const ordered = element.tagName.toLowerCase() === "ol";
  const items = Array.from(element.children).filter((child) => child.tagName === "LI");
  if (!items.length) {
    return "";
  }
  const lines: string[] = [];
  items.forEach((item, index) => {
    const nested = Array.from(item.children).filter((child) => {
      const tag = child.tagName.toLowerCase();
      return tag === "ul" || tag === "ol";
    });
    const marker = ordered ? `${index + 1}. ` : "- ";
    const text = collapse(inline(item, options, new Set(nested)));
    lines.push(`${"  ".repeat(depth)}${marker}${text}`);
    for (const child of nested) {
      const nestedList = list(child, options, depth + 1);
      if (nestedList) {
        lines.push(nestedList);
      }
    }
  });
  const markdown = lines.join("\n");
  return depth === 0 ? `\n\n${markdown}\n\n` : markdown;
}

function table(element: Element, options: PageMarkdownSettings) {
  const grid = tableRows(element)
    .map((row) =>
      Array.from(row.children)
        .filter((cell) => cell.tagName === "TH" || cell.tagName === "TD")
        .map((cell) => collapse(inline(cell, options)).replace(/\|/g, "\\|")),
    )
    .filter((row) => row.some((cell) => cell.length > 0));
  if (!grid.length) {
    return "";
  }
  const width = Math.max(...grid.map((row) => row.length));
  const pad = (row: string[]) => {
    const next = row.slice();
    while (next.length < width) {
      next.push("");
    }
    return next;
  };
  const line = (row: string[]) => `| ${pad(row).join(" | ")} |`;
  const header = pad(grid[0] ?? []);
  const body = grid.slice(1).map((row) => line(row));
  return `\n\n${line(header)}\n| ${header.map(() => "---").join(" | ")} |\n${body.join("\n")}\n\n`;
}

function tableRows(tableElement: Element) {
  const rows: Element[] = [];
  for (const child of tableElement.children) {
    const tag = child.tagName.toLowerCase();
    if (tag === "thead" || tag === "tbody" || tag === "tfoot") {
      rows.push(...Array.from(child.children).filter((row) => row.tagName === "TR"));
    } else if (tag === "tr") {
      rows.push(child);
    }
  }
  return rows;
}

function codeBlock(element: Element) {
  const code = element.querySelector("code");
  const body = ((code ?? element).textContent ?? "").replace(/^\n/, "").replace(/\n$/, "");
  if (!body.trim()) {
    return "";
  }
  const match = `${code?.getAttribute("class") ?? ""} ${element.getAttribute("class") ?? ""}`.match(/language-([\w-]+)/);
  const lang = match?.[1] ?? "";
  let fence = "```";
  while (body.includes(fence)) {
    fence += "`";
  }
  return `\n\n${fence}${lang}\n${body}\n${fence}\n\n`;
}

function inline(node: Node, options: PageMarkdownSettings, skip: Set<Node> = new Set()): string {
  if (skip.has(node)) {
    return "";
  }
  if (node.nodeType === Node.TEXT_NODE) {
    return (node.textContent ?? "").replace(/\s+/g, " ");
  }
  if (!(node instanceof Element) || skipped(node, options)) {
    return "";
  }
  const tag = node.tagName.toLowerCase();
  if (tag === "br") {
    return " ";
  }
  if (tag === "img") {
    const markdown = image(node, options);
    return markdown ? ` ${markdown} ` : "";
  }
  if (tag === "a") {
    return link(node, options, skip);
  }
  const inner = Array.from(node.childNodes)
    .map((child) => inline(child, options, skip))
    .join("");
  if (tag === "strong" || tag === "b") {
    return wrap(inner, "**");
  }
  if (tag === "em" || tag === "i") {
    return wrap(inner, "*");
  }
  if (tag === "s" || tag === "del" || tag === "strike") {
    return wrap(inner, "~~");
  }
  if (tag === "code") {
    const text = collapse(inner).replace(/`/g, "'");
    return text ? `\`${text}\`` : "";
  }
  return inner;
}

function link(element: Element, options: PageMarkdownSettings, skip: Set<Node>) {
  const text = collapse(
    Array.from(element.childNodes)
      .map((child) => inline(child, options, skip))
      .join(""),
  );
  const href = (element.getAttribute("href") ?? "").trim();
  if (!options.links || !href || href.startsWith("#") || /^javascript:/i.test(href)) {
    return text;
  }
  if (!text) {
    return "";
  }
  return `[${text.replace(/[[\]]/g, "")}](${destination(href)})`;
}

function image(element: Element, options: PageMarkdownSettings) {
  if (!options.images) {
    return "";
  }
  const src = (element.getAttribute("src") ?? "").trim();
  if (!src || /^(data|blob|javascript):/i.test(src)) {
    return "";
  }
  const alt = collapse(element.getAttribute("alt") ?? "").replace(/[[\]]/g, "") || "image";
  return `![${alt}](${destination(src)})`;
}

function skipped(element: Element, options: PageMarkdownSettings) {
  const tag = element.tagName.toLowerCase();
  if (SKIP.has(tag)) {
    return true;
  }
  return options.scope === "article" && ARTICLE_SKIP.has(tag);
}

function wrap(text: string, marker: string) {
  const trimmed = collapse(text);
  return trimmed ? `${marker}${trimmed}${marker}` : "";
}

function collapse(text: string) {
  return text.replace(/\s+/g, " ").trim();
}

function destination(value: string) {
  if (/[\s<>()]/.test(value)) {
    return `<${value.replace(/[<>]/g, "")}>`;
  }
  return value;
}

function tidy(value: string) {
  return value
    .replace(/[ \t]+\n/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}
