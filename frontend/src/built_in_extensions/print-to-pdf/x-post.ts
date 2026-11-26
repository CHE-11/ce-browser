/**
 * X (Twitter) posts are an app shell, not an article. The text lives in one
 * `whitespace-pre-wrap` div whose line breaks and link labels only make sense
 * with X's CSS, and a status also carries a video still and an article card.
 * This rebuilds that post into plain blocks the print stylesheet can format.
 */

import { ExtractedPage } from "@type/browser";
import { CleanupStats } from "./cleanup";

/** Same attribute the rest of the print document uses for stable edits. */
const ID_ATTR = "data-ce-id";

const X_HOSTS = new Set(["x.com", "twitter.com", "mobile.twitter.com"]);

export const X_PAGE_CSS = `
.ce-x-post + .ce-x-post { margin-top: 1.3em; padding-top: 1.1em; border-top: 1px solid #ddd; }
.ce-x-byline { display: flex; align-items: center; gap: 0.7em; margin: 0 0 0.75em; }
.ce-x-avatar { width: 40px !important; height: 40px !important; max-width: 40px !important; max-height: 40px !important; border-radius: 999px; object-fit: cover; }
.ce-x-byline strong { display: block; font-size: 1em; line-height: 1.25; }
.ce-x-byline span { display: block; color: #666; font-size: 0.85em; line-height: 1.3; }
.ce-x-body p { margin: 0 0 0.4em; }
.ce-x-body p.ce-x-url-line { margin: 0 0 0.95em; }
.ce-x-body a { overflow-wrap: anywhere; word-break: break-word; }
.ce-x-media { margin: 0.7em 0 0.9em; break-inside: avoid; }
.ce-x-media img { display: block; width: auto !important; max-width: 100% !important; height: auto !important; max-height: 520px; margin: 0 auto; border-radius: 8px; background: #f3f3f3; }
.ce-x-media figcaption, .ce-x-meta, .ce-x-engage { color: #555; font-size: 0.82em; margin: 0.35em 0 0.2em; }
.ce-x-card { display: block; margin: 0.8em 0 1em; border: 1px solid #ddd; border-radius: 10px; overflow: hidden; break-inside: avoid; }
a.ce-x-card, a.ce-x-card:visited, a.ce-x-card * { color: #1a1a1a !important; text-decoration: none !important; }
.ce-x-card img { display: block; width: 100% !important; max-width: 100% !important; height: 200px !important; max-height: 220px; object-fit: cover !important; object-position: center; background: #ececec; }
.ce-x-card-body { padding: 0.8em 0.95em 0.95em; }
.ce-x-kicker { margin: 0 0 0.35em; font-size: 0.78em; font-weight: 650; }
.ce-x-card .ce-x-kicker { color: #666 !important; }
.ce-x-card h3 { margin: 0 0 0.4em; font-size: 1.12em; line-height: 1.3; }
.ce-x-card-body > p { margin: 0; color: #444 !important; font-size: 0.92em; line-height: 1.45; white-space: pre-wrap; }
.ce-x-quote { margin: 0.8em 0 1em; padding: 0.7em 0.9em; border-left: 3px solid #ccc; break-inside: avoid; }
.ce-x-replies { margin-top: 1.5em; padding-top: 0.2em; }
.ce-x-replies > h2 { margin: 0 0 0.2em; color: #666; font-size: 0.78em; letter-spacing: 0.06em; text-transform: uppercase; }
.ce-x-reply { padding: 0.85em 0; border-top: 1px solid #e4e4e4; }
.ce-x-reply .ce-x-body p { margin-bottom: 0.3em; }
.ce-x-title { margin: 0.2em 0 0.3em; font-size: 1.9em; }
.ce-x-article { margin: 0.9em 0 0; }
.ce-x-article p { margin: 0 0 0.9em; }
.ce-x-article figure { margin: 1.1em 0; break-inside: avoid; }
.ce-x-article img { display: block; width: auto !important; max-width: 100% !important; height: auto !important; max-height: 720px; margin: 0 auto; border-radius: 8px; }
`;

/** X Articles: long-form posts with a cover, a title and a rich-text body. */
export type XLongform = {
  /** The `data-article-part="body"` wrapper. */
  part: HTMLElement;
  body: HTMLElement;
  title: string;
  /** Reading time, such as "11 min read". */
  meta: string;
  cover: HTMLImageElement | null;
};

export type XPost = {
  article: HTMLElement;
  name: string;
  handle: string;
  avatar: HTMLImageElement | null;
  body: HTMLElement | null;
  longform: XLongform | null;
  time: string;
  permalink: string;
  views: string;
  engagement: string;
  ad: boolean;
};

export type XPage = {
  posts: XPost[];
  /** Index of the status this URL points at, or -1 for a timeline. */
  focalIndex: number;
};

type RenderOptions = {
  pageUrl: string;
  showAuthor: boolean;
  removeAds: boolean;
  removeSocial: boolean;
  removeMedia: boolean;
};

export function isXPage(url: string) {
  return X_HOSTS.has(hostOf(url));
}

export function readXPage(doc: Document, pageUrl: string): XPage | null {
  if (!isXPage(pageUrl)) {
    return null;
  }
  const posts = Array.from(doc.querySelectorAll("article"))
    .filter((article) => !article.parentElement?.closest("article") && !article.closest("nav, aside, [role='navigation']"))
    .map((article) => describe(article))
    .filter((post): post is XPost => post !== null);
  if (posts.length === 0) {
    return null;
  }
  const id = statusId(pageUrl);
  let focalIndex = id ? posts.findIndex((post) => statusId(post.permalink) === id) : -1;
  if (id && focalIndex < 0) {
    focalIndex = posts.reduce((best, post, index) => (bodyLength(post) > bodyLength(posts[best]) ? index : best), 0);
  }
  return { posts, focalIndex };
}

/** Header fields for a rebuilt X page. The byline meta is a profile URL, which reads as a second link. */
export function xHeaderPage(page: ExtractedPage, x: XPage): ExtractedPage {
  const focal = x.focalIndex >= 0 ? x.posts[x.focalIndex] : undefined;
  if (focal?.longform?.title) {
    return {
      ...page,
      siteName: "X",
      title: focal.longform.title,
      byline: [authorHeading(focal), focal.longform.meta].filter(Boolean).join(" · "),
    };
  }
  const title = focal ? authorHeading(focal) : cleanTitle(page.title);
  return {
    ...page,
    siteName: "X",
    title: title || cleanTitle(page.title),
    byline: "",
  };
}

export function renderXPage(doc: Document, root: HTMLElement, x: XPage, options: RenderOptions): CleanupStats {
  const stats: CleanupStats = { ads: 0, navigation: 0, popups: 0, social: 0, media: 0 };
  const tag = idFactory();
  const focal = x.focalIndex >= 0 ? x.posts[x.focalIndex] : null;
  const replies = focal ? x.posts.filter((post) => post !== focal) : [];
  const main = focal ? [focal] : x.posts;
  let used = false;

  for (const post of main) {
    if (!focal && options.removeAds && post.ad) {
      stats.ads += 1;
      continue;
    }
    root.append(renderPost(doc, post, options, tag, options.showAuthor || !focal));
    used = true;
    if (options.removeSocial && post.engagement) {
      stats.social += 1;
    }
    if (options.removeMedia) {
      stats.media += mediaCount(post);
    }
  }

  const visibleReplies = options.removeSocial ? [] : replies.filter((post) => !(options.removeAds && post.ad));
  if (options.removeSocial) {
    stats.social += replies.length;
  } else if (options.removeAds) {
    stats.ads += replies.filter((post) => post.ad).length;
  }
  if (visibleReplies.length > 0) {
    const section = doc.createElement("section");
    section.className = "ce-x-replies";
    tag(section);
    const heading = doc.createElement("h2");
    heading.textContent = "Replies";
    tag(heading);
    section.append(heading);
    for (const post of visibleReplies) {
      const reply = renderPost(doc, post, options, tag, true);
      reply.classList.add("ce-x-reply");
      section.append(reply);
    }
    root.append(section);
    used = true;
  }

  if (!used) {
    return stats;
  }
  root.dataset.ceX = "true";
  return stats;
}

export function xPageUsed(root: HTMLElement) {
  return root.dataset.ceX === "true";
}

function renderPost(
  doc: Document,
  post: XPost,
  options: RenderOptions,
  tag: (el: Element) => Element,
  showAuthor: boolean,
) {
  const section = doc.createElement("section");
  section.className = "ce-x-post";
  tag(section);
  if (showAuthor && (post.name || post.handle || post.avatar)) {
    section.append(renderByline(doc, post, tag));
  }
  if (post.longform) {
    section.append(...renderLongform(doc, post.longform, options, tag, showAuthor));
  }
  if (post.body) {
    section.append(flowBody(doc, post.body, tag));
  }
  for (const block of blocksOf(post)) {
    if (options.removeMedia && (block.kind === "video" || block.kind === "photo")) {
      continue;
    }
    const node = renderBlock(doc, block, options, tag);
    if (node) {
      section.append(node);
    }
  }
  const meta = [showAuthor ? "" : post.time, post.views].filter(Boolean).join(" · ");
  if (meta) {
    section.append(line(doc, "p", "ce-x-meta", meta, tag));
  }
  if (!options.removeSocial && post.engagement) {
    section.append(line(doc, "p", "ce-x-engage", post.engagement, tag));
  }
  return section;
}

function renderByline(doc: Document, post: XPost, tag: (el: Element) => Element) {
  const row = doc.createElement("div");
  row.className = "ce-x-byline";
  tag(row);
  if (post.avatar) {
    const avatar = adoptImage(doc, post.avatar, tag);
    avatar.className = "ce-x-avatar";
    row.append(avatar);
  }
  const text = doc.createElement("div");
  tag(text);
  if (post.name) {
    const name = doc.createElement("strong");
    name.textContent = post.name;
    tag(name);
    text.append(name);
  }
  const detail = [post.handle, post.time].filter(Boolean).join(" · ");
  if (detail) {
    const span = doc.createElement("span");
    span.textContent = detail;
    tag(span);
    text.append(span);
  }
  row.append(text);
  return row;
}

type Block =
  | { kind: "video"; img: HTMLImageElement | null; poster: string }
  | { kind: "photo"; img: HTMLImageElement }
  | { kind: "card"; article: HTMLElement }
  | { kind: "quote"; article: HTMLElement };

function blocksOf(post: XPost): Block[] {
  const blocks: Block[] = [];
  const seen = new Set<Element>();
  const nodes = post.article.querySelectorAll("video, img, article");
  nodes.forEach((node) => {
    if (seen.has(node) || post.body?.contains(node) || inLongform(post, node)) {
      return;
    }
    if (node.tagName === "ARTICLE") {
      const nested = node as HTMLElement;
      if (nested.parentElement?.closest("article") !== post.article) {
        return;
      }
      seen.add(nested);
      if (nested.querySelector("[class*='article-card']")) {
        blocks.push({ kind: "card", article: nested });
      } else if (nested.querySelector("div.whitespace-pre-wrap")) {
        blocks.push({ kind: "quote", article: nested });
      }
      return;
    }
    if (node.closest("article") !== post.article) {
      return;
    }
    if (node.tagName === "VIDEO") {
      const video = node as HTMLVideoElement;
      const thumb = thumbnailNear(video);
      if (thumb) {
        seen.add(thumb);
      }
      blocks.push({ kind: "video", img: thumb, poster: video.getAttribute("poster") ?? "" });
      return;
    }
    const img = node as HTMLImageElement;
    if (isAvatar(img)) {
      return;
    }
    const alt = img.getAttribute("alt") ?? "";
    if (alt === "Article cover image") {
      return;
    }
    seen.add(img);
    if (alt === "Video thumbnail") {
      blocks.push({ kind: "video", img, poster: "" });
      return;
    }
    blocks.push({ kind: "photo", img });
  });
  return blocks;
}

function renderBlock(doc: Document, block: Block, options: RenderOptions, tag: (el: Element) => Element) {
  if (block.kind === "card") {
    return renderCard(doc, block.article, options, tag);
  }
  if (block.kind === "quote") {
    return renderQuote(doc, block.article, tag);
  }
  const image = block.img ? adoptImage(doc, block.img, tag) : posterImage(doc, block.kind === "video" ? block.poster : "", tag);
  if (!image) {
    return null;
  }
  const figure = doc.createElement("figure");
  figure.className = "ce-x-media";
  tag(figure);
  figure.append(image);
  if (block.kind === "video") {
    const caption = doc.createElement("figcaption");
    caption.textContent = "Video";
    tag(caption);
    figure.append(caption);
  }
  return figure;
}

function renderCard(doc: Document, article: HTMLElement, options: RenderOptions, tag: (el: Element) => Element) {
  const described = describe(article);
  const copy = cardCopy(article);
  if (!copy.title && !copy.excerpt && !article.querySelector("img")) {
    return null;
  }
  const link = doc.createElement("a");
  link.className = "ce-x-card";
  const href = cardHref(article, options.pageUrl);
  if (href) {
    link.setAttribute("href", href);
  }
  tag(link);
  const cover = Array.from(article.querySelectorAll("img")).find((img) => (img.getAttribute("alt") ?? "") === "Article cover image")
    ?? Array.from(article.querySelectorAll("img")).find((img) => !isAvatar(img));
  if (cover && !options.removeMedia) {
    link.append(adoptImage(doc, cover, tag));
  }
  const body = doc.createElement("div");
  body.className = "ce-x-card-body";
  tag(body);
  const kicker = ["Article", described?.name, described?.time].filter(Boolean).join(" · ");
  if (kicker) {
    body.append(line(doc, "p", "ce-x-kicker", kicker, tag));
  }
  if (copy.title) {
    const title = doc.createElement("h3");
    title.textContent = copy.title;
    tag(title);
    body.append(title);
  }
  if (copy.excerpt) {
    const excerpt = doc.createElement("p");
    excerpt.textContent = copy.excerpt;
    tag(excerpt);
    body.append(excerpt);
  }
  link.append(body);
  return link;
}

function renderLongform(
  doc: Document,
  longform: XLongform,
  options: RenderOptions,
  tag: (el: Element) => Element,
  showTitle: boolean,
) {
  const nodes: Element[] = [];
  if (longform.cover && !options.removeMedia) {
    const figure = doc.createElement("figure");
    figure.className = "ce-x-media";
    tag(figure);
    figure.append(adoptImage(doc, longform.cover, tag));
    nodes.push(figure);
  }
  if (showTitle && longform.title) {
    const title = doc.createElement("h1");
    title.className = "ce-x-title";
    title.textContent = longform.title;
    tag(title);
    nodes.push(title);
    if (longform.meta) {
      nodes.push(line(doc, "p", "ce-x-meta", longform.meta, tag));
    }
  }
  const body = doc.createElement("div");
  body.className = "ce-x-article";
  tag(body);
  body.append(...Array.from(longform.body.childNodes).flatMap((node) => richCopy(doc, node, options, tag)));
  nodes.push(body);
  return nodes;
}

const RICH_BLOCKS = new Set([
  "P", "H1", "H2", "H3", "H4", "H5", "H6", "UL", "OL", "LI", "BLOCKQUOTE", "PRE", "FIGURE", "FIGCAPTION",
  "TABLE", "THEAD", "TBODY", "TR", "TH", "TD",
]);
const RICH_INLINE = new Set(["B", "STRONG", "I", "EM", "U", "S", "DEL", "CODE", "SUB", "SUP", "MARK"]);
const RICH_SKIP = new Set(["BUTTON", "SVG", "STYLE", "SCRIPT", "TEMPLATE", "INPUT", "TEXTAREA", "SELECT"]);

/**
 * Copies an X Article body without X's classes. Wrapper divs and spans are unwrapped, and
 * the text keeps its blocks, emphasis and links. `whitespace-pre-wrap` is gone, so line breaks become `<br>`.
 */
function richCopy(doc: Document, node: Node, options: RenderOptions, tag: (el: Element) => Element): Node[] {
  if (node.nodeType === Node.TEXT_NODE) {
    const parts = (node.textContent ?? "").split("\n");
    return parts.flatMap((part, index) => {
      const out: Node[] = index > 0 ? [tag(doc.createElement("br"))] : [];
      if (part) {
        out.push(doc.createTextNode(part));
      }
      return out;
    });
  }
  if (node.nodeType !== Node.ELEMENT_NODE) {
    return [];
  }
  const element = node as HTMLElement;
  const name = element.tagName.toUpperCase();
  if (RICH_SKIP.has(name)) {
    return [];
  }
  if (name === "BR" || name === "HR") {
    return [tag(doc.createElement(name.toLowerCase()))];
  }
  if (name === "IMG") {
    return options.removeMedia ? [] : [adoptImage(doc, element as HTMLImageElement, tag)];
  }
  if (name === "VIDEO") {
    const poster = options.removeMedia ? null : posterImage(doc, element.getAttribute("poster") ?? "", tag);
    return poster ? [poster] : [];
  }
  const children = Array.from(element.childNodes).flatMap((child) => richCopy(doc, child, options, tag));
  if (name === "A") {
    if (!clean(element.textContent)) {
      return children;
    }
    const link = doc.createElement("a");
    const href = element.getAttribute("href");
    if (href) {
      link.setAttribute("href", absoluteHref(href, options.pageUrl));
    }
    tag(link);
    link.append(...children);
    return [link];
  }
  if (!RICH_BLOCKS.has(name) && !RICH_INLINE.has(name)) {
    return children;
  }
  const copy = doc.createElement(name.toLowerCase());
  tag(copy);
  copy.append(...children);
  if (RICH_BLOCKS.has(name) && name !== "TD" && name !== "TH" && !hasContent(copy)) {
    return [];
  }
  return [copy];
}

function renderQuote(doc: Document, article: HTMLElement, tag: (el: Element) => Element) {
  const described = describe(article);
  const quote = doc.createElement("blockquote");
  quote.className = "ce-x-quote";
  tag(quote);
  if (described && (described.name || described.handle)) {
    quote.append(renderByline(doc, described, tag));
  }
  if (described?.body) {
    quote.append(flowBody(doc, described.body, tag));
  }
  return quote;
}

/**
 * X keeps real line breaks in the text and puts each link's "…" in the following node.
 * URLs are pulled onto their own line so a list of links does not run together.
 */
function flowBody(doc: Document, source: HTMLElement, tag: (el: Element) => Element) {
  const wrap = doc.createElement("div");
  wrap.className = "ce-x-body";
  const dir = source.getAttribute("dir");
  if (dir) {
    wrap.setAttribute("dir", dir);
  }
  tag(wrap);
  let paragraph = startParagraph();
  let urlLine = false;

  const flush = () => {
    if (!hasContent(paragraph)) {
      paragraph.remove();
    } else if (urlLine) {
      paragraph.classList.add("ce-x-url-line");
    }
    paragraph = startParagraph();
    urlLine = false;
  };

  function startParagraph() {
    const paragraph = doc.createElement("p");
    tag(paragraph);
    wrap.append(paragraph);
    return paragraph;
  }

  const appendText = (value: string) => {
    const parts = value.split("\n");
    parts.forEach((part, index) => {
      if (index > 0) {
        flush();
      }
      if (part) {
        paragraph.append(part);
      }
    });
  };

  const walk = (node: Node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      let value = node.textContent ?? "";
      const ellipsis = value.match(/^\s*(…+)/);
      const link = paragraph.lastElementChild;
      if (ellipsis && link?.tagName === "A") {
        link.append(ellipsis[1]);
        value = value.slice(ellipsis[0].length);
      }
      if (urlLine && (value.includes("\n") || value.trim() !== "")) {
        flush();
      }
      appendText(value);
      return;
    }
    if (node.nodeType !== Node.ELEMENT_NODE) {
      return;
    }
    const element = node as HTMLElement;
    if (element.tagName === "BR") {
      flush();
      return;
    }
    if (element.tagName === "BUTTON" || element.tagName === "SVG") {
      return;
    }
    if (element.tagName === "A") {
      const label = linkLabel(element);
      const url = looksLikeUrl(label);
      if (url && hasContent(paragraph)) {
        flush();
      }
      const link = doc.createElement("a");
      const href = element.getAttribute("href");
      if (href) {
        link.setAttribute("href", href);
      }
      link.textContent = label;
      tag(link);
      paragraph.append(link);
      urlLine = url;
      return;
    }
    if (element.tagName === "IMG") {
      const image = adoptImage(doc, element as HTMLImageElement, tag);
      paragraph.append(image);
      return;
    }
    element.childNodes.forEach(walk);
  };

  source.childNodes.forEach(walk);
  flush();
  const last = wrap.lastElementChild;
  if (last && !hasContent(last as HTMLElement)) {
    last.remove();
  }
  return wrap;
}

function describe(article: Element): XPost | null {
  const longform = longformOf(article);
  // An X Article's only pre-wrap div is the author's bio in the card under it.
  const body = longform ? null : ownBody(article);
  const scope = longform?.part ?? body;
  const { name, handle } = names(article, scope);
  const stamp = stampOf(article, scope);
  const avatar = avatarOf(article, scope);
  if (!scope && !stamp.text && !avatar && !article.querySelector("video, img")) {
    return null;
  }
  return {
    article: article as HTMLElement,
    name,
    handle,
    avatar,
    body,
    longform,
    time: stamp.text,
    permalink: stamp.href,
    views: viewsOf(article),
    engagement: engagementOf(article),
    ad: promoted(article, scope),
  };
}

function longformOf(article: Element): XLongform | null {
  const own = (selector: string) =>
    Array.from(article.querySelectorAll<HTMLElement>(selector)).find((element) => element.closest("article") === article) ?? null;
  const part = own("[data-article-part='body']");
  if (!part) {
    return null;
  }
  return {
    part,
    body: part.querySelector<HTMLElement>("[data-article-body-html]") ?? part,
    title: clean(own("[data-article-part='title']")?.textContent),
    meta: clean(own("[data-article-part='byline']")?.textContent),
    cover: own("[data-article-part='cover']")?.querySelector("img") ?? null,
  };
}

function inLongform(post: XPost, node: Element) {
  return post.longform !== null && node.closest("[data-article-part]")?.closest("article") === post.article;
}

function ownBody(article: Element) {
  const found = Array.from(article.querySelectorAll("div")).find((element) => {
    return element.classList.contains("whitespace-pre-wrap") && element.closest("article") === article;
  });
  return found ?? null;
}

function names(article: Element, body: HTMLElement | null) {
  let name = "";
  let handle = "";
  for (const link of Array.from(article.querySelectorAll("a"))) {
    if (link.closest("article") !== article || body?.contains(link)) {
      continue;
    }
    if (body && (body.compareDocumentPosition(link) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0) {
      continue;
    }
    const text = clean(link.textContent);
    if (!text || text === "·") {
      continue;
    }
    if (text.startsWith("@")) {
      handle ||= text;
    } else if (!name && text.length <= 80 && !looksLikeUrl(text) && !/^\d{1,2}:\d{2}/.test(text) && !/^[A-Z][a-z]{2,8} \d{1,2}/.test(text)) {
      name = text;
    }
  }
  return { name, handle };
}

function stampOf(article: Element, body: HTMLElement | null) {
  for (const link of Array.from(article.querySelectorAll("a"))) {
    if (link.closest("article") !== article || body?.contains(link)) {
      continue;
    }
    const text = clean(link.textContent);
    if (/^\d{1,2}:\d{2}\s*(AM|PM)\b/i.test(text) || /^[A-Z][a-z]{2,8} \d{1,2}(, \d{4})?$/.test(text)) {
      return { text, href: link.getAttribute("href") ?? "" };
    }
  }
  return { text: "", href: "" };
}

function avatarOf(article: Element, body: HTMLElement | null) {
  const img = Array.from(article.querySelectorAll("img")).find((element) => {
    if (element.closest("article") !== article || body?.contains(element)) {
      return false;
    }
    return isAvatar(element);
  });
  return img ?? null;
}

function viewsOf(article: Element) {
  const label = Array.from(article.querySelectorAll("div, span")).find((element) => {
    return element.closest("article") === article && element.childElementCount === 0 && element.textContent?.trim() === "Views";
  });
  const count = clean(label?.previousElementSibling?.textContent);
  return label && count ? `${count} views` : "";
}

function engagementOf(article: Element) {
  const group = Array.from(article.querySelectorAll("[role='group']")).find((element) => {
    return element.closest("article") === article && /replies|likes|reposts/i.test(element.getAttribute("aria-label") ?? "");
  });
  return (group?.getAttribute("aria-label") ?? "")
    .split(",")
    .map((part) => part.trim())
    .filter((part) => part && !/\bviews\b/i.test(part))
    .join(" · ");
}

function promoted(article: Element, body: HTMLElement | null) {
  return Array.from(article.querySelectorAll("span, div")).some((element) => {
    if (element.closest("article") !== article || body?.contains(element) || element.children.length > 0) {
      return false;
    }
    const text = element.textContent?.trim();
    return text === "Ad" || text === "Promoted";
  });
}

function cardCopy(article: Element) {
  const card = article.querySelector("[class*='article-card']") ?? article;
  const blocks = Array.from(card.querySelectorAll("div")).filter((element) => {
    const name = element.getAttribute("class") ?? "";
    return /font-article|text-headline/.test(name) && clean(element.textContent).length > 0;
  });
  const title = blocks.find((element) => /font-bold|text-headline1/.test(element.getAttribute("class") ?? ""));
  const excerpt = blocks.find((element) => element !== title && !title?.contains(element) && !element.contains(title ?? element));
  return {
    title: clip(title?.textContent ?? "", 240),
    excerpt: clip(excerpt?.textContent ?? "", 520),
  };
}

function cardHref(article: Element, pageUrl: string) {
  const card = article.querySelector("[class*='article-card']");
  const raw = (card?.tagName === "A" ? card.getAttribute("href") : null)
    ?? card?.closest("a")?.getAttribute("href")
    ?? article.querySelector("a[href*='/status/']")?.getAttribute("href")
    ?? "";
  return absoluteHref(raw, pageUrl);
}

function absoluteHref(raw: string, pageUrl: string) {
  try {
    return new URL(raw, pageUrl).href;
  } catch {
    return raw;
  }
}

function thumbnailNear(video: HTMLVideoElement) {
  const scope = video.parentElement ?? video;
  const img = scope.querySelector("img");
  if (!img || isAvatar(img)) {
    return null;
  }
  return img;
}

function posterImage(doc: Document, poster: string, tag: (el: Element) => Element) {
  if (!poster) {
    return null;
  }
  const image = doc.createElement("img");
  image.setAttribute("src", poster);
  image.setAttribute("alt", "Video thumbnail");
  tag(image);
  return image;
}

function adoptImage(doc: Document, img: HTMLImageElement, tag: (el: Element) => Element) {
  const copy = doc.importNode(img, false);
  const src = img.getAttribute("src") ?? "";
  if (src) {
    copy.setAttribute("src", src);
  }
  const alt = img.getAttribute("alt");
  if (alt) {
    copy.setAttribute("alt", alt);
  }
  tag(copy);
  return copy;
}

function line(doc: Document, name: string, className: string, text: string, tag: (el: Element) => Element) {
  const element = doc.createElement(name);
  element.className = className;
  element.textContent = text;
  tag(element);
  return element;
}

function linkLabel(element: Element) {
  const text = clean(element.textContent);
  const title = element.getAttribute("title")?.trim() ?? "";
  if (/^https?:\/\//i.test(title)) {
    const pretty = title.replace(/^https?:\/\//i, "").replace(/^www\./i, "").replace(/\/$/, "");
    const stem = text.replace(/…+$/, "");
    if (stem && pretty.length > stem.length && pretty.startsWith(stem)) {
      return pretty;
    }
  }
  return text;
}

function looksLikeUrl(value: string) {
  return /^[\w.-]+\.[a-z]{2,}(\/[\w./~:@%+-]*)?$/i.test(value.trim());
}

function isAvatar(img: Element) {
  const alt = img.getAttribute("alt") ?? "";
  return alt.startsWith("@") || img.closest("[data-post-avatar]") !== null;
}

function authorHeading(post: XPost) {
  if (post.name && post.handle) {
    return `${post.name} (${post.handle})`;
  }
  return post.name || post.handle;
}

function cleanTitle(title: string) {
  return title.replace(/\s+on X\s*$/i, "").replace(/\s*[|/\\-]+\s*X\s*$/i, "").trim();
}

function hostOf(url: string) {
  try {
    return new URL(url).hostname.replace(/^www\./i, "").toLowerCase();
  } catch {
    return "";
  }
}

function statusId(url: string) {
  return url.match(/\/status\/(\d+)/)?.[1] ?? "";
}

function bodyLength(post: XPost) {
  return (post.longform?.body ?? post.body)?.textContent?.length ?? 0;
}

function mediaCount(post: XPost) {
  const longform = post.longform
    ? post.longform.body.querySelectorAll("img, video").length + (post.longform.cover ? 1 : 0)
    : 0;
  return longform + blocksOf(post).filter((block) => block.kind === "video" || block.kind === "photo").length;
}

function hasContent(element: HTMLElement) {
  return clean(element.textContent).length > 0 || element.querySelector("a, img") !== null;
}

function clean(value: string | null | undefined) {
  return (value ?? "").replace(/\s+/g, " ").trim();
}

function clip(value: string, max: number) {
  const text = value.replace(/\r/g, "").replace(/[ \t]+\n/g, "\n").replace(/\n{3,}/g, "\n\n").trim();
  if (text.length <= max) {
    return text;
  }
  const cut = text.slice(0, max).replace(/\s+\S*$/, "");
  return `${cut || text.slice(0, max)}…`;
}

function idFactory() {
  let next = 0;
  return (element: Element) => {
    element.setAttribute(ID_ATTR, `x${(next++).toString(36)}`);
    return element;
  };
}
