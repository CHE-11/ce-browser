/**
 * Clutter removal and article extraction for the print editor. Everything here works on a
 * parsed `Document` without layout, so it runs the same in the editor and in the quick PDF.
 */

export type CleanupKind = "ads" | "navigation" | "popups" | "social" | "media";

export type CleanupStats = Record<CleanupKind, number>;

type Rule = {
  selectors: string[];
  /** Matched against each class name and the id, split on spaces, `-` and `_`. */
  tokens?: RegExp;
};

const RULES: Record<CleanupKind, Rule> = {
  ads: {
    selectors: [
      "ins.adsbygoogle",
      ".adsbygoogle",
      "[id^='google_ads']",
      "[id^='div-gpt-ad']",
      "[id^='gpt-ad']",
      "[data-ad]",
      "[data-ad-slot]",
      "[data-ad-unit]",
      "[data-ad-client]",
      "[data-adunit]",
      "[data-google-query-id]",
      "[data-native-ad]",
      "[aria-label='Advertisement' i]",
      "[aria-label='Sponsored' i]",
      "[class*='taboola' i]",
      "[id*='taboola' i]",
      "[class*='outbrain' i]",
      "[id*='outbrain' i]",
      "[class*='OUTBRAIN']",
      "[class*='revcontent' i]",
      "[class*='mgid' i]",
      "[class*='zergnet' i]",
      "iframe[src*='doubleclick']",
      "iframe[src*='googlesyndication']",
      "iframe[src*='adservice']",
      "iframe[src*='amazon-adsystem']",
      "iframe[id*='google_ads']",
      "img[src*='doubleclick']",
      "img[width='1'][height='1']",
      "a[href*='doubleclick.net']",
      "a[href*='/aclk?']",
    ],
    tokens:
      /^(ad|ads|adv|advert|adverts|advertisement|advertising|adslot|adunit|adbox|adblock|adbanner|adcontainer|adwrapper|adzone|dfp|gpt|sponsor|sponsored|sponsorship|promoted|native-ad|banner-ad|ad-slot|ad-unit|ad-container|ad-wrapper|ad-banner|ad-placeholder|ad-label|ad-holder|leaderboard|skyscraper|billboard|mrec)$/i,
  },
  navigation: {
    selectors: [
      "nav",
      "[role='navigation']",
      "[role='banner']",
      "[role='contentinfo']",
      "[role='complementary']",
      "[role='search']",
      "[role='menubar']",
      "[role='toolbar']",
      "body > header",
      "body > footer",
      "aside",
      "#sidebar",
      ".sidebar",
      ".breadcrumb",
      ".breadcrumbs",
      ".skip-link",
      ".skip-to-content",
    ],
    tokens:
      /^(nav|navbar|navigation|topnav|subnav|menu|mainmenu|megamenu|menubar|sidebar|side-bar|breadcrumb|breadcrumbs|masthead|site-header|site-footer|global-header|global-footer|page-header|page-footer|footer|toolbar|pagination|pager|skiplink|skip-link|widget|widgets)$/i,
  },
  popups: {
    selectors: [
      "[role='dialog']",
      "[role='alertdialog']",
      "[aria-modal='true']",
      "dialog",
      "#onetrust-consent-sdk",
      "#onetrust-banner-sdk",
      "#CybotCookiebotDialog",
      ".fc-consent-root",
      "#cookie-law-info-bar",
      ".cc-window",
      "#didomi-host",
      ".qc-cmp2-container",
      "#usercentrics-root",
      "[id*='cookie' i]",
      "[class*='cookie' i]",
      "[id*='consent' i]",
      "[class*='consent' i]",
      "[class*='gdpr' i]",
      "[class*='paywall' i]",
      "[id*='paywall' i]",
    ],
    tokens:
      /^(modal|modals|popup|pop-up|popover|overlay|lightbox|interstitial|newsletter|subscribe|subscription|signup|sign-up|optin|opt-in|toast|notification|alert-bar|announcement|app-banner|smartbanner|backdrop|regwall|piano)$/i,
  },
  social: {
    selectors: [
      "#comments",
      "#disqus_thread",
      ".comments",
      ".comment-list",
      ".fb-like",
      ".fb-comments",
      ".twitter-share-button",
      "[class*='share' i]",
      "[id*='share' i]",
      "[class*='social' i]",
      "[class*='related' i]",
      "[id*='related' i]",
      "[class*='recommend' i]",
      "[class*='more-stories' i]",
      "[class*='read-next' i]",
      "[class*='trending' i]",
      "[class*='author-bio' i]",
      "[class*='tags' i]",
    ],
    tokens:
      /^(share|sharing|shares|sharebar|social|socials|comments?|commentary|disqus|respond|related|recommended|recommendations|more|follow|follow-us|tags|tag-list|newsletter|reactions?|likes?|print|email|whatsapp|facebook|twitter|linkedin|pinterest|reddit)$/i,
  },
  media: {
    selectors: [
      "video",
      "audio",
      "iframe",
      "embed",
      "object",
      "[class*='video-player' i]",
      "[class*='player-container' i]",
      "[class*='media-player' i]",
      "[class*='jwplayer' i]",
      "[class*='brightcove' i]",
    ],
  },
};

/** Elements that are never removed by a rule, however they are named. */
const PROTECTED = new Set(["HTML", "HEAD", "BODY", "MAIN", "TITLE", "BASE", "META", "LINK", "STYLE"]);

const SPLIT = /[\s_]+/;

function tokensOf(element: Element) {
  const tokens: string[] = [];
  const id = element.getAttribute("id");
  if (id) {
    tokens.push(...id.split(SPLIT), id);
  }
  const classes = element.getAttribute("class");
  if (classes) {
    for (const name of classes.split(/\s+/)) {
      if (!name) {
        continue;
      }
      tokens.push(name, ...name.split(/[-_]+/));
    }
  }
  return tokens;
}

function textLength(element: Element | null) {
  return element?.textContent?.replace(/\s+/g, " ").trim().length ?? 0;
}

/**
 * Removes elements that match a rule. Skips anything holding most of the page's text, so a
 * wrapper named `with-sidebar` or `has-ads` does not take the content with it.
 */
export function removeClutter(doc: Document, kind: CleanupKind, keep?: Element | null) {
  const rule = RULES[kind];
  const body = doc.body;
  if (!body) {
    return 0;
  }
  const total = Math.max(1, textLength(body));
  const matches = new Set<Element>();
  for (const selector of rule.selectors) {
    try {
      body.querySelectorAll(selector).forEach((element) => matches.add(element));
    } catch {
      // A selector this engine rejects is skipped.
    }
  }
  if (rule.tokens) {
    const tokens = rule.tokens;
    body.querySelectorAll("[class], [id]").forEach((element) => {
      if (tokensOf(element).some((token) => tokens.test(token))) {
        matches.add(element);
      }
    });
  }
  let removed = 0;
  for (const element of matches) {
    if (!element.isConnected || PROTECTED.has(element.tagName)) {
      continue;
    }
    if (keep && (element === keep || element.contains(keep))) {
      continue;
    }
    if (element.closest("[data-ce-keep]")) {
      continue;
    }
    if (kind === "navigation" && (element.tagName === "HEADER" || element.tagName === "FOOTER")) {
      if (element.closest("article")) {
        continue;
      }
    }
    if (kind !== "media" && textLength(element) > total * 0.45) {
      continue;
    }
    if (kind === "social" && element.closest("pre, code")) {
      continue;
    }
    element.remove();
    removed += 1;
  }
  return removed;
}

// Article extraction, after the ideas in Mozilla Readability: score paragraphs, give the score
// to their containers, prefer containers with little link text, and keep the best one along
// with siblings that look like part of the same story.

const POSITIVE =
  /article|body|content|entry|hentry|h-entry|main|page|pagination|post|text|blog|story|prose|markdown|rich-text|instapaper_body/i;
const NEGATIVE =
  /-ad-|hidden|^hid$| hid$| hid |^hid |banner|combx|comment|com-|contact|foot|footer|footnote|gdpr|masthead|media|meta|outbrain|promo|related|scroll|share|shoutbox|sidebar|skyscraper|sponsor|shopping|tags|tool|widget|subscribe|newsletter|cookie|consent|nav|menu/i;
const UNLIKELY =
  /-ad-|ai2html|banner|breadcrumbs|combx|comment|community|cover-wrap|disqus|extra|footer|gdpr|header|legends|menu|related|remark|replies|rss|shoutbox|sidebar|skyscraper|social|sponsor|supplemental|ad-break|agegate|pagination|pager|popup|yom-remote|cookie|consent|newsletter/i;
const MAYBE = /and|article|body|column|content|main|shadow|story|post/i;

const ARTICLE_SELECTORS = [
  "[itemprop='articleBody']",
  "article [class*='article-body' i]",
  "[class*='article-body' i]",
  "[class*='article__body' i]",
  "[class*='articleBody' i]",
  "[class*='post-content' i]",
  "[class*='entry-content' i]",
  "[class*='story-body' i]",
  "[class*='post-body' i]",
  "[class*='rich-text' i]",
  "article",
  "[role='main'] article",
  "main",
  "[role='main']",
];

function classWeight(element: Element) {
  let weight = 0;
  const name = `${element.getAttribute("class") ?? ""} ${element.getAttribute("id") ?? ""}`;
  if (NEGATIVE.test(name)) {
    weight -= 25;
  }
  if (POSITIVE.test(name)) {
    weight += 25;
  }
  return weight;
}

function linkDensity(element: Element) {
  const length = textLength(element);
  if (length === 0) {
    return 0;
  }
  let links = 0;
  element.querySelectorAll("a").forEach((link) => {
    const href = link.getAttribute("href") ?? "";
    const coefficient = href.startsWith("#") ? 0.3 : 1;
    links += textLength(link) * coefficient;
  });
  return links / length;
}

function initialScore(element: Element) {
  switch (element.tagName) {
    case "DIV":
    case "SECTION":
    case "ARTICLE":
      return 5 + classWeight(element);
    case "PRE":
    case "TD":
    case "BLOCKQUOTE":
      return 3 + classWeight(element);
    case "ADDRESS":
    case "OL":
    case "UL":
    case "DL":
    case "DD":
    case "DT":
    case "LI":
    case "FORM":
      return -3 + classWeight(element);
    case "H1":
    case "H2":
    case "H3":
    case "H4":
    case "H5":
    case "H6":
    case "TH":
      return -5 + classWeight(element);
    default:
      return classWeight(element);
  }
}

function byKnownSelector(doc: Document) {
  for (const selector of ARTICLE_SELECTORS) {
    const candidates = Array.from(doc.querySelectorAll(selector));
    const best = candidates
      .map((element) => ({ element, length: textLength(element) }))
      .filter((candidate) => candidate.length > 600 && linkDensity(candidate.element) < 0.4)
      .sort((a, b) => b.length - a.length)[0];
    if (best) {
      return best.element;
    }
  }
  return null;
}

function byScore(doc: Document) {
  const body = doc.body;
  if (!body) {
    return null;
  }
  const scores = new Map<Element, number>();
  const paragraphs = body.querySelectorAll("p, pre, td, blockquote, li, h2, h3, div > br");
  paragraphs.forEach((node) => {
    const paragraph = node.tagName === "BR" ? node.parentElement : node;
    if (!paragraph) {
      return;
    }
    const name = `${paragraph.getAttribute("class") ?? ""} ${paragraph.getAttribute("id") ?? ""}`;
    if (UNLIKELY.test(name) && !MAYBE.test(name)) {
      return;
    }
    const text = paragraph.textContent?.replace(/\s+/g, " ").trim() ?? "";
    if (text.length < 25) {
      return;
    }
    const score = 1 + text.split(",").length + Math.min(3, Math.floor(text.length / 100));
    let ancestor = paragraph.parentElement;
    for (let level = 0; ancestor && ancestor !== body.parentElement && level < 3; level += 1) {
      if (!scores.has(ancestor)) {
        scores.set(ancestor, initialScore(ancestor));
      }
      const divider = level === 0 ? 1 : level === 1 ? 2 : level * 3;
      scores.set(ancestor, (scores.get(ancestor) ?? 0) + score / divider);
      ancestor = ancestor.parentElement;
    }
  });
  let top: Element | null = null;
  let topScore = 0;
  for (const [element, score] of scores) {
    const final = score * (1 - linkDensity(element));
    scores.set(element, final);
    if (final > topScore) {
      top = element;
      topScore = final;
    }
  }
  if (!top) {
    return null;
  }
  // A parent that scores nearly as well usually holds the rest of the story too.
  let parent = top.parentElement;
  while (parent && parent !== body) {
    const score = scores.get(parent) ?? 0;
    if (score < topScore * 0.75) {
      break;
    }
    top = parent;
    topScore = Math.max(topScore, score);
    parent = parent.parentElement;
  }
  const container = doc.createElement("div");
  const threshold = Math.max(10, topScore * 0.2);
  const siblings = top.parentElement ? Array.from(top.parentElement.children) : [top];
  for (const sibling of siblings) {
    if (sibling === top) {
      container.appendChild(sibling.cloneNode(true));
      continue;
    }
    const bonus = sibling.className && sibling.className === top.className ? topScore * 0.2 : 0;
    const score = (scores.get(sibling) ?? 0) + bonus;
    if (score >= threshold) {
      container.appendChild(sibling.cloneNode(true));
    } else if (sibling.tagName === "P") {
      const density = linkDensity(sibling);
      const length = textLength(sibling);
      if ((length > 80 && density < 0.25) || (length > 0 && length <= 80 && density === 0 && /\.( |$)/.test(sibling.textContent ?? ""))) {
        container.appendChild(sibling.cloneNode(true));
      }
    }
  }
  return container;
}

/**
 * The main story of the page, or null when nothing looks like an article. The result is a
 * detached copy, so the source document is not changed.
 */
export function extractArticle(doc: Document): HTMLElement | null {
  const known = byKnownSelector(doc);
  const picked = known ? (known.cloneNode(true) as HTMLElement) : byScore(doc);
  if (!picked || textLength(picked) < 200) {
    return null;
  }
  tidyArticle(picked);
  return picked;
}

/** Drops leftovers inside the article: forms, buttons, and link-heavy blocks. */
function tidyArticle(root: HTMLElement) {
  root.querySelectorAll("form, button, input, select, textarea, [hidden]").forEach((element) => element.remove());
  root.querySelectorAll("div, section, ul, ol, aside, table").forEach((element) => {
    if (!element.isConnected) {
      return;
    }
    const length = textLength(element);
    const images = element.querySelectorAll("img, picture, figure, svg, video").length;
    if (length === 0 && images === 0) {
      element.remove();
      return;
    }
    const name = `${element.getAttribute("class") ?? ""} ${element.getAttribute("id") ?? ""}`;
    if (NEGATIVE.test(name) && !POSITIVE.test(name) && length < 600) {
      element.remove();
      return;
    }
    if (length < 400 && images === 0 && linkDensity(element) > 0.5) {
      element.remove();
    }
  });
}
