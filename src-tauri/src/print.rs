//! Print to PDF: reads a tab's document for the editor, and prints edited HTML or a tab to PDF.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Webview, WebviewUrl};
use url::Url;

use crate::{files, tabs, webview2};

const READ_FAILURE: &str = "Couldn't read this page.";
const PRINT_FAILURE: &str = "Couldn't save the PDF.";

static NEXT_HOST: AtomicU32 = AtomicU32::new(1);
/// The webview behind an open print dialog, and the temp file to delete with it.
/// A saved download is not deleted. Closed when the next print starts.
static DIALOG_HOST: Mutex<Option<(String, Option<PathBuf>)>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ExtractedPage {
    pub title: String,
    pub url: String,
    pub html: String,
    pub lang: String,
    pub byline: String,
    pub site_name: String,
    pub published: String,
}

/// Paper and layout for a PDF. Sizes are inches and describe the page in portrait.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PdfOptions {
    pub page_width: f64,
    pub page_height: f64,
    pub landscape: bool,
    pub margin: f64,
    pub backgrounds: bool,
    pub header_footer: bool,
    /// `current/total`, such as `2/12`, along the bottom of every page.
    pub page_numbers: bool,
    /// `left`, `center`, or `right`.
    pub page_number_location: String,
    /// Text size percent from the print settings. Page numbers are 85% of the body size this scales.
    pub text_size: f64,
    pub scale: f64,
    pub title: String,
    pub url: String,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            page_width: 8.5,
            page_height: 11.0,
            landscape: false,
            margin: 0.5,
            backgrounds: false,
            header_footer: false,
            page_numbers: false,
            page_number_location: "right".into(),
            text_size: 100.0,
            scale: 1.0,
            title: String::new(),
            url: String::new(),
        }
    }
}

impl PdfOptions {
    fn layout(&self) -> webview2::PdfLayout {
        let margin = self.margin.clamp(0.0, 3.0);
        webview2::PdfLayout {
            page_width: self.page_width.clamp(1.0, 50.0),
            page_height: self.page_height.clamp(1.0, 50.0),
            landscape: self.landscape,
            margin,
            backgrounds: self.backgrounds,
            header_footer: self.header_footer,
            page_numbers: self.page_numbers,
            header_template: page_number_header(self.header_footer),
            footer_template: page_number_footer(
                self.header_footer,
                &self.page_number_location,
                margin,
                self.text_size,
            ),
            title: self.title.clone(),
            url: self.url.clone(),
            scale: if self.scale > 0.0 { self.scale } else { 1.0 },
        }
    }
}

/// Chromium's print footer fills `pageNumber` and `totalPages`. An empty template is replaced
/// by the default header, which also prints the title, so the unused header stays a blank div.
pub fn page_number_header(header_footer: bool) -> String {
    if header_footer {
        r#"<div style="font-size:9px;width:100%;color:#444;padding:0 8px;"><span style="float:left;"><span class="title"></span></span><span style="float:right;"><span class="date"></span></span></div>"#.to_string()
    } else {
        r#"<div style="font-size:9px;"></div>"#.to_string()
    }
}

/// Article body in the print document (`17px`, then the text-size percent).
const PRINT_BODY_FONT_PX: u32 = 17;
/// Page numbers are 85% of the body size. Stored as hundredths so the product stays exact.
const PAGE_NUMBER_RATIO_HUNDREDTHS: u32 = 85;
/// The footer spans the full sheet. Numbers sit at least this far from the paper edge,
/// and further in when the body margin is wider, so they line up with the text.
const PAGE_NUMBER_INSET_MIN_IN: f64 = 0.5;

/// `17px * textSize% * 0.85`, as a CSS pixel length such as `14.45px`.
fn page_number_font_px(text_size: f64) -> String {
    let percent = if text_size <= 0.0 {
        100
    } else {
        text_size.clamp(70.0, 150.0).round() as u32
    };
    let hundredths = (PRINT_BODY_FONT_PX * percent * PAGE_NUMBER_RATIO_HUNDREDTHS + 50) / 100;
    format!("{}.{:02}px", hundredths / 100, hundredths % 100)
}

fn page_number_inset(margin_in: f64) -> f64 {
    margin_in.max(PAGE_NUMBER_INSET_MIN_IN)
}

fn css_num(value: f64) -> String {
    format!("{value:.2}")
}

/// `current/total` at `location`. With headers on, the page URL keeps the opposite side.
pub fn page_number_footer(
    header_footer: bool,
    location: &str,
    margin_in: f64,
    text_size: f64,
) -> String {
    let align = match location {
        "left" => "left",
        "center" => "center",
        _ => "right",
    };
    let font = page_number_font_px(text_size);
    let inset = css_num(page_number_inset(margin_in));
    let number = format!(
        r#"<span style="font-size:{font}"><span class="pageNumber"></span>/<span class="totalPages"></span></span>"#
    );
    if !header_footer {
        return format!(
            r#"<div style="width:100%;text-align:{align};color:#444;padding:0 {inset}in;">{number}</div>"#
        );
    }
    let url = r#"<span class="url" style="font-size:9px"></span>"#;
    let (left, center, right) = match align {
        "left" => (number.as_str(), "", url),
        "center" => (url, number.as_str(), ""),
        _ => (url, "", number.as_str()),
    };
    format!(
        r#"<table style="width:100%;color:#444;border-spacing:0;"><tr><td style="width:33%;text-align:left;padding-left:{inset}in;">{left}</td><td style="width:34%;text-align:center;">{center}</td><td style="width:33%;text-align:right;padding-right:{inset}in;">{right}</td></tr></table>"#
    )
}

/// Copies the live document into standalone HTML: no scripts, absolute URLs, lazy images
/// resolved, and styles that only exist in the CSSOM written out as text.
const EXTRACT_SCRIPT: &str = r##"(async () => {
  const base = document.baseURI;
  const absolute = (value) => {
    try { return new URL(value, base).href; } catch { return value; }
  };
  const absoluteSrcset = (value) => value.split(",").map((part) => {
    const bits = part.trim().split(/\s+/);
    if (!bits[0]) return "";
    bits[0] = absolute(bits[0]);
    return bits.join(" ");
  }).filter(Boolean).join(", ");
  const liveStyles = Array.from(document.querySelectorAll("style"));
  const liveCanvases = Array.from(document.querySelectorAll("canvas"));
  const liveImages = Array.from(document.querySelectorAll("img"));
  const clone = document.documentElement.cloneNode(true);

  clone.querySelectorAll("style").forEach((style, index) => {
    const live = liveStyles[index];
    if (!live || style.textContent.trim()) return;
    try {
      const rules = live.sheet ? Array.from(live.sheet.cssRules).map((rule) => rule.cssText) : [];
      style.textContent = rules.join("\n");
    } catch {}
  });
  clone.querySelectorAll("canvas").forEach((canvas, index) => {
    const live = liveCanvases[index];
    if (!live) return;
    try {
      const image = document.createElement("img");
      image.src = live.toDataURL("image/png");
      image.width = live.width;
      image.height = live.height;
      image.setAttribute("style", live.getAttribute("style") || "");
      image.className = live.className;
      canvas.replaceWith(image);
    } catch {}
  });
  clone.querySelectorAll("img").forEach((image, index) => {
    const live = liveImages[index];
    const lazy = image.getAttribute("data-src") || image.getAttribute("data-lazy-src")
      || image.getAttribute("data-original") || image.getAttribute("data-url");
    const current = live && live.currentSrc && !live.currentSrc.startsWith("data:") ? live.currentSrc : "";
    const src = image.getAttribute("src") || "";
    if (current) {
      image.setAttribute("src", current);
    } else if (lazy && (!src || src.startsWith("data:") || /blank|placeholder|lazy|spacer/i.test(src))) {
      image.setAttribute("src", lazy);
    }
    const lazySet = image.getAttribute("data-srcset") || image.getAttribute("data-lazy-srcset");
    if (lazySet && !image.getAttribute("srcset")) image.setAttribute("srcset", lazySet);
    image.removeAttribute("loading");
    image.removeAttribute("decoding");
  });
  clone.querySelectorAll("source[data-srcset]").forEach((source) => {
    if (!source.getAttribute("srcset")) source.setAttribute("srcset", source.getAttribute("data-srcset"));
  });

  clone.querySelectorAll("script, noscript, base, link[rel~='preload'], link[rel~='modulepreload'], link[rel~='prefetch'], link[rel~='dns-prefetch'], link[rel~='preconnect'], meta[http-equiv]").forEach((node) => node.remove());
  clone.querySelectorAll("*").forEach((node) => {
    for (const attribute of Array.from(node.attributes)) {
      const name = attribute.name.toLowerCase();
      if (name.startsWith("on")) {
        node.removeAttribute(attribute.name);
      } else if (name === "src" || name === "href" || name === "poster" || name === "action" || name === "data") {
        if (/^\s*javascript:/i.test(attribute.value)) {
          node.removeAttribute(attribute.name);
        } else if (attribute.value && !attribute.value.startsWith("#") && !/^(data|blob|about|mailto|tel):/i.test(attribute.value)) {
          node.setAttribute(attribute.name, absolute(attribute.value));
        }
      } else if (name === "srcset") {
        node.setAttribute(attribute.name, absoluteSrcset(attribute.value));
      }
    }
  });

  let head = clone.querySelector("head");
  if (!head) {
    head = document.createElement("head");
    clone.insertBefore(head, clone.firstChild);
  }
  const baseElement = document.createElement("base");
  baseElement.setAttribute("href", base);
  head.insertBefore(baseElement, head.firstChild);
  if (document.adoptedStyleSheets && document.adoptedStyleSheets.length) {
    const adopted = document.createElement("style");
    try {
      adopted.textContent = document.adoptedStyleSheets
        .map((sheet) => Array.from(sheet.cssRules).map((rule) => rule.cssText).join("\n"))
        .join("\n");
      head.appendChild(adopted);
    } catch {}
  }

  const meta = (selector) => {
    const element = document.querySelector(selector);
    return element ? (element.getAttribute("content") || "").trim() : "";
  };
  const readBlob = async (url) => {
    try {
      const blob = await fetch(url).then((response) => response.blob());
      return await new Promise((resolve) => {
        const reader = new FileReader();
        reader.onload = () => resolve(typeof reader.result === "string" ? reader.result : "");
        reader.onerror = () => resolve("");
        reader.readAsDataURL(blob);
      });
    } catch {
      return "";
    }
  };
  await Promise.all(Array.from(clone.querySelectorAll("[src], [poster]")).map(async (node) => {
    for (const name of ["src", "poster"]) {
      const value = node.getAttribute(name) || "";
      if (!value.startsWith("blob:")) continue;
      const data = await readBlob(value);
      if (data) node.setAttribute(name, data);
    }
  }));
  return {
    title: (meta("meta[property='og:title']") || document.title || "").trim(),
    url: location.href,
    html: "<!DOCTYPE html>\n" + clone.outerHTML,
    lang: document.documentElement.lang || "",
    byline: meta("meta[name='author']") || meta("meta[property='article:author']"),
    siteName: meta("meta[property='og:site_name']") || location.hostname,
    published: meta("meta[property='article:published_time']") || meta("meta[name='date']"),
  };
})()"##;

/// Waits for images and web fonts in the print host so the PDF does not catch them half loaded.
const SETTLE_SCRIPT: &str = r#"(async () => {
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const pending = Array.from(document.images).filter((image) => !image.complete);
  await Promise.race([
    Promise.all([
      ...pending.map((image) => new Promise((resolve) => {
        image.addEventListener("load", resolve, { once: true });
        image.addEventListener("error", resolve, { once: true });
      })),
      document.fonts ? document.fonts.ready : Promise.resolve(),
    ]),
    sleep(10000),
  ]);
  await sleep(150);
  return true;
})()"#;

/// Title, author and date from a downloaded page. Same fields as `extract_page`, without the document.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PrintPageFacts {
    pub title: String,
    pub url: String,
    pub lang: String,
    pub byline: String,
    pub site_name: String,
    pub published: String,
}

const PAGE_FACTS_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";
const PAGE_FACTS_LIMIT: u64 = 512 * 1024;

/// Reads the same title, author, site and date a live extract would, from the page's HTML.
pub fn page_facts(input: &str) -> Result<PrintPageFacts, String> {
    let url = Url::parse(input.trim()).map_err(|_| "Enter a web address.".to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Enter a web address.".into());
    }
    let (final_url, html) = download_html(&url)?;
    Ok(facts_from_html(&final_url, &html))
}

fn download_html(url: &Url) -> Result<(String, String), String> {
    let response = match ureq::get(url.as_str())
        .timeout(Duration::from_secs(12))
        .set("User-Agent", PAGE_FACTS_AGENT)
        .set("Accept", "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8")
        .call()
    {
        Ok(response) => response,
        Err(ureq::Error::Status(404, _)) => return Err("That page was not found.".into()),
        Err(_) => return Err(READ_FAILURE.into()),
    };
    let final_url = response.get_url().to_string();
    let parsed = Url::parse(&final_url).map_err(|_| READ_FAILURE.to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(READ_FAILURE.into());
    }
    let kind = response.content_type().to_ascii_lowercase();
    let html = if kind.is_empty() || kind.contains("html") || kind.contains("xml") {
        let mut body = Vec::new();
        response
            .into_reader()
            .take(PAGE_FACTS_LIMIT)
            .read_to_end(&mut body)
            .map_err(|_| READ_FAILURE.to_string())?;
        String::from_utf8_lossy(&body).into_owned()
    } else {
        String::new()
    };
    Ok((parsed.to_string(), html))
}

fn facts_from_html(url: &str, html: &str) -> PrintPageFacts {
    let host = Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_string))
        .unwrap_or_default();
    let site = meta_content(html, "property", "og:site_name");
    PrintPageFacts {
        title: first_text(&[
            meta_content(html, "property", "og:title"),
            element_text(html, "title"),
        ]),
        url: url.to_string(),
        lang: html_lang(html),
        byline: first_text(&[
            meta_content(html, "name", "author"),
            meta_content(html, "property", "article:author"),
        ]),
        site_name: if site.is_empty() { host } else { site },
        published: first_text(&[
            meta_content(html, "property", "article:published_time"),
            meta_content(html, "name", "date"),
        ]),
    }
}

fn first_text(values: &[String]) -> String {
    values
        .iter()
        .find(|value| !value.is_empty())
        .cloned()
        .unwrap_or_default()
}

fn meta_content(html: &str, attr: &str, key: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut search = 0;
    while let Some(relative) = lower[search..].find("<meta") {
        let start = search + relative;
        let after = start + 5;
        let boundary = match html.as_bytes().get(after) {
            Some(byte) => byte.is_ascii_whitespace() || *byte == b'/' || *byte == b'>',
            None => true,
        };
        if !boundary {
            search = after;
            continue;
        }
        let end = html[start..]
            .find('>')
            .map(|relative| start + relative)
            .unwrap_or(html.len());
        let tag = &html[start..=end.min(html.len().saturating_sub(1))];
        if attr_value(tag, attr).is_some_and(|value| value.eq_ignore_ascii_case(key)) {
            if let Some(content) = attr_value(tag, "content") {
                let decoded = decode_entities(&content);
                if !decoded.is_empty() {
                    return decoded;
                }
            }
        }
        search = end.saturating_add(1);
        if search >= html.len() {
            break;
        }
    }
    String::new()
}

fn element_text(html: &str, name: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{name}");
    let close = format!("</{name}");
    let mut search = 0;
    while let Some(relative) = lower[search..].find(&open) {
        let start = search + relative;
        let after = start + open.len();
        let boundary = match html.as_bytes().get(after) {
            Some(byte) => byte.is_ascii_whitespace() || *byte == b'>' || *byte == b'/',
            None => true,
        };
        if !boundary {
            search = after;
            continue;
        }
        let content_start = html[start..]
            .find('>')
            .map(|relative| start + relative + 1)
            .unwrap_or(html.len());
        if let Some(relative) = lower[content_start..].find(&close) {
            let decoded = decode_entities(html[content_start..content_start + relative].trim());
            if !decoded.is_empty() {
                return decoded;
            }
        }
        search = content_start.max(after);
        if search >= html.len() {
            break;
        }
    }
    String::new()
}

fn html_lang(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let Some(start) = lower.find("<html") else {
        return String::new();
    };
    let after = start + 5;
    let boundary = match html.as_bytes().get(after) {
        Some(byte) => byte.is_ascii_whitespace() || *byte == b'>' || *byte == b'/',
        None => true,
    };
    if !boundary {
        return String::new();
    }
    let end = html[start..]
        .find('>')
        .map(|relative| start + relative)
        .unwrap_or(html.len());
    decode_entities(
        &attr_value(&html[start..=end.min(html.len().saturating_sub(1))], "lang")
            .unwrap_or_default(),
    )
}

fn attr_value(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let name = name.to_ascii_lowercase();
    let mut search = 0;
    while let Some(relative) = lower[search..].find(&name) {
        let index = search + relative;
        let before = index
            .checked_sub(1)
            .and_then(|at| lower.as_bytes().get(at))
            .copied();
        let boundary = before.is_none_or(|byte| byte.is_ascii_whitespace() || byte == b'<');
        let after_name = index + name.len();
        if !boundary {
            search = index + 1;
            continue;
        }
        let after = &lower[after_name..];
        let trimmed = after.trim_start();
        if !trimmed.starts_with('=') {
            search = index + 1;
            continue;
        }
        let skipped = after.len() - trimmed.len();
        let past_eq = trimmed[1..].trim_start();
        let eq_gap = trimmed[1..].len() - past_eq.len();
        let value_at = after_name + skipped + 1 + eq_gap;
        if past_eq.is_empty() {
            return None;
        }
        let quote = past_eq.as_bytes()[0] as char;
        if quote == '"' || quote == '\'' {
            let start = value_at + 1;
            let end = tag[start..].find(quote)? + start;
            return Some(tag[start..end].to_string());
        }
        let end = tag[value_at..]
            .find(|character: char| character.is_whitespace() || character == '>')
            .map(|relative| value_at + relative)
            .unwrap_or(tag.len());
        return Some(tag[value_at..end].to_string());
    }
    None
}

fn decode_entities(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(index) = rest.find('&') {
        out.push_str(&rest[..index]);
        rest = &rest[index..];
        let Some(end) = rest.find(';') else {
            out.push_str(rest);
            return collapse(&out);
        };
        if end > 16 {
            out.push('&');
            rest = &rest[1..];
            continue;
        }
        let entity = &rest[1..end];
        let character = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            _ if entity.len() > 2
                && (entity.as_bytes()[1] == b'x' || entity.as_bytes()[1] == b'X')
                && entity.starts_with('#') =>
            {
                u32::from_str_radix(&entity[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ => entity
                .strip_prefix('#')
                .and_then(|digits| digits.parse::<u32>().ok())
                .and_then(char::from_u32),
        };
        if let Some(character) = character {
            out.push(character);
            rest = &rest[end + 1..];
        } else {
            out.push('&');
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    collapse(&out)
}

fn collapse(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn extract_page(app: &AppHandle, id: u32) -> Result<ExtractedPage, String> {
    let webview =
        tabs::page_view(app, id).ok_or("That tab is closed or its page is not loaded.")?;
    let value = webview2::evaluate(
        &webview,
        EXTRACT_SCRIPT,
        Duration::from_secs(30),
        READ_FAILURE,
    )?;
    serde_json::from_value(value).map_err(|error| format!("{READ_FAILURE} {error}"))
}

pub fn print_tab_to_pdf(
    app: &AppHandle,
    id: u32,
    options: PdfOptions,
    path: &Path,
) -> Result<(), String> {
    let webview =
        tabs::page_view(app, id).ok_or("That tab is closed or its page is not loaded.")?;
    prepare_target(path)?;
    webview2::print_to_pdf(&webview, path, options.layout())?;
    crate::downloads::record_completed(app, &options.url, path, "application/pdf");
    Ok(())
}

pub fn print_html_to_pdf(
    app: &AppHandle,
    html: &str,
    options: PdfOptions,
    path: &Path,
    tab_id: Option<u32>,
) -> Result<(), String> {
    prepare_target(path)?;
    let html = crate::print_images::embed(app, html, &options.url, tab_id);
    let (webview, file) = open_host(app, &html)?;
    let result = webview2::print_to_pdf(&webview, path, options.layout());
    let _ = webview.close();
    let _ = std::fs::remove_file(file);
    if result.is_ok() {
        crate::downloads::record_completed(app, &options.url, path, "application/pdf");
    }
    result
}

pub fn print_html_dialog(app: &AppHandle, html: &str, tab_id: Option<u32>) -> Result<(), String> {
    close_dialog_host(app);
    let html = crate::print_images::embed(app, html, "", tab_id);
    let (webview, file) = open_host(app, &html)?;
    if let Ok(mut host) = DIALOG_HOST.lock() {
        *host = Some((webview.label().to_string(), Some(file)));
    }
    webview2::show_print_dialog(&webview)
}

const DIALOG_FAILURE: &str = "Couldn't open the print dialog.";

/// Opens the Windows print dialog for a file this browser can show.
/// Other documents go to the app Windows has registered to print them.
pub fn print_saved_file(app: &AppHandle, path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err("That file is no longer on disk.".into());
    }
    if renders_for_print(path) {
        print_file_dialog(app, path)
    } else {
        files::print_document(path)
    }
}

fn print_file_dialog(app: &AppHandle, path: &Path) -> Result<(), String> {
    close_dialog_host(app);
    let url = Url::from_file_path(path)
        .map_err(|_| format!("{DIALOG_FAILURE} That file path can't be opened."))?;
    let webview = spawn_host(app, url, DIALOG_FAILURE)?;
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
    {
        std::thread::sleep(Duration::from_millis(400));
    }
    if let Ok(mut host) = DIALOG_HOST.lock() {
        *host = Some((webview.label().to_string(), None));
    }
    let shown = webview2::show_print_dialog(&webview);
    if shown.is_err() {
        close_dialog_host(app);
    }
    shown
}

fn renders_for_print(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref(),
        Some(
            "pdf"
                | "html"
                | "htm"
                | "xhtml"
                | "svg"
                | "xml"
                | "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "webp"
                | "bmp"
                | "txt"
                | "md"
                | "markdown"
                | "log"
                | "csv"
                | "json"
        )
    )
}

pub fn open_file(path: &str) -> Result<(), String> {
    files::open_with_default(Path::new(path))
}

pub fn show_file(path: &str) -> Result<(), String> {
    files::reveal_path(Path::new(path))
}

fn prepare_target(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{PRINT_FAILURE} Choose a full file path."));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("{PRINT_FAILURE} {error}"))?;
    }
    Ok(())
}

fn close_dialog_host(app: &AppHandle) {
    let previous = DIALOG_HOST.lock().ok().and_then(|mut host| host.take());
    if let Some((label, file)) = previous {
        if let Some(webview) = app.get_webview(&label) {
            let _ = webview.close();
        }
        if let Some(file) = file {
            let _ = std::fs::remove_file(file);
        }
    }
}

/// Loads `html` in a webview parked off the window and waits until it has finished loading.
fn open_host(app: &AppHandle, html: &str) -> Result<(Webview, PathBuf), String> {
    let folder = app
        .path()
        .app_cache_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("ce-browser"))
        .join("print");
    std::fs::create_dir_all(&folder).map_err(|error| format!("{PRINT_FAILURE} {error}"))?;
    let number = NEXT_HOST.fetch_add(1, Ordering::Relaxed);
    let file = folder.join(format!("print-{}-{number}.html", std::process::id()));
    std::fs::write(&file, html).map_err(|error| format!("{PRINT_FAILURE} {error}"))?;
    let url = Url::from_file_path(&file).map_err(|_| format!("{PRINT_FAILURE} Bad temp path."))?;
    match spawn_host(app, url, PRINT_FAILURE) {
        Ok(webview) => Ok((webview, file)),
        Err(error) => {
            let _ = std::fs::remove_file(&file);
            Err(error)
        }
    }
}

/// Loads `url` in a webview parked off the window and waits until it has finished loading.
fn spawn_host(app: &AppHandle, url: Url, failure: &str) -> Result<Webview, String> {
    let window = app
        .get_window(&tabs::focused_window(app))
        .or_else(|| app.get_window(tabs::WINDOW))
        .ok_or("The browser window is missing.")?;
    let number = NEXT_HOST.fetch_add(1, Ordering::Relaxed);
    let label = format!("print-host-{number}");
    let (tx, rx) = mpsc::channel::<()>();
    let loaded = Mutex::new(Some(tx));
    let builder = tabs::base_builder(app, &label, WebviewUrl::External(url))
        .focused(false)
        .on_page_load(move |_, payload| {
            if matches!(payload.event(), PageLoadEvent::Finished) {
                if let Some(tx) = loaded.lock().ok().and_then(|mut sender| sender.take()) {
                    let _ = tx.send(());
                }
            }
        });
    let webview = window
        .add_child(
            builder,
            LogicalPosition::new(-20000.0, -20000.0),
            LogicalSize::new(1024.0, 1400.0),
        )
        .map_err(|error| format!("{failure} {error}"))?;
    if rx.recv_timeout(Duration::from_secs(45)).is_err() {
        let _ = webview.close();
        return Err(format!("{failure} The page took too long to load."));
    }
    let _ = webview2::evaluate(&webview, SETTLE_SCRIPT, Duration::from_secs(15), failure);
    Ok(webview)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{facts_from_html, page_number_footer, page_number_header, renders_for_print};

    #[test]
    fn page_numbers_are_current_over_total_at_the_chosen_side() {
        let right = page_number_footer(false, "right", 0.5, 100.0);
        assert!(right.contains(r#"text-align:right"#));
        assert!(right.contains(r#"font-size:14.45px"#));
        assert!(right.contains(r#"padding:0 0.50in"#));
        assert!(
            right.contains(r#"<span class="pageNumber"></span>/<span class="totalPages"></span>"#)
        );
        assert!(!right.contains("url"));

        let center = page_number_footer(false, "center", 0.25, 0.0);
        assert!(center.contains(r#"text-align:center"#));
        assert!(center.contains(r#"padding:0 0.50in"#));
        assert!(center.contains(r#"font-size:14.45px"#));

        let wide = page_number_footer(false, "left", 1.0, 150.0);
        assert!(wide.contains(r#"text-align:left"#));
        assert!(wide.contains(r#"padding:0 1.00in"#));
        assert!(wide.contains(r#"font-size:21.68px"#));

        let left = page_number_footer(false, "nope", 0.5, 100.0);
        assert!(left.contains(r#"text-align:right"#));

        let with_url = page_number_footer(true, "left", 0.5, 100.0);
        assert!(with_url.contains(r#"text-align:left"#));
        assert!(with_url.contains(r#"padding-left:0.50in"#));
        assert!(with_url.contains(r#"padding-right:0.50in"#));
        assert!(with_url.find(r#"pageNumber"#).unwrap() < with_url.find(r#"class="url""#).unwrap());
        let header = page_number_header(false);
        assert!(!header.contains("title"));
        assert!(page_number_header(true).contains(r#"class="title""#));
    }

    #[test]
    fn page_facts_read_the_same_fields_as_a_live_extract() {
        let html = r#"
            <html lang="en">
              <head>
                <title>Why the sky is blue | Example Times</title>
                <meta property="og:title" content="Why the sky is blue">
                <meta name="author" content="Ada &amp; Lovelace">
                <meta property="og:site_name" content="Example Times">
                <meta property="article:published_time" content="2026-03-14T15:00:00Z">
              </head>
            </html>
        "#;
        let facts = facts_from_html("https://www.example.com/science/why-the-sky-is-blue", html);
        assert_eq!(facts.title, "Why the sky is blue");
        assert_eq!(facts.byline, "Ada & Lovelace");
        assert_eq!(facts.site_name, "Example Times");
        assert_eq!(facts.published, "2026-03-14T15:00:00Z");
        assert_eq!(facts.lang, "en");
        assert_eq!(
            facts.url,
            "https://www.example.com/science/why-the-sky-is-blue"
        );
    }

    #[test]
    fn files_the_browser_can_show_print_in_the_dialog() {
        assert!(renders_for_print(Path::new("notes.PDF")));
        assert!(renders_for_print(Path::new(r"C:\Downloads\photo.png")));
        assert!(renders_for_print(Path::new("page.html")));
        assert!(!renders_for_print(Path::new("report.docx")));
        assert!(!renders_for_print(Path::new("archive.zip")));
    }

    #[test]
    fn page_facts_fall_back_to_the_title_and_the_host() {
        let facts = facts_from_html(
            "https://www.example.com/news/story",
            "<html><head><title>Hello &amp; co</title><meta property=\"article:author\" content=\"Grace Hopper\"><meta name=\"date\" content=\"2024-01-02\"></head></html>",
        );
        assert_eq!(facts.title, "Hello & co");
        assert_eq!(facts.byline, "Grace Hopper");
        assert_eq!(facts.published, "2024-01-02");
        assert_eq!(facts.site_name, "www.example.com");
        assert_eq!(facts.lang, "");
    }
}
