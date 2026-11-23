//! Pins saved from a page: screenshots, sections, link cards and images.

use std::collections::HashSet;
use std::time::Duration;

use base64::Engine;
use image::RgbaImage;
use tauri::AppHandle;

use super::model::{Pin, TEXT_LIMIT, TITLE_LIMIT, clip_text, new_id};
use super::{change, snapshot, storage};
use crate::{capture, page_menu, tabs, undo, webview2};

/// Images saved by one "Images on this page" request.
const MAX_IMAGES: usize = 60;
const PICK_FAILURE: &str = "Couldn't read this page.";
/// Largest image read from a page. Animated GIFs are often several megabytes.
const MAX_SAVED_IMAGE_BYTES: u64 = 64 * 1024 * 1024;

/// A pin with its text fields filled in and nothing stored yet.
pub(super) fn draft(kind: &str, title: &str, page_url: &str, link_url: &str, text: &str) -> Pin {
    Pin {
        id: new_id("p"),
        kind: kind.into(),
        title: clip_text(title, TITLE_LIMIT),
        page_url: page_url.into(),
        link_url: link_url.into(),
        text: clip_text(text, TEXT_LIMIT),
        created_at: tabs::now_ms(),
        ..Pin::default()
    }
}

/// Puts the pin in its folder and saves it. Removes its files when that fails.
pub(super) fn commit(app: &AppHandle, folder: &str, pin: Pin) -> Result<Pin, String> {
    let id = pin.id.clone();
    let ext = pin.ext.clone();
    let result = change(app, |boards| {
        let mut pin = pin;
        pin.folder_ids = boards.target_for_save(folder)?.into_iter().collect();
        boards.add_pins(vec![pin.clone()]);
        Ok(pin)
    });
    if result.is_err() {
        storage::remove_new_files(app, &id, &ext);
    }
    result
}

/// Stores a capture as PNG with its thumbnail and palette.
fn save_capture(
    app: &AppHandle,
    folder: &str,
    mut pin: Pin,
    image: &RgbaImage,
) -> Result<Pin, String> {
    let stored = (|| {
        let png = storage::write_capture(app, &pin.id, image)?;
        pin.palette = storage::write_thumb(app, &pin.id, image)?;
        pin.size = png.len() as u64;
        pin.hash = storage::hash_bytes(&png);
        Ok::<(), String>(())
    })();
    if let Err(error) = stored {
        storage::remove_new_files(app, &pin.id, "png");
        return Err(error);
    }
    pin.ext = "png".into();
    pin.mime = "image/png".into();
    pin.has_image = true;
    pin.width = image.width();
    pin.height = image.height();
    commit(app, folder, pin)
}

/// Stores downloaded image bytes as they are, with a thumbnail.
fn save_image_bytes(
    app: &AppHandle,
    folder: &str,
    mut pin: Pin,
    bytes: &[u8],
) -> Result<Pin, String> {
    let ext = storage::image_ext(bytes).ok_or("That image format isn't supported.")?;
    let image = storage::decode_image(bytes)?;
    let stored = (|| {
        storage::write_original(app, &pin.id, ext, bytes)?;
        pin.palette = storage::write_thumb(app, &pin.id, &image)?;
        Ok::<(), String>(())
    })();
    if let Err(error) = stored {
        storage::remove_new_files(app, &pin.id, ext);
        return Err(error);
    }
    pin.ext = ext.into();
    pin.mime = storage::mime_for_ext(ext).into();
    pin.has_image = true;
    pin.width = image.width();
    pin.height = image.height();
    pin.size = bytes.len() as u64;
    pin.hash = storage::hash_bytes(bytes);
    commit(app, folder, pin)
}

/// Title and address of tab `id`.
fn tab_page(app: &AppHandle, id: u32) -> (String, String) {
    tabs::snapshot(app)
        .tabs
        .into_iter()
        .find(|tab| tab.id == id)
        .map(|tab| (tab.title, tab.url))
        .unwrap_or_default()
}

fn text_of(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

pub fn pin_page(app: &AppHandle, id: u32, folder: &str, full_page: bool) -> Result<Pin, String> {
    let (title, url) = tab_page(app, id);
    let (image, _) = capture::page_image(app, id, full_page, full_page)?;
    save_capture(app, folder, draft("page", &title, &url, &url, ""), &image)
}

pub fn pin_region(app: &AppHandle, id: u32, folder: &str) -> Result<Pin, String> {
    let (title, url) = tab_page(app, id);
    let image = capture::region_image(app, id)?;
    save_capture(app, folder, draft("region", &title, &url, &url, ""), &image)
}

pub fn pin_element(app: &AppHandle, id: u32, folder: &str) -> Result<Pin, String> {
    let (title, url) = tab_page(app, id);
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let value = webview2::evaluate(
        &webview,
        ELEMENT_SCRIPT,
        Duration::from_secs(150),
        PICK_FAILURE,
    )?;
    let number = |key: &str| value.get(key).and_then(|item| item.as_f64());
    let (Some(x), Some(y), Some(width), Some(height)) =
        (number("x"), number("y"), number("width"), number("height"))
    else {
        return Err("The capture was cancelled.".into());
    };
    if width < 1.0 || height < 1.0 {
        return Err("That section has no size.".into());
    }
    let image = capture::clip_image(app, id, x, y, width, height)?;
    let heading = text_of(&value, "heading");
    let href = text_of(&value, "href");
    let pin = draft(
        "element",
        if heading.is_empty() { &title } else { &heading },
        &url,
        if href.is_empty() { &url } else { &href },
        &text_of(&value, "text"),
    );
    save_capture(app, folder, pin, &image)
}

pub fn pin_link(app: &AppHandle, id: u32, folder: &str) -> Result<Pin, String> {
    let (tab_title, url) = tab_page(app, id);
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let value = webview2::evaluate(&webview, LINK_SCRIPT, Duration::from_secs(10), PICK_FAILURE)?;
    let preview = text_of(&value, "image");
    let image = (!preview.is_empty())
        .then(|| {
            page_menu::image_bytes(&preview)
                .and_then(|bytes| storage::decode_image(&bytes))
                .ok()
        })
        .flatten();
    let image = match image {
        Some(image) => image,
        None => capture::page_image(app, id, false, false)?.0,
    };
    let title = text_of(&value, "title");
    let link = text_of(&value, "url");
    let pin = draft(
        "link",
        if title.is_empty() { &tab_title } else { &title },
        &url,
        if link.is_empty() { &url } else { &link },
        &text_of(&value, "description"),
    );
    save_capture(app, folder, pin, &image)
}

/// Downloads each image and pins it as it is. Fails only when none could be saved.
pub fn pin_images(
    app: &AppHandle,
    id: u32,
    folder: &str,
    urls: &[String],
) -> Result<Vec<Pin>, String> {
    let (title, url) = tab_page(app, id);
    let mut seen = HashSet::new();
    let mut target = folder.to_string();
    let mut pins = Vec::new();
    let mut first_error = None;
    for source in urls
        .iter()
        .map(|source| source.trim())
        .filter(|source| !source.is_empty() && seen.insert(source.to_string()))
        .take(MAX_IMAGES)
    {
        let saved = tab_image_bytes(app, id, source).and_then(|bytes| {
            let mut pin = draft("image", &title, &url, source, "");
            pin.file_name = file_name_from_url(source);
            save_image_bytes(app, &target, pin, &bytes)
        });
        match saved {
            Ok(pin) => {
                target = pin
                    .folder_ids
                    .first()
                    .cloned()
                    .unwrap_or_else(|| super::model::UNCATEGORIZED.to_string());
                pins.push(pin);
            }
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    if pins.is_empty() {
        return Err(first_error.unwrap_or_else(|| "Choose at least one image.".into()));
    }
    Ok(pins)
}

/// Image bytes as the tab has them: what the page already loaded, then the page's own fetch
/// through its HTTP cache. A direct download is the last resort, since CDNs often refuse
/// requests without the page's cookies and referrer.
fn tab_image_bytes(app: &AppHandle, id: u32, source: &str) -> Result<Vec<u8>, String> {
    if source.starts_with("data:") {
        return page_menu::image_bytes(source);
    }
    if let Some(webview) = tabs::page_view(app, id) {
        if let Some(bytes) =
            loaded_resource(&webview, source).or_else(|| page_fetch(&webview, source))
        {
            return Ok(bytes);
        }
    }
    page_menu::image_bytes(source)
}

/// The resource's bytes from the page's memory cache. Makes no network request.
fn loaded_resource(webview: &tauri::Webview, source: &str) -> Option<Vec<u8>> {
    let tree = webview2::call_cdp_json(
        webview,
        "Page.getResourceTree",
        "{}",
        Duration::from_secs(5),
        PICK_FAILURE,
    )
    .ok()?;
    let root = tree.get("frameTree")?;
    let wanted = without_fragment(source);
    let mut frames = Vec::new();
    resource_frames(root, &wanted, &mut frames);
    if frames.is_empty() {
        frames.push(root.pointer("/frame/id")?.as_str()?.to_string());
    }
    frames.into_iter().find_map(|frame_id| {
        let params = serde_json::json!({ "frameId": frame_id, "url": source }).to_string();
        let value = webview2::call_cdp_json(
            webview,
            "Page.getResourceContent",
            &params,
            Duration::from_secs(10),
            PICK_FAILURE,
        )
        .ok()?;
        let content = value.get("content")?.as_str()?;
        let bytes = if value
            .get("base64Encoded")
            .and_then(|item| item.as_bool())
            .unwrap_or(false)
        {
            base64::engine::general_purpose::STANDARD
                .decode(content)
                .ok()?
        } else {
            content.as_bytes().to_vec()
        };
        (!bytes.is_empty()
            && bytes.len() as u64 <= MAX_SAVED_IMAGE_BYTES
            && storage::image_ext(&bytes).is_some())
        .then_some(bytes)
    })
}

/// Ids of the frames whose resources include `wanted`.
fn resource_frames(node: &serde_json::Value, wanted: &str, out: &mut Vec<String>) {
    let frame_id = node
        .pointer("/frame/id")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    let listed = node
        .get("resources")
        .and_then(|item| item.as_array())
        .is_some_and(|resources| {
            resources
                .iter()
                .filter_map(|resource| resource.get("url").and_then(|item| item.as_str()))
                .any(|url| without_fragment(url) == wanted)
        });
    if listed && !frame_id.is_empty() {
        out.push(frame_id.to_string());
    }
    for child in node
        .get("childFrames")
        .and_then(|item| item.as_array())
        .into_iter()
        .flatten()
    {
        resource_frames(child, wanted, out);
    }
}

fn without_fragment(url: &str) -> &str {
    url.split_once('#').map_or(url, |(base, _)| base)
}

/// The page fetches the image itself, so its cache, cookies and referrer apply.
fn page_fetch(webview: &tauri::Webview, source: &str) -> Option<Vec<u8>> {
    let script = PAGE_FETCH_SCRIPT
        .replace("__SOURCE__", &serde_json::to_string(source).ok()?)
        .replace("__MAX__", &MAX_SAVED_IMAGE_BYTES.to_string());
    let value = webview2::evaluate(webview, &script, Duration::from_secs(30), PICK_FAILURE).ok()?;
    let data_url = value.as_str().filter(|text| text.starts_with("data:"))?;
    let bytes = page_menu::image_bytes(data_url).ok()?;
    storage::image_ext(&bytes).is_some().then_some(bytes)
}

const PAGE_FETCH_SCRIPT: &str = r#"(async function () {
  try {
    var response = await fetch(__SOURCE__, { cache: "force-cache" });
    if (!response.ok) return "";
    var blob = await response.blob();
    if (!blob.size || blob.size > __MAX__) return "";
    return await new Promise(function (resolve) {
      var reader = new FileReader();
      reader.onload = function () { resolve(String(reader.result)); };
      reader.onerror = function () { resolve(""); };
      reader.readAsDataURL(blob);
    });
  } catch (error) {
    return "";
  }
})()"#;

fn file_name_from_url(source: &str) -> String {
    if source.starts_with("data:") {
        return String::new();
    }
    url::Url::parse(source)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut segments| segments.next_back().map(str::to_string))
        })
        .map(|name| clip_text(&name, super::model::FILE_NAME_LIMIT))
        .unwrap_or_default()
}

/// "Save image to board" in the page menu. Saves to the last folder used. Runs on a worker thread.
pub fn save_menu_image(app: &AppHandle, id: u32, source: &str) {
    let window = tabs::tab_window_label(app, id).unwrap_or_else(|| tabs::WINDOW.to_string());
    let message = match pin_images(app, id, "", &[source.to_string()]) {
        Ok(pins) => {
            let boards = snapshot(app);
            let place = pins
                .first()
                .map(|pin| boards.place_name(&pin.folder_ids))
                .unwrap_or_else(|| "Uncategorized".into());
            format!("Saved to {place}")
        }
        Err(error) => error,
    };
    undo::notice(app, &window, &message);
}

/// Outlines the element under the mouse. Click picks it, Alt+wheel or `[` / `]` picks the
/// parent or child, Esc cancels. Resolves with the element's rect in document CSS pixels.
const ELEMENT_SCRIPT: &str = r##"(function () {
  return new Promise(function (resolve) {
    var root = document.documentElement;
    var font = "font:12px/1.4 'Segoe UI',system-ui,sans-serif;";
    var box = document.createElement("div");
    box.style.cssText = "position:fixed;z-index:2147483647;pointer-events:none;display:none;border:2px solid #38bdf8;background:rgba(56,189,248,.18);border-radius:3px;box-sizing:border-box";
    var label = document.createElement("div");
    label.style.cssText = "position:fixed;z-index:2147483647;pointer-events:none;display:none;background:#0c4a6e;color:#e0f2fe;padding:2px 6px;border-radius:4px;white-space:nowrap;" + font;
    var hint = document.createElement("div");
    hint.textContent = "Click a section to save it \u00b7 Alt+scroll or [ ] to grow or shrink \u00b7 Esc to cancel";
    hint.style.cssText = "position:fixed;left:50%;top:12px;transform:translateX(-50%);z-index:2147483647;pointer-events:none;background:rgba(9,9,11,.92);color:#fafafa;padding:6px 12px;border-radius:8px;box-shadow:0 4px 16px rgba(0,0,0,.4);" + font;
    root.appendChild(box);
    root.appendChild(label);
    root.appendChild(hint);
    var previousCursor = root.style.cursor;
    root.style.cursor = "crosshair";
    var hovered = null;
    var current = null;
    var children = [];
    var timer = setTimeout(function () { finish(null); }, 140000);

    function show(element) {
      current = element;
      if (!element) {
        box.style.display = "none";
        label.style.display = "none";
        return;
      }
      var rect = element.getBoundingClientRect();
      box.style.display = "block";
      box.style.left = rect.left + "px";
      box.style.top = rect.top + "px";
      box.style.width = rect.width + "px";
      box.style.height = rect.height + "px";
      var name = element.tagName.toLowerCase() + (element.id ? "#" + element.id : "");
      label.textContent = name + "  " + Math.round(rect.width) + " \u00d7 " + Math.round(rect.height);
      label.style.display = "block";
      label.style.left = Math.max(0, rect.left) + "px";
      label.style.top = (rect.top > 24 ? rect.top - 22 : rect.top + 2) + "px";
    }
    function grow() {
      if (current && current.parentElement && current.parentElement !== root) {
        children.push(current);
        show(current.parentElement);
      }
    }
    function shrink() {
      if (children.length) show(children.pop());
    }
    function onMove(event) {
      var element = document.elementFromPoint(event.clientX, event.clientY);
      if (!element || element === root) return;
      if (element !== hovered) {
        hovered = element;
        children = [];
        show(element);
      }
    }
    function onWheel(event) {
      if (!event.altKey) return;
      event.preventDefault();
      event.stopPropagation();
      if (event.deltaY < 0) grow();
      else shrink();
    }
    function block(event) {
      event.preventDefault();
      event.stopPropagation();
    }
    function onClick(event) {
      block(event);
      if (!current) return;
      var element = current;
      var rect = element.getBoundingClientRect();
      var heading = element.matches("h1,h2,h3,h4") ? element : element.querySelector("h1,h2,h3,h4");
      var link = element.closest("a[href]") || element.querySelector("a[href]");
      finish({
        x: rect.left + window.scrollX,
        y: rect.top + window.scrollY,
        width: rect.width,
        height: rect.height,
        text: (element.innerText || "").replace(/\n{3,}/g, "\n\n").trim().slice(0, 2000),
        heading: heading ? (heading.innerText || "").trim().slice(0, 300) : "",
        href: link ? link.href : ""
      });
    }
    function onKey(event) {
      if (event.key === "Escape") {
        block(event);
        finish(null);
      } else if (event.key === "[" || event.key === "ArrowUp") {
        block(event);
        grow();
      } else if (event.key === "]" || event.key === "ArrowDown") {
        block(event);
        shrink();
      }
    }
    function onScroll() {
      show(current);
    }
    function finish(value) {
      clearTimeout(timer);
      document.removeEventListener("mousemove", onMove, true);
      document.removeEventListener("wheel", onWheel, { capture: true });
      document.removeEventListener("mousedown", block, true);
      document.removeEventListener("mouseup", block, true);
      document.removeEventListener("click", onClick, true);
      document.removeEventListener("keydown", onKey, true);
      window.removeEventListener("scroll", onScroll, true);
      box.remove();
      label.remove();
      hint.remove();
      root.style.cursor = previousCursor;
      requestAnimationFrame(function () {
        requestAnimationFrame(function () {
          setTimeout(function () { resolve(value); }, 40);
        });
      });
    }
    document.addEventListener("mousemove", onMove, true);
    document.addEventListener("wheel", onWheel, { capture: true, passive: false });
    document.addEventListener("mousedown", block, true);
    document.addEventListener("mouseup", block, true);
    document.addEventListener("click", onClick, true);
    document.addEventListener("keydown", onKey, true);
    window.addEventListener("scroll", onScroll, true);
  });
})()"##;

/// Title, description, preview image and canonical address for a link card.
const LINK_SCRIPT: &str = r#"(function () {
  function meta(selector) {
    var element = document.querySelector(selector);
    return element ? (element.getAttribute("content") || "").trim() : "";
  }
  function absolute(value) {
    try { return value ? new URL(value, location.href).href : ""; } catch (error) { return ""; }
  }
  var canonical = document.querySelector('link[rel="canonical"]');
  var image = meta('meta[property="og:image"]') || meta('meta[property="og:image:url"]') ||
    meta('meta[name="twitter:image"]') || meta('meta[name="twitter:image:src"]');
  return {
    title: meta('meta[property="og:title"]') || meta('meta[name="twitter:title"]') || document.title || "",
    description: meta('meta[property="og:description"]') || meta('meta[name="description"]') || meta('meta[name="twitter:description"]'),
    image: absolute(image),
    url: absolute(canonical ? canonical.getAttribute("href") : "") || location.href
  };
})()"#;
