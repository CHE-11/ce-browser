//! Site icons for bookmarks, keyed by host. Icons come from tabs that visit a site, or from a
//! background fetch of the site's `<link rel="icon">` or `/favicon.ico`. The folder is a cache
//! under local app data, so deleting it only costs a refetch.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use base64::Engine;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use url::Url;

use crate::paths;

const FOLDER: &str = "favicons";
const ICON_SIZE: u32 = 32;
const MAX_PAGE_BYTES: u64 = 512 * 1024;
const MAX_ICON_BYTES: u64 = 1024 * 1024;
const MAX_WORKERS: usize = 4;
const TIMEOUT: Duration = Duration::from_secs(8);
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteFavicon {
    pub host: String,
    pub data_url: String,
}

#[derive(Default)]
struct State {
    icons: HashMap<String, String>,
    /// Hosts queued or being fetched.
    pending: HashSet<String>,
    /// Hosts whose fetch found nothing this session. A visit can still fill them in.
    failed: HashSet<String>,
    queue: VecDeque<String>,
    workers: usize,
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(State::default()))
}

pub fn host_of(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return None;
    }
    parsed
        .host_str()
        .filter(|host| !host.is_empty())
        .map(str::to_ascii_lowercase)
}

fn folder(app: &AppHandle) -> PathBuf {
    paths::root_local_dir(app).join(FOLDER)
}

fn file_for(app: &AppHandle, host: &str) -> PathBuf {
    let name: String = host
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '.' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect();
    folder(app).join(format!("{name}.png"))
}

fn data_url(png: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    )
}

/// Shrinks the icon to `ICON_SIZE` and encodes it as PNG. A near-black glyph is drawn on a
/// light plate first, so it stays visible on the dark toolbar.
fn normalize(image: DynamicImage) -> Option<Vec<u8>> {
    encode_png(&finish_icon(image))
}

fn fit_icon(image: DynamicImage) -> DynamicImage {
    if image.width() > ICON_SIZE || image.height() > ICON_SIZE {
        image.resize(ICON_SIZE, ICON_SIZE, FilterType::Lanczos3)
    } else {
        image
    }
}

fn finish_icon(image: DynamicImage) -> RgbaImage {
    let fitted = fit_icon(image).to_rgba8();
    if needs_light_plate(&fitted) {
        light_plate(&fitted)
    } else {
        fitted
    }
}

fn encode_png(image: &RgbaImage) -> Option<Vec<u8>> {
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .ok()?;
    Some(png)
}

/// Data URL of a dark glyph placed on a light plate. `None` when the icon is already
/// readable, so the tab keeps the site's own favicon URL.
pub fn contrasted_data_url(bytes: &[u8]) -> Option<String> {
    let fitted = fit_icon(image::load_from_memory(bytes).ok()?).to_rgba8();
    if !needs_light_plate(&fitted) {
        return None;
    }
    Some(data_url(&encode_png(&light_plate(&fitted))?))
}

/// Cached PNG with a light plate, when the stored glyph would disappear on the toolbar.
fn lift_png(bytes: &[u8]) -> Option<Vec<u8>> {
    let fitted = fit_icon(image::load_from_memory(bytes).ok()?).to_rgba8();
    if !needs_light_plate(&fitted) {
        return None;
    }
    encode_png(&light_plate(&fitted))
}

/// Relative luminance, 0 for black and 1 for white. Gamma is left linear: the test is
/// "is this nearly black", not a display-referred color.
fn luminance(pixel: &Rgba<u8>) -> f32 {
    let channel = |value: u8| f32::from(value) / 255.0;
    0.2126 * channel(pixel[0]) + 0.7152 * channel(pixel[1]) + 0.0722 * channel(pixel[2])
}

fn chroma(pixel: &Rgba<u8>) -> f32 {
    let red = pixel[0];
    let green = pixel[1];
    let blue = pixel[2];
    f32::from(red.max(green).max(blue) - red.min(green).min(blue)) / 255.0
}

/// True when most of the opaque pixels are a near-black, unsaturated glyph. A red or
/// blue mark stays as it is, and so does a black mark that already sits on a light tile.
fn needs_light_plate(image: &RgbaImage) -> bool {
    let mut opaque = 0u32;
    let mut lost = 0u32;
    let mut light = 0u32;
    for pixel in image.pixels() {
        if pixel[3] < 128 {
            continue;
        }
        opaque += 1;
        let luma = luminance(pixel);
        if luma >= 0.72 {
            light += 1;
        } else if luma < 0.28 && chroma(pixel) < 0.18 {
            lost += 1;
        }
    }
    if opaque < 8 {
        return false;
    }
    let opaque = opaque as f32;
    lost as f32 / opaque >= 0.6 && light as f32 / opaque < 0.12
}

/// Light rounded tile behind `source`. A glyph that already has padding is left at its
/// size. One that fills the canvas is shrunk so the tile shows as a rim.
fn light_plate(source: &RgbaImage) -> RgbaImage {
    let (width, height) = source.dimensions();
    let mut canvas = RgbaImage::new(width, height);
    let radius = (width.min(height) as f32 * 0.22).clamp(2.0, 8.0);
    fill_rounded(&mut canvas, radius);

    let wanted = (width.min(height) / 8).max(1);
    let icon = if content_margin(source) >= wanted {
        source.clone()
    } else {
        let inner_w = width.saturating_sub(wanted * 2).max(1);
        let inner_h = height.saturating_sub(wanted * 2).max(1);
        let scaled = image::imageops::resize(source, inner_w, inner_h, FilterType::Lanczos3);
        let mut placed = RgbaImage::new(width, height);
        let x = i64::from((width - inner_w) / 2);
        let y = i64::from((height - inner_h) / 2);
        image::imageops::overlay(&mut placed, &scaled, x, y);
        placed
    };
    image::imageops::overlay(&mut canvas, &icon, 0, 0);
    canvas
}

fn content_margin(image: &RgbaImage) -> u32 {
    let (width, height) = image.dimensions();
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut any = false;
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[3] < 32 {
            continue;
        }
        any = true;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    if !any {
        return 0;
    }
    let right = width.saturating_sub(max_x + 1);
    let bottom = height.saturating_sub(max_y + 1);
    min_x.min(min_y).min(right).min(bottom)
}

fn fill_rounded(image: &mut RgbaImage, radius: f32) {
    let (width, height) = image.dimensions();
    let w = width as f32;
    let h = height as f32;
    let radius = radius.min(w / 2.0).min(h / 2.0);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let coverage = rounded_coverage(x as f32 + 0.5, y as f32 + 0.5, w, h, radius);
        if coverage <= 0.0 {
            continue;
        }
        let alpha = (255.0 * coverage).round() as u8;
        *pixel = Rgba([244, 244, 245, alpha]);
    }
}

/// How much of the pixel sits inside a rounded rectangle. 1 is fully inside.
fn rounded_coverage(px: f32, py: f32, width: f32, height: f32, radius: f32) -> f32 {
    let qx = (px - width / 2.0).abs() - width / 2.0 + radius;
    let qy = (py - height / 2.0).abs() - height / 2.0 + radius;
    let distance = qx.max(qy).min(0.0) + qx.max(0.0).hypot(qy.max(0.0)) - radius;
    (0.5 - distance).clamp(0.0, 1.0)
}

fn store(app: &AppHandle, host: &str, image: DynamicImage) -> bool {
    let Some(png) = normalize(image) else {
        return false;
    };
    let url = data_url(&png);
    let changed = {
        let mut state = state().lock().expect("favicons lock");
        state.failed.remove(host);
        let changed = state.icons.get(host) != Some(&url);
        if changed {
            state.icons.insert(host.to_string(), url.clone());
        }
        changed
    };
    if !changed {
        return true;
    }
    let path = file_for(app, host);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, &png);
    let _ = app.emit(
        "favicons-changed",
        SiteFavicon {
            host: host.to_string(),
            data_url: url,
        },
    );
    true
}

/// Saves the icon a tab reported for `page_url`.
pub fn remember(app: &AppHandle, page_url: &str, bytes: &[u8]) {
    let Some(host) = host_of(page_url) else {
        return;
    };
    if let Ok(image) = image::load_from_memory(bytes) {
        store(app, &host, image);
    }
}

/// Icons already known for `urls`. Hosts without one are fetched in the background and
/// arrive later through `favicons-changed`.
pub fn lookup(app: &AppHandle, urls: &[String]) -> HashMap<String, String> {
    let hosts: HashSet<String> = urls.iter().filter_map(|url| host_of(url)).collect();
    let mut found = HashMap::new();
    let mut missing = Vec::new();
    {
        let state = state().lock().expect("favicons lock");
        for host in hosts {
            match state.icons.get(&host) {
                Some(url) => {
                    found.insert(host, url.clone());
                }
                None => missing.push(host),
            }
        }
    }
    let mut fetch = Vec::new();
    for host in missing {
        match fs::read(file_for(app, &host)) {
            Ok(png) if !png.is_empty() => {
                let png = match lift_png(&png) {
                    Some(lifted) => {
                        let _ = fs::write(file_for(app, &host), &lifted);
                        lifted
                    }
                    None => png,
                };
                let url = data_url(&png);
                state()
                    .lock()
                    .expect("favicons lock")
                    .icons
                    .insert(host.clone(), url.clone());
                found.insert(host, url);
            }
            _ => fetch.push(host),
        }
    }
    enqueue(app, fetch);
    found
}

fn enqueue(app: &AppHandle, hosts: Vec<String>) {
    let spawn = {
        let mut state = state().lock().expect("favicons lock");
        for host in hosts {
            if state.pending.contains(&host) || state.failed.contains(&host) {
                continue;
            }
            state.pending.insert(host.clone());
            state.queue.push_back(host);
        }
        let wanted = state
            .queue
            .len()
            .min(MAX_WORKERS.saturating_sub(state.workers));
        state.workers += wanted;
        wanted
    };
    for _ in 0..spawn {
        let app = app.clone();
        std::thread::spawn(move || work(&app));
    }
}

fn work(app: &AppHandle) {
    loop {
        let next = {
            let mut state = state().lock().expect("favicons lock");
            let next = state.queue.pop_front();
            if next.is_none() {
                state.workers = state.workers.saturating_sub(1);
            }
            next
        };
        let Some(host) = next else {
            return;
        };
        let image = fetch_icon(&host);
        let stored = image.is_some_and(|image| store(app, &host, image));
        let mut state = state().lock().expect("favicons lock");
        state.pending.remove(&host);
        if !stored && !state.icons.contains_key(&host) {
            state.failed.insert(host);
        }
    }
}

fn get(url: &str) -> Option<ureq::Response> {
    ureq::get(url)
        .timeout(TIMEOUT)
        .set("User-Agent", USER_AGENT)
        .call()
        .ok()
}

fn read_limited(response: ureq::Response, limit: u64) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(limit)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(bytes)
}

fn fetch_icon(host: &str) -> Option<DynamicImage> {
    let mut candidates = Vec::new();
    for scheme in ["https", "http"] {
        let page = format!("{scheme}://{host}/");
        let Some(response) = get(&page) else {
            continue;
        };
        let base = Url::parse(response.get_url())
            .or_else(|_| Url::parse(&page))
            .ok()?;
        if let Some(html) = read_limited(response, MAX_PAGE_BYTES) {
            let html = String::from_utf8_lossy(&html);
            candidates.extend(
                icon_links(&html)
                    .into_iter()
                    .filter_map(|href| base.join(&href).ok()),
            );
        }
        candidates.extend(base.join("/favicon.ico").ok());
        break;
    }
    if candidates.is_empty() {
        candidates.extend(Url::parse(&format!("https://{host}/favicon.ico")).ok());
    }
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|url| seen.insert(url.to_string()))
        .find_map(|url| load_icon(&url))
}

fn load_icon(url: &Url) -> Option<DynamicImage> {
    let bytes = if url.scheme() == "data" {
        decode_data_url(url.as_str())?
    } else if url.scheme() == "http" || url.scheme() == "https" {
        read_limited(get(url.as_str())?, MAX_ICON_BYTES)?
    } else {
        return None;
    };
    image::load_from_memory(&bytes).ok()
}

fn decode_data_url(url: &str) -> Option<Vec<u8>> {
    let (meta, data) = url.strip_prefix("data:")?.split_once(',')?;
    if !meta.ends_with(";base64") {
        return None;
    }
    base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .ok()
}

/// `href`s of the page's icon links, best first. SVG icons are skipped: they cannot be decoded.
fn icon_links(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut icons = Vec::new();
    let mut touch = Vec::new();
    let mut from = 0;
    while let Some(start) = lower[from..].find("<link") {
        let start = from + start;
        let Some(end) = lower[start..].find('>') else {
            break;
        };
        let end = start + end;
        from = end + 1;
        let tag = &html[start + 5..end];
        let attributes = parse_attributes(tag);
        let Some(rel) = attributes.get("rel") else {
            continue;
        };
        let Some(href) = attributes.get("href").filter(|href| !href.is_empty()) else {
            continue;
        };
        let svg = attributes
            .get("type")
            .is_some_and(|kind| kind.contains("svg"))
            || href
                .to_ascii_lowercase()
                .split(['?', '#'])
                .next()
                .unwrap_or("")
                .ends_with(".svg");
        if svg {
            continue;
        }
        let rel = rel.to_ascii_lowercase();
        let tokens: Vec<&str> = rel.split_ascii_whitespace().collect();
        if tokens.contains(&"icon") {
            icons.push(decode_entities(href));
        } else if tokens
            .iter()
            .any(|token| token.starts_with("apple-touch-icon"))
        {
            touch.push(decode_entities(href));
        }
    }
    icons.extend(touch);
    icons
}

fn parse_attributes(tag: &str) -> HashMap<String, String> {
    let mut attributes = HashMap::new();
    let chars: Vec<char> = tag.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        while index < chars.len() && (chars[index].is_whitespace() || chars[index] == '/') {
            index += 1;
        }
        let name_start = index;
        while index < chars.len()
            && !chars[index].is_whitespace()
            && chars[index] != '='
            && chars[index] != '/'
        {
            index += 1;
        }
        let name: String = chars[name_start..index]
            .iter()
            .collect::<String>()
            .to_ascii_lowercase();
        while index < chars.len() && chars[index].is_whitespace() {
            index += 1;
        }
        let mut value = String::new();
        if index < chars.len() && chars[index] == '=' {
            index += 1;
            while index < chars.len() && chars[index].is_whitespace() {
                index += 1;
            }
            if index < chars.len() && (chars[index] == '"' || chars[index] == '\'') {
                let quote = chars[index];
                index += 1;
                let value_start = index;
                while index < chars.len() && chars[index] != quote {
                    index += 1;
                }
                value = chars[value_start..index].iter().collect();
                index += 1;
            } else {
                let value_start = index;
                while index < chars.len() && !chars[index].is_whitespace() {
                    index += 1;
                }
                value = chars[value_start..index].iter().collect();
            }
        }
        if !name.is_empty() {
            attributes.entry(name).or_insert(value);
        } else if index == name_start {
            index += 1;
        }
    }
    attributes
}

fn decode_entities(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&#38;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use image::{ImageFormat, Rgba, RgbaImage};

    use super::{content_margin, contrasted_data_url, host_of, icon_links, light_plate, needs_light_plate};

    fn png_of(image: &RgbaImage) -> Vec<u8> {
        let mut png = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();
        png
    }

    #[test]
    fn icon_links_prefer_icons_and_skip_svg() {
        let html = r#"<head>
            <link rel="apple-touch-icon" href="/touch.png">
            <LINK REL="Shortcut Icon" HREF='/favicon-32.png?v=1&amp;x=2'>
            <link rel="icon" type="image/svg+xml" href="/icon.svg">
            <link rel=stylesheet href=/site.css>
            <link href="/alt.ico" rel="alternate icon">
        </head>"#;
        assert_eq!(
            icon_links(html),
            vec!["/favicon-32.png?v=1&x=2", "/alt.ico", "/touch.png"]
        );
    }

    #[test]
    fn only_web_hosts_have_icons() {
        assert_eq!(host_of("https://GitHub.com/a"), Some("github.com".into()));
        assert_eq!(host_of("ce://bookmarks"), None);
        assert_eq!(host_of("file:///C:/x.html"), None);
    }

    #[test]
    fn a_black_glyph_is_drawn_on_a_light_plate() {
        let mut glyph = RgbaImage::new(32, 32);
        for x in 10..22 {
            for y in 8..24 {
                glyph.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        assert!(needs_light_plate(&glyph));
        assert!(content_margin(&glyph) > 4);
        let plated = light_plate(&glyph);
        let backdrop = plated.get_pixel(4, 16);
        assert!(backdrop[0] > 200 && backdrop[3] > 200);
        let mark = plated.get_pixel(16, 16);
        assert!(mark[0] < 40 && mark[3] > 200);
        assert!(!needs_light_plate(&plated));
        assert!(contrasted_data_url(&png_of(&glyph)).is_some());
    }

    #[test]
    fn a_full_bleed_black_icon_keeps_a_light_rim() {
        let icon = RgbaImage::from_pixel(32, 32, Rgba([0, 0, 0, 255]));
        let plated = light_plate(&icon);
        let rim = plated.get_pixel(1, 16);
        assert!(rim[0] > 200 && rim[3] > 200);
        assert!(!needs_light_plate(&plated));
    }

    #[test]
    fn colored_and_light_icons_stay_untouched() {
        let red = RgbaImage::from_pixel(16, 16, Rgba([228, 67, 50, 255]));
        assert!(!needs_light_plate(&red));
        assert!(contrasted_data_url(&png_of(&red)).is_none());

        let mut white = RgbaImage::new(32, 32);
        for x in 10..22 {
            for y in 8..24 {
                white.put_pixel(x, y, Rgba([250, 250, 250, 255]));
            }
        }
        assert!(!needs_light_plate(&white));
    }
}
