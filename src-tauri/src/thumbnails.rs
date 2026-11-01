use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use base64::Engine;
use image::ImageFormat;
use tauri::AppHandle;

use crate::{tabs, webview2};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 200;
const FAILURE: &str = "Couldn't capture the tab.";

/// JPEG data URLs by tab id. Kept in memory only.
fn cache() -> &'static Mutex<HashMap<u32, String>> {
    static CACHE: OnceLock<Mutex<HashMap<u32, String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn get(id: u32) -> Option<String> {
    cache().lock().expect("thumbnails").get(&id).cloned()
}

pub fn forget(id: u32) {
    cache().lock().expect("thumbnails").remove(&id);
}

/// Captures what the tab draws now. Waits on the main thread, so call it from a worker thread.
pub fn capture(app: &AppHandle, id: u32) -> Option<String> {
    if !tabs::capturable(app, id) {
        return None;
    }
    let webview = tabs::page_view(app, id)?;
    let params = serde_json::json!({
        "format": "jpeg",
        "quality": 70,
        "fromSurface": true,
        "optimizeForSpeed": true,
    })
    .to_string();
    let response = webview2::call_cdp_json(
        &webview,
        "Page.captureScreenshot",
        &params,
        Duration::from_secs(2),
        FAILURE,
    )
    .ok()?;
    let data = response.get("data")?.as_str()?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .ok()?;
    let url = shrink(&bytes)?;
    cache().lock().expect("thumbnails").insert(id, url.clone());
    Some(url)
}

/// JPEG bytes from the page, made small enough for a hover card, as a data URL.
fn shrink(bytes: &[u8]) -> Option<String> {
    let image = image::load_from_memory_with_format(bytes, ImageFormat::Jpeg).ok()?;
    let small = image.thumbnail(WIDTH, HEIGHT).to_rgb8();
    let mut out = Vec::new();
    small
        .write_to(&mut Cursor::new(&mut out), ImageFormat::Jpeg)
        .ok()?;
    Some(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(out)
    ))
}

/// Captures the tab on a worker thread, so the caller does not wait.
pub fn capture_later(app: &AppHandle, id: u32) {
    let app = app.clone();
    std::thread::spawn(move || {
        capture(&app, id);
    });
}

/// A fresh capture when the tab can draw one, otherwise the last one taken.
pub fn thumbnail(app: &AppHandle, id: u32) -> Option<String> {
    capture(app, id).or_else(|| get(id))
}

#[cfg(test)]
mod tests {
    use super::shrink;
    use image::{ImageFormat, RgbImage};
    use std::io::Cursor;

    #[test]
    fn a_large_capture_shrinks_to_a_jpeg_data_url() {
        let mut bytes = Vec::new();
        RgbImage::new(1600, 900)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
            .expect("encode");
        let url = shrink(&bytes).expect("thumbnail");
        assert!(url.starts_with("data:image/jpeg;base64,"));
    }
}
