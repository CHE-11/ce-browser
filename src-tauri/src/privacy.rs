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
    /// `allow` (every visit), `session` (until this process exits), `deny`, or `asked`.
    pub state: String,
    /// WASAPI endpoint id. Empty uses the Windows default capture device.
    #[serde(default)]
    pub device_id: String,
    /// Friendly name matched against the page's `enumerateDevices()` label.
    #[serde(default)]
    pub device_label: String,
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
    /// Unix milliseconds when the response arrived.
    pub time: u64,
    /// Milliseconds from the request start until the response, when the start was seen.
    pub duration_ms: Option<u64>,
}

struct PrivacyState {
    grants: Mutex<Vec<PermissionGrant>>,
    containers: Mutex<Vec<Container>>,
    log: Mutex<HashMap<u32, VecDeque<NetworkEntry>>>,
    seeds: Mutex<HashMap<String, u32>>,
    /// Start time for a request still waiting on its response, keyed by tab and the request pointer.
    started: Mutex<HashMap<(u32, usize), u64>>,
}

pub fn init(app: &AppHandle) {
    let permissions: PermissionFile = files::read_json(&paths::data_file(app, PERMISSIONS));
    let containers: ContainerFile = files::read_json(&paths::data_file(app, CONTAINERS));
    let (grants, drop_session) = load_grants(permissions.grants);
    if drop_session {
        let _ = save_grants(app, &grants);
    }
    app.manage(PrivacyState {
        grants: Mutex::new(grants),
        containers: Mutex::new(normalize_containers(containers.containers)),
        log: Mutex::new(HashMap::new()),
        seeds: Mutex::new(HashMap::new()),
        started: Mutex::new(HashMap::new()),
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
    grants.push(new_grant(origin, kind, "asked"));
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
        .and_then(|grant| decided(&grant.state))
}

/// `persist` omitted or true keeps an allow across launches. `false` allows it until this process exits.
pub fn set_permission(
    app: &AppHandle,
    origin: &str,
    kind: &str,
    allow: bool,
    persist: Option<bool>,
) -> Result<(), String> {
    let origin = origin_key(origin);
    let kind = kind_key(kind);
    if origin.is_empty() || kind.is_empty() {
        return Err("That permission is not one this browser stores.".into());
    }
    let next_state = if !allow {
        "deny"
    } else if persist == Some(false) {
        "session"
    } else {
        "allow"
    };
    let guard = state(app);
    let mut grants = guard.grants.lock().expect("permissions");
    let previous = grants
        .iter()
        .find(|grant| grant.origin == origin && grant.kind == kind)
        .cloned();
    grants.retain(|grant| !(grant.origin == origin && grant.kind == kind));
    let mut grant = new_grant(origin.clone(), kind.clone(), next_state);
    if let Some(previous) = previous {
        grant.device_id = previous.device_id;
        grant.device_label = previous.device_label;
    }
    grants.push(grant);
    save_grants(app, &grants)?;
    let _ = app.emit("permissions-changed", &*grants);
    drop(grants);
    if !allow && kind == "microphone" {
        crate::tabs::eval_on_origin(
            app,
            &origin,
            "if (window.__ceStopMic) window.__ceStopMic();",
        );
    }
    Ok(())
}

/// Remembers which microphone a site should use the next time it captures audio.
pub fn set_microphone_device(
    app: &AppHandle,
    origin: &str,
    device_id: &str,
    device_label: &str,
) -> Result<(), String> {
    let origin = origin_key(origin);
    if origin.is_empty() {
        return Err("That site cannot use the microphone.".into());
    }
    let device_id = clamp_text(device_id, 300);
    let mut device_label = clamp_text(device_label, 200);
    if device_id.is_empty() {
        device_label.clear();
    }
    let guard = state(app);
    let mut grants = guard.grants.lock().expect("permissions");
    let Some(grant) = grants
        .iter_mut()
        .find(|grant| grant.origin == origin && grant.kind == "microphone")
    else {
        return Err("This site has not asked for the microphone.".into());
    };
    grant.device_id = device_id;
    grant.device_label.clone_from(&device_label);
    save_grants(app, &grants)?;
    let _ = app.emit("permissions-changed", &*grants);
    drop(grants);
    crate::tabs::eval_on_origin(app, &origin, &microphone_label_script(&device_label));
    Ok(())
}

fn decided(state: &str) -> Option<bool> {
    match state {
        "allow" | "session" => Some(true),
        "deny" => Some(false),
        _ => None,
    }
}

fn new_grant(origin: String, kind: String, state: &str) -> PermissionGrant {
    PermissionGrant {
        origin,
        kind,
        state: state.to_string(),
        device_id: String::new(),
        device_label: String::new(),
    }
}

fn clamp_text(value: &str, max: usize) -> String {
    value.trim().chars().take(max).collect()
}

fn microphone_label_script(label: &str) -> String {
    let quoted = serde_json::to_string(label).unwrap_or_else(|_| "\"\"".into());
    format!(
        "window.__ceMicLabel={quoted};try{{if({quoted})localStorage.setItem(\"ce-microphone\",{quoted});else localStorage.removeItem(\"ce-microphone\");}}catch(e){{}}"
    )
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

/// `*.localhost` covers `tauri.localhost`, which serves the `ce://` pages over plain HTTP.
fn local_host(url: &Url) -> bool {
    matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    ) || url
        .host_str()
        .is_some_and(|host| host.to_ascii_lowercase().ends_with(".localhost"))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// Remember when this WebView2 request object started. `request` is the COM pointer, which the
/// response event hands back for the same request.
pub fn note_request_started(app: &AppHandle, tab: u32, request: usize) {
    let now = now_ms();
    let guard = state(app);
    let mut started = guard.started.lock().expect("network start");
    started.retain(|_, at| now.saturating_sub(*at) < 120_000);
    if started.len() >= 4_000 {
        started.clear();
    }
    started.insert((tab, request), now);
}

pub fn take_request_started(app: &AppHandle, tab: u32, request: usize) -> Option<u64> {
    let guard = state(app);
    guard
        .started
        .lock()
        .expect("network start")
        .remove(&(tab, request))
}

pub fn record_request(
    app: &AppHandle,
    tab: u32,
    method: &str,
    url: &str,
    kind: &str,
    status: Option<i32>,
    started_at: Option<u64>,
) {
    if url.starts_with("ce:") || url.starts_with("chrome-extension:") || url.starts_with("data:") {
        return;
    }
    let time = now_ms();
    let guard = state(app);
    let mut log = guard.log.lock().expect("network log");
    let entries = log.entry(tab).or_default();
    entries.push_back(NetworkEntry {
        method: method.chars().take(16).collect(),
        url: url.chars().take(2000).collect(),
        kind: kind.chars().take(32).collect(),
        status,
        time,
        duration_ms: started_at.map(|started| time.saturating_sub(started)),
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

/// Drops `session` grants. A this-visit allow does not survive the next launch.
fn load_grants(grants: Vec<PermissionGrant>) -> (Vec<PermissionGrant>, bool) {
    let drop_session = grants.iter().any(|grant| grant.state == "session");
    (normalize_grants(grants), drop_session)
}

fn normalize_grants(grants: Vec<PermissionGrant>) -> Vec<PermissionGrant> {
    let mut kept = Vec::new();
    for grant in grants {
        if grant.state == "session" {
            continue;
        }
        let origin = origin_key(&grant.origin);
        let kind = kind_key(&grant.kind);
        let state = match grant.state.as_str() {
            "allow" => "allow",
            "asked" => "asked",
            "deny" => "deny",
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
            device_id: grant.device_id,
            device_label: grant.device_label,
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
        assert!(rewrite(&settings(), "http://tauri.localhost/#/newtab").is_none());
    }

    #[test]
    fn a_clean_url_is_left_alone() {
        assert!(rewrite(&settings(), "https://example.com/a?id=1").is_none());
    }

    fn grant(origin: &str, kind: &str, state: &str) -> PermissionGrant {
        PermissionGrant {
            origin: origin.into(),
            kind: kind.into(),
            state: state.into(),
            device_id: String::new(),
            device_label: String::new(),
        }
    }

    #[test]
    fn session_grants_do_not_survive_load() {
        let (grants, dropped) = load_grants(vec![
            PermissionGrant {
                device_id: "endpoint".into(),
                device_label: "Headset".into(),
                ..grant("https://example.com", "microphone", "session")
            },
            grant("https://example.com", "camera", "allow"),
        ]);
        assert!(dropped);
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].kind, "camera");
        let (kept, dropped) = load_grants(vec![grant("https://example.com", "camera", "allow")]);
        assert!(!dropped);
        assert_eq!(kept[0].state, "allow");
    }

    #[test]
    fn a_saved_microphone_is_kept() {
        let grants = normalize_grants(vec![PermissionGrant {
            device_id: "{endpoint}".into(),
            device_label: "Headset (AirPods)".into(),
            ..grant("https://example.com/path", "microphone", "allow")
        }]);
        assert_eq!(grants[0].origin, "https://example.com");
        assert_eq!(grants[0].device_id, "{endpoint}");
        assert_eq!(grants[0].device_label, "Headset (AirPods)");
    }

    #[test]
    fn an_unknown_permission_state_is_blocked() {
        let grants = normalize_grants(vec![grant("https://example.com", "location", "maybe")]);
        assert_eq!(grants[0].state, "deny");
    }

    #[test]
    fn a_session_allow_counts_as_granted() {
        assert_eq!(decided("session"), Some(true));
        assert_eq!(decided("allow"), Some(true));
        assert_eq!(decided("deny"), Some(false));
        assert_eq!(decided("asked"), None);
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
