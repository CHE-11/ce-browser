//! Media on a page, the media hub, and saving a page as HTML or Markdown.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{tabs, webview2};

const FAILURE: &str = "Couldn't read the media on this page.";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaElement {
    pub tab_id: u32,
    pub index: u32,
    pub kind: String,
    pub label: String,
    pub current: f64,
    pub duration: f64,
    pub paused: bool,
    pub muted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawElement {
    index: u32,
    kind: String,
    label: String,
    current: f64,
    duration: f64,
    paused: bool,
    muted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageMedia {
    pub url: String,
    pub kind: String,
    pub width: u32,
    pub height: u32,
}

const LIST_SCRIPT: &str = r#"(function () {
  return Array.from(document.querySelectorAll("video, audio")).map(function (el, index) {
    var label = el.currentSrc || el.src || el.tagName;
    return {
      index: index,
      kind: el.tagName.toLowerCase(),
      label: String(label).slice(0, 180),
      current: Number.isFinite(el.currentTime) ? el.currentTime : 0,
      duration: Number.isFinite(el.duration) ? el.duration : 0,
      paused: el.paused,
      muted: el.muted
    };
  });
})()"#;

const SOURCES_SCRIPT: &str = r#"(function () {
  var items = [];
  document.querySelectorAll("img").forEach(function (img) {
    if (!img.src || img.naturalWidth < 48) return;
    items.push({ url: img.src, kind: "image", width: img.naturalWidth || 0, height: img.naturalHeight || 0 });
  });
  document.querySelectorAll("video, audio").forEach(function (el) {
    var src = el.currentSrc || el.src;
    if (!src) return;
    items.push({ url: src, kind: el.tagName.toLowerCase(), width: el.videoWidth || 0, height: el.videoHeight || 0 });
  });
  var seen = {};
  return items.filter(function (item) {
    if (seen[item.url]) return false;
    seen[item.url] = true;
    return true;
  }).slice(0, 40);
})()"#;

const PAGE_SCRIPT: &str = r#"(async function () {
  var css = "";
  Array.from(document.styleSheets).forEach(function (sheet) {
    try {
      css += Array.from(sheet.cssRules).map(function (rule) { return rule.cssText; }).join("\n") + "\n";
    } catch (error) {}
  });
  var root = document.documentElement.cloneNode(true);
  var live = Array.from(document.images);
  var copies = Array.from(root.querySelectorAll("img"));
  var limit = Math.min(live.length, copies.length, 30);
  for (var i = 0; i < limit; i++) {
    try {
      if (!live[i].naturalWidth || !live[i].naturalHeight) continue;
      var canvas = document.createElement("canvas");
      canvas.width = live[i].naturalWidth;
      canvas.height = live[i].naturalHeight;
      canvas.getContext("2d").drawImage(live[i], 0, 0);
      var data = canvas.toDataURL("image/png");
      if (data.length < 1500000) copies[i].setAttribute("src", data);
    } catch (error) {}
  }
  return { title: document.title || "", html: root.outerHTML, css: css.slice(0, 500000) };
})()"#;

pub fn list_playing(app: &AppHandle) -> Vec<MediaElement> {
    let tabs = tabs::snapshot(app)
        .tabs
        .into_iter()
        .filter(|tab| {
            tab.loaded && (tab.url.starts_with("http://") || tab.url.starts_with("https://"))
        })
        .map(|tab| tab.id)
        .collect::<Vec<_>>();
    let mut found = Vec::new();
    for id in tabs {
        let Some(webview) = tabs::page_view(app, id) else {
            continue;
        };
        let Ok(value) = webview2::evaluate(&webview, LIST_SCRIPT, Duration::from_secs(3), FAILURE)
        else {
            continue;
        };
        let Ok(items) = serde_json::from_value::<Vec<RawElement>>(value) else {
            continue;
        };
        if items.is_empty() {
            continue;
        }
        for item in items {
            found.push(MediaElement {
                tab_id: id,
                index: item.index,
                kind: item.kind,
                label: item.label,
                current: item.current,
                duration: item.duration,
                paused: item.paused,
                muted: item.muted,
            });
        }
    }
    found
}

pub fn control(
    app: &AppHandle,
    tab: u32,
    index: u32,
    action: &str,
    time: Option<f64>,
) -> Result<(), String> {
    let webview = tabs::page_view(app, tab).ok_or("That tab is not loaded.")?;
    let time = time.unwrap_or(0.0);
    let script = format!(
        r#"(function () {{
          var el = document.querySelectorAll("video, audio")[{index}];
          if (!el) return false;
          if ({action:?} === "play") el.play();
          else if ({action:?} === "pause") el.pause();
          else if ({action:?} === "seek") el.currentTime = {time};
          return true;
        }})()"#
    );
    let value = webview2::evaluate(&webview, &script, Duration::from_secs(3), FAILURE)?;
    if value.as_bool() == Some(true) {
        Ok(())
    } else {
        Err("That media is gone.".into())
    }
}

pub fn page_media(app: &AppHandle, id: u32) -> Result<Vec<PageMedia>, String> {
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let value = webview2::evaluate(&webview, SOURCES_SCRIPT, Duration::from_secs(8), FAILURE)?;
    serde_json::from_value(value).map_err(|error| format!("{FAILURE} {error}"))
}

pub fn download_media(app: &AppHandle, id: u32, urls: &[String]) -> Result<u32, String> {
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let mut started = 0u32;
    for url in urls.iter().take(40) {
        let script = format!(
            r#"(function () {{
              var link = document.createElement("a");
              link.href = {url};
              link.download = "";
              document.body.appendChild(link);
              link.click();
              link.remove();
              return true;
            }})()"#,
            url = serde_json::to_string(url).unwrap_or_else(|_| "\"\"".into()),
        );
        if webview2::evaluate(&webview, &script, Duration::from_secs(4), FAILURE).is_ok() {
            started += 1;
            std::thread::sleep(Duration::from_millis(150));
        }
    }
    Ok(started)
}

pub fn save_html(app: &AppHandle, id: u32, path: &std::path::Path) -> Result<(), String> {
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let value = webview2::evaluate(
        &webview,
        PAGE_SCRIPT,
        Duration::from_secs(15),
        "Couldn't read this page.",
    )?;
    let title = value
        .get("title")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    let html = value
        .get("html")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    let css = value
        .get("css")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    let document = inline_style(title, css, html);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, document).map_err(|error| format!("Couldn't save the page. {error}"))
}

pub fn save_markdown(app: &AppHandle, id: u32, path: &std::path::Path) -> Result<(), String> {
    let page = crate::print::extract_page(app, id)?;
    let markdown = html_to_markdown(&page.title, &page.html);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, markdown).map_err(|error| format!("Couldn't save the page. {error}"))
}

fn inline_style(title: &str, css: &str, html: &str) -> String {
    let style = format!("<style>\n{css}\n</style>");
    let with_style = if let Some(index) = html.find("<head>") {
        let at = index + "<head>".len();
        format!("{}{style}{}", &html[..at], &html[at..])
    } else {
        format!(
            "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><title>{}</title>{style}</head><body>{html}</body></html>",
            escape(title)
        )
    };
    with_style
}

/// A small walk from tags to Markdown. Layout is not preserved.
pub fn html_to_markdown(title: &str, html: &str) -> String {
    let mut out = String::new();
    if !title.trim().is_empty() {
        out.push_str("# ");
        out.push_str(title.trim());
        out.push_str("\n\n");
    }
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        let text = rest[..start].trim();
        if !text.is_empty() && text.len() > 1 {
            // Text between tags is kept only when we are inside a block we already opened.
        }
        let Some(end) = rest[start..].find('>') else {
            break;
        };
        let tag = &rest[start + 1..start + end];
        rest = &rest[start + end + 1..];
        let name = tag
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches('/')
            .to_ascii_lowercase();
        let closing = tag.starts_with('/');
        if matches!(name.as_str(), "script" | "style") && !closing {
            if let Some(close) = rest.to_ascii_lowercase().find(&format!("</{name}>")) {
                rest = &rest[close + name.len() + 3..];
            }
            continue;
        }
        if closing {
            continue;
        }
        match name.as_str() {
            "h1" | "h2" | "h3" | "p" | "li" => {
                let body = text_until(rest);
                if body.is_empty() {
                    continue;
                }
                let prefix = match name.as_str() {
                    "h1" => "# ",
                    "h2" => "## ",
                    "h3" => "### ",
                    "li" => "- ",
                    _ => "",
                };
                out.push_str(prefix);
                out.push_str(&body);
                out.push_str("\n\n");
            }
            "a" => {
                if let Some(href) = attr(tag, "href") {
                    let body = text_until(rest);
                    if !body.is_empty() {
                        out.push('[');
                        out.push_str(&body);
                        out.push_str("](");
                        out.push_str(&href);
                        out.push_str(")\n\n");
                    }
                }
            }
            "img" => {
                if let Some(src) = attr(tag, "src") {
                    let alt = attr(tag, "alt").unwrap_or_else(|| "image".into());
                    out.push_str(&format!("![{alt}]({src})\n\n"));
                }
            }
            _ => {}
        }
    }
    out
}

fn text_until(html: &str) -> String {
    let end = html.find('<').unwrap_or(html.len());
    decode(html[..end].trim())
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(decode(&tag[start..end]))
}

fn decode(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::html_to_markdown;

    #[test]
    fn markdown_keeps_the_title_link_and_image() {
        let html = r#"<h1>Hello</h1><p>A paragraph.</p><a href="https://example.com">Example</a><img alt="Shot" src="https://example.com/a.png"><script>alert(1)</script>"#;
        let markdown = html_to_markdown("Notes", html);
        assert!(markdown.starts_with("# Notes"));
        assert!(markdown.contains("# Hello"));
        assert!(markdown.contains("A paragraph."));
        assert!(markdown.contains("[Example](https://example.com)"));
        assert!(markdown.contains("![Shot](https://example.com/a.png)"));
        assert!(!markdown.contains("alert"));
    }
}
