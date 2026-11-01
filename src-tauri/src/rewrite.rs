//! Local request rules: block a URL, send it somewhere else, or answer with a file.

use std::fs;
use std::path::Path;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{files, paths};

const FILE: &str = "rewrite-rules.json";
const MAX_RULES: usize = 100;
const MAX_NEEDLE: usize = 500;
const MAX_TARGET: usize = 2_000;
const MAX_FILE: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct RewriteRule {
    pub id: u32,
    pub enabled: bool,
    /// The request URL must contain this text.
    pub contains: String,
    /// `block`, `redirect`, or `file`.
    pub action: String,
    /// Redirect address, or the path of a local file.
    pub target: String,
}

impl Default for RewriteRule {
    fn default() -> Self {
        Self {
            id: 0,
            enabled: true,
            contains: String::new(),
            action: "block".into(),
            target: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RewriteHit {
    Block,
    Redirect(String),
    File(String),
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RewriteFile {
    rules: Vec<RewriteRule>,
}

pub struct Rewrites(Mutex<Vec<RewriteRule>>);

pub fn init(app: &AppHandle) {
    let file: RewriteFile = files::read_json(&paths::data_file(app, FILE));
    app.manage(Rewrites(Mutex::new(normalize(file.rules))));
}

pub fn list(app: &AppHandle) -> Vec<RewriteRule> {
    app.state::<Rewrites>()
        .inner()
        .0
        .lock()
        .expect("rewrites")
        .clone()
}

pub fn save(app: &AppHandle, rules: Vec<RewriteRule>) -> Result<Vec<RewriteRule>, String> {
    let rules = normalize(rules);
    files::write_json(
        &paths::data_file(app, FILE),
        &RewriteFile {
            rules: rules.clone(),
        },
    )?;
    *app.state::<Rewrites>().inner().0.lock().expect("rewrites") = rules.clone();
    Ok(rules)
}

pub fn find(url: &str, rules: &[RewriteRule]) -> Option<RewriteHit> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return None;
    }
    for rule in rules {
        if !rule.enabled {
            continue;
        }
        let needle = rule.contains.trim();
        if needle.is_empty() || !url.contains(needle) {
            continue;
        }
        match rule.action.as_str() {
            "block" => return Some(RewriteHit::Block),
            "redirect" => {
                let target = rule.target.trim();
                if (target.starts_with("http://") || target.starts_with("https://"))
                    && target != url
                {
                    return Some(RewriteHit::Redirect(target.to_string()));
                }
            }
            "file" => {
                let path = rule.target.trim();
                if !path.is_empty() {
                    return Some(RewriteHit::File(path.to_string()));
                }
            }
            _ => {}
        }
    }
    None
}

pub fn file_bytes(path: &str) -> Option<Vec<u8>> {
    let path = Path::new(path);
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE {
        return None;
    }
    fs::read(path).ok()
}

fn normalize(rules: Vec<RewriteRule>) -> Vec<RewriteRule> {
    let mut next_id = rules.iter().map(|rule| rule.id).max().unwrap_or(0);
    let mut kept = Vec::new();
    for mut rule in rules {
        rule.contains = rule.contains.trim().chars().take(MAX_NEEDLE).collect();
        rule.target = rule.target.trim().chars().take(MAX_TARGET).collect();
        if rule.contains.is_empty() {
            continue;
        }
        rule.action = match rule.action.as_str() {
            "redirect" => "redirect".into(),
            "file" => "file".into(),
            _ => "block".into(),
        };
        if rule.id == 0 {
            next_id += 1;
            rule.id = next_id;
        }
        kept.push(rule);
        if kept.len() == MAX_RULES {
            break;
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::{RewriteHit, RewriteRule, find};

    fn rule(action: &str, contains: &str, target: &str) -> RewriteRule {
        RewriteRule {
            id: 1,
            enabled: true,
            contains: contains.into(),
            action: action.into(),
            target: target.into(),
        }
    }

    #[test]
    fn a_block_rule_matches_the_substring() {
        let hit = find(
            "https://ads.example/banner.js",
            &[rule("block", "ads.example", "")],
        );
        assert_eq!(hit, Some(RewriteHit::Block));
        assert_eq!(
            find(
                "https://example.com/ok",
                &[rule("block", "ads.example", "")]
            ),
            None
        );
    }

    #[test]
    fn a_redirect_rule_sends_the_request_to_the_target() {
        let hit = find(
            "https://example.com/old.js",
            &[rule("redirect", "/old.js", "https://cdn.example/new.js")],
        );
        assert_eq!(
            hit,
            Some(RewriteHit::Redirect("https://cdn.example/new.js".into()))
        );
        assert_eq!(
            find("ce://settings", &[rule("block", "settings", "")]),
            None
        );
    }
}
