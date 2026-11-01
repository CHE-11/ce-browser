//! Page to PNG: captures a tab's whole document, or what is on screen, as one image.

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
/// Chromium cannot render one screenshot taller than its largest texture, so tall pages are
/// captured in strips of at most this many device pixels and stitched.
const SLICE_PIXELS: f64 = 8000.0;
/// Longer pages are cut off here. Past this the image is too large for most viewers.
const MAX_CSS_HEIGHT: f64 = 60000.0;
/// Pixel budget for the stitched image. Bigger pages are captured at a lower scale.
const MAX_PIXELS: f64 = 120_000_000.0;

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
  for (let y = 0; y < root.scrollHeight && y < 60000 && Date.now() < deadline; y += step) {
    window.scrollTo(0, y);
    await sleep(120);
  }
  window.scrollTo(0, root.scrollHeight);
  await sleep(250);
  const pending = Array.from(document.images).filter((image) => !image.complete);
  await Promise.race([
    Promise.all(pending.map((image) => new Promise((resolve) => {
      image.addEventListener("load", resolve, { once: true });
      image.addEventListener("error", resolve, { once: true });
    }))),
    sleep(3000),
  ]);
  window.scrollTo(startX, startY);
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
    let ratio = webview2::evaluate(
        &webview,
        "window.devicePixelRatio",
        Duration::from_secs(5),
        FAILURE,
    )?
    .as_f64()
    .filter(|ratio| *ratio > 0.0)
    .unwrap_or(1.0);
    let metrics = cdp(&webview, "Page.getLayoutMetrics", "{}")?;
    let number = |pointer: &str| metrics.pointer(pointer).and_then(|value| value.as_f64());
    let view_width = number("/cssLayoutViewport/clientWidth")
        .unwrap_or(1280.0)
        .max(1.0);
    let view_height = number("/cssLayoutViewport/clientHeight")
        .unwrap_or(800.0)
        .max(1.0);

    if !full_page {
        let x = number("/cssVisualViewport/pageX").unwrap_or(0.0);
        let y = number("/cssVisualViewport/pageY").unwrap_or(0.0);
        let image = shot(&webview, x, y, view_width, view_height, 1.0, false)?;
        return Ok(Capture {
            image,
            truncated: false,
        });
    }

    let content_width = number("/cssContentSize/width").unwrap_or(view_width);
    let content_height = number("/cssContentSize/height").unwrap_or(view_height);
    let width = content_width.max(view_width).min(view_width * 4.0).ceil();
    let truncated = content_height > MAX_CSS_HEIGHT;
    let height = content_height.min(MAX_CSS_HEIGHT).max(1.0).ceil();

    let pixels = width * height * ratio * ratio;
    let scale = if pixels > MAX_PIXELS {
        (MAX_PIXELS / pixels).sqrt()
    } else {
        1.0
    };
    let device = ratio * scale;
    let slice_css = (SLICE_PIXELS / device).floor().max(256.0);

    let mut slices = Vec::new();
    let mut top = 0.0;
    while top < height {
        let slice_height = slice_css.min(height - top);
        slices.push(shot(&webview, 0.0, top, width, slice_height, scale, true)?);
        top += slice_height;
    }
    Ok(Capture {
        image: stitch(slices)?,
        truncated,
    })
}

fn cdp(webview: &Webview, method: &str, params: &str) -> Result<serde_json::Value, String> {
    webview2::call_cdp_json(webview, method, params, Duration::from_secs(30), FAILURE)
}

fn shot(
    webview: &Webview,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    scale: f64,
    beyond_viewport: bool,
) -> Result<RgbaImage, String> {
    let params = serde_json::json!({
        "format": "png",
        "fromSurface": true,
        "captureBeyondViewport": beyond_viewport,
        "clip": { "x": x, "y": y, "width": width, "height": height, "scale": scale },
    })
    .to_string();
    let response = webview2::call_cdp_json(
        webview,
        "Page.captureScreenshot",
        &params,
        Duration::from_secs(60),
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
    let metrics = cdp(webview, "Page.getLayoutMetrics", "{}")?;
    let page_width = metrics
        .pointer("/cssContentSize/width")
        .and_then(|item| item.as_f64())
        .unwrap_or(width);
    let page_height = metrics
        .pointer("/cssContentSize/height")
        .and_then(|item| item.as_f64())
        .unwrap_or(height);
    let rect = clip_inside(page_width, page_height, x, y, width, height);
    let image = shot(webview, rect.0, rect.1, rect.2, rect.3, 1.0, true)?;
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
      resolve(value);
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
    shot(&webview, x, y, width, height, 1.0, true)
}

fn visible_frame(app: &AppHandle, id: u32) -> Result<RgbaImage, String> {
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let metrics = cdp(&webview, "Page.getLayoutMetrics", "{}")?;
    let number = |pointer: &str| metrics.pointer(pointer).and_then(|value| value.as_f64());
    let width = number("/cssLayoutViewport/clientWidth")
        .unwrap_or(1280.0)
        .max(1.0);
    let height = number("/cssLayoutViewport/clientHeight")
        .unwrap_or(800.0)
        .max(1.0);
    let x = number("/cssVisualViewport/pageX").unwrap_or(0.0);
    let y = number("/cssVisualViewport/pageY").unwrap_or(0.0);
    shot(&webview, x, y, width, height, 1.0, false)
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

fn stitch(slices: Vec<RgbaImage>) -> Result<RgbaImage, String> {
    let width = slices.iter().map(RgbaImage::width).max().unwrap_or(0);
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
    use super::{clip_inside, gif_delay_cs, limit_long_side};

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
}
