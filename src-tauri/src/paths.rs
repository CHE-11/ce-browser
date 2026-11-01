use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use url::Url;

/// Sync folder that holds every profile. Empty until a folder is chosen. The path itself stays
/// in app data so the browser can find the folder.
static STORAGE_ROOT: Mutex<Option<PathBuf>> = Mutex::new(None);

pub const INTERNAL_SCHEME: &str = "ce://";
pub const NEW_TAB: &str = "ce://newtab";
pub const BOOKMARKS: &str = "ce://bookmarks";

pub fn root_data_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("ce-browser"))
}

pub fn data_dir(app: &AppHandle) -> PathBuf {
    profile_data_dir(app, &crate::profiles::active_id())
}

pub fn storage_root() -> Option<PathBuf> {
    STORAGE_ROOT.lock().ok().and_then(|guard| guard.clone())
}

pub fn set_storage_root(path: Option<PathBuf>) {
    if let Ok(mut guard) = STORAGE_ROOT.lock() {
        *guard = path;
    }
}

/// Folder that holds `profiles.json` and one folder per profile: the sync folder when one is set,
/// otherwise app data.
pub fn data_root(app: &AppHandle) -> PathBuf {
    storage_root().unwrap_or_else(|| root_data_dir(app))
}

pub fn profiles_registry(app: &AppHandle) -> PathBuf {
    data_root(app).join("profiles.json")
}

pub fn machine_sync_file(app: &AppHandle) -> PathBuf {
    root_data_dir(app).join("sync-folder.json")
}

pub fn profile_data_dir(app: &AppHandle, id: &str) -> PathBuf {
    data_root(app).join(crate::profiles::folder_name(app, id))
}

pub fn data_file(app: &AppHandle, name: &str) -> PathBuf {
    data_dir(app).join(name)
}

/// Shared by every webview. WebView2 only shares one browser process (and its cookies and
/// extensions) between webviews that use the same folder and the same environment options.
pub fn root_local_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_local_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("ce-browser-local"))
}

pub fn profile_dir(app: &AppHandle) -> PathBuf {
    profile_webview_dir(app, &crate::profiles::active_id())
}

pub fn profile_webview_dir(app: &AppHandle, id: &str) -> PathBuf {
    let root = root_local_dir(app);
    if id == crate::profiles::DEFAULT_ID {
        root.join("profile")
    } else {
        root.join("profiles").join(id)
    }
}

pub fn extensions_dir(app: &AppHandle) -> PathBuf {
    data_dir(app).join("extensions")
}

/// Exe that shortcuts, relaunch commands and the browser registration start. A dev build only
/// works while the Vite server runs, so it points them at the release build beside it when one exists.
pub fn launch_exe() -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    if !tauri::is_dev() {
        return Ok(exe);
    }
    let release = exe
        .parent()
        .and_then(|dir| dir.parent())
        .zip(exe.file_name())
        .map(|(target, name)| target.join("release").join(name));
    Ok(release.filter(|path| path.is_file()).unwrap_or(exe))
}

/// Origin that serves the React app: the Vite server in dev, `tauri.localhost` in a build.
pub fn app_origin(app: &AppHandle) -> Url {
    if tauri::is_dev() {
        if let Some(url) = app.config().build.dev_url.clone() {
            return url;
        }
    }
    Url::parse("http://tauri.localhost/").expect("static url")
}

/// `ce://bookmarks` -> `<app origin>/#/bookmarks`.
pub fn internal_url(app: &AppHandle, page: &str) -> Url {
    let mut url = app_origin(app);
    url.set_path("/");
    url.set_query(None);
    url.set_fragment(Some(&format!("/{}", page.trim_start_matches('/'))));
    url
}

/// The address bar and saved data show `ce://<page>` instead of the app origin.
pub fn display_url(app: &AppHandle, raw: &str) -> String {
    let Ok(url) = Url::parse(raw) else {
        return raw.to_string();
    };
    if is_app_url(&app_origin(app), &url) {
        let page = url
            .fragment()
            .map(|fragment| fragment.trim_start_matches('/'))
            .filter(|page| !page.is_empty() && !page.starts_with("popup"))
            .unwrap_or("newtab");
        return format!("{INTERNAL_SCHEME}{page}");
    }
    raw.to_string()
}

/// Matches the app origin over `http` or `https`. HTTPS-only mode used to upgrade
/// `http://tauri.localhost`, so saved tabs can hold the `https` form, which serves nothing.
fn is_app_url(origin: &Url, url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && url.host_str() == origin.host_str()
        && url.port() == origin.port()
}

/// Address bar text -> a URL to load. Anything that does not look like an address is a search.
pub fn resolve_input(app: &AppHandle, input: &str, search_url: &str) -> Url {
    let text = input.trim();
    if text.is_empty() {
        return internal_url(app, "newtab");
    }
    if let Some(page) = text.strip_prefix(INTERNAL_SCHEME) {
        return internal_url(app, page);
    }
    // `?` in front always searches, even for text that looks like an address.
    if let Some(query) = text.strip_prefix('?').map(str::trim).filter(|query| !query.is_empty()) {
        return search(search_url, query);
    }
    let has_scheme = [
        "http://",
        "https://",
        "file:",
        "about:",
        "data:",
        "chrome-extension://",
        "view-source:",
    ]
    .iter()
    .any(|scheme| text.to_ascii_lowercase().starts_with(scheme));
    if has_scheme {
        if let Ok(url) = Url::parse(text) {
            let origin = app_origin(app);
            if is_app_url(&origin, &url) && url.scheme() != origin.scheme() {
                let mut fixed = url;
                let _ = fixed.set_scheme(origin.scheme());
                return fixed;
            }
            return url;
        }
    }
    if !text.contains(char::is_whitespace) && looks_like_host(text) {
        let scheme = if is_local_host(text) { "http" } else { "https" };
        if let Ok(url) = Url::parse(&format!("{scheme}://{text}")) {
            return url;
        }
    }
    search(search_url, text)
}

pub fn search(search_url: &str, query: &str) -> Url {
    let encoded: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    let target = if search_url.contains("%s") {
        search_url.replace("%s", &encoded)
    } else {
        format!("{search_url}{encoded}")
    };
    Url::parse(&target).unwrap_or_else(|_| {
        Url::parse(&format!("https://www.google.com/search?q={encoded}")).expect("static url")
    })
}

/// A startup pin: `ce://` pages, `http`, `https`, `file`, or a bare host. Search text is left out.
pub fn pin_url(app: &AppHandle, input: &str, search_url: &str) -> Option<String> {
    let text = input.trim();
    if text.is_empty() || text.len() > 2048 || text.contains(char::is_whitespace) {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    let allowed = lower.starts_with("ce://")
        || lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("file:")
        || looks_like_host(text);
    if !allowed {
        return None;
    }
    let resolved = resolve_input(app, text, search_url);
    let display = pin_key(&display_url(app, resolved.as_str()));
    if display.is_empty() {
        None
    } else {
        Some(display)
    }
}

/// Compares pin URLs so a trailing slash on the host or path still matches.
pub fn pin_key(url: &str) -> String {
    let url = url.trim();
    let Some(stripped) = url.strip_suffix('/') else {
        return url.to_string();
    };
    let Some((_, after)) = stripped.split_once("://") else {
        return url.to_string();
    };
    if after.is_empty() {
        return url.to_string();
    }
    stripped.to_string()
}

struct PinParts {
    scheme: String,
    host: String,
    port: u16,
    /// Path without a trailing slash. Empty is the site root.
    path: String,
}

fn pin_parts(raw: &str) -> Option<PinParts> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(page) = trimmed
        .get(..5)
        .filter(|prefix| prefix.eq_ignore_ascii_case("ce://"))
        .map(|_| &trimmed[5..])
    {
        let page = page
            .split(['?', '#'])
            .next()
            .unwrap_or("")
            .trim_matches('/')
            .to_ascii_lowercase();
        return Some(PinParts {
            scheme: "ce".into(),
            host: page,
            port: 0,
            path: String::new(),
        });
    }
    let url = Url::parse(trimmed).ok()?;
    let raw_host = url.host_str().unwrap_or("").to_ascii_lowercase();
    let host = raw_host
        .strip_prefix("www.")
        .unwrap_or(&raw_host)
        .to_string();
    let scheme = url.scheme().to_ascii_lowercase();
    let mut path = url.path().to_string();
    while path.ends_with('/') {
        path.pop();
    }
    if path == "/" {
        path.clear();
    }
    Some(PinParts {
        scheme,
        host,
        port: url.port_or_known_default().unwrap_or(0),
        path,
    })
}

fn schemes_match(left: &str, right: &str) -> bool {
    left == right
        || (matches!(left, "http" | "https") && matches!(right, "http" | "https"))
}

fn default_port(parts: &PinParts) -> bool {
    matches!(
        (parts.scheme.as_str(), parts.port),
        ("http", 80) | ("https", 443)
    )
}

/// `http://` and `https://` on their usual ports are the same pin. A custom port stays distinct.
fn ports_match(left: &PinParts, right: &PinParts) -> bool {
    left.port == right.port
        || (left.scheme != right.scheme
            && schemes_match(&left.scheme, &right.scheme)
            && default_port(left)
            && default_port(right))
}

fn paths_align(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    let (shorter, longer) = if left.len() <= right.len() {
        (left, right)
    } else {
        (right, left)
    };
    longer.starts_with(shorter)
        && (shorter.is_empty() || longer[shorter.len()..].starts_with('/'))
}

/// A saved pin and an open tab are the same slot. A redirect, `www`, or a path under the pin still matches.
pub fn same_pin(pin: &str, url: &str) -> bool {
    let Some(pin) = pin_parts(pin) else {
        return false;
    };
    let Some(url) = pin_parts(url) else {
        return false;
    };
    schemes_match(&pin.scheme, &url.scheme)
        && pin.host == url.host
        && ports_match(&pin, &url)
        && paths_align(&pin.path, &url.path)
}

/// The same address, not merely a page under it. `www` and a trailing slash do not count as different.
pub fn same_pin_page(pin: &str, url: &str) -> bool {
    let Some(pin) = pin_parts(pin) else {
        return false;
    };
    let Some(url) = pin_parts(url) else {
        return false;
    };
    schemes_match(&pin.scheme, &url.scheme)
        && pin.host == url.host
        && ports_match(&pin, &url)
        && pin.path == url.path
}

/// Same scheme, host and port, whatever the path. A pin sent to a login page is still on its site.
pub fn same_pin_site(pin: &str, url: &str) -> bool {
    let Some(pin) = pin_parts(pin) else {
        return false;
    };
    let Some(url) = pin_parts(url) else {
        return false;
    };
    schemes_match(&pin.scheme, &url.scheme) && pin.host == url.host && ports_match(&pin, &url)
}

/// Longer paths are more specific, so `/mail` is claimed before the site root.
pub fn pin_specificity(url: &str) -> usize {
    pin_parts(url).map(|parts| parts.path.len()).unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinMatch {
    /// Index of the open tab that already stands for this pin.
    Open(usize),
    /// Another saved pin already claimed that same address.
    Duplicate,
    /// Nothing open matches, so the pin should be created.
    Missing,
}

/// Pairs each saved pin with one open tab, given as `(url, pinned)`. A navigated, `www`, or
/// trailing-slash address matches first. A pinned tab elsewhere on the pin's site is the fallback.
pub fn match_pins(pins: &[String], open: &[(&str, bool)]) -> Vec<PinMatch> {
    let mut claimed = vec![false; open.len()];
    let mut order: Vec<usize> = (0..pins.len()).collect();
    order.sort_by(|&left, &right| pin_specificity(&pins[right]).cmp(&pin_specificity(&pins[left])));
    let mut matches = vec![PinMatch::Missing; pins.len()];
    for &index in &order {
        if pin_key(&pins[index]).is_empty() {
            matches[index] = PinMatch::Duplicate;
            continue;
        }
        let wanted = pin_specificity(&pins[index]);
        let best = open
            .iter()
            .enumerate()
            .filter(|(at, (url, _))| !claimed[*at] && same_pin(&pins[index], url))
            .min_by_key(|(at, (url, pinned))| {
                (!pinned, pin_specificity(url).abs_diff(wanted), *at)
            })
            .map(|(at, _)| at);
        if let Some(at) = best {
            claimed[at] = true;
            matches[index] = PinMatch::Open(at);
        }
    }
    for &index in &order {
        if matches[index] != PinMatch::Missing {
            continue;
        }
        let fallback = (0..open.len()).find(|&at| {
            let (url, pinned) = open[at];
            pinned && !claimed[at] && same_pin_site(&pins[index], url)
        });
        if let Some(at) = fallback {
            claimed[at] = true;
            matches[index] = PinMatch::Open(at);
        }
    }
    for &index in &order {
        if matches[index] != PinMatch::Missing {
            continue;
        }
        let duplicate = matches.iter().enumerate().any(|(other, found)| {
            other != index
                && matches!(found, PinMatch::Open(_))
                && same_pin_page(&pins[index], &pins[other])
        }) || open
            .iter()
            .enumerate()
            .any(|(at, (url, _))| claimed[at] && same_pin_page(&pins[index], url));
        if duplicate {
            matches[index] = PinMatch::Duplicate;
        }
    }
    matches
}

fn looks_like_host(text: &str) -> bool {
    let host = text
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .rsplit('@')
        .next()
        .unwrap_or_default();
    let host_only = host.split(':').next().unwrap_or_default();
    if is_local_host(text) {
        return true;
    }
    if host_only.parse::<std::net::Ipv4Addr>().is_ok() {
        return true;
    }
    let Some((name, tld)) = host_only.rsplit_once('.') else {
        return false;
    };
    !name.is_empty()
        && tld.len() >= 2
        && tld.chars().all(|character| character.is_ascii_alphabetic())
}

/// Security origin for an `http` or `https` page, such as `https://example.com:8443`.
pub fn http_origin(url: &str) -> Option<String> {
    let url = Url::parse(url).ok()?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return None;
    }
    let origin = url.origin().ascii_serialization();
    (origin != "null").then_some(origin)
}

/// The part of a host a person registers, such as `example.co.uk` for `www.example.co.uk`.
/// IP addresses, `localhost` and unknown suffixes give the host itself.
pub fn registrable_domain(host: &str) -> String {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() || host.parse::<std::net::IpAddr>().is_ok() || host.starts_with('[') {
        return host;
    }
    match psl::domain_str(&host) {
        Some(domain) => domain.to_string(),
        None => host,
    }
}

/// Registrable domain of an `http` or `https` address. Empty for other schemes.
pub fn site_of(url: &str) -> String {
    let Ok(url) = Url::parse(url) else {
        return String::new();
    };
    if url.scheme() != "http" && url.scheme() != "https" {
        return String::new();
    }
    url.host_str().map(registrable_domain).unwrap_or_default()
}

fn is_local_host(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower == "localhost"
        || lower.starts_with("localhost:")
        || lower.starts_with("localhost/")
        || lower.starts_with("127.0.0.1")
}

#[cfg(test)]
mod tests {
    use super::{
        PinMatch, http_origin, match_pins, pin_key, registrable_domain, same_pin, same_pin_page,
        site_of,
    };

    #[test]
    fn the_registrable_domain_drops_subdomains_but_keeps_public_suffixes() {
        assert_eq!(registrable_domain("www.example.co.uk"), "example.co.uk");
        assert_eq!(registrable_domain("docs.github.com"), "github.com");
        assert_eq!(registrable_domain("localhost"), "localhost");
        assert_eq!(registrable_domain("127.0.0.1"), "127.0.0.1");
        assert_eq!(site_of("https://mail.google.com/inbox"), "google.com");
        assert_eq!(site_of("ce://settings"), "");
    }

    #[test]
    fn http_origin_keeps_the_site_and_drops_the_path() {
        assert_eq!(
            http_origin("https://example.com/docs?q=1"),
            Some("https://example.com".to_string())
        );
        assert_eq!(
            http_origin("https://example.com:443/docs"),
            Some("https://example.com".to_string())
        );
        assert_eq!(
            http_origin("http://localhost:1420/app"),
            Some("http://localhost:1420".to_string())
        );
        assert_eq!(http_origin("ce://newtab"), None);
        assert_eq!(http_origin("file:///C:/page.html"), None);
    }

    #[test]
    fn a_navigated_pin_still_matches_the_saved_url() {
        assert!(same_pin("https://a.example", "https://a.example/inbox"));
        assert!(same_pin("https://a.example", "https://www.a.example/"));
        assert!(same_pin("http://a.example/mail", "https://a.example/mail/u/0"));
        assert!(same_pin("ce://bookmarks", "ce://bookmarks"));
        assert!(!same_pin("https://a.example/mail", "https://a.example/mailbox"));
        assert!(!same_pin("ce://bookmarks", "ce://history"));
        assert!(same_pin_page(
            "https://www.a.example/",
            "https://a.example"
        ));
        assert!(!same_pin_page(
            "https://a.example",
            "https://a.example/inbox"
        ));
    }

    #[test]
    fn a_pinned_tab_sent_to_a_login_page_still_matches_its_pin() {
        let pins = vec![
            "https://app.example/today".to_string(),
            "https://cms.example".to_string(),
        ];
        let open = [
            ("https://cms.example/sign-in?next=%2F", true),
            ("https://app.example/auth/login", true),
            ("https://other.example/auth/login", true),
        ];
        assert_eq!(
            match_pins(&pins, &open),
            vec![PinMatch::Open(1), PinMatch::Open(0)]
        );
        let loose = [("https://app.example/auth/login", false)];
        assert_eq!(
            match_pins(&pins[..1], &loose),
            vec![PinMatch::Missing]
        );
    }

    #[test]
    fn pin_key_ignores_one_trailing_slash() {
        assert_eq!(
            pin_key("https://mail.example.com/"),
            "https://mail.example.com"
        );
        assert_eq!(
            pin_key("https://mail.example.com/inbox/"),
            "https://mail.example.com/inbox"
        );
        assert_eq!(pin_key("file:///"), "file:///");
    }
}
