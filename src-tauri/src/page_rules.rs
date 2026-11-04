//! Per-site page control. `styles.css` still styles the browser; this file styles and scripts sites.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::{files, paths, print, settings, tabs};

const FILE: &str = "site-controls.json";
const MAX_SCRIPTS: usize = 100;
const MAX_SITES: usize = 500;
const MAX_CODE: usize = 64 * 1024;
const MAX_HIDDEN: usize = 200;
const MAX_SELECTOR: usize = 800;
const MAX_AGENT: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct DarkMode {
    pub enabled: bool,
    /// Percent, 50 to 150.
    pub brightness: u32,
    pub contrast: u32,
}

impl Default for DarkMode {
    fn default() -> Self {
        Self {
            enabled: false,
            brightness: 100,
            contrast: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct SiteControl {
    pub force_interaction: bool,
    pub clean_page: bool,
    pub dark: DarkMode,
    /// Empty keeps the browser's normal user agent.
    pub user_agent: String,
    /// 1 leaves the page's own speed.
    pub video_speed: f64,
    pub block_script: bool,
    pub block_images: bool,
    pub block_fonts: bool,
    /// CSS selectors hidden by the element zapper.
    pub hidden: Vec<String>,
    /// `default`, `none`, `origin` or `same-origin`.
    pub referrer: String,
    /// Page zoom in percent remembered for this site. Zero follows the default page zoom.
    /// Older files omit this.
    pub zoom: u32,
}

impl Default for SiteControl {
    fn default() -> Self {
        Self {
            force_interaction: false,
            clean_page: false,
            dark: DarkMode::default(),
            user_agent: String::new(),
            video_speed: 1.0,
            block_script: false,
            block_images: false,
            block_fonts: false,
            hidden: Vec::new(),
            referrer: "default".into(),
            zoom: 0,
        }
    }
}

impl SiteControl {
    fn normalize(mut self) -> Self {
        self.dark.brightness = self.dark.brightness.clamp(50, 150);
        self.dark.contrast = self.contrast_or(self.dark.contrast);
        self.user_agent = clean_agent(&self.user_agent);
        self.video_speed = snap_speed(self.video_speed);
        self.hidden = clean_selectors(self.hidden);
        self.referrer = crate::privacy::normalize_referrer(&self.referrer);
        if self.zoom != 0 {
            self.zoom = self.zoom.clamp(25, 300);
        }
        self
    }

    fn contrast_or(&self, value: u32) -> u32 {
        value.clamp(50, 150)
    }

    pub fn active(&self) -> bool {
        self.force_interaction
            || self.clean_page
            || self.dark.enabled
            || !self.user_agent.is_empty()
            || (self.video_speed - 1.0).abs() > 0.01
            || self.block_script
            || self.block_images
            || self.block_fonts
            || !self.hidden.is_empty()
            || self.referrer != "default"
            || self.zoom != 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct UserScript {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    /// Empty matches every http(s) site.
    pub hosts: Vec<String>,
    pub css: String,
    pub js: String,
}

impl Default for UserScript {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            enabled: true,
            hosts: Vec::new(),
            css: String::new(),
            js: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PageRules {
    pub sites: HashMap<String, SiteControl>,
    pub scripts: Vec<UserScript>,
}

impl Default for PageRules {
    fn default() -> Self {
        Self {
            sites: HashMap::new(),
            scripts: Vec::new(),
        }
    }
}

/// Fields the popover sends. Absent fields stay as they are.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SitePatch {
    pub force_interaction: Option<bool>,
    pub clean_page: Option<bool>,
    pub dark_enabled: Option<bool>,
    pub brightness: Option<u32>,
    pub contrast: Option<u32>,
    pub user_agent: Option<String>,
    pub video_speed: Option<f64>,
    pub block_script: Option<bool>,
    pub block_images: Option<bool>,
    pub block_fonts: Option<bool>,
    pub hide: Option<String>,
    pub clear_hidden: Option<bool>,
    pub referrer: Option<String>,
    /// Percent. Zero forgets the site's zoom.
    pub zoom: Option<u32>,
}

struct RulesState {
    rules: Mutex<PageRules>,
    reader: Mutex<Option<print::ExtractedPage>>,
}

pub fn init(app: &AppHandle) {
    let loaded: PageRules = files::read_json(&paths::data_file(app, FILE));
    app.manage(RulesState {
        rules: Mutex::new(loaded.normalize()),
        reader: Mutex::new(None),
    });
}

fn state(app: &AppHandle) -> tauri::State<'_, RulesState> {
    app.state::<RulesState>()
}

pub fn get(app: &AppHandle) -> PageRules {
    state(app).rules.lock().expect("page rules").clone()
}

fn write(app: &AppHandle, rules: &PageRules) -> Result<(), String> {
    files::write_json(&paths::data_file(app, FILE), rules)
}

fn publish(app: &AppHandle, rules: &PageRules) {
    let _ = app.emit("page-rules-changed", rules);
    tabs::reinject_page_rules(app);
}

fn save_rules(app: &AppHandle, rules: PageRules) -> Result<PageRules, String> {
    let rules = rules.normalize();
    write(app, &rules)?;
    *state(app).rules.lock().expect("page rules") = rules.clone();
    publish(app, &rules);
    Ok(rules)
}

pub fn update_site(app: &AppHandle, host: &str, patch: SitePatch) -> Result<PageRules, String> {
    let host = settings::sleep_site_key(host);
    if host.is_empty() {
        return Err("This page has no site to change.".into());
    }
    let mut rules = get(app);
    let mut site = rules.sites.get(&host).cloned().unwrap_or_default();
    if let Some(value) = patch.force_interaction {
        site.force_interaction = value;
    }
    if let Some(value) = patch.clean_page {
        site.clean_page = value;
    }
    if let Some(value) = patch.dark_enabled {
        site.dark.enabled = value;
    }
    if let Some(value) = patch.brightness {
        site.dark.brightness = value;
    }
    if let Some(value) = patch.contrast {
        site.dark.contrast = value;
    }
    if let Some(value) = patch.user_agent {
        site.user_agent = value;
    }
    if let Some(value) = patch.video_speed {
        site.video_speed = value;
    }
    if let Some(value) = patch.block_script {
        site.block_script = value;
    }
    if let Some(value) = patch.block_images {
        site.block_images = value;
    }
    if let Some(value) = patch.block_fonts {
        site.block_fonts = value;
    }
    if let Some(value) = patch.referrer {
        site.referrer = value;
    }
    if let Some(value) = patch.zoom {
        site.zoom = value;
    }
    if patch.clear_hidden == Some(true) {
        site.hidden.clear();
    }
    if let Some(selector) = patch.hide.filter(|selector| !selector.trim().is_empty()) {
        site.hidden.push(selector);
    }
    site = site.normalize();
    if site.active() {
        rules.sites.insert(host, site);
    } else {
        rules.sites.remove(&host);
    }
    save_rules(app, rules)
}

pub fn save_script(app: &AppHandle, mut script: UserScript) -> Result<PageRules, String> {
    let is_new = script.id.trim().is_empty();
    script = normalize_script(script)?;
    let mut rules = get(app);
    if is_new {
        let base = script.id.clone();
        let mut suffix = 2;
        while rules.scripts.iter().any(|item| item.id == script.id) {
            script.id = format!("{base}-{suffix}");
            suffix += 1;
        }
    }
    if let Some(existing) = rules.scripts.iter_mut().find(|item| item.id == script.id) {
        *existing = script;
    } else {
        if rules.scripts.len() >= MAX_SCRIPTS {
            return Err(format!("You can save up to {MAX_SCRIPTS} scripts."));
        }
        rules.scripts.push(script);
    }
    save_rules(app, rules)
}

pub fn delete_script(app: &AppHandle, id: &str) -> Result<PageRules, String> {
    let mut rules = get(app);
    rules.scripts.retain(|script| script.id != id);
    save_rules(app, rules)
}

/// The control that applies to `url`: the longest matching host, or the defaults.
pub fn control_for(app: &AppHandle, url: &str) -> SiteControl {
    let host = settings::sleep_site_key(url);
    let rules = get(app);
    best_control(&host, &rules.sites)
        .cloned()
        .unwrap_or_default()
}

pub fn blocks_images(app: &AppHandle, url: &str) -> bool {
    control_for(app, url).block_images
}

pub fn blocks_fonts(app: &AppHandle, url: &str) -> bool {
    control_for(app, url).block_fonts
}

/// User agent to set for this navigation. `None` means leave the current one, which happens for
/// pages that are not http(s). An empty string restores the browser default.
pub fn user_agent_for(app: &AppHandle, url: &str) -> Option<String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Some(String::new());
    }
    Some(control_for(app, url).user_agent)
}

pub fn scripts_enabled(app: &AppHandle, url: &str) -> bool {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return true;
    }
    !control_for(app, url).block_script
}

/// Script run as the document loads. Empty for pages that are not http(s).
pub fn injection(app: &AppHandle, url: &str) -> String {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return String::new();
    }
    let host = settings::sleep_site_key(url);
    let rules = get(app);
    let site = best_control(&host, &rules.sites)
        .cloned()
        .unwrap_or_default();
    let scripts: Vec<serde_json::Value> = rules
        .scripts
        .iter()
        .filter(|script| script.enabled && script_matches(&host, script))
        .map(|script| {
            serde_json::json!({
                "id": script.id,
                "name": script.name,
                "js": script.js,
            })
        })
        .collect();
    let styles: Vec<&str> = rules
        .scripts
        .iter()
        .filter(|script| script.enabled && script_matches(&host, script))
        .map(|script| script.css.as_str())
        .filter(|css| !css.is_empty())
        .collect();
    let spec = serde_json::json!({
        "forceInteraction": site.force_interaction,
        "cleanPage": site.clean_page,
        "dark": site.dark.enabled,
        "brightness": format!("{}%", site.dark.brightness),
        "contrast": format!("{}%", site.dark.contrast),
        "videoSpeed": site.video_speed,
        "hidden": site.hidden,
        "styles": styles,
        "scripts": scripts,
    });
    let mut script = format!("{INJECTION_JS}\nceApply({spec});");
    if settings::get(app).strip_tracking {
        script.push('\n');
        script.push_str(&crate::privacy::copy_script());
    }
    script.push('\n');
    script.push_str(&crate::privacy::fingerprint_script(app, url));
    script.push('\n');
    script.push_str(crate::privacy::PROTOCOL_JS);
    script.push('\n');
    script.push_str(ERROR_JS);
    script.push('\n');
    script.push_str(THEME_JS);
    if crate::privacy::decision(app, url, "notifications") == Some(true) {
        script.push('\n');
        script.push_str(crate::system::NOTIFY_JS);
    }
    script
}

pub fn zapper_script() -> &'static str {
    ZAPPER_JS
}

pub fn inspector_script() -> &'static str {
    INSPECTOR_JS
}

pub fn remember_speed(app: &AppHandle, page_url: &str, speed: f64) {
    let host = settings::sleep_site_key(page_url);
    if host.is_empty() {
        return;
    }
    let _ = update_site(
        app,
        &host,
        SitePatch {
            video_speed: Some(speed),
            ..SitePatch::default()
        },
    );
}

/// Stores the zoom for the page's site. Zero forgets it. Nothing on the page changes, so the
/// page scripts are not run again the way other control changes do.
pub fn remember_zoom(app: &AppHandle, page_url: &str, percent: u32) {
    let host = settings::sleep_site_key(page_url);
    if host.is_empty() {
        return;
    }
    let mut rules = get(app);
    let mut site = rules.sites.get(&host).cloned().unwrap_or_default();
    if site.zoom == percent {
        return;
    }
    site.zoom = percent;
    site = site.normalize();
    if site.active() {
        rules.sites.insert(host, site);
    } else {
        rules.sites.remove(&host);
    }
    let rules = rules.normalize();
    if let Err(error) = write(app, &rules) {
        eprintln!("site zoom: {error}");
        return;
    }
    *state(app).rules.lock().expect("page rules") = rules.clone();
    let _ = app.emit("page-rules-changed", &rules);
}

/// Zoom stored for the site of `url` as a factor, or `None` to use the default.
pub fn zoom_for(app: &AppHandle, url: &str) -> Option<f64> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return None;
    }
    let zoom = control_for(app, url).zoom;
    (zoom != 0).then(|| f64::from(zoom) / 100.0)
}

pub fn remember_zap(app: &AppHandle, page_url: &str, selector: &str) {
    let host = settings::sleep_site_key(page_url);
    if host.is_empty() {
        return;
    }
    let _ = update_site(
        app,
        &host,
        SitePatch {
            hide: Some(selector.to_string()),
            ..SitePatch::default()
        },
    );
}

pub fn store_reader(app: &AppHandle, page: print::ExtractedPage) {
    *state(app).reader.lock().expect("reader") = Some(page);
}

pub fn reader(app: &AppHandle) -> Option<print::ExtractedPage> {
    state(app).reader.lock().expect("reader").clone()
}

impl PageRules {
    fn normalize(mut self) -> Self {
        let mut sites = HashMap::new();
        for (host, site) in self.sites {
            let host = settings::sleep_site_key(&host);
            let site = site.normalize();
            if host.is_empty() || !site.active() {
                continue;
            }
            sites.insert(host, site);
            if sites.len() >= MAX_SITES {
                break;
            }
        }
        self.sites = sites;
        self.scripts = self
            .scripts
            .into_iter()
            .filter_map(|script| normalize_script(script).ok())
            .take(MAX_SCRIPTS)
            .collect();
        self
    }
}

fn normalize_script(mut script: UserScript) -> Result<UserScript, String> {
    script.name = script.name.trim().chars().take(80).collect();
    if script.name.is_empty() {
        script.name = "Script".into();
    }
    if script.css.len() > MAX_CODE || script.js.len() > MAX_CODE {
        return Err(format!(
            "A script can hold up to {} KB of CSS and of JS.",
            MAX_CODE / 1024
        ));
    }
    script.id = script_id(&script.id, &script.name);
    let mut hosts = Vec::new();
    for host in script.hosts {
        let host = settings::sleep_site_key(&host);
        if !host.is_empty() && !hosts.contains(&host) {
            hosts.push(host);
        }
    }
    script.hosts = hosts;
    Ok(script)
}

fn script_id(raw: &str, name: &str) -> String {
    let source = if raw.trim().is_empty() { name } else { raw };
    let mut id: String = source
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
    if id.is_empty() { "script".into() } else { id }
}

fn clean_agent(raw: &str) -> String {
    raw.chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_AGENT)
        .collect::<String>()
        .trim()
        .to_string()
}

fn snap_speed(value: f64) -> f64 {
    if !value.is_finite() {
        return 1.0;
    }
    let snapped = (value.clamp(0.25, 4.0) * 20.0).round() / 20.0;
    if (snapped - 1.0).abs() < 0.02 {
        1.0
    } else {
        snapped
    }
}

fn clean_selectors(selectors: Vec<String>) -> Vec<String> {
    let mut kept = Vec::new();
    for selector in selectors {
        let selector = selector.trim();
        if selector.is_empty()
            || selector.len() > MAX_SELECTOR
            || selector
                .chars()
                .any(|ch| ch.is_control() || matches!(ch, '{' | '}' | '<' | ';'))
            || kept.iter().any(|item: &String| item == selector)
        {
            continue;
        }
        kept.push(selector.to_string());
        if kept.len() >= MAX_HIDDEN {
            break;
        }
    }
    kept
}

fn site_matches(host: &str, rule: &str) -> bool {
    !host.is_empty() && !rule.is_empty() && (host == rule || host.ends_with(&format!(".{rule}")))
}

fn best_control<'a>(
    host: &str,
    sites: &'a HashMap<String, SiteControl>,
) -> Option<&'a SiteControl> {
    sites
        .iter()
        .filter(|(rule, _)| site_matches(host, rule))
        .max_by_key(|(rule, _)| rule.len())
        .map(|(_, site)| site)
}

fn script_matches(host: &str, script: &UserScript) -> bool {
    script.hosts.is_empty() || script.hosts.iter().any(|rule| site_matches(host, rule))
}

const INJECTION_JS: &str = r#"(function () {
  function ceHideMatches() {
    var selectors = window.__ceHidden || [];
    if (!selectors.length) return;
    selectors.forEach(function (selector) {
      var parts = selector.split(" >>> ");
      var nodes = [document];
      for (var i = 0; i < parts.length; i++) {
        var next = [];
        for (var n = 0; n < nodes.length; n++) {
          var found;
          try { found = nodes[n].querySelectorAll(parts[i]); } catch (error) { continue; }
          for (var f = 0; f < found.length; f++) next.push(found[f]);
        }
        if (i === parts.length - 1) {
          next.forEach(function (el) {
            el.style.setProperty("display", "none", "important");
            el.setAttribute("data-ce-hidden", "1");
          });
        } else {
          nodes = [];
          next.forEach(function (el) {
            if (!el.shadowRoot) return;
            nodes.push(el.shadowRoot);
            if (el.shadowRoot.__ceShadowWatch) return;
            el.shadowRoot.__ceShadowWatch = true;
            new MutationObserver(function () {
              if (window.__ceScheduleHide) window.__ceScheduleHide();
            }).observe(el.shadowRoot, { childList: true, subtree: true });
          });
        }
      }
    });
  }
  function ceReleaseHidden(root) {
    var marked;
    try { marked = root.querySelectorAll("[data-ce-hidden]"); } catch (error) { marked = []; }
    for (var i = 0; i < marked.length; i++) {
      marked[i].style.removeProperty("display");
      marked[i].removeAttribute("data-ce-hidden");
    }
    var all;
    try { all = root.querySelectorAll("*"); } catch (error) { return; }
    for (var n = 0; n < all.length; n++) {
      if (all[n].shadowRoot) ceReleaseHidden(all[n].shadowRoot);
    }
  }
  function ceApply(spec) {
    if (!/^https?:/.test(location.protocol)) return;
    window.__ceSpec = spec;
    var root = document.documentElement;
    var style = document.getElementById("ce-page-controls");
    if (!style) {
      style = document.createElement("style");
      style.id = "ce-page-controls";
      root.appendChild(style);
    }
    var css = [];
    if (spec.forceInteraction) {
      css.push("html, body, * { user-select: text !important; -webkit-user-select: text !important; }");
    }
    if (spec.dark) {
      css.push("html { filter: invert(1) hue-rotate(180deg) brightness(" + spec.brightness + ") contrast(" + spec.contrast + ") !important; }");
      css.push("img, video, picture, canvas, iframe { filter: invert(1) hue-rotate(180deg) !important; }");
    }
    var hidden = spec.hidden || [];
    hidden.forEach(function (selector) {
      if (selector.indexOf(">>>") === -1) css.push(selector + " { display: none !important; }");
    });
    (spec.styles || []).forEach(function (text) { css.push(text); });
    style.textContent = css.join("\n");
    var hadHidden = window.__ceHidden && window.__ceHidden.length;
    window.__ceHidden = hidden;
    if (hidden.length) ceHideMatches();
    else if (hadHidden) ceReleaseHidden(document);

    if (spec.forceInteraction && !window.__ceForce) {
      window.__ceForce = true;
      ["contextmenu", "copy", "cut", "paste", "selectstart"].forEach(function (name) {
        window.addEventListener(name, function (event) { event.stopImmediatePropagation(); }, true);
      });
      var clear = function () {
        document.oncontextmenu = null;
        document.onselectstart = null;
        if (document.body) {
          document.body.oncontextmenu = null;
          document.body.onselectstart = null;
        }
      };
      clear();
      document.addEventListener("DOMContentLoaded", clear);
    }

    if (spec.cleanPage) {
      var words = /cookie|consent|gdpr|newsletter|subscribe|paywall|modal|popup/i;
      var sweep = function () {
        if (!window.__ceSpec || !window.__ceSpec.cleanPage) return;
        document.querySelectorAll("body *").forEach(function (el) {
          if (el.id === "ce-page-controls" || el.id === "ce-zapper") return;
          if (el.closest && el.closest("video, audio")) return;
          if (el.getAttribute("role") === "dialog" && el.querySelector("input, textarea, select")) return;
          var name = ((el.id || "") + " " + (typeof el.className === "string" ? el.className : "")).toString();
          if (words.test(name)) {
            el.style.setProperty("display", "none", "important");
            return;
          }
          var pos = getComputedStyle(el).position;
          if (pos !== "fixed" && pos !== "sticky") return;
          var rect = el.getBoundingClientRect();
          var vh = window.innerHeight || 800;
          if (rect.height > vh * 0.3 && (rect.top < vh * 0.3 || rect.bottom > vh * 0.7)) {
            el.style.setProperty("display", "none", "important");
          }
        });
      };
      sweep();
      if (!window.__ceClean) {
        window.__ceClean = true;
        new MutationObserver(sweep).observe(root, { childList: true, subtree: true });
      }
    }

    var ran = window.__ceScripts || (window.__ceScripts = {});
    (spec.scripts || []).forEach(function (script) {
      if (ran[script.id] === script.js) return;
      ran[script.id] = script.js;
      try { (0, eval)(script.js); } catch (error) { console.warn("ce-browser script", script.name, error); }
    });

    var applyRate = function () {
      var rate = window.__ceSpec && window.__ceSpec.videoSpeed;
      if (!rate || rate === 1) return;
      document.querySelectorAll("video, audio").forEach(function (media) {
        if (media.playbackRate !== rate) media.playbackRate = rate;
      });
    };
    applyRate();
    if (!window.__ceHideWatch) {
      window.__ceHideWatch = true;
      var hidePending = false;
      window.__ceScheduleHide = function () {
        if (hidePending || !window.__ceHidden || !window.__ceHidden.length) return;
        hidePending = true;
        requestAnimationFrame(function () {
          hidePending = false;
          ceHideMatches();
        });
      };
      new MutationObserver(window.__ceScheduleHide).observe(document.documentElement, { childList: true, subtree: true });
    }
    if (!window.__ceSpeedKeys) {
      window.__ceSpeedKeys = true;
      document.addEventListener("play", applyRate, true);
      document.addEventListener("keydown", function (event) {
        if (event.key !== "[" && event.key !== "]") return;
        var target = event.target;
        if (target && target.closest && target.closest("input, textarea, select, [contenteditable=true]")) return;
        var current = (window.__ceSpec && window.__ceSpec.videoSpeed) || 1;
        var next = current + (event.key === "]" ? 0.25 : -0.25);
        next = Math.round(Math.min(4, Math.max(0.25, next)) * 20) / 20;
        window.__ceSpec.videoSpeed = next;
        applyRate();
        if (window.chrome && window.chrome.webview) {
          window.chrome.webview.postMessage({ type: "ce-speed", speed: next });
        }
      }, true);
    }
  }
  window.ceApply = ceApply;
})();"#;

const THEME_JS: &str = r#"(function () {
  if (window.__ceTheme) return;
  window.__ceTheme = true;
  function send() {
    var meta = document.querySelector('meta[name="theme-color"]');
    var color = meta && meta.content ? String(meta.content) : "";
    try {
      if (window.chrome && window.chrome.webview) {
        window.chrome.webview.postMessage({ type: "ce-theme", color: color });
      }
    } catch (error) {}
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", send);
  else send();
})();"#;

const ERROR_JS: &str = r#"(function () {
  if (window.__ceErrors) return;
  window.__ceErrors = true;
  function report() {
    try {
      if (window.chrome && window.chrome.webview) {
        window.chrome.webview.postMessage({ type: "ce-error" });
      }
    } catch (error) {}
  }
  window.addEventListener("error", report);
  window.addEventListener("unhandledrejection", report);
  var original = console.error;
  console.error = function () {
    report();
    return original.apply(console, arguments);
  };
})();"#;

const INSPECTOR_JS: &str = r##"(function () {
  if (window.__ceInspect) return;
  window.__ceInspect = true;
  var box = document.createElement("div");
  box.id = "ce-inspector";
  box.style.cssText = "position:fixed;z-index:2147483647;pointer-events:none;max-width:320px;padding:6px 8px;border-radius:8px;background:#18181b;color:#e4e4e7;font:12px/1.4 sans-serif;box-shadow:0 8px 24px rgba(0,0,0,.35)";
  document.documentElement.appendChild(box);
  var line = "";
  function cssColor(value) {
    var probe = document.createElement("canvas").getContext("2d");
    if (!probe) return value;
    probe.fillStyle = value;
    return probe.fillStyle;
  }
  function describe(el) {
    var style = getComputedStyle(el);
    var color = cssColor(style.color);
    var background = cssColor(style.backgroundColor);
    return "color: " + color + "; background: " + background + "; font: " + style.fontWeight + " " + style.fontSize + " " + style.fontFamily;
  }
  function move(event) {
    var el = event.target;
    if (!el || el.id === "ce-inspector") return;
    line = describe(el);
    box.textContent = line;
    box.style.left = Math.min(event.clientX + 12, window.innerWidth - 340) + "px";
    box.style.top = Math.min(event.clientY + 16, window.innerHeight - 48) + "px";
  }
  function cleanup() {
    box.remove();
    document.removeEventListener("mousemove", move, true);
    document.removeEventListener("click", click, true);
    document.removeEventListener("keydown", key, true);
    window.__ceInspect = false;
  }
  function click(event) {
    if (event.target && event.target.id === "ce-inspector") return;
    event.preventDefault();
    event.stopPropagation();
    var text = line;
    cleanup();
    if (text && window.chrome && window.chrome.webview) {
      window.chrome.webview.postMessage({ type: "ce-inspect", text: text });
    }
  }
  function key(event) {
    if (event.key === "Escape") {
      event.preventDefault();
      cleanup();
    }
  }
  document.addEventListener("mousemove", move, true);
  document.addEventListener("click", click, true);
  document.addEventListener("keydown", key, true);
})();"##;

const ZAPPER_JS: &str = r##"(function () {
  if (window.__ceZap) return;
  window.__ceZap = true;
  var root = document.documentElement;
  var font = "font:12px/1.4 'Segoe UI',system-ui,sans-serif;";
  var shield = document.createElement("div");
  shield.id = "ce-zapper";
  shield.style.cssText = "position:fixed;inset:0;z-index:2147483646;cursor:crosshair;background:transparent";
  var box = document.createElement("div");
  box.style.cssText = "position:fixed;z-index:2147483647;pointer-events:none;display:none;border:2px solid #38bdf8;background:rgba(56,189,248,.2);box-sizing:border-box";
  var label = document.createElement("div");
  label.style.cssText = "position:fixed;z-index:2147483647;pointer-events:none;display:none;background:#0c4a6e;color:#e0f2fe;padding:2px 6px;border-radius:4px;white-space:nowrap;" + font;
  var hint = document.createElement("div");
  hint.textContent = "Click to hide \u00b7 Alt+scroll or \u2191 \u2193 to grow or shrink \u00b7 Esc to cancel";
  hint.style.cssText = "position:fixed;left:50%;top:12px;transform:translateX(-50%);z-index:2147483647;pointer-events:none;background:rgba(9,9,11,.92);color:#fafafa;padding:6px 12px;border-radius:8px;box-shadow:0 4px 16px rgba(0,0,0,.4);" + font;
  root.appendChild(shield);
  root.appendChild(box);
  root.appendChild(label);
  root.appendChild(hint);
  var hovered = null;
  var current = null;
  var children = [];
  var armed = false;
  setTimeout(function () { armed = true; }, 300);

  function cssEscape(value) {
    if (window.CSS && CSS.escape) return CSS.escape(value);
    return String(value).replace(/[^a-zA-Z0-9_-]/g, "\\$&");
  }
  function ownRoot(el) {
    var node = el.getRootNode ? el.getRootNode() : document;
    return node && node.querySelectorAll ? node : document;
  }
  function parentOf(el) {
    if (!el || el === root) return null;
    if (el.parentElement && el.parentElement !== root) return el.parentElement;
    var node = el.getRootNode ? el.getRootNode() : null;
    return node && node.host ? node.host : null;
  }
  function uniqueIn(scope, el) {
    if (el.id) {
      var byId = "#" + cssEscape(el.id);
      try { if (scope.querySelectorAll(byId).length === 1) return byId; } catch (error) {}
    }
    var parts = [];
    var node = el;
    while (node && node.nodeType === 1 && node !== scope && node !== root) {
      var part = node.tagName.toLowerCase();
      if (node.id) part = "#" + cssEscape(node.id);
      else if (node.classList) {
        for (var c = 0; c < node.classList.length && part.indexOf(".") === -1; c++) {
          var name = node.classList[c];
          if (name && name.length < 48 && !/[{}<>;]/.test(name)) part += "." + cssEscape(name);
        }
        if (node.parentElement) {
          var same = 0;
          var index = 0;
          for (var i = 0; i < node.parentElement.children.length; i++) {
            var child = node.parentElement.children[i];
            if (child.tagName === node.tagName) {
              same++;
              if (child === node) index = same;
            }
          }
          if (same > 1) part += ":nth-of-type(" + index + ")";
        }
      }
      parts.unshift(part);
      var built = parts.join(" > ");
      try { if (scope.querySelectorAll(built).length === 1) return built; } catch (error) {}
      node = node.parentElement;
      if (parts.length > 12) break;
    }
    return parts.join(" > ");
  }
  function selectorFor(el) {
    var segments = [];
    var node = el;
    var guard = 0;
    while (node && guard < 6) {
      guard++;
      var scope = ownRoot(node);
      var piece = uniqueIn(scope, node);
      if (piece) segments.unshift(piece);
      if (!scope.host) break;
      node = scope.host;
    }
    return segments.join(" >>> ");
  }
  function show(element) {
    current = element;
    if (!element) {
      box.style.display = "none";
      label.style.display = "none";
      return;
    }
    var rect = element.getBoundingClientRect();
    box.style.display = "block";
    box.style.left = rect.left + "px";
    box.style.top = rect.top + "px";
    box.style.width = Math.max(rect.width, 2) + "px";
    box.style.height = Math.max(rect.height, 2) + "px";
    var name = element.tagName.toLowerCase() + (element.id ? "#" + element.id : "");
    label.textContent = name + "  " + Math.round(rect.width) + " \u00d7 " + Math.round(rect.height);
    label.style.display = "block";
    label.style.left = Math.max(8, rect.left) + "px";
    label.style.top = (rect.top > 28 ? rect.top - 24 : rect.bottom + 4) + "px";
  }
  function elementAt(x, y) {
    shield.style.pointerEvents = "none";
    var element = document.elementFromPoint(x, y);
    shield.style.pointerEvents = "auto";
    if (!element || element === shield || element === box || element === label || element === hint || element === root) return null;
    return element;
  }
  function move(event) {
    var element = elementAt(event.clientX, event.clientY);
    if (!element || element === hovered) return;
    hovered = element;
    children = [];
    show(element);
  }
  function grow() {
    var parent = parentOf(current);
    if (!parent) return;
    children.push(current);
    hovered = parent;
    show(parent);
  }
  function shrink() {
    if (!children.length) return;
    var child = children.pop();
    hovered = child;
    show(child);
  }
  function cleanup() {
    shield.remove();
    box.remove();
    label.remove();
    hint.remove();
    document.removeEventListener("mousemove", move, true);
    document.removeEventListener("wheel", wheel, true);
    document.removeEventListener("keydown", key, true);
    window.removeEventListener("scroll", scrolled, true);
    window.__ceZap = false;
  }
  function pick(event) {
    event.preventDefault();
    event.stopPropagation();
    if (!armed) return;
    var element = current || elementAt(event.clientX, event.clientY);
    if (!element) return;
    var selector = selectorFor(element);
    element.style.setProperty("display", "none", "important");
    element.setAttribute("data-ce-hidden", "1");
    cleanup();
    if (selector && window.chrome && window.chrome.webview) {
      window.chrome.webview.postMessage({ type: "ce-zap", selector: selector });
    }
  }
  function wheel(event) {
    if (!event.altKey) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.deltaY < 0) grow();
    else shrink();
  }
  function key(event) {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      cleanup();
    } else if (event.key === "ArrowUp" || event.key === "[") {
      event.preventDefault();
      event.stopPropagation();
      grow();
    } else if (event.key === "ArrowDown" || event.key === "]") {
      event.preventDefault();
      event.stopPropagation();
      shrink();
    }
  }
  function scrolled() {
    show(current);
  }
  ["mousedown", "mouseup", "click", "auxclick", "contextmenu"].forEach(function (name) {
    shield.addEventListener(name, function (event) {
      event.preventDefault();
      event.stopPropagation();
      if (name === "click") pick(event);
    });
  });
  document.addEventListener("mousemove", move, true);
  document.addEventListener("wheel", wheel, { capture: true, passive: false });
  document.addEventListener("keydown", key, true);
  window.addEventListener("scroll", scrolled, true);
})();"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_site_saved_before_zoom_still_loads_and_a_zoom_alone_is_kept() {
        let rules: PageRules =
            serde_json::from_str(r#"{"sites":{"example.com":{"cleanPage":true}}}"#).unwrap();
        let rules = rules.normalize();
        assert_eq!(rules.sites["example.com"].zoom, 0);
        let zoomed = SiteControl {
            zoom: 900,
            ..SiteControl::default()
        }
        .normalize();
        assert!(zoomed.active());
        assert_eq!(zoomed.zoom, 300);
    }

    #[test]
    fn an_empty_file_loads() {
        let rules: PageRules = serde_json::from_str("{}").unwrap();
        let rules = rules.normalize();
        assert!(rules.sites.is_empty());
        assert!(rules.scripts.is_empty());
    }

    #[test]
    fn a_host_rule_matches_its_subdomain() {
        let mut sites = HashMap::new();
        sites.insert(
            "example.com".into(),
            SiteControl {
                clean_page: true,
                ..SiteControl::default()
            },
        );
        sites.insert(
            "news.example.com".into(),
            SiteControl {
                force_interaction: true,
                ..SiteControl::default()
            },
        );
        let best = best_control("news.example.com", &sites).unwrap();
        assert!(best.force_interaction);
        assert!(!best.clean_page);
        let parent = best_control("blog.example.com", &sites).unwrap();
        assert!(parent.clean_page);
        assert!(site_matches("example.com", "example.com"));
        assert!(site_matches("a.example.com", "example.com"));
        assert!(!site_matches("notexample.com", "example.com"));
    }

    #[test]
    fn a_script_over_the_size_cap_is_rejected() {
        let script = UserScript {
            name: "Big".into(),
            js: "x".repeat(MAX_CODE + 1),
            ..UserScript::default()
        };
        assert!(normalize_script(script).is_err());
    }

    #[test]
    fn inactive_sites_and_bad_selectors_are_dropped() {
        let rules = PageRules {
            sites: HashMap::from([(
                "WWW.Example.com".into(),
                SiteControl {
                    hidden: vec![
                        "#ok".into(),
                        "a{color:red}".into(),
                        "#ok".into(),
                        "div:nth-of-type(2) > a.banner".into(),
                        "main >>> .ad".into(),
                        "div > span { display: none }".into(),
                    ],
                    video_speed: 9.0,
                    ..SiteControl::default()
                },
            )]),
            scripts: vec![UserScript {
                name: "Notes".into(),
                hosts: vec!["https://www.News.example.com/path".into()],
                css: "body{}".into(),
                ..UserScript::default()
            }],
        };
        let rules = rules.normalize();
        let site = rules.sites.get("example.com").unwrap();
        assert_eq!(
            site.hidden,
            vec![
                "#ok".to_string(),
                "div:nth-of-type(2) > a.banner".to_string(),
                "main >>> .ad".to_string(),
            ]
        );
        assert_eq!(site.video_speed, 4.0);
        assert_eq!(rules.scripts[0].hosts, vec!["news.example.com".to_string()]);
        assert_eq!(rules.scripts[0].id, "notes");
    }
}
