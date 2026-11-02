//! Page to PNG. What is on screen is the webview's own surface. A taller or wider area is
//! scrolled through that surface and stitched. A clip past the widget comes back blank or
//! cropped on WebView2, including on a scaled display.

use std::io::Cursor;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use image::{ImageFormat, RgbaImage};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Webview};

use crate::{tabs, webview2};

const FAILURE: &str = "Couldn't capture this page.";
/// Longer pages are cut off here. Past this the image is too large for most viewers.
const MAX_CSS_HEIGHT: f64 = 60000.0;
/// Pixel budget for the stitched image. Bigger pages are captured at a lower scale.
const MAX_PIXELS: f64 = 120_000_000.0;
/// How many viewport shots one capture may take. Stops a scroll that never moves forward.
const MAX_SHOTS: usize = 400;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageCaptureResult {
    pub path: Option<String>,
    pub width: u32,
    pub height: u32,
    /// The page was taller than the capture limit, so the image stops early.
    pub truncated: bool,
}

struct Capture {
    image: RgbaImage,
    truncated: bool,
}

const LOAD_LAZY_SCRIPT: &str = r#"(async () => {
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const root = document.scrollingElement || document.documentElement;
  const startX = window.scrollX;
  const startY = window.scrollY;
  const step = Math.max(200, Math.floor(window.innerHeight * 0.85));
  const deadline = Date.now() + 15000;
  const jump = (top) => {
    try { window.scrollTo({ left: 0, top, behavior: "instant" }); }
    catch (error) { window.scrollTo(0, top); }
  };
  for (let y = 0; y < root.scrollHeight && y < 60000 && Date.now() < deadline; y += step) {
    jump(y);
    await sleep(120);
  }
  jump(root.scrollHeight);
  await sleep(250);
  const pending = Array.from(document.images).filter((image) => !image.complete);
  await Promise.race([
    Promise.all(pending.map((image) => new Promise((resolve) => {
      image.addEventListener("load", resolve, { once: true });
      image.addEventListener("error", resolve, { once: true });
    }))),
    sleep(3000),
  ]);
  try { window.scrollTo({ left: startX, top: startY, behavior: "instant" }); }
  catch (error) { window.scrollTo(startX, startY); }
  await sleep(200);
  return true;
})()"#;

pub fn save_png(
    app: &AppHandle,
    id: u32,
    full_page: bool,
    load_lazy: bool,
    path: &Path,
) -> Result<PageCaptureResult, String> {
    let capture = capture(app, id, full_page, load_lazy)?;
    let bytes = encode_png(&capture.image)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("{FAILURE} {error}"))?;
    }
    std::fs::write(path, bytes).map_err(|error| format!("Couldn't save the image. {error}"))?;
    Ok(PageCaptureResult {
        path: Some(path.to_string_lossy().into_owned()),
        width: capture.image.width(),
        height: capture.image.height(),
        truncated: capture.truncated,
    })
}

pub fn copy_png(
    app: &AppHandle,
    id: u32,
    full_page: bool,
    load_lazy: bool,
) -> Result<PageCaptureResult, String> {
    let capture = capture(app, id, full_page, load_lazy)?;
    let png = encode_png(&capture.image)?;
    clipboard::write_image(&capture.image, &png)?;
    Ok(PageCaptureResult {
        path: None,
        width: capture.image.width(),
        height: capture.image.height(),
        truncated: capture.truncated,
    })
}

/// The whole page or the visible area as an image, and whether a long page was cut off.
pub fn page_image(
    app: &AppHandle,
    id: u32,
    full_page: bool,
    load_lazy: bool,
) -> Result<(RgbaImage, bool), String> {
    let capture = capture(app, id, full_page, load_lazy)?;
    Ok((capture.image, capture.truncated))
}

pub fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|error| format!("Couldn't encode the image. {error}"))?;
    Ok(bytes)
}

fn capture(app: &AppHandle, id: u32, full_page: bool, load_lazy: bool) -> Result<Capture, String> {
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    if full_page && load_lazy {
        // Lazy loading is a nicety. A page that blocks the script is still captured.
        let _ = webview2::evaluate(&webview, LOAD_LAZY_SCRIPT, Duration::from_secs(30), FAILURE);
    }
    if !full_page {
        return Ok(Capture {
            image: viewport_image(&webview)?,
            truncated: false,
        });
    }
    let page = page_box(&webview)?;
    let width = page.page_width.max(page.width).min(page.width * 4.0);
    let truncated = page.page_height > MAX_CSS_HEIGHT;
    let height = page.page_height.min(MAX_CSS_HEIGHT).max(page.height);
    let (image, short) = capture_area(&webview, &page, 0.0, 0.0, width, height, true)?;
    Ok(Capture {
        image,
        truncated: truncated || short,
    })
}

struct PageBox {
    scroll_x: f64,
    scroll_y: f64,
    width: f64,
    height: f64,
    page_width: f64,
    page_height: f64,
}

struct View {
    scroll_x: f64,
    scroll_y: f64,
    width: f64,
    height: f64,
}

impl PageBox {
    fn view(&self) -> View {
        View {
            scroll_x: self.scroll_x,
            scroll_y: self.scroll_y,
            width: self.width,
            height: self.height,
        }
    }
}

struct PxCrop {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    doc_right: f64,
    doc_bottom: f64,
}

/// Where a document rectangle sits inside a viewport screenshot. None when it misses the view.
fn crop_pixels(
    image_width: u32,
    image_height: u32,
    view: &View,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Option<PxCrop> {
    if image_width == 0
        || image_height == 0
        || view.width < 1.0
        || view.height < 1.0
        || width < 0.5
        || height < 0.5
    {
        return None;
    }
    let left = x.max(view.scroll_x);
    let top = y.max(view.scroll_y);
    let right = (x + width).min(view.scroll_x + view.width);
    let bottom = (y + height).min(view.scroll_y + view.height);
    let css_width = right - left;
    let css_height = bottom - top;
    if css_width < 0.5 || css_height < 0.5 {
        return None;
    }
    let scale_x = f64::from(image_width) / view.width;
    let scale_y = f64::from(image_height) / view.height;
    let px = ((left - view.scroll_x) * scale_x).round().max(0.0) as u32;
    let py = ((top - view.scroll_y) * scale_y).round().max(0.0) as u32;
    if px >= image_width || py >= image_height {
        return None;
    }
    let pw = (css_width * scale_x)
        .round()
        .max(1.0)
        .min(f64::from(image_width - px)) as u32;
    let ph = (css_height * scale_y)
        .round()
        .max(1.0)
        .min(f64::from(image_height - py)) as u32;
    Some(PxCrop {
        x: px,
        y: py,
        width: pw,
        height: ph,
        doc_right: right,
        doc_bottom: bottom,
    })
}

fn fit_scale(pixels: f64) -> f64 {
    if pixels <= MAX_PIXELS || pixels <= 0.0 {
        1.0
    } else {
        (MAX_PIXELS / pixels).sqrt()
    }
}

fn field(value: &serde_json::Value, key: &str) -> f64 {
    value
        .get(key)
        .and_then(|item| item.as_f64())
        .filter(|item| item.is_finite())
        .unwrap_or(0.0)
}

fn js_num(value: f64) -> String {
    let value = if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    };
    format!("{value:.2}")
}

const PAGE_BOX_SCRIPT: &str = r#"(function () {
  var root = document.scrollingElement || document.documentElement;
  var body = document.body;
  return {
    scrollX: window.scrollX || 0,
    scrollY: window.scrollY || 0,
    width: window.innerWidth || 1,
    height: window.innerHeight || 1,
    pageWidth: Math.max(root.scrollWidth || 0, body ? body.scrollWidth : 0, window.innerWidth || 1),
    pageHeight: Math.max(root.scrollHeight || 0, body ? body.scrollHeight : 0, window.innerHeight || 1)
  };
})()"#;

const SCROLL_SCRIPT: &str = r#"(function () {
  var root = document.documentElement;
  if (!root.hasAttribute("data-ce-scroll-behavior")) {
    root.setAttribute("data-ce-scroll-behavior", root.style.scrollBehavior || "");
    root.style.scrollBehavior = "auto";
  }
  try { window.scrollTo({ left: LEFT, top: TOP, behavior: "instant" }); }
  catch (error) { window.scrollTo(LEFT, TOP); }
  return new Promise(function (resolve) {
    requestAnimationFrame(function () {
      requestAnimationFrame(function () {
        setTimeout(function () {
          resolve({
            scrollX: window.scrollX || 0,
            scrollY: window.scrollY || 0,
            width: window.innerWidth || 1,
            height: window.innerHeight || 1
          });
        }, 40);
      });
    });
  });
})()"#;

/// Headers and corner widgets repeat in every slice. A fixed shell the size of the page stays.
const HIDE_PINNED_SCRIPT: &str = r#"(function () {
  var viewW = window.innerWidth || 1;
  var viewH = window.innerHeight || 1;
  var all = document.body ? document.body.querySelectorAll("*") : [];
  for (var index = 0; index < all.length; index++) {
    var el = all[index];
    if (el.hasAttribute("data-ce-capture-hide")) continue;
    var pos = getComputedStyle(el).position;
    if (pos !== "fixed" && pos !== "sticky") continue;
    var rect = el.getBoundingClientRect();
    if (rect.width >= viewW * 0.8 && rect.height >= viewH * 0.8) continue;
    if (rect.width < 1 || rect.height < 1) continue;
    el.setAttribute("data-ce-capture-hide", el.style.visibility || "");
    el.style.visibility = "hidden";
  }
  return true;
})()"#;

const RESTORE_SCRIPT: &str = r#"(function () {
  try { window.scrollTo({ left: LEFT, top: TOP, behavior: "instant" }); }
  catch (error) { window.scrollTo(LEFT, TOP); }
  var root = document.documentElement;
  if (root.hasAttribute("data-ce-scroll-behavior")) {
    root.style.scrollBehavior = root.getAttribute("data-ce-scroll-behavior");
    root.removeAttribute("data-ce-scroll-behavior");
  }
  var hidden = document.querySelectorAll("[data-ce-capture-hide]");
  for (var index = 0; index < hidden.length; index++) {
    var el = hidden[index];
    el.style.visibility = el.getAttribute("data-ce-capture-hide");
    el.removeAttribute("data-ce-capture-hide");
  }
  return true;
})()"#;

fn page_box(webview: &Webview) -> Result<PageBox, String> {
    let value = webview2::evaluate(webview, PAGE_BOX_SCRIPT, Duration::from_secs(5), FAILURE)?;
    let width = field(&value, "width").max(1.0);
    let height = field(&value, "height").max(1.0);
    Ok(PageBox {
        scroll_x: field(&value, "scrollX").max(0.0),
        scroll_y: field(&value, "scrollY").max(0.0),
        width,
        height,
        page_width: field(&value, "pageWidth").max(width),
        page_height: field(&value, "pageHeight").max(height),
    })
}

fn view_from(value: &serde_json::Value) -> View {
    View {
        scroll_x: field(value, "scrollX").max(0.0),
        scroll_y: field(value, "scrollY").max(0.0),
        width: field(value, "width").max(1.0),
        height: field(value, "height").max(1.0),
    }
}

fn scroll_view(webview: &Webview, x: f64, y: f64) -> Result<View, String> {
    let script = SCROLL_SCRIPT
        .replace("LEFT", &js_num(x))
        .replace("TOP", &js_num(y));
    let value = webview2::evaluate(webview, &script, Duration::from_secs(5), FAILURE)?;
    Ok(view_from(&value))
}

/// Puts the page back where it was, including pinned bars hidden while stitching.
struct ScrollRestore<'a> {
    webview: &'a Webview,
    x: f64,
    y: f64,
}

impl Drop for ScrollRestore<'_> {
    fn drop(&mut self) {
        let script = RESTORE_SCRIPT
            .replace("LEFT", &js_num(self.x))
            .replace("TOP", &js_num(self.y));
        let _ = webview2::evaluate(self.webview, &script, Duration::from_secs(2), FAILURE);
    }
}

fn contains_rect(page: &PageBox, x: f64, y: f64, width: f64, height: f64) -> bool {
    x >= page.scroll_x - 1.0
        && y >= page.scroll_y - 1.0
        && x + width <= page.scroll_x + page.width + 1.0
        && y + height <= page.scroll_y + page.height + 1.0
}

fn covers_view(page: &PageBox, x: f64, y: f64, width: f64, height: f64) -> bool {
    x <= page.scroll_x + 1.0
        && y <= page.scroll_y + 1.0
        && x + width >= page.scroll_x + page.width - 1.0
        && y + height >= page.scroll_y + page.height - 1.0
}

/// The webview surface, with no clip. This is the picture the window is already drawing.
fn viewport_image(webview: &Webview) -> Result<RgbaImage, String> {
    let response = webview2::call_cdp_json(
        webview,
        "Page.captureScreenshot",
        r#"{"format":"png","fromSurface":true}"#,
        Duration::from_secs(30),
        FAILURE,
    )?;
    let data = response
        .get("data")
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("{FAILURE} The page returned no image."))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|error| format!("{FAILURE} {error}"))?;
    let image = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
        .map_err(|error| format!("{FAILURE} {error}"))?;
    Ok(image.to_rgba8())
}

fn crop_rgba(image: &RgbaImage, x: u32, y: u32, width: u32, height: u32) -> RgbaImage {
    let x = x.min(image.width().saturating_sub(1));
    let y = y.min(image.height().saturating_sub(1));
    let width = width.max(1).min(image.width() - x);
    let height = height.max(1).min(image.height() - y);
    image::imageops::crop_imm(image, x, y, width, height).to_image()
}

fn scale_image(image: RgbaImage, scale: f64) -> RgbaImage {
    if scale >= 0.999 {
        return image;
    }
    let width = (f64::from(image.width()) * scale).round().max(1.0) as u32;
    let height = (f64::from(image.height()) * scale).round().max(1.0) as u32;
    if width == image.width() && height == image.height() {
        image
    } else {
        image::imageops::resize(&image, width, height, image::imageops::FilterType::Triangle)
    }
}

/// Document rectangle `(x, y, width, height)` in CSS pixels. `hide_after_first` drops fixed and
/// sticky bars after the first tile so a long page does not repeat its header. The bool is set
/// when the walk stopped before the bottom of the rectangle.
fn capture_area(
    webview: &Webview,
    page: &PageBox,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    hide_after_first: bool,
) -> Result<(RgbaImage, bool), String> {
    let width = width.max(1.0);
    let height = height.max(1.0);
    if contains_rect(page, x, y, width, height) {
        let image = viewport_image(webview)?;
        if covers_view(page, x, y, width, height) {
            return Ok((image, false));
        }
        let crop = crop_pixels(
            image.width(),
            image.height(),
            &page.view(),
            x,
            y,
            width,
            height,
        )
        .ok_or_else(|| format!("{FAILURE} That section is off the page."))?;
        return Ok((
            crop_rgba(&image, crop.x, crop.y, crop.width, crop.height),
            false,
        ));
    }

    let _restore = ScrollRestore {
        webview,
        x: page.scroll_x,
        y: page.scroll_y,
    };
    let target_right = x + width;
    let target_bottom = y + height;
    let mut rows = Vec::new();
    let mut y_cursor = y;
    let mut hidden = false;
    let mut scale = 1.0;
    let mut known_scale = false;
    let mut shots = 0usize;
    while y_cursor < target_bottom - 0.5 && shots < MAX_SHOTS {
        let mut tiles = Vec::new();
        let mut x_cursor = x;
        let mut row_bottom = y_cursor;
        while x_cursor < target_right - 0.5 && tiles.len() < 4 && shots < MAX_SHOTS {
            shots += 1;
            let mut view = scroll_view(webview, x_cursor, y_cursor)?;
            if !(rows.is_empty() && tiles.is_empty()) && hide_after_first && !hidden {
                let _ = webview2::evaluate(
                    webview,
                    HIDE_PINNED_SCRIPT,
                    Duration::from_secs(5),
                    FAILURE,
                );
                hidden = true;
                view = scroll_view(webview, x_cursor, y_cursor)?;
            }
            let image = viewport_image(webview)?;
            let Some(crop) = crop_pixels(
                image.width(),
                image.height(),
                &view,
                x_cursor,
                y_cursor,
                target_right - x_cursor,
                target_bottom - y_cursor,
            ) else {
                break;
            };
            // The page would not scroll any further, so this shot repeats the last one.
            if crop.doc_right <= x_cursor + 0.5 {
                break;
            }
            if !known_scale {
                let ratio = f64::from(image.width()) / view.width.max(1.0);
                scale = fit_scale(width * height * ratio * ratio);
                known_scale = true;
            }
            if row_bottom <= y_cursor + 0.5 {
                row_bottom = crop.doc_bottom;
            }
            let next_x = crop.doc_right;
            tiles.push(scale_image(
                crop_rgba(&image, crop.x, crop.y, crop.width, crop.height),
                scale,
            ));
            if next_x <= x_cursor + 0.5 {
                break;
            }
            x_cursor = next_x;
        }
        if tiles.is_empty() || row_bottom <= y_cursor + 0.5 {
            break;
        }
        rows.push(stitch_row(tiles)?);
        y_cursor = row_bottom;
    }
    let short = y_cursor < target_bottom - 1.0;
    Ok((stitch(rows)?, short))
}

/// Keeps a dragged rectangle inside the page. Width and height stay at least 1 pixel.
pub fn clip_inside(
    page_width: f64,
    page_height: f64,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> (f64, f64, f64, f64) {
    let page_width = page_width.max(1.0);
    let page_height = page_height.max(1.0);
    let x = x.max(0.0).min(page_width - 1.0);
    let y = y.max(0.0).min(page_height - 1.0);
    let width = width.max(1.0).min(page_width - x);
    let height = height.max(1.0).min(page_height - y);
    (x, y, width, height)
}

/// Centiseconds between GIF frames. 8 frames a second is 12.
pub fn gif_delay_cs(fps: u32) -> u16 {
    (100 / fps.max(1)) as u16
}

pub fn limit_long_side(width: u32, height: u32, max_side: u32) -> (u32, u32) {
    let long = width.max(height).max(1);
    if long <= max_side || max_side == 0 {
        return (width.max(1), height.max(1));
    }
    let scale = f64::from(max_side) / f64::from(long);
    (
        (f64::from(width) * scale).round().max(1.0) as u32,
        (f64::from(height) * scale).round().max(1.0) as u32,
    )
}

static LAST_PNG: Mutex<Option<Vec<u8>>> = Mutex::new(None);
static LAST_REGION: Mutex<Option<(f64, f64, f64, f64)>> = Mutex::new(None);

pub fn remember_png(bytes: &[u8]) {
    *LAST_PNG.lock().expect("capture") = Some(bytes.to_vec());
}

pub fn last_png() -> Option<Vec<u8>> {
    LAST_PNG.lock().expect("capture").clone()
}

pub fn last_png_base64() -> Result<String, String> {
    let bytes = last_png().ok_or_else(|| "Capture a page first.".to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

pub fn decode_png(data: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|error| format!("{FAILURE} {error}"))
}

/// A rectangle the user drags on the page, as an image.
pub fn region_image(app: &AppHandle, id: u32) -> Result<RgbaImage, String> {
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let value = webview2::evaluate(&webview, REGION_SCRIPT, Duration::from_secs(60), FAILURE)?;
    let x = value.get("x").and_then(|item| item.as_f64());
    let y = value.get("y").and_then(|item| item.as_f64());
    let width = value.get("width").and_then(|item| item.as_f64());
    let height = value.get("height").and_then(|item| item.as_f64());
    let (Some(x), Some(y), Some(width), Some(height)) = (x, y, width, height) else {
        return Err("The capture was cancelled.".into());
    };
    let (image, rect) = clip_shot(&webview, x, y, width, height)?;
    *LAST_REGION.lock().expect("region") = Some(rect);
    Ok(image)
}

/// A rectangle of the page in document CSS pixels, as an image. Tall rectangles stop at the
/// full-page capture limit.
pub fn clip_image(
    app: &AppHandle,
    id: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<RgbaImage, String> {
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    clip_shot(&webview, x, y, width, height.min(MAX_CSS_HEIGHT)).map(|(image, _)| image)
}

fn clip_shot(
    webview: &Webview,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(RgbaImage, (f64, f64, f64, f64)), String> {
    let page = page_box(webview)?;
    let rect = clip_inside(page.page_width, page.page_height, x, y, width, height);
    let (image, _) = capture_area(webview, &page, rect.0, rect.1, rect.2, rect.3, false)?;
    Ok((image, rect))
}

pub fn capture_region(app: &AppHandle, id: u32) -> Result<PageCaptureResult, String> {
    let image = region_image(app, id)?;
    let png = encode_png(&image)?;
    remember_png(&png);
    Ok(PageCaptureResult {
        path: None,
        width: image.width(),
        height: image.height(),
        truncated: false,
    })
}

pub fn store_visible(app: &AppHandle, id: u32) -> Result<PageCaptureResult, String> {
    let capture = capture(app, id, false, false)?;
    let png = encode_png(&capture.image)?;
    remember_png(&png);
    Ok(PageCaptureResult {
        path: None,
        width: capture.image.width(),
        height: capture.image.height(),
        truncated: false,
    })
}

pub fn save_edited_png(bytes: &[u8], path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("{FAILURE} {error}"))?;
    }
    std::fs::write(path, bytes).map_err(|error| format!("Couldn't save the image. {error}"))
}

pub fn copy_edited_png(bytes: &[u8]) -> Result<(), String> {
    let image = image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .map_err(|error| format!("{FAILURE} {error}"))?
        .to_rgba8();
    clipboard::write_image(&image, bytes)
}

const REGION_SCRIPT: &str = r#"(function () {
  return new Promise(function (resolve) {
    var overlay = document.createElement("div");
    overlay.id = "ce-region";
    overlay.style.cssText = "position:fixed;inset:0;z-index:2147483646;cursor:crosshair;background:rgba(0,0,0,.15)";
    var box = document.createElement("div");
    box.style.cssText = "position:fixed;border:2px solid #38bdf8;background:rgba(56,189,248,.25);pointer-events:none;z-index:2147483647";
    document.documentElement.appendChild(overlay);
    document.documentElement.appendChild(box);
    var start = null;
    function cleanup(value) {
      overlay.remove();
      box.remove();
      document.removeEventListener("keydown", onKey, true);
      requestAnimationFrame(function () {
        requestAnimationFrame(function () {
          setTimeout(function () { resolve(value); }, 40);
        });
      });
    }
    function onKey(event) {
      if (event.key === "Escape") cleanup(null);
    }
    overlay.addEventListener("mousedown", function (event) {
      start = { x: event.clientX, y: event.clientY };
    });
    overlay.addEventListener("mousemove", function (event) {
      if (!start) return;
      var x = Math.min(start.x, event.clientX);
      var y = Math.min(start.y, event.clientY);
      box.style.left = x + "px";
      box.style.top = y + "px";
      box.style.width = Math.abs(event.clientX - start.x) + "px";
      box.style.height = Math.abs(event.clientY - start.y) + "px";
    });
    overlay.addEventListener("mouseup", function (event) {
      if (!start) return;
      var width = Math.abs(event.clientX - start.x);
      var height = Math.abs(event.clientY - start.y);
      if (width < 4 || height < 4) cleanup(null);
      else cleanup({
        x: Math.min(start.x, event.clientX) + window.scrollX,
        y: Math.min(start.y, event.clientY) + window.scrollY,
        width: width,
        height: height
      });
    });
    document.addEventListener("keydown", onKey, true);
  });
})()"#;

struct Recording {
    stop: Arc<AtomicBool>,
}

static RECORDING: Mutex<Option<Recording>> = Mutex::new(None);

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStatus {
    pub running: bool,
    pub seconds_left: u32,
    pub path: Option<String>,
    pub error: Option<String>,
}

const RECORD_FPS: u32 = 8;
const RECORD_SECONDS: u32 = 15;
const GIF_LONG_SIDE: u32 = 1280;

pub fn start_recording(
    app: &AppHandle,
    id: u32,
    kind: &str,
    path: std::path::PathBuf,
) -> Result<(), String> {
    if RECORDING.lock().expect("recording").is_some() {
        return Err("A recording is already running.".into());
    }
    let kind = kind.to_string();
    if kind == "area" {
        let _ = capture_region(app, id)?;
    }
    let stop = Arc::new(AtomicBool::new(false));
    *RECORDING.lock().expect("recording") = Some(Recording { stop: stop.clone() });
    let app = app.clone();
    let window = tabs::snapshot(&app)
        .tabs
        .iter()
        .find(|tab| tab.id == id)
        .map(|tab| tab.window.clone())
        .unwrap_or_else(|| tabs::WINDOW.to_string());
    std::thread::spawn(move || {
        let frames = record_frames(&app, id, &kind, &window, &stop);
        let status = match frames.and_then(|frames| write_gif(&frames, &path).map(|()| path)) {
            Ok(path) => RecordingStatus {
                running: false,
                seconds_left: 0,
                path: Some(path.to_string_lossy().into_owned()),
                error: None,
            },
            Err(error) => RecordingStatus {
                running: false,
                seconds_left: 0,
                path: None,
                error: Some(error),
            },
        };
        *RECORDING.lock().expect("recording") = None;
        let _ = app.emit("recording-status", &status);
    });
    Ok(())
}

pub fn stop_recording() {
    if let Some(recording) = RECORDING.lock().expect("recording").as_ref() {
        recording.stop.store(true, Ordering::Relaxed);
    }
}

fn record_frames(
    app: &AppHandle,
    id: u32,
    kind: &str,
    window: &str,
    stop: &AtomicBool,
) -> Result<Vec<RgbaImage>, String> {
    let total = RECORD_FPS * RECORD_SECONDS;
    let mut frames = Vec::new();
    for index in 0..total {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let frame = match kind {
            "window" => capture_browser_window(app, window),
            "area" => region_frame(app, id),
            _ => visible_frame(app, id),
        };
        if let Ok(frame) = frame {
            frames.push(fit_long_side(frame, GIF_LONG_SIDE));
        }
        let left = RECORD_SECONDS.saturating_sub(index / RECORD_FPS);
        let _ = app.emit(
            "recording-status",
            &RecordingStatus {
                running: true,
                seconds_left: left,
                path: None,
                error: None,
            },
        );
        std::thread::sleep(Duration::from_millis(1000 / u64::from(RECORD_FPS)));
    }
    if frames.is_empty() {
        return Err("The recording has no frames.".into());
    }
    let (width, height) = (frames[0].width(), frames[0].height());
    Ok(frames
        .into_iter()
        .map(|frame| {
            if frame.width() == width && frame.height() == height {
                frame
            } else {
                image::imageops::resize(
                    &frame,
                    width,
                    height,
                    image::imageops::FilterType::Triangle,
                )
            }
        })
        .collect())
}

fn region_frame(app: &AppHandle, id: u32) -> Result<RgbaImage, String> {
    let Some((x, y, width, height)) = *LAST_REGION.lock().expect("region") else {
        return visible_frame(app, id);
    };
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let page = page_box(&webview)?;
    let image = viewport_image(&webview)?;
    let Some(crop) = crop_pixels(
        image.width(),
        image.height(),
        &page.view(),
        x,
        y,
        width,
        height,
    ) else {
        return Ok(image);
    };
    Ok(crop_rgba(&image, crop.x, crop.y, crop.width, crop.height))
}

fn visible_frame(app: &AppHandle, id: u32) -> Result<RgbaImage, String> {
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    viewport_image(&webview)
}

fn fit_long_side(image: RgbaImage, max_side: u32) -> RgbaImage {
    let (width, height) = limit_long_side(image.width(), image.height(), max_side);
    if width == image.width() && height == image.height() {
        image
    } else {
        image::imageops::resize(&image, width, height, image::imageops::FilterType::Triangle)
    }
}

pub fn write_gif(frames: &[RgbaImage], path: &Path) -> Result<(), String> {
    if frames.is_empty() {
        return Err("The recording has no frames.".into());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("{FAILURE} {error}"))?;
    }
    let file =
        std::fs::File::create(path).map_err(|error| format!("Couldn't save the GIF. {error}"))?;
    let mut encoder = image::codecs::gif::GifEncoder::new(file);
    encoder
        .set_repeat(image::codecs::gif::Repeat::Infinite)
        .map_err(|error| format!("Couldn't save the GIF. {error}"))?;
    let delay = gif_delay_cs(RECORD_FPS);
    for frame in frames {
        let gif_frame = image::Frame::from_parts(
            frame.clone(),
            0,
            0,
            image::Delay::from_numer_denom_ms(u32::from(delay) * 10, 1),
        );
        encoder
            .encode_frame(gif_frame)
            .map_err(|error| format!("Couldn't save the GIF. {error}"))?;
    }
    Ok(())
}

fn capture_browser_window(app: &AppHandle, label: &str) -> Result<RgbaImage, String> {
    let window = app
        .get_webview_window(label)
        .ok_or("The browser window is missing.")?;
    let hwnd = window.hwnd().map_err(|error| error.to_string())?;
    window_image(hwnd.0 as isize)
}

fn window_image(hwnd: isize) -> Result<RgbaImage, String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS,
        DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject,
    };
    use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
    use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, PW_RENDERFULLCONTENT};
    unsafe {
        let hwnd = HWND(hwnd as *mut std::ffi::c_void);
        let mut rect = windows::Win32::Foundation::RECT::default();
        GetClientRect(hwnd, &mut rect).map_err(|error| format!("{FAILURE} {error}"))?;
        let width = rect.right.max(1);
        let height = rect.bottom.max(1);
        let screen = GetDC(Some(hwnd));
        let memory = CreateCompatibleDC(Some(screen));
        let bitmap = CreateCompatibleBitmap(screen, width, height);
        let previous = SelectObject(memory, bitmap.into());
        let _ = PrintWindow(hwnd, memory, PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT));
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let rows = GetDIBits(
            memory,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        );
        SelectObject(memory, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memory);
        ReleaseDC(Some(hwnd), screen);
        if rows == 0 {
            return Err(format!("{FAILURE} Couldn't read the window."));
        }
        let mut image = RgbaImage::new(width as u32, height as u32);
        for (pixel, chunk) in image.pixels_mut().zip(pixels.chunks_exact(4)) {
            *pixel = image::Rgba([chunk[2], chunk[1], chunk[0], 255]);
        }
        Ok(image)
    }
}

fn stitch_row(tiles: Vec<RgbaImage>) -> Result<RgbaImage, String> {
    let height = tiles.iter().map(RgbaImage::height).min().unwrap_or(0);
    let width: u32 = tiles.iter().map(RgbaImage::width).sum();
    if width == 0 || height == 0 {
        return Err(format!("{FAILURE} The page is empty."));
    }
    if tiles.len() == 1 {
        return Ok(tiles.into_iter().next().unwrap_or_default());
    }
    let mut canvas = RgbaImage::from_pixel(width, height, image::Rgba([255, 255, 255, 255]));
    let mut left = 0i64;
    for tile in &tiles {
        image::imageops::replace(&mut canvas, tile, left, 0);
        left += i64::from(tile.width());
    }
    Ok(canvas)
}

fn stitch(slices: Vec<RgbaImage>) -> Result<RgbaImage, String> {
    let width = slices.iter().map(RgbaImage::width).min().unwrap_or(0);
    let height: u32 = slices.iter().map(RgbaImage::height).sum();
    if width == 0 || height == 0 {
        return Err(format!("{FAILURE} The page is empty."));
    }
    if slices.len() == 1 {
        return Ok(slices.into_iter().next().unwrap_or_default());
    }
    let mut canvas = RgbaImage::from_pixel(width, height, image::Rgba([255, 255, 255, 255]));
    let mut top = 0i64;
    for slice in &slices {
        image::imageops::replace(&mut canvas, slice, 0, top);
        top += i64::from(slice.height());
    }
    Ok(canvas)
}

mod clipboard {
    use image::RgbaImage;
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
    use windows::Win32::Graphics::Gdi::BITMAPINFOHEADER;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
    use windows::Win32::System::Ole::CF_DIB;
    use windows::core::w;

    const FAILURE: &str = "Couldn't copy the image.";

    /// Puts the image on the clipboard as a DIB for most apps and as PNG for apps that read it.
    pub fn write_image(image: &RgbaImage, png: &[u8]) -> Result<(), String> {
        let dib = dib_bytes(image);
        unsafe {
            OpenClipboard(None).map_err(|error| format!("{FAILURE} {error}"))?;
            let result = (|| {
                EmptyClipboard().map_err(|error| format!("{FAILURE} {error}"))?;
                set(u32::from(CF_DIB.0), &dib)?;
                let png_format = RegisterClipboardFormatW(w!("PNG"));
                if png_format != 0 {
                    let _ = set(png_format, png);
                }
                Ok(())
            })();
            let _ = CloseClipboard();
            result
        }
    }

    unsafe fn set(format: u32, bytes: &[u8]) -> Result<(), String> {
        unsafe {
            let memory = GlobalAlloc(GMEM_MOVEABLE, bytes.len())
                .map_err(|error| format!("{FAILURE} {error}"))?;
            let target = GlobalLock(memory);
            if target.is_null() {
                let _ = GlobalFree(Some(memory));
                return Err(FAILURE.to_string());
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), target.cast::<u8>(), bytes.len());
            let _ = GlobalUnlock(memory);
            if let Err(error) = SetClipboardData(format, Some(HANDLE(memory.0))) {
                let _ = GlobalFree(Some(HGLOBAL(memory.0)));
                return Err(format!("{FAILURE} {error}"));
            }
            Ok(())
        }
    }

    /// A bottom-up 32-bit BGRA bitmap with its header, the layout `CF_DIB` expects.
    fn dib_bytes(image: &RgbaImage) -> Vec<u8> {
        let (width, height) = image.dimensions();
        let header = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: height as i32,
            biPlanes: 1,
            biBitCount: 32,
            biSizeImage: width * height * 4,
            ..Default::default()
        };
        let header_size = size_of::<BITMAPINFOHEADER>();
        let mut bytes = Vec::with_capacity(header_size + (width * height * 4) as usize);
        let header_bytes = unsafe {
            std::slice::from_raw_parts(std::ptr::addr_of!(header).cast::<u8>(), header_size)
        };
        bytes.extend_from_slice(header_bytes);
        let stride = width as usize * 4;
        let raw = image.as_raw();
        for row in (0..height as usize).rev() {
            for pixel in raw[row * stride..(row + 1) * stride].chunks_exact(4) {
                bytes.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
            }
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::{View, clip_inside, crop_pixels, fit_scale, gif_delay_cs, limit_long_side};

    #[test]
    fn a_region_stays_inside_the_page() {
        let (x, y, width, height) = clip_inside(800.0, 600.0, -20.0, 10.0, 900.0, 50.0);
        assert_eq!(x, 0.0);
        assert_eq!(y, 10.0);
        assert_eq!(width, 800.0);
        assert_eq!(height, 50.0);
    }

    #[test]
    fn eight_frames_a_second_is_twelve_centiseconds() {
        assert_eq!(gif_delay_cs(8), 12);
        assert_eq!(limit_long_side(2000, 1000, 1280), (1280, 640));
    }

    #[test]
    fn a_css_rect_maps_onto_the_screenshot() {
        let view = View {
            scroll_x: 0.0,
            scroll_y: 0.0,
            width: 100.0,
            height: 50.0,
        };
        let crop = crop_pixels(200, 100, &view, 10.0, 5.0, 20.0, 10.0).expect("crop");
        assert_eq!((crop.x, crop.y, crop.width, crop.height), (20, 10, 40, 20));
        assert_eq!(crop.doc_right, 30.0);
        assert_eq!(crop.doc_bottom, 15.0);
    }

    #[test]
    fn a_rect_below_the_fold_crops_the_visible_part() {
        let view = View {
            scroll_x: 0.0,
            scroll_y: 100.0,
            width: 80.0,
            height: 50.0,
        };
        let crop = crop_pixels(80, 50, &view, 0.0, 120.0, 80.0, 100.0).expect("crop");
        assert_eq!((crop.y, crop.height), (20, 30));
        assert_eq!(crop.doc_bottom, 150.0);
        assert!(crop_pixels(80, 50, &view, 0.0, 10.0, 40.0, 10.0).is_none());
    }

    #[test]
    fn a_page_over_the_pixel_budget_is_scaled_down() {
        assert_eq!(fit_scale(super::MAX_PIXELS), 1.0);
        let scale = fit_scale(super::MAX_PIXELS * 4.0);
        assert!((scale - 0.5).abs() < 0.0001);
    }
}
