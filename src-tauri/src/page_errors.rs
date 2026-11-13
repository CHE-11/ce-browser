use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tauri::AppHandle;
use url::Url;
use webview2_com::Microsoft::Web::WebView2::Win32::*;

/// WebView2 repeats the unresponsive report while a page stays hung, so an older one has
/// probably recovered.
const UNRESPONSIVE_FOR: Duration = Duration::from_secs(30);

/// Hosts whose certificate error the user chose to pass this session. Not saved.
static ALLOWED_CERTIFICATES: Mutex<Option<HashSet<String>>> = Mutex::new(None);
static RESTARTING: AtomicBool = AtomicBool::new(false);
static EXITING: AtomicBool = AtomicBool::new(false);
/// How this main-frame navigation was started. WebView2 reports it on `NavigationStarting`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadKind {
    NewDocument,
    Reload,
    BackOrForward,
}

/// What Back or Forward should do with the load that is underway.
#[derive(Debug, PartialEq, Eq)]
pub enum HistoryInterrupt {
    /// The current document is in history. Move to the neighboring entry.
    Entry,
    /// A new document has not committed. Stop it and show this page again.
    Stay(String),
}

/// Address each tab's current main-frame navigation is loading, when it started, and the
/// page it left. A download abort uses the previous page to decide whether to go back.
#[derive(Clone)]
struct NavigationTarget {
    url: String,
    previous: String,
    at: Instant,
    navigation_id: u64,
    kind: LoadKind,
    /// The new document has taken the webview. Back moves through history from here.
    committed: bool,
    /// `NavigationCompleted` has run for `navigation_id`.
    finished: bool,
    /// The user left this load with Back, Forward, or Stop. Its failure is not a dropped connection.
    ignore_failure: bool,
    /// Document those actions left. A late load event for it must not replace the page on screen.
    abandoned: String,
}

/// Address each tab's current main-frame navigation is loading.
static TARGETS: Mutex<Option<HashMap<u32, NavigationTarget>>> = Mutex::new(None);
/// Page a typed address left, recorded before the tab URL changes. The next navigation consumes it.
static LEAVING: Mutex<Option<HashMap<u32, String>>> = Mutex::new(None);
/// Renderer processes of tabs WebView2 reported as not answering, and when it last said so.
static UNRESPONSIVE: Mutex<Option<HashMap<u32, (Vec<u32>, Instant)>>> = Mutex::new(None);

/// The address bar moved to a new document before WebView2 started loading it.
pub fn note_leaving(id: u32, url: &str) {
    let url = url.trim();
    if url.is_empty() {
        return;
    }
    LEAVING
        .lock()
        .expect("leaving pages")
        .get_or_insert_with(HashMap::new)
        .insert(id, url.to_string());
}

fn take_leaving(id: u32) -> String {
    LEAVING
        .lock()
        .expect("leaving pages")
        .as_mut()
        .and_then(|pages| pages.remove(&id))
        .unwrap_or_default()
}

pub fn note_navigation(id: u32, url: &str, navigation_id: u64, kind: LoadKind) {
    let mut guard = TARGETS.lock().expect("navigation targets");
    let map = guard.get_or_insert_with(HashMap::new);
    if let Some(old) = map.get(&id)
        && old.navigation_id == navigation_id
        && navigation_id != 0
    {
        // A redirect of the load the user already cancelled. Keep the page that is showing.
        if old.ignore_failure {
            return;
        }
        let mut next = old.clone();
        next.url = url.to_string();
        next.kind = kind;
        map.insert(id, next);
        return;
    }
    let leaving = take_leaving(id);
    let (previous, abandoned) = map
        .get(&id)
        .map(|old| {
            let mut previous = if old.committed {
                old.url.clone()
            } else if !old.previous.is_empty() {
                old.previous.clone()
            } else {
                String::new()
            };
            if previous.is_empty() {
                previous = leaving.clone();
            }
            let mut abandoned = if !old.abandoned.is_empty() {
                old.abandoned.clone()
            } else if old.ignore_failure {
                old.url.clone()
            } else {
                String::new()
            };
            if !abandoned.is_empty() && same_document(&abandoned, url) {
                abandoned.clear();
            }
            (previous, abandoned)
        })
        .unwrap_or((leaving, String::new()));
    map.insert(
        id,
        NavigationTarget {
            url: url.to_string(),
            previous,
            at: Instant::now(),
            navigation_id,
            kind,
            committed: false,
            finished: false,
            ignore_failure: false,
            abandoned,
        },
    );
}

/// The page this navigation is replacing. A redirect keeps the page that was actually showing.
pub fn note_previous_page(id: u32, previous: &str) {
    let previous = previous.trim();
    if previous.is_empty() {
        return;
    }
    let mut guard = TARGETS.lock().expect("navigation targets");
    let Some(target) = guard.get_or_insert_with(HashMap::new).get_mut(&id) else {
        return;
    };
    if target.previous.is_empty() {
        target.previous = previous.to_string();
    }
}

fn with_target<T>(id: u32, change: impl FnOnce(&mut NavigationTarget) -> T) -> Option<T> {
    let mut guard = TARGETS.lock().expect("navigation targets");
    Some(change(guard.as_mut()?.get_mut(&id)?))
}

/// The document for `navigation_id` has committed, so Back moves through history.
pub fn note_committed(id: u32, navigation_id: u64) {
    with_target(id, |target| {
        if navigation_id != 0 && target.navigation_id != navigation_id {
            return;
        }
        // The load the user already left can still commit. It is not the page on screen.
        if target.ignore_failure {
            return;
        }
        target.committed = true;
        target.abandoned.clear();
    });
}

/// `Source` changed to a real document. `about:` hops are not a commit.
pub fn note_committed_source(id: u32, source: &str) {
    if source.is_empty() || source.to_ascii_lowercase().starts_with("about:") {
        return;
    }
    with_target(id, |target| {
        if target.ignore_failure || !same_document(&target.url, source) {
            return;
        }
        target.committed = true;
        target.abandoned.clear();
    });
}

pub fn note_finished(id: u32, navigation_id: u64) {
    with_target(id, |target| {
        if navigation_id != 0 && target.navigation_id != navigation_id {
            return;
        }
        target.finished = true;
    });
}

/// Back or Forward while a new document has not committed stays on the page that is showing.
/// Either way, the load being left must not surface as a dropped connection.
pub fn interrupt_history(id: u32) -> HistoryInterrupt {
    with_target(id, |target| {
        if target.finished {
            return HistoryInterrupt::Entry;
        }
        let stay = target.kind == LoadKind::NewDocument
            && !target.committed
            && !target.previous.is_empty()
            && !same_document(&target.previous, &target.url);
        target.ignore_failure = true;
        if target.abandoned.is_empty() {
            target.abandoned = target.url.clone();
        }
        if !stay {
            return HistoryInterrupt::Entry;
        }
        let previous = target.previous.clone();
        target.url = previous.clone();
        target.finished = true;
        target.committed = true;
        HistoryInterrupt::Stay(previous)
    })
    .unwrap_or(HistoryInterrupt::Entry)
}

/// `navigation_id` failed. False when that load was cancelled or a newer one has started.
pub fn failure_applies(id: u32, navigation_id: u64) -> bool {
    let guard = TARGETS.lock().expect("navigation targets");
    let Some(target) = guard.as_ref().and_then(|targets| targets.get(&id)) else {
        return true;
    };
    if navigation_id != 0 && target.navigation_id != navigation_id {
        return false;
    }
    !target.ignore_failure
}

/// A load event for the document Back, Forward, or Stop already left.
pub fn ignore_load_event(id: u32, url: &str) -> bool {
    let guard = TARGETS.lock().expect("navigation targets");
    let Some(target) = guard.as_ref().and_then(|targets| targets.get(&id)) else {
        return false;
    };
    if target.abandoned.is_empty() || !same_document(&target.abandoned, url) {
        return false;
    }
    !same_document(&target.url, url)
}

/// Same page, ignoring a hash. A fragment navigation is already a history entry.
fn same_document(left: &str, right: &str) -> bool {
    fn identity(value: &str) -> Option<String> {
        let mut url = Url::parse(value).ok()?;
        url.set_fragment(None);
        Some(url.to_string())
    }
    match (identity(left), identity(right)) {
        (Some(left), Some(right)) => left == right,
        _ => left == right,
    }
}

/// When the current main-frame navigation started. `None` after the tab is gone.
pub fn navigation_started_at(id: u32) -> Option<Instant> {
    TARGETS
        .lock()
        .expect("navigation targets")
        .as_ref()
        .and_then(|targets| targets.get(&id).map(|target| target.at))
}

/// The page shown before the current navigation, when one was recorded.
pub fn previous_page(id: u32) -> String {
    TARGETS
        .lock()
        .expect("navigation targets")
        .as_ref()
        .and_then(|targets| targets.get(&id))
        .map(|target| target.previous.clone())
        .unwrap_or_default()
}

pub fn forget_tab(id: u32) {
    if let Some(targets) = TARGETS.lock().expect("navigation targets").as_mut() {
        targets.remove(&id);
    }
    if let Some(leaving) = LEAVING.lock().expect("leaving pages").as_mut() {
        leaving.remove(&id);
    }
    clear_unresponsive(id);
}

pub fn note_unresponsive(id: u32, pids: Vec<u32>) {
    let mut guard = UNRESPONSIVE.lock().expect("unresponsive pages");
    let pages = guard.get_or_insert_with(HashMap::new);
    let known = pages
        .remove(&id)
        .map(|(known, _)| known)
        .unwrap_or_default();
    let pids = if pids.is_empty() { known } else { pids };
    pages.insert(id, (pids, Instant::now()));
}

pub fn clear_unresponsive(id: u32) {
    if let Some(pages) = UNRESPONSIVE.lock().expect("unresponsive pages").as_mut() {
        pages.remove(&id);
    }
}

/// Renderer processes of tab `id` while it is reported as hung. Empty once it recovered.
pub fn unresponsive_pids(id: u32) -> Vec<u32> {
    UNRESPONSIVE
        .lock()
        .expect("unresponsive pages")
        .as_ref()
        .and_then(|pages| pages.get(&id))
        .filter(|(_, at)| at.elapsed() < UNRESPONSIVE_FOR)
        .map(|(pids, _)| pids.clone())
        .unwrap_or_default()
}

/// Tabs WebView2 still reports as hung. An older report has probably recovered.
pub fn unresponsive_ids() -> HashSet<u32> {
    let guard = UNRESPONSIVE.lock().expect("unresponsive pages");
    guard
        .iter()
        .flat_map(|pages| pages.iter())
        .filter(|(_, (_, at))| at.elapsed() < UNRESPONSIVE_FOR)
        .map(|(id, _)| *id)
        .collect()
}

/// Renderer processes of every page currently reported as hung.
pub fn all_unresponsive_pids() -> Vec<u32> {
    let guard = UNRESPONSIVE.lock().expect("unresponsive pages");
    let mut pids: Vec<u32> = guard
        .iter()
        .flat_map(|pages| pages.values())
        .filter(|(_, at)| at.elapsed() < UNRESPONSIVE_FOR)
        .flat_map(|(pids, _)| pids.iter().copied())
        .collect();
    pids.sort_unstable();
    pids.dedup();
    pids
}

/// The certificate error is for the page itself, not an image or script on another host.
pub fn is_page_request(id: u32, url: &str) -> bool {
    let target = TARGETS
        .lock()
        .expect("navigation targets")
        .as_ref()
        .and_then(|targets| targets.get(&id).map(|target| target.url.clone()));
    match (target.as_deref().and_then(host_of), host_of(url)) {
        (Some(page), Some(request)) => page == request,
        _ => true,
    }
}

fn host_of(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()?
        .host_str()
        .map(|host| host.to_ascii_lowercase())
}

pub fn allow_certificate(url: &str) -> Result<(), String> {
    let host = host_of(url).ok_or("That address has no host.")?;
    ALLOWED_CERTIFICATES
        .lock()
        .expect("certificates")
        .get_or_insert_with(HashSet::new)
        .insert(host);
    Ok(())
}

pub fn certificate_allowed(url: &str) -> bool {
    let Some(host) = host_of(url) else {
        return false;
    };
    ALLOWED_CERTIFICATES
        .lock()
        .expect("certificates")
        .as_ref()
        .is_some_and(|hosts| hosts.contains(&host))
}

/// The failure kind and WebView2 name for a navigation that did not succeed. `None` means the
/// browser's own handling is right: a cancelled load or a sign-in prompt. A download, Back, and
/// Stop also end as `CONNECTION_ABORTED`; the navigation handler drops those.
pub fn web_error(status: COREWEBVIEW2_WEB_ERROR_STATUS) -> Option<(&'static str, &'static str)> {
    let found = match status {
        COREWEBVIEW2_WEB_ERROR_STATUS_HOST_NAME_NOT_RESOLVED => ("dns", "HOST_NAME_NOT_RESOLVED"),
        COREWEBVIEW2_WEB_ERROR_STATUS_TIMEOUT => ("timeout", "TIMEOUT"),
        COREWEBVIEW2_WEB_ERROR_STATUS_DISCONNECTED => ("offline", "DISCONNECTED"),
        COREWEBVIEW2_WEB_ERROR_STATUS_CONNECTION_ABORTED => ("offline", "CONNECTION_ABORTED"),
        COREWEBVIEW2_WEB_ERROR_STATUS_CONNECTION_RESET => ("offline", "CONNECTION_RESET"),
        COREWEBVIEW2_WEB_ERROR_STATUS_CANNOT_CONNECT => ("refused", "CANNOT_CONNECT"),
        COREWEBVIEW2_WEB_ERROR_STATUS_SERVER_UNREACHABLE => ("refused", "SERVER_UNREACHABLE"),
        COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_COMMON_NAME_IS_INCORRECT => {
            ("certificate", "CERTIFICATE_COMMON_NAME_IS_INCORRECT")
        }
        COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_EXPIRED => ("certificate", "CERTIFICATE_EXPIRED"),
        COREWEBVIEW2_WEB_ERROR_STATUS_CLIENT_CERTIFICATE_CONTAINS_ERRORS => {
            ("certificate", "CLIENT_CERTIFICATE_CONTAINS_ERRORS")
        }
        COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_REVOKED => ("certificate", "CERTIFICATE_REVOKED"),
        COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_IS_INVALID => {
            ("certificate", "CERTIFICATE_IS_INVALID")
        }
        COREWEBVIEW2_WEB_ERROR_STATUS_ERROR_HTTP_INVALID_SERVER_RESPONSE => {
            ("other", "ERROR_HTTP_INVALID_SERVER_RESPONSE")
        }
        COREWEBVIEW2_WEB_ERROR_STATUS_REDIRECT_FAILED => ("other", "REDIRECT_FAILED"),
        COREWEBVIEW2_WEB_ERROR_STATUS_UNEXPECTED_ERROR => ("other", "UNEXPECTED_ERROR"),
        _ => return None,
    };
    Some(found)
}

/// A readable date for seconds since 1970, such as `2026-11-01`.
pub fn date_of(seconds: f64) -> String {
    if !seconds.is_finite() || seconds <= 0.0 {
        return String::new();
    }
    let days = (seconds / 86_400.0).floor() as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// The app is closing, so a browser process that goes away is expected.
pub fn note_exiting() {
    EXITING.store(true, Ordering::SeqCst);
}

/// Every page view died with the WebView2 browser process. The session is saved and the app
/// starts again, which restores every tab.
pub fn browser_process_exited(app: &AppHandle) {
    if EXITING.load(Ordering::SeqCst) || RESTARTING.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = crate::session::save_now(app);
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_seconds_become_a_calendar_date() {
        assert_eq!(date_of(0.0), "");
        assert_eq!(date_of(86_400.0), "1970-01-02");
        assert_eq!(date_of(1_700_000_000.0), "2023-11-14");
    }

    #[test]
    fn a_cancelled_navigation_is_not_an_error_page() {
        assert!(web_error(COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED).is_none());
        assert_eq!(
            web_error(COREWEBVIEW2_WEB_ERROR_STATUS_HOST_NAME_NOT_RESOLVED).map(|(kind, _)| kind),
            Some("dns")
        );
    }

    #[test]
    fn an_allowed_certificate_covers_its_host_only() {
        allow_certificate("https://self-signed.test/login").expect("allow");
        assert!(certificate_allowed("https://SELF-SIGNED.test/other"));
        assert!(!certificate_allowed("https://other.test/"));
    }

    #[test]
    fn back_before_a_link_commits_stays_on_the_current_page() {
        let id = 910_001;
        note_navigation(id, "https://example.com/next", 5, LoadKind::NewDocument);
        note_previous_page(id, "https://example.com/now");
        assert_eq!(
            interrupt_history(id),
            HistoryInterrupt::Stay("https://example.com/now".into())
        );
        assert!(!failure_applies(id, 5));
        assert!(ignore_load_event(id, "https://example.com/next"));
        assert!(!ignore_load_event(id, "https://example.com/now"));
        forget_tab(id);
    }

    #[test]
    fn back_after_the_document_commits_leaves_that_navigation() {
        let id = 910_002;
        note_navigation(id, "https://example.com/next", 6, LoadKind::NewDocument);
        note_previous_page(id, "https://example.com/now");
        note_committed(id, 6);
        assert_eq!(interrupt_history(id), HistoryInterrupt::Entry);
        assert!(!failure_applies(id, 6));
        note_navigation(id, "https://example.com/now", 7, LoadKind::BackOrForward);
        assert!(!failure_applies(id, 6));
        assert!(failure_applies(id, 7));
        assert!(ignore_load_event(id, "https://example.com/next"));
        assert!(!ignore_load_event(id, "https://example.com/now"));
        forget_tab(id);
    }

    #[test]
    fn a_redirect_keeps_the_page_that_was_showing() {
        let id = 910_003;
        note_navigation(id, "https://example.com/next", 8, LoadKind::NewDocument);
        note_previous_page(id, "https://example.com/now");
        note_navigation(
            id,
            "https://example.com/redirected",
            8,
            LoadKind::NewDocument,
        );
        note_previous_page(id, "https://example.com/next");
        assert_eq!(previous_page(id), "https://example.com/now");
        assert_eq!(
            interrupt_history(id),
            HistoryInterrupt::Stay("https://example.com/now".into())
        );
        forget_tab(id);
    }

    #[test]
    fn a_finished_page_and_a_reload_use_history() {
        let id = 910_004;
        note_navigation(id, "https://example.com/next", 9, LoadKind::NewDocument);
        note_previous_page(id, "https://example.com/now");
        note_committed(id, 9);
        note_finished(id, 9);
        assert_eq!(interrupt_history(id), HistoryInterrupt::Entry);
        assert!(failure_applies(id, 9));

        note_navigation(id, "https://example.com/next", 10, LoadKind::Reload);
        note_previous_page(id, "https://example.com/next");
        assert_eq!(interrupt_history(id), HistoryInterrupt::Entry);
        assert!(!failure_applies(id, 10));
        forget_tab(id);
    }

    #[test]
    fn a_hash_change_and_a_typed_address_go_back_correctly() {
        let id = 910_005;
        note_navigation(
            id,
            "https://example.com/now#section",
            11,
            LoadKind::NewDocument,
        );
        note_previous_page(id, "https://example.com/now");
        assert_eq!(interrupt_history(id), HistoryInterrupt::Entry);
        forget_tab(id);

        note_leaving(id, "https://example.com/now");
        note_navigation(id, "https://example.com/typed", 12, LoadKind::NewDocument);
        assert_eq!(previous_page(id), "https://example.com/now");
        assert_eq!(
            interrupt_history(id),
            HistoryInterrupt::Stay("https://example.com/now".into())
        );
        forget_tab(id);
    }
}
