//! Page to PNG: captures a tab's whole document, or what is on screen, as one image.

use std::io::Cursor;
use std::path::Path;
use std::time::Duration;

use base64::Engine;
use image::{ImageFormat, RgbaImage};
use serde::Serialize;
use tauri::{AppHandle, Webview};

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

pub fn copy_png(app: &AppHandle, id: u32, full_page: bool, load_lazy: bool) -> Result<PageCaptureResult, String> {
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

fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, String> {
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
    let ratio = webview2::evaluate(&webview, "window.devicePixelRatio", Duration::from_secs(5), FAILURE)?
        .as_f64()
        .filter(|ratio| *ratio > 0.0)
        .unwrap_or(1.0);
    let metrics = cdp(&webview, "Page.getLayoutMetrics", "{}")?;
    let number = |pointer: &str| metrics.pointer(pointer).and_then(|value| value.as_f64());
    let view_width = number("/cssLayoutViewport/clientWidth").unwrap_or(1280.0).max(1.0);
    let view_height = number("/cssLayoutViewport/clientHeight").unwrap_or(800.0).max(1.0);

    if !full_page {
        let x = number("/cssVisualViewport/pageX").unwrap_or(0.0);
        let y = number("/cssVisualViewport/pageY").unwrap_or(0.0);
        let image = shot(&webview, x, y, view_width, view_height, 1.0, false)?;
        return Ok(Capture { image, truncated: false });
    }

    let content_width = number("/cssContentSize/width").unwrap_or(view_width);
    let content_height = number("/cssContentSize/height").unwrap_or(view_height);
    let width = content_width.max(view_width).min(view_width * 4.0).ceil();
    let truncated = content_height > MAX_CSS_HEIGHT;
    let height = content_height.min(MAX_CSS_HEIGHT).max(1.0).ceil();

    let pixels = width * height * ratio * ratio;
    let scale = if pixels > MAX_PIXELS { (MAX_PIXELS / pixels).sqrt() } else { 1.0 };
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
            let memory = GlobalAlloc(GMEM_MOVEABLE, bytes.len()).map_err(|error| format!("{FAILURE} {error}"))?;
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
        let header_bytes =
            unsafe { std::slice::from_raw_parts(std::ptr::addr_of!(header).cast::<u8>(), header_size) };
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
