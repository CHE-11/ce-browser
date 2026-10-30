use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use url::Url;

/// Sync folder that holds every profile. Empty until a folder is chosen. The path itself stays
/// in app data so the browser can find the folder.
static STORAGE_ROOT: Mutex<Option<PathBuf>> = Mutex::new(None);

pub const INTERNAL_SCHEME: &str = "ce://";
pub const NEW_TAB: &str = "ce://newtab";

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

/// `profiles.json` for every profile. In the sync folder when one is set, otherwise app data.
pub fn profiles_registry(app: &AppHandle) -> PathBuf {
    if let Some(root) = storage_root() {
        root.join("profiles.json")
    } else {
        root_data_dir(app).join("profiles.json")
    }
}

pub fn machine_sync_file(app: &AppHandle) -> PathBuf {
    root_data_dir(app).join("sync-folder.json")
}

pub fn profile_data_dir(app: &AppHandle, id: &str) -> PathBuf {
    if let Some(root) = storage_root() {
        return root.join(crate::profiles::sync_dir_name(app, id));
    }
    appdata_profile_dir(&root_data_dir(app), id)
}

pub fn appdata_profile_dir(appdata: &std::path::Path, id: &str) -> PathBuf {
    if id == crate::profiles::DEFAULT_ID {
        appdata.to_path_buf()
    } else {
        appdata.join("profiles").join(id)
    }
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
    let origin = app_origin(app);
    if url.origin() == origin.origin() {
        let page = url
            .fragment()
            .map(|fragment| fragment.trim_start_matches('/'))
            .filter(|page| !page.is_empty() && !page.starts_with("popup"))
            .unwrap_or("newtab");
        return format!("{INTERNAL_SCHEME}{page}");
    }
    raw.to_string()
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

fn is_local_host(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower == "localhost"
        || lower.starts_with("localhost:")
        || lower.starts_with("localhost/")
        || lower.starts_with("127.0.0.1")
}

#[cfg(test)]
mod tests {
    use super::{http_origin, pin_key};

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
