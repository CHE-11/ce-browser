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
/// Address each tab's current main-frame navigation is loading.
static TARGETS: Mutex<Option<HashMap<u32, String>>> = Mutex::new(None);
/// Renderer processes of tabs WebView2 reported as not answering, and when it last said so.
static UNRESPONSIVE: Mutex<Option<HashMap<u32, (Vec<u32>, Instant)>>> = Mutex::new(None);

pub fn note_navigation(id: u32, url: &str) {
    TARGETS
        .lock()
        .expect("navigation targets")
        .get_or_insert_with(HashMap::new)
        .insert(id, url.to_string());
}

pub fn forget_tab(id: u32) {
    if let Some(targets) = TARGETS.lock().expect("navigation targets").as_mut() {
        targets.remove(&id);
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
        .and_then(|targets| targets.get(&id).cloned());
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
/// browser's own handling is right: a cancelled load, a sign-in prompt, or a download.
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
        COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_IS_INVALID => ("certificate", "CERTIFICATE_IS_INVALID"),
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
}
