//! Omnibox keywords such as `gh react` or `!yt cats`.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::{files, paths};

const FILE: &str = "keywords.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Keyword {
    pub keyword: String,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeywordHit {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct KeywordFile {
    keywords: Vec<Keyword>,
}

struct KeywordState(std::sync::Mutex<Vec<Keyword>>);

pub fn init(app: &AppHandle) {
    let loaded: KeywordFile = files::read_json(&paths::data_file(app, FILE));
    let keywords = if loaded.keywords.is_empty() {
        defaults()
    } else {
        normalize(loaded.keywords)
    };
    app.manage(KeywordState(std::sync::Mutex::new(keywords)));
}

pub fn list(app: &AppHandle) -> Vec<Keyword> {
    app.state::<KeywordState>()
        .0
        .lock()
        .expect("keywords")
        .clone()
}

pub fn save(app: &AppHandle, keywords: Vec<Keyword>) -> Result<Vec<Keyword>, String> {
    let keywords = normalize(keywords);
    files::write_json(
        &paths::data_file(app, FILE),
        &KeywordFile {
            keywords: keywords.clone(),
        },
    )?;
    *app.state::<KeywordState>().0.lock().expect("keywords") = keywords.clone();
    let _ = app.emit("keywords-changed", &keywords);
    Ok(keywords)
}

pub fn defaults() -> Vec<Keyword> {
    vec![
        keyword("gh", "GitHub", "https://github.com/search?q=%s"),
        keyword(
            "yt",
            "YouTube",
            "https://www.youtube.com/results?search_query=%s",
        ),
        keyword(
            "w",
            "Wikipedia",
            "https://en.wikipedia.org/wiki/Special:Search?search=%s",
        ),
        keyword("r", "Reddit", "https://www.reddit.com/search/?q=%s"),
    ]
}

/// `!gh react` or `gh react`. A bare keyword suggests the template with the query removed.
pub fn expand(keywords: &[Keyword], query: &str) -> Option<KeywordHit> {
    let query = query.trim();
    let bare = query.strip_prefix('!').unwrap_or(query);
    let (word, rest) = bare.split_once([' ', '\t']).unwrap_or((bare, ""));
    let word = word.trim().to_ascii_lowercase();
    if word.is_empty() {
        return None;
    }
    let keyword = keywords.iter().find(|item| item.keyword == word)?;
    let rest = rest.trim();
    let url = if rest.is_empty() {
        keyword
            .url
            .replace("%s", "")
            .trim_end_matches(['?', '&', '='])
            .to_string()
    } else {
        keyword.url.replace("%s", &urlencoding(&rest))
    };
    let title = if rest.is_empty() {
        format!("Search {}", keyword.title)
    } else {
        format!("Search {} for {rest}", keyword.title)
    };
    Some(KeywordHit { title, url })
}

fn keyword(keyword: &str, title: &str, url: &str) -> Keyword {
    Keyword {
        keyword: keyword.into(),
        title: title.into(),
        url: url.into(),
    }
}

fn normalize(keywords: Vec<Keyword>) -> Vec<Keyword> {
    let mut kept = Vec::new();
    for mut keyword in keywords {
        keyword.keyword = keyword
            .keyword
            .trim()
            .trim_start_matches('!')
            .to_ascii_lowercase();
        keyword.title = keyword.title.trim().chars().take(60).collect();
        keyword.url = keyword.url.trim().to_string();
        if !valid_keyword(&keyword.keyword)
            || !keyword.url.contains("%s")
            || kept
                .iter()
                .any(|item: &Keyword| item.keyword == keyword.keyword)
        {
            continue;
        }
        if keyword.title.is_empty() {
            keyword.title = keyword.keyword.clone();
        }
        kept.push(keyword);
        if kept.len() >= 100 {
            break;
        }
    }
    kept
}

fn valid_keyword(keyword: &str) -> bool {
    (1..=20).contains(&keyword.len()) && keyword.chars().all(|ch| ch.is_ascii_alphanumeric())
}

fn urlencoding(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bang_and_a_bare_keyword_search() {
        let keywords = defaults();
        let hit = expand(&keywords, "!gh react hooks").unwrap();
        assert_eq!(hit.title, "Search GitHub for react hooks");
        assert!(
            hit.url.contains("react+hooks")
                || hit.url.contains("react%20hooks")
                || hit.url.contains("react+hooks")
        );
        assert!(hit.url.starts_with("https://github.com/search"));
        let bare = expand(&keywords, "yt cats").unwrap();
        assert!(bare.url.contains("cats"));
        assert!(bare.title.contains("YouTube"));
    }

    #[test]
    fn a_keyword_without_a_query_drops_the_placeholder() {
        let hit = expand(&defaults(), "gh").unwrap();
        assert!(!hit.url.contains("%s"));
        assert!(expand(&defaults(), "github react").is_none());
    }
}
