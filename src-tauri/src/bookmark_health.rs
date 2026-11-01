use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::{bookmarks, paths};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkHealthBookmark {
    pub id: String,
    pub title: String,
    pub url: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkDuplicateGroup {
    pub url: String,
    pub bookmarks: Vec<BookmarkHealthBookmark>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkHealthLink {
    pub id: String,
    pub title: String,
    pub url: String,
    /// `dead`, `blocked`, or `unreachable`.
    pub status: String,
    pub detail: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkHealthReport {
    pub duplicates: Vec<BookmarkDuplicateGroup>,
    pub failed: Vec<BookmarkHealthLink>,
    pub checked: u32,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BookmarkHealthProgress {
    done: u32,
    total: u32,
}

struct LinkRef {
    id: String,
    title: String,
    url: String,
}

pub fn check(app: &AppHandle) -> Result<BookmarkHealthReport, String> {
    let tree = bookmarks::tree(app);
    let mut links = Vec::new();
    collect(&tree.bar, &mut links);
    collect(&tree.other, &mut links);
    let duplicates = duplicate_groups(&links);
    let http: Vec<LinkRef> = links
        .into_iter()
        .filter(|link| is_http(&link.url))
        .collect();
    let total = http.len() as u32;
    emit_progress(app, 0, total);
    let done = AtomicU32::new(0);
    let failed = Mutex::new(Vec::new());
    thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                loop {
                    let index = done.fetch_add(1, Ordering::Relaxed);
                    if index >= total {
                        break;
                    }
                    let link = &http[index as usize];
                    if let Some(problem) = probe(&link.url) {
                        failed
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .push(BookmarkHealthLink {
                                id: link.id.clone(),
                                title: link.title.clone(),
                                url: link.url.clone(),
                                status: problem.0,
                                detail: problem.1,
                            });
                    }
                    let finished = (index + 1).min(total);
                    emit_progress(app, finished, total);
                }
            });
        }
    });
    let mut failed = failed
        .into_inner()
        .unwrap_or_else(|error| error.into_inner());
    failed.sort_by(|left, right| left.title.to_lowercase().cmp(&right.title.to_lowercase()));
    Ok(BookmarkHealthReport {
        duplicates,
        failed,
        checked: total,
    })
}

fn collect(node: &bookmarks::BookmarkNode, links: &mut Vec<LinkRef>) {
    if let Some(url) = &node.url {
        links.push(LinkRef {
            id: node.id.clone(),
            title: node.title.clone(),
            url: url.clone(),
        });
    }
    for child in node.children.as_deref().unwrap_or(&[]) {
        collect(child, links);
    }
}

fn duplicate_groups(links: &[LinkRef]) -> Vec<BookmarkDuplicateGroup> {
    let mut groups: BTreeMap<String, Vec<BookmarkHealthBookmark>> = BTreeMap::new();
    for link in links {
        if !is_http(&link.url) {
            continue;
        }
        groups
            .entry(paths::pin_key(&link.url))
            .or_default()
            .push(BookmarkHealthBookmark {
                id: link.id.clone(),
                title: link.title.clone(),
                url: link.url.clone(),
            });
    }
    groups
        .into_iter()
        .filter(|(_, bookmarks)| bookmarks.len() > 1)
        .map(|(url, bookmarks)| BookmarkDuplicateGroup { url, bookmarks })
        .collect()
}

fn is_http(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

pub(crate) fn classify(code: u16) -> Option<(&'static str, String)> {
    match code {
        200..=399 => None,
        401 | 403 | 429 => Some(("blocked", code.to_string())),
        404 | 410 => Some(("dead", code.to_string())),
        405 | 501 => Some(("unreachable", code.to_string())),
        400..=499 => Some(("dead", code.to_string())),
        _ => Some(("unreachable", code.to_string())),
    }
}

fn probe(url: &str) -> Option<(String, String)> {
    match request(url, true) {
        Probe::Ok => None,
        Probe::Status(405) | Probe::Status(501) => match request(url, false) {
            Probe::Ok => None,
            Probe::Status(code) => {
                classify(code).map(|(status, detail)| (status.to_string(), detail))
            }
            Probe::Error(detail) => Some(("unreachable".into(), detail)),
        },
        Probe::Status(code) => classify(code).map(|(status, detail)| (status.to_string(), detail)),
        Probe::Error(detail) => Some(("unreachable".into(), detail)),
    }
}

enum Probe {
    Ok,
    Status(u16),
    Error(String),
}

fn request(url: &str, head: bool) -> Probe {
    let agent = "ce-browser";
    let result = if head {
        ureq::head(url)
            .timeout(Duration::from_secs(8))
            .set("User-Agent", agent)
            .call()
    } else {
        ureq::get(url)
            .timeout(Duration::from_secs(8))
            .set("User-Agent", agent)
            .call()
    };
    match result {
        Ok(response) => match classify(response.status()) {
            None => Probe::Ok,
            Some(_) => Probe::Status(response.status()),
        },
        Err(ureq::Error::Status(code, _)) => Probe::Status(code),
        Err(error) => Probe::Error(error.to_string()),
    }
}

fn emit_progress(app: &AppHandle, done: u32, total: u32) {
    let _ = app.emit("bookmark-health", BookmarkHealthProgress { done, total });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(id: &str, url: &str) -> LinkRef {
        LinkRef {
            id: id.into(),
            title: id.into(),
            url: url.into(),
        }
    }

    #[test]
    fn duplicate_links_share_a_trailing_slash() {
        let groups = duplicate_groups(&[
            link("a", "https://example.com/docs"),
            link("b", "https://example.com/docs/"),
            link("c", "https://example.com/other"),
            link("d", "javascript:void(0)"),
        ]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].bookmarks.len(), 2);
    }

    #[test]
    fn status_codes_separate_dead_links_from_blocked_ones() {
        assert!(classify(204).is_none());
        assert!(classify(301).is_none());
        assert_eq!(classify(404).unwrap().0, "dead");
        assert_eq!(classify(410).unwrap().0, "dead");
        assert_eq!(classify(403).unwrap().0, "blocked");
        assert_eq!(classify(401).unwrap().0, "blocked");
        assert_eq!(classify(500).unwrap().0, "unreachable");
    }
}
