//! Tracking-parameter removal, HTTPS-only, permissions, containers and the network log.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use url::Url;

use crate::{files, paths, settings};

const PERMISSIONS: &str = "permissions.json";
const CONTAINERS: &str = "containers.json";
const MAX_LOG: usize = 300;
const KINDS: &[&str] = &["camera", "microphone", "location", "notifications"];
const REFERRERS: &[&str] = &["default", "none", "origin", "same-origin"];

const TRACKING: &[&str] = &[
    "fbclid",
    "gclid",
    "gclsrc",
    "dclid",
    "gbraid",
    "wbraid",
    "mc_eid",
    "mc_cid",
    "igshid",
    "_ga",
    "vero_id",
    "oly_anon_id",
    "oly_enc_id",
    "twclid",
    "ttclid",
    "msclkid",
    "yclid",
    "ref_src",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionGrant {
    pub origin: String,
    pub kind: String,
    pub state: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct PermissionFile {
    grants: Vec<PermissionGrant>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Container {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct ContainerFile {
    containers: Vec<Container>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkEntry {
    pub method: String,
    pub url: String,
    pub kind: String,
    pub status: Option<i32>,
}

struct PrivacyState {
    grants: Mutex<Vec<PermissionGrant>>,
    containers: Mutex<Vec<Container>>,
    log: Mutex<HashMap<u32, VecDeque<NetworkEntry>>>,
    seeds: Mutex<HashMap<String, u32>>,
}

pub fn init(app: &AppHandle) {
    let permissions: PermissionFile = files::read_json(&paths::data_file(app, PERMISSIONS));
    let containers: ContainerFile = files::read_json(&paths::data_file(app, CONTAINERS));
    app.manage(PrivacyState {
        grants: Mutex::new(normalize_grants(permissions.grants)),
        containers: Mutex::new(normalize_containers(containers.containers)),
        log: Mutex::new(HashMap::new()),
        seeds: Mutex::new(HashMap::new()),
    });
}

fn state(app: &AppHandle) -> tauri::State<'_, PrivacyState> {
    app.state::<PrivacyState>()
}

pub fn grants(app: &AppHandle) -> Vec<PermissionGrant> {
    state(app).grants.lock().expect("permissions").clone()
}

/// Records that a site asked, without choosing for the user. The browser prompt still runs.
pub fn note_request(app: &AppHandle, origin: &str, kind: &str) {
    let origin = origin_key(origin);
    let kind = kind_key(kind);
    if origin.is_empty() || kind.is_empty() {
        return;
    }
    let guard = state(app);
    let mut grants = guard.grants.lock().expect("permissions");
    if grants
        .iter()
        .any(|grant| grant.origin == origin && grant.kind == kind)
    {
        return;
    }
    grants.push(PermissionGrant {
        origin,
        kind,
        state: "asked".into(),
    });
    let _ = save_grants(app, &grants);
    let _ = app.emit("permissions-changed", &*grants);
}

pub fn decision(app: &AppHandle, origin: &str, kind: &str) -> Option<bool> {
    let origin = origin_key(origin);
    let kind = kind_key(kind);
    state(app)
        .grants
        .lock()
        .expect("permissions")
        .iter()
        .find(|grant| grant.origin == origin && grant.kind == kind)
        .and_then(|grant| match grant.state.as_str() {
            "allow" => Some(true),
            "deny" => Some(false),
            _ => None,
        })
}

pub fn set_permission(
    app: &AppHandle,
    origin: &str,
    kind: &str,
    allow: bool,
) -> Result<(), String> {
    let origin = origin_key(origin);
    let kind = kind_key(kind);
    if origin.is_empty() || kind.is_empty() {
        return Err("That permission is not one this browser stores.".into());
    }
    let guard = state(app);
    let mut grants = guard.grants.lock().expect("permissions");
    grants.retain(|grant| !(grant.origin == origin && grant.kind == kind));
    grants.push(PermissionGrant {
        origin,
        kind,
        state: if allow { "allow" } else { "deny" }.into(),
    });
    save_grants(app, &grants)?;
    let _ = app.emit("permissions-changed", &*grants);
    Ok(())
}

pub fn containers(app: &AppHandle) -> Vec<Container> {
    state(app).containers.lock().expect("containers").clone()
}

pub fn create_container(app: &AppHandle, name: &str, color: &str) -> Result<Container, String> {
    let guard = state(app);
    let mut containers = guard.containers.lock().expect("containers");
    if containers.len() >= 20 {
        return Err("You can have up to 20 containers.".into());
    }
    let name = name.trim().chars().take(40).collect::<String>();
    let name = if name.is_empty() {
        format!("Container {}", containers.len() + 1)
    } else {
        name
    };
    let mut id = container_id(&name);
    let mut suffix = 2;
    while containers.iter().any(|item| item.id == id) {
        id = format!("{}-{suffix}", container_id(&name));
        suffix += 1;
    }
    let container = Container {
        id,
        name,
        color: crate::tabs::group_color_owned(color),
    };
    containers.push(container.clone());
    save_containers(app, &containers)?;
    let _ = app.emit("containers-changed", &*containers);
    Ok(container)
}

pub fn delete_container(app: &AppHandle, id: &str) -> Result<(), String> {
    let guard = state(app);
    let mut containers = guard.containers.lock().expect("containers");
    containers.retain(|item| item.id != id);
    save_containers(app, &containers)?;
    drop(containers);
    crate::tabs::clear_container(app, id);
    let list = {
        let guard = state(app);
        guard.containers.lock().expect("containers").clone()
    };
    let _ = app.emit("containers-changed", &list);
    Ok(())
}

pub fn known_container(app: &AppHandle, id: &str) -> bool {
    if id.is_empty() {
        return true;
    }
    let guard = state(app);
    guard
        .containers
        .lock()
        .expect("containers")
        .iter()
        .any(|item| item.id == id)
}

/// A stable noise seed for this host, kept until the browser exits.
pub fn fingerprint_seed(app: &AppHandle, host: &str) -> u32 {
    let host = settings::sleep_site_key(host);
    let guard = state(app);
    let mut seeds = guard.seeds.lock().expect("fingerprint seeds");
    if let Some(seed) = seeds.get(&host) {
        return *seed;
    }
    let seed = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos())
        .unwrap_or(1)
        ^ (host.len() as u32).wrapping_mul(0x9E37_79B9))
        | 1;
    seeds.insert(host, seed);
    seed
}

pub fn fingerprint_script(app: &AppHandle, url: &str) -> String {
    if !settings::get(app).fingerprint_shield {
        return String::new();
    }
    let seed = fingerprint_seed(app, url);
    format!(
        r#"(function(){{
  if (window.__ceFp) return;
  window.__ceFp = true;
  var seed = {seed};
  function nudge(value) {{ return (value + (seed & 7)) & 255; }}
  var toData = HTMLCanvasElement.prototype.toDataURL;
  HTMLCanvasElement.prototype.toDataURL = function() {{
    try {{
      var ctx = this.getContext("2d");
      if (ctx && this.width && this.height) {{
        var img = ctx.getImageData(0, 0, Math.min(this.width, 8), 1);
        img.data[0] = nudge(img.data[0]);
        ctx.putImageData(img, 0, 0);
      }}
    }} catch (e) {{}}
    return toData.apply(this, arguments);
  }};
  var read = CanvasRenderingContext2D.prototype.getImageData;
  CanvasRenderingContext2D.prototype.getImageData = function() {{
    var img = read.apply(this, arguments);
    if (img && img.data && img.data.length) img.data[0] = nudge(img.data[0]);
    return img;
  }};
  if (window.WebGLRenderingContext) {{
    var pixels = WebGLRenderingContext.prototype.readPixels;
    WebGLRenderingContext.prototype.readPixels = function() {{
      pixels.apply(this, arguments);
      var buf = arguments[6];
      if (buf && buf.length) buf[0] = nudge(buf[0]);
    }};
  }}
  if (window.AudioBuffer) {{
    var channel = AudioBuffer.prototype.getChannelData;
    AudioBuffer.prototype.getChannelData = function() {{
      var data = channel.apply(this, arguments);
      if (data && data.length) data[0] = data[0] + (seed % 5) * 1e-7;
      return data;
    }};
  }}
}})();"#,
        seed = seed,
    )
}

pub fn copy_script() -> String {
    r#"(function(){
  if (window.__ceCopy) return;
  window.__ceCopy = true;
  var names = {names};
  function tracking(name) {
    if (name.indexOf("utm_") === 0) return true;
    return names.indexOf(name) !== -1;
  }
  function clean(value) {
    try {
      var url = new URL(value, location.href);
      if (url.protocol !== "http:" && url.protocol !== "https:") return value;
      var dropped = false;
      Array.from(url.searchParams.keys()).forEach(function (key) {
        if (tracking(key)) { url.searchParams.delete(key); dropped = true; }
      });
      return dropped ? url.toString() : value;
    } catch (e) { return value; }
  }
  document.addEventListener("copy", function (event) {
    var link = event.target && event.target.closest && event.target.closest("a[href]");
    var text = link ? link.href : String(window.getSelection ? window.getSelection() : "");
    var cleaned = clean(text);
    if (!cleaned || cleaned === text || !event.clipboardData) return;
    event.preventDefault();
    event.clipboardData.setData("text/plain", cleaned);
  }, true);
})();"#
        .replace(
            "{names}",
            &serde_json::to_string(TRACKING).unwrap_or_else(|_| "[]".into()),
        )
}

/// `Some` when the navigation should be cancelled and started again at the returned URL.
pub fn rewrite_navigation(app: &AppHandle, raw: &str) -> Option<String> {
    let settings = settings::get(app);
    rewrite(&settings, raw)
}

pub fn rewrite(settings: &settings::BrowserSettings, raw: &str) -> Option<String> {
    let mut url = Url::parse(raw).ok()?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return None;
    }
    let mut changed = false;
    if settings.https_only
        && url.scheme() == "http"
        && !local_host(&url)
        && !https_exception(settings, &url)
    {
        if url.set_scheme("https").is_ok() {
            changed = true;
        }
    }
    if settings.strip_tracking {
        let pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        if pairs.iter().any(|(name, _)| is_tracking(name)) {
            let kept: Vec<(String, String)> = pairs
                .into_iter()
                .filter(|(name, _)| !is_tracking(name))
                .collect();
            if kept.is_empty() {
                url.set_query(None);
            } else {
                url.query_pairs_mut().clear().extend_pairs(kept);
            }
            changed = true;
        }
    }
    changed.then(|| url.to_string())
}

pub fn is_tracking(name: &str) -> bool {
    let name = name.trim();
    name.starts_with("utm_") || TRACKING.iter().any(|item| *item == name)
}

pub fn https_exception(settings: &settings::BrowserSettings, url: &Url) -> bool {
    let host = settings::sleep_site_key(url.as_str());
    settings
        .https_exceptions
        .iter()
        .any(|site| host == *site || host.ends_with(&format!(".{site}")))
}

fn local_host(url: &Url) -> bool {
    matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    )
}

pub fn record_request(
    app: &AppHandle,
    tab: u32,
    method: &str,
    url: &str,
    kind: &str,
    status: Option<i32>,
) {
    if url.starts_with("ce:") || url.starts_with("chrome-extension:") || url.starts_with("data:") {
        return;
    }
    let guard = state(app);
    let mut log = guard.log.lock().expect("network log");
    let entries = log.entry(tab).or_default();
    entries.push_back(NetworkEntry {
        method: method.chars().take(16).collect(),
        url: url.chars().take(2000).collect(),
        kind: kind.chars().take(32).collect(),
        status,
    });
    while entries.len() > MAX_LOG {
        entries.pop_front();
    }
}

pub fn network_log(app: &AppHandle, tab: u32) -> Vec<NetworkEntry> {
    state(app)
        .log
        .lock()
        .expect("network log")
        .get(&tab)
        .map(|entries| entries.iter().cloned().collect())
        .unwrap_or_default()
}

pub fn clear_network_log(app: &AppHandle, tab: u32) {
    state(app).log.lock().expect("network log").remove(&tab);
}

pub fn referrer_value(rule: &str, page_url: &str, request_url: &str) -> ReferrerAction {
    match rule {
        "none" => ReferrerAction::Remove,
        "origin" => page_origin(page_url)
            .map(ReferrerAction::Set)
            .unwrap_or(ReferrerAction::Remove),
        "same-origin" => {
            let page = page_origin(page_url);
            let request = page_origin(request_url);
            if page.is_some() && page == request {
                ReferrerAction::Keep
            } else {
                ReferrerAction::Remove
            }
        }
        _ => ReferrerAction::Keep,
    }
}

pub enum ReferrerAction {
    Keep,
    Remove,
    Set(String),
}

fn page_origin(raw: &str) -> Option<String> {
    let url = Url::parse(raw).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    Some(url.origin().ascii_serialization())
}

fn origin_key(raw: &str) -> String {
    if let Ok(url) = Url::parse(raw) {
        if matches!(url.scheme(), "http" | "https") {
            return url.origin().ascii_serialization();
        }
    }
    let raw = raw.trim().trim_end_matches('/');
    if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.to_string()
    } else {
        String::new()
    }
}

fn kind_key(raw: &str) -> String {
    let raw = raw.trim().to_ascii_lowercase();
    if KINDS.contains(&raw.as_str()) {
        raw
    } else {
        String::new()
    }
}

pub fn normalize_referrer(raw: &str) -> String {
    if REFERRERS.contains(&raw) {
        raw.to_string()
    } else {
        "default".into()
    }
}

fn container_id(name: &str) -> String {
    let mut id: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while id.contains("--") {
        id = id.replace("--", "-");
    }
    let id = id.trim_matches('-').chars().take(40).collect::<String>();
    if id.is_empty() {
        "container".into()
    } else {
        id
    }
}

fn normalize_grants(grants: Vec<PermissionGrant>) -> Vec<PermissionGrant> {
    let mut kept = Vec::new();
    for grant in grants {
        let origin = origin_key(&grant.origin);
        let kind = kind_key(&grant.kind);
        let state = match grant.state.as_str() {
            "allow" => "allow",
            "asked" => "asked",
            _ => "deny",
        };
        if origin.is_empty()
            || kind.is_empty()
            || kept
                .iter()
                .any(|item: &PermissionGrant| item.origin == origin && item.kind == kind)
        {
            continue;
        }
        kept.push(PermissionGrant {
            origin,
            kind,
            state: state.into(),
        });
    }
    kept
}

fn normalize_containers(containers: Vec<Container>) -> Vec<Container> {
    let mut kept = Vec::new();
    for mut container in containers {
        container.id = container_id(&container.id);
        container.name = container.name.trim().chars().take(40).collect();
        if container.id.is_empty()
            || container.name.is_empty()
            || kept.iter().any(|item: &Container| item.id == container.id)
        {
            continue;
        }
        container.color = crate::tabs::group_color_owned(&container.color);
        kept.push(container);
        if kept.len() >= 20 {
            break;
        }
    }
    kept
}

fn save_grants(app: &AppHandle, grants: &[PermissionGrant]) -> Result<(), String> {
    files::write_json(
        &paths::data_file(app, PERMISSIONS),
        &PermissionFile {
            grants: grants.to_vec(),
        },
    )
}

fn save_containers(app: &AppHandle, containers: &[Container]) -> Result<(), String> {
    files::write_json(
        &paths::data_file(app, CONTAINERS),
        &ContainerFile {
            containers: containers.to_vec(),
        },
    )
}

pub fn container_dir(app: &AppHandle, id: &str) -> std::path::PathBuf {
    paths::root_local_dir(app)
        .join("containers")
        .join(container_id(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> settings::BrowserSettings {
        let mut settings = settings::BrowserSettings::default();
        settings.strip_tracking = true;
        settings.https_only = true;
        settings
    }

    #[test]
    fn tracking_parameters_are_removed() {
        let cleaned = rewrite(
            &settings(),
            "https://example.com/a?utm_source=x&id=1&fbclid=abc&gclid=z",
        );
        let cleaned = cleaned.unwrap();
        assert!(cleaned.contains("id=1"));
        assert!(!cleaned.contains("utm_"));
        assert!(!cleaned.contains("fbclid"));
        assert!(!cleaned.contains("gclid"));
    }

    #[test]
    fn an_https_exception_stays_on_http() {
        let mut settings = settings();
        settings.https_exceptions = vec!["example.com".into()];
        assert!(rewrite(&settings, "http://www.example.com/a").is_none());
        let other = rewrite(&settings, "http://other.test/a").unwrap();
        assert!(other.starts_with("https://"));
    }

    #[test]
    fn localhost_is_not_upgraded() {
        assert!(rewrite(&settings(), "http://localhost:1420/").is_none());
        assert!(rewrite(&settings(), "http://127.0.0.1/x").is_none());
    }

    #[test]
    fn a_clean_url_is_left_alone() {
        assert!(rewrite(&settings(), "https://example.com/a?id=1").is_none());
    }

    #[test]
    fn referrer_none_removes_the_header() {
        assert!(matches!(
            referrer_value("none", "https://a.test/x", "https://b.test/y"),
            ReferrerAction::Remove
        ));
        assert!(matches!(
            referrer_value("default", "https://a.test/x", "https://b.test/y"),
            ReferrerAction::Keep
        ));
        assert!(matches!(
            referrer_value("same-origin", "https://a.test/x", "https://a.test/y"),
            ReferrerAction::Keep
        ));
        assert!(matches!(
            referrer_value("same-origin", "https://a.test/x", "https://b.test/y"),
            ReferrerAction::Remove
        ));
    }
}
