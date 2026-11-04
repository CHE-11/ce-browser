//! Print to PDF: reads a tab's document for the editor, and prints edited HTML or a tab to PDF.

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
/// The webview behind an open print dialog. Closed when the next print starts.
static DIALOG_HOST: Mutex<Option<(String, PathBuf)>> = Mutex::new(None);

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
            scale: 1.0,
            title: String::new(),
            url: String::new(),
        }
    }
}

impl PdfOptions {
    fn layout(&self) -> webview2::PdfLayout {
        webview2::PdfLayout {
            page_width: self.page_width.clamp(1.0, 50.0),
            page_height: self.page_height.clamp(1.0, 50.0),
            landscape: self.landscape,
            margin: self.margin.clamp(0.0, 3.0),
            backgrounds: self.backgrounds,
            header_footer: self.header_footer,
            title: self.title.clone(),
            url: self.url.clone(),
            scale: if self.scale > 0.0 { self.scale } else { 1.0 },
        }
    }
}

/// Copies the live document into standalone HTML: no scripts, absolute URLs, lazy images
/// resolved, and styles that only exist in the CSSOM written out as text.
const EXTRACT_SCRIPT: &str = r##"(() => {
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
) -> Result<(), String> {
    prepare_target(path)?;
    let (webview, file) = open_host(app, html)?;
    let result = webview2::print_to_pdf(&webview, path, options.layout());
    let _ = webview.close();
    let _ = std::fs::remove_file(file);
    if result.is_ok() {
        crate::downloads::record_completed(app, &options.url, path, "application/pdf");
    }
    result
}

pub fn print_html_dialog(app: &AppHandle, html: &str) -> Result<(), String> {
    close_dialog_host(app);
    let (webview, file) = open_host(app, html)?;
    if let Ok(mut host) = DIALOG_HOST.lock() {
        *host = Some((webview.label().to_string(), file));
    }
    webview2::show_print_dialog(&webview)
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
        let _ = std::fs::remove_file(file);
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

    let window = app
        .get_window(&tabs::focused_window(app))
        .or_else(|| app.get_window(tabs::WINDOW))
        .ok_or("The browser window is missing.")?;
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
        .map_err(|error| {
            let _ = std::fs::remove_file(&file);
            format!("{PRINT_FAILURE} {error}")
        })?;
    if rx.recv_timeout(Duration::from_secs(45)).is_err() {
        let _ = webview.close();
        let _ = std::fs::remove_file(&file);
        return Err(format!("{PRINT_FAILURE} The page took too long to load."));
    }
    let _ = webview2::evaluate(
        &webview,
        SETTLE_SCRIPT,
        Duration::from_secs(15),
        PRINT_FAILURE,
    );
    Ok((webview, file))
}
