use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock, RwLock};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use url::Url;

use crate::settings::AdblockSettings;

const DOMAINS: &str = include_str!("../resources/adblock/domains.txt");
const PATHS: &str = include_str!("../resources/adblock/paths.txt");
const EARLY_JS: &str = include_str!("../resources/adblock/early.js");
const COSMETIC_CSS: &str = include_str!("../resources/adblock/cosmetic.css");
const STATS_DELAY: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdblockKind {
    pub kind: String,
    pub count: u32,
}

/// A host with requests blocked on the current page.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdblockHost {
    pub host: String,
    pub count: u32,
    /// Blocklist domain that matched. Same as `host` when the host itself is listed.
    pub rule: String,
    pub kinds: Vec<AdblockKind>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdblockStats {
    pub tab_id: u32,
    pub blocked: u32,
    pub hosts: Vec<AdblockHost>,
}

#[derive(Default)]
struct Config {
    enabled: bool,
    allowed: HashSet<String>,
}

struct HostHit {
    count: u32,
    rule: String,
    kinds: HashMap<&'static str, u32>,
}

#[derive(Default)]
struct TabState {
    page_host: Option<String>,
    blocked: u32,
    /// A stats event is already scheduled for this tab.
    pending: bool,
    hosts: HashMap<String, HostHit>,
}

struct PathRule {
    host: String,
    prefix: String,
    label: String,
}

fn blocklist() -> &'static HashSet<String> {
    static LIST: OnceLock<HashSet<String>> = OnceLock::new();
    LIST.get_or_init(|| {
        DOMAINS
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(str::to_ascii_lowercase)
            .collect()
    })
}

fn path_rules() -> &'static Vec<PathRule> {
    static RULES: OnceLock<Vec<PathRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        PATHS
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(|line| {
                let line = line.to_ascii_lowercase();
                let (host, rest) = line.split_once('/')?;
                if host.is_empty() || rest.is_empty() {
                    return None;
                }
                Some(PathRule {
                    host: host.to_string(),
                    prefix: format!("/{rest}"),
                    label: line,
                })
            })
            .collect()
    })
}

/// The listed `host/path` rule that covers this URL, if the host matches exactly.
fn matched_path(url: &str) -> Option<&'static str> {
    let parsed = Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https" | "ws" | "wss") {
        return None;
    }
    let host = parsed
        .host_str()?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let path = parsed.path();
    path_rules().iter().find_map(|rule| {
        (rule.host == host && path.starts_with(&rule.prefix)).then_some(rule.label.as_str())
    })
}

fn config() -> &'static RwLock<Config> {
    static CONFIG: OnceLock<RwLock<Config>> = OnceLock::new();
    CONFIG.get_or_init(|| {
        RwLock::new(Config {
            enabled: true,
            allowed: HashSet::new(),
        })
    })
}

fn tabs() -> &'static Mutex<HashMap<u32, TabState>> {
    static TABS: OnceLock<Mutex<HashMap<u32, TabState>>> = OnceLock::new();
    TABS.get_or_init(Default::default)
}

/// The form a host takes on the paused-sites list: lowercase, no trailing dot, no leading `www.`.
pub fn site_key(host: &str) -> String {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    match host.strip_prefix("www.") {
        Some(rest) if !rest.is_empty() => rest.to_string(),
        _ => host,
    }
}

pub fn configure(settings: &AdblockSettings) {
    let mut config = config().write().expect("adblock config lock");
    config.enabled = settings.enabled;
    config.allowed = settings
        .allowed_sites
        .iter()
        .map(|site| site_key(site))
        .collect();
}

fn http_host(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https" | "ws" | "wss") {
        return None;
    }
    parsed
        .host_str()
        .map(|host| host.trim_end_matches('.').to_ascii_lowercase())
}

/// The host and each parent domain, longest first: `a.b.c.com`, `b.c.com`, `c.com`.
fn host_and_parents(host: &str) -> impl Iterator<Item = &str> {
    let mut rest = Some(host);
    std::iter::from_fn(move || {
        let current = rest?;
        rest = current
            .split_once('.')
            .map(|(_, parent)| parent)
            .filter(|parent| parent.contains('.'));
        Some(current)
    })
}

/// The listed domain that covers `host`, which may be a parent of it.
fn matched_rule(host: &str) -> Option<&str> {
    let list = blocklist();
    host_and_parents(host).find(|candidate| list.contains(*candidate))
}

fn host_blocked(host: &str) -> bool {
    matched_rule(host).is_some()
}

/// Registrable domain without a public suffix list: the last two labels, or three under
/// short country second-level domains such as `co.uk`.
fn base_domain(host: &str) -> &str {
    let labels: Vec<&str> = host.rsplitn(4, '.').collect();
    let keep = match labels.as_slice() {
        [tld, second, _, ..]
            if tld.len() == 2
                && matches!(
                    *second,
                    "co" | "com" | "net" | "org" | "gov" | "ac" | "edu" | "ne" | "or"
                ) =>
        {
            3
        }
        _ => 2,
    };
    let mut dots = 0;
    for (index, character) in host.char_indices().rev() {
        if character == '.' {
            dots += 1;
            if dots == keep {
                return &host[index + 1..];
            }
        }
    }
    host
}

fn same_site(left: &str, right: &str) -> bool {
    base_domain(left) == base_domain(right)
}

fn paused(config: &Config, page_host: &str) -> bool {
    let key = site_key(page_host);
    host_and_parents(&key).any(|candidate| config.allowed.contains(candidate))
}

/// Blocking runs on this page: it is on, and the page's site is not paused.
pub fn active_for(page_url: &str) -> bool {
    let config = config().read().expect("adblock config lock");
    if !config.enabled {
        return false;
    }
    http_host(page_url).is_none_or(|host| !paused(&config, &host))
}

/// A main-frame navigation in tab `id` started. Resets the tab's count.
pub fn page_started(app: &AppHandle, id: u32, url: &str) {
    let stats = {
        let mut tabs = tabs().lock().expect("adblock tabs lock");
        let tab = tabs.entry(id).or_default();
        tab.page_host = http_host(url);
        let changed = tab.blocked != 0 || !tab.hosts.is_empty();
        tab.blocked = 0;
        tab.hosts.clear();
        changed.then(|| snapshot(id, tab))
    };
    if let Some(stats) = stats {
        let _ = app.emit("adblock-stats", stats);
    }
}

pub fn forget(id: u32) {
    tabs().lock().expect("adblock tabs lock").remove(&id);
}

pub fn stats(id: u32) -> AdblockStats {
    tabs()
        .lock()
        .expect("adblock tabs lock")
        .get(&id)
        .map(|tab| snapshot(id, tab))
        .unwrap_or(AdblockStats {
            tab_id: id,
            blocked: 0,
            hosts: Vec::new(),
        })
}

fn snapshot(id: u32, tab: &TabState) -> AdblockStats {
    let mut hosts: Vec<AdblockHost> = tab
        .hosts
        .iter()
        .map(|(host, hit)| {
            let mut kinds: Vec<AdblockKind> = hit
                .kinds
                .iter()
                .map(|(kind, count)| AdblockKind {
                    kind: (*kind).to_string(),
                    count: *count,
                })
                .collect();
            kinds.sort_by(|left, right| {
                right
                    .count
                    .cmp(&left.count)
                    .then_with(|| left.kind.cmp(&right.kind))
            });
            AdblockHost {
                host: host.clone(),
                count: hit.count,
                rule: hit.rule.clone(),
                kinds,
            }
        })
        .collect();
    hosts.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.host.cmp(&right.host))
    });
    AdblockStats {
        tab_id: id,
        blocked: tab.blocked,
        hosts,
    }
}

/// Decides whether a request from tab `id` is dropped, and counts it when it is.
/// The tab's own page is never blocked, so a link to a listed host still opens.
/// `kind` is a short request type such as `script` or `image`.
pub fn should_block(
    app: &AppHandle,
    id: u32,
    request_url: &str,
    is_document: bool,
    kind: &'static str,
) -> bool {
    let Some(request_host) = http_host(request_url) else {
        return false;
    };
    let host_rule = matched_rule(&request_host);
    let path_rule = matched_path(request_url);
    if host_rule.is_none() && path_rule.is_none() {
        return false;
    }
    let page_host = tabs()
        .lock()
        .expect("adblock tabs lock")
        .get(&id)
        .and_then(|tab| tab.page_host.clone());
    let rule = {
        let config = config().read().expect("adblock config lock");
        if !config.enabled {
            return false;
        }
        if let Some(page_host) = &page_host {
            if paused(&config, page_host) {
                return false;
            }
        } else if is_document {
            return false;
        }
        if is_document {
            return false;
        }
        if let Some(path_rule) = path_rule {
            path_rule.to_string()
        } else if page_host
            .as_deref()
            .is_some_and(|page_host| same_site(page_host, &request_host))
        {
            return false;
        } else {
            host_rule.unwrap_or("").to_string()
        }
    };
    record_block(app, id, &request_host, &rule, kind);
    true
}

fn record_block(app: &AppHandle, id: u32, host: &str, rule: &str, kind: &'static str) {
    let schedule = {
        let mut tabs = tabs().lock().expect("adblock tabs lock");
        let tab = tabs.entry(id).or_default();
        tab.blocked = tab.blocked.saturating_add(1);
        let hit = tab
            .hosts
            .entry(host.to_string())
            .or_insert_with(|| HostHit {
                count: 0,
                rule: rule.to_string(),
                kinds: HashMap::new(),
            });
        hit.count = hit.count.saturating_add(1);
        let kind_count = hit.kinds.entry(kind).or_insert(0);
        *kind_count = kind_count.saturating_add(1);
        !std::mem::replace(&mut tab.pending, true)
    };
    if !schedule {
        return;
    }
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(STATS_DELAY);
        let stats = {
            let mut tabs = tabs().lock().expect("adblock tabs lock");
            let Some(tab) = tabs.get_mut(&id) else {
                return;
            };
            tab.pending = false;
            snapshot(id, tab)
        };
        let _ = app.emit("adblock-stats", stats);
    });
}

fn early_parts() -> (bool, Vec<String>) {
    let config = config().read().expect("adblock config lock");
    let mut paused: Vec<String> = config.allowed.iter().cloned().collect();
    paused.sort();
    (config.enabled, paused)
}

/// Changes when the document-start script must be registered again.
pub fn early_stamp() -> String {
    let (enabled, paused) = early_parts();
    serde_json::json!([enabled, paused]).to_string()
}

/// Document-start script: Google Analytics opt-out, ad stubs, and the cosmetic stylesheet.
/// Paused sites and non-http pages leave the page alone.
pub fn early_script() -> String {
    let (enabled, paused) = early_parts();
    let paused = serde_json::to_string(&paused).unwrap_or_else(|_| "[]".into());
    let css = serde_json::to_string(COSMETIC_CSS).unwrap_or_else(|_| "\"\"".into());
    EARLY_JS
        .replace("__CE_ENABLED__", if enabled { "true" } else { "false" })
        .replace("__CE_PAUSED__", &paused)
        .replace("__CE_CSS__", &css)
}

/// Script that adds the element-hiding stylesheet to the current document.
pub fn cosmetic_script() -> String {
    let css = serde_json::to_string(COSMETIC_CSS).unwrap_or_else(|_| "\"\"".into());
    format!(
        "(() => {{ const mount = () => {{ if (document.getElementById('ce-adblock')) return true; \
         const root = document.head || document.documentElement; if (!root) return false; \
         const style = document.createElement('style'); style.id = 'ce-adblock'; \
         style.textContent = {css}; root.appendChild(style); return true; }}; \
         if (!mount() && document.readyState === 'loading') {{ \
         document.addEventListener('DOMContentLoaded', mount, {{ once: true }}); }} }})();"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subdomain_reports_the_listed_parent() {
        assert_eq!(
            matched_rule("securepubads.g.doubleclick.net"),
            Some("doubleclick.net")
        );
        assert_eq!(matched_rule("example.com"), None);
    }

    #[test]
    fn listed_hosts_and_their_subdomains_are_blocked() {
        assert!(host_blocked("doubleclick.net"));
        assert!(host_blocked("securepubads.g.doubleclick.net"));
        assert!(host_blocked("cdn.taboola.com"));
        assert!(!host_blocked("example.com"));
        assert!(!host_blocked("net"));
        assert!(!host_blocked("notdoubleclick.net"));
    }

    #[test]
    fn parents_stop_before_the_top_level_domain() {
        let parents: Vec<&str> = host_and_parents("a.b.ads.com").collect();
        assert_eq!(parents, vec!["a.b.ads.com", "b.ads.com", "ads.com"]);
    }

    #[test]
    fn base_domain_handles_country_second_levels() {
        assert_eq!(base_domain("www.cnn.com"), "cnn.com");
        assert_eq!(base_domain("news.bbc.co.uk"), "bbc.co.uk");
        assert_eq!(base_domain("localhost"), "localhost");
        assert!(same_site("www.taboola.com", "cdn.taboola.com"));
        assert!(!same_site("www.cnn.com", "cdn.taboola.com"));
    }

    #[test]
    fn site_keys_drop_www_and_case() {
        assert_eq!(site_key(" WWW.Example.COM. "), "example.com");
        assert_eq!(site_key("www."), "www");
    }

    #[test]
    fn paused_sites_cover_subdomains() {
        let config = Config {
            enabled: true,
            allowed: ["example.com".to_string()].into_iter().collect(),
        };
        assert!(paused(&config, "www.example.com"));
        assert!(paused(&config, "news.example.com"));
        assert!(!paused(&config, "example.org"));
    }

    #[test]
    fn path_rules_block_collect_urls_and_leave_search_alone() {
        assert_eq!(
            matched_path("https://www.google.com/pagead/ads?client=ca-pub-1"),
            Some("www.google.com/pagead/")
        );
        assert_eq!(
            matched_path("https://WWW.Google.com/ccm/collect"),
            Some("www.google.com/ccm/")
        );
        assert_eq!(
            matched_path("https://www.google.com/ads/ga-audiences/extra"),
            Some("www.google.com/ads/ga-audiences")
        );
        assert_eq!(matched_path("https://www.google.com/search?q=news"), None);
        assert_eq!(matched_path("https://www.google.com/gen_204"), None);
        assert_eq!(matched_path("https://news.google.com/pagead/ads"), None);
    }

    #[test]
    fn path_rules_are_a_host_and_a_prefix() {
        for rule in path_rules() {
            assert!(
                rule.host.contains('.') && !rule.host.contains('/'),
                "{}",
                rule.label
            );
            assert!(
                rule.prefix.starts_with('/') && rule.prefix.len() > 1,
                "{}",
                rule.label
            );
        }
    }

    #[test]
    fn the_early_script_opts_out_of_google_analytics() {
        let script = early_script();
        assert!(script.contains("_gaUserPrefs"));
        assert!(script.contains("ga-disable-"));
        assert!(!script.contains("__CE_"));
    }

    #[test]
    fn the_blocklist_has_no_paths_or_blanks() {
        for host in blocklist() {
            assert!(
                !host.contains('/') && !host.contains(' ') && host.contains('.'),
                "{host}"
            );
        }
    }
}
