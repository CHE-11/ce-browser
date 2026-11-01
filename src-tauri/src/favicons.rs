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
use image::{DynamicImage, ImageFormat};
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

/// Squares the icon down to `ICON_SIZE` and encodes it as PNG.
fn normalize(image: DynamicImage) -> Option<Vec<u8>> {
    let image = if image.width() > ICON_SIZE || image.height() > ICON_SIZE {
        image.resize(ICON_SIZE, ICON_SIZE, FilterType::Lanczos3)
    } else {
        image
    };
    let mut png = Vec::new();
    image
        .to_rgba8()
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .ok()?;
    Some(png)
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
    use super::{host_of, icon_links};

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
}
