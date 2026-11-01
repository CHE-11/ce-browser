use std::io::{Cursor, Read};
use std::time::Duration;

use base64::Engine;
use tauri::{AppHandle, Manager};
use url::Url;

use crate::{media, ocr, paths, privacy, settings, tabs, undo};

const MAX_IMAGE_BYTES: u64 = 15 * 1024 * 1024;
/// Selections longer than this many words link to their first and last words only.
const FRAGMENT_WORDS: usize = 10;
const FRAGMENT_EDGE: usize = 4;

/// Short text for a menu label, such as `Search for "a long sente…"`.
pub fn label_text(text: &str, limit: usize) -> String {
    let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if words.chars().count() <= limit {
        return words;
    }
    let cut: String = words.chars().take(limit).collect();
    format!("{}\u{2026}", cut.trim_end())
}

/// Percent-encoding for a text fragment. `-`, `,` and `&` have meaning there, so they are encoded too.
fn fragment_encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"._~!*'()".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// A link that scrolls to and highlights `selection` on `page`, without tracking parameters.
pub fn link_to_text(settings: &settings::BrowserSettings, page: &str, selection: &str) -> Option<String> {
    let clean = privacy::rewrite(settings, page).unwrap_or_else(|| page.to_string());
    let mut url = Url::parse(&clean).ok()?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return None;
    }
    url.set_fragment(None);
    let words: Vec<&str> = selection.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }
    let directive = if words.len() > FRAGMENT_WORDS {
        format!(
            "{},{}",
            fragment_encode(&words[..FRAGMENT_EDGE].join(" ")),
            fragment_encode(&words[words.len() - FRAGMENT_EDGE..].join(" "))
        )
    } else {
        fragment_encode(&words.join(" "))
    };
    Some(format!("{url}#:~:text={directive}"))
}

fn window_of(app: &AppHandle, id: u32) -> String {
    tabs::tab_window_label(app, id).unwrap_or_else(|| tabs::WINDOW.to_string())
}

pub fn search_selection(app: &AppHandle, id: u32, text: &str) {
    let query = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let url = paths::search(&settings::get(app).search_url(), &query);
    let _ = tabs::open_beside(app, id, url, false);
}

pub fn copy_link_to_text(app: &AppHandle, id: u32, page: &str, selection: &str) {
    let window = window_of(app, id);
    match link_to_text(&settings::get(app), page, selection) {
        Some(link) => match ocr::copy_text(&link) {
            Ok(()) => undo::notice(app, &window, "Copied a link to the highlighted text"),
            Err(error) => undo::notice(app, &window, &error),
        },
        None => undo::notice(app, &window, "Only web pages can link to text."),
    }
}

/// Opens a popup for tab `id` where the menu was, `x` and `y` being physical pixels inside the page.
pub fn open_popup_at(app: &AppHandle, id: u32, x: i32, y: i32, kind: &str, params: String, width: f64, height: f64) {
    let window = window_of(app, id);
    let Some(handle) = app.get_window(&window) else {
        return;
    };
    let scale = handle.scale_factor().unwrap_or(1.0);
    let offset = tabs::page_view(app, id)
        .and_then(|webview| webview.position().ok())
        .map(|position| (position.x, position.y))
        .unwrap_or_default();
    let request = tabs::PopupRequest {
        kind: kind.to_string(),
        x: f64::from(offset.0 + x) / scale,
        y: f64::from(offset.1 + y) / scale,
        width,
        height,
        params,
        focus: true,
    };
    if let Err(error) = tabs::open_popup(app, &window, request) {
        eprintln!("page menu popup: {error}");
    }
}

/// Image bytes from an `http(s)` or `data:` address.
fn image_bytes(source: &str) -> Result<Vec<u8>, String> {
    if let Some(rest) = source.strip_prefix("data:") {
        let (meta, data) = rest.split_once(',').ok_or("That image address is not readable.")?;
        if !meta.ends_with(";base64") {
            return Err("That image address is not readable.".into());
        }
        return base64::engine::general_purpose::STANDARD
            .decode(data.trim())
            .map_err(|error| error.to_string());
    }
    if !source.starts_with("http://") && !source.starts_with("https://") {
        return Err("This image can't be read from outside the page.".into());
    }
    let response = ureq::get(source)
        .timeout(Duration::from_secs(20))
        .set("User-Agent", "Mozilla/5.0")
        .call()
        .map_err(|error| format!("Couldn't download the image. {error}"))?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_IMAGE_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

/// Reads the words in an image with Windows OCR and copies them. Runs on a worker thread.
pub fn copy_image_text(app: &AppHandle, id: u32, source: &str) {
    let window = window_of(app, id);
    let result = image_bytes(source).and_then(|bytes| {
        let image = image::load_from_memory(&bytes).map_err(|error| format!("That image format isn't supported. {error}"))?;
        let mut png = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|error| error.to_string())?;
        ocr::recognize_png(&png)
    });
    let message = match result {
        Ok(text) if text.trim().is_empty() => "No text was found in the image.".to_string(),
        Ok(text) => match ocr::copy_text(&text) {
            Ok(()) => format!("Copied {} characters from the image", text.trim().chars().count()),
            Err(error) => error,
        },
        Err(error) => error,
    };
    undo::notice(app, &window, &message);
}

pub fn search_image(app: &AppHandle, id: u32, source: &str) {
    let window = window_of(app, id);
    if !source.starts_with("http://") && !source.starts_with("https://") {
        undo::notice(app, &window, "Only images on the web can be searched.");
        return;
    }
    let encoded: String = url::form_urlencoded::byte_serialize(source.as_bytes()).collect();
    if let Ok(url) = Url::parse(&format!("https://lens.google.com/uploadbyurl?url={encoded}")) {
        let _ = tabs::open_beside(app, id, url, false);
    }
}

pub fn save_image(app: &AppHandle, id: u32, source: &str) {
    if let Err(error) = media::download_media(app, id, &[source.to_string()]) {
        undo::notice(app, &window_of(app, id), &error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_selection_links_to_the_whole_text() {
        let settings = settings::BrowserSettings::default();
        let link = link_to_text(&settings, "https://example.com/a?utm_source=x#top", "Hello, world - again").expect("link");
        assert_eq!(link, "https://example.com/a#:~:text=Hello%2C%20world%20%2D%20again");
    }

    #[test]
    fn a_long_selection_links_to_its_first_and_last_words() {
        let settings = settings::BrowserSettings::default();
        let text = "one two three four five six seven eight nine ten eleven twelve";
        let link = link_to_text(&settings, "https://example.com/", text).expect("link");
        assert!(link.ends_with("#:~:text=one%20two%20three%20four,nine%20ten%20eleven%20twelve"));
        assert!(link_to_text(&settings, "ce://settings", "x").is_none());
    }

    #[test]
    fn menu_labels_are_cut_at_a_word_boundary_with_an_ellipsis() {
        assert_eq!(label_text("  short   text ", 20), "short text");
        assert_eq!(label_text("a selection that is much too long", 12), "a selection\u{2026}");
    }
}
