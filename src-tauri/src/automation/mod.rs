//! Browser automation: the page driver, the AI agent, recorded flows, extraction,
//! the scheduler and the local MCP server.

pub mod agent;
pub mod desktop;
pub mod dialog;
pub mod driver;
pub mod effects;
pub mod extract;
pub mod flows;
pub mod mcp;
pub mod panel;
pub mod policy;
pub mod recorder;
pub mod scheduler;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use serde_json::json;
use tauri::AppHandle;

use crate::{paths, tabs};

const LOG_FILE: &str = "automation-log.jsonl";
const LOG_LIMIT: u64 = 2_000_000;

/// Whether a flow or the agent is driving this tab. JavaScript dialogs are deferred then.
pub fn driving(tab: u32) -> bool {
    agent::run_for_tab(tab).is_some() || flows::is_driving(tab)
}

/// Stops whatever was driving the tab and forgets its page state.
pub fn tab_closed(app: &AppHandle, tab: u32) {
    agent::tab_closed(app, tab);
    recorder::tab_closed(app, tab);
    flows::tab_closed(app, tab);
    driver::forget(tab);
}

/// A `data:` URL for a JPEG, for thumbnails shown in the UI.
pub fn jpeg_data_url(bytes: &[u8]) -> String {
    format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

/// Today's date in UTC as `YYYY-MM-DD`.
pub fn today() -> String {
    let days = (now_ms() / 86_400_000) as i64;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Days since 1970-01-01 to a calendar date (Howard Hinnant's algorithm).
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

/// Appends one automated action to `automation-log.jsonl` in the profile folder.
pub fn audit(app: &AppHandle, source: &str, tab: u32, action: &str, detail: &str) {
    let path = paths::data_file(app, LOG_FILE);
    if fs::metadata(&path).is_ok_and(|meta| meta.len() > LOG_LIMIT) {
        let _ = fs::rename(&path, path.with_extension("jsonl.old"));
    }
    let url = tabs::tab_info(app, tab)
        .map(|info| info.url)
        .unwrap_or_default();
    let line = json!({
        "at": now_ms(),
        "source": source,
        "tab": tab,
        "url": url,
        "action": action,
        "detail": clip(detail, 400),
    });
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{line}");
    }
}

/// Resolves `{{secret:name}}` placeholders from the automation vault. Returns the text and
/// whether it held a secret.
pub fn fill_secrets(app: &AppHandle, text: &str) -> Result<(String, bool), String> {
    let mut out = String::new();
    let mut rest = text;
    let mut found = false;
    while let Some(start) = rest.find("{{secret:") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 9..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let name = after[..end].trim();
        let value = crate::secrets::vault_resolve(app, name)?
            .ok_or_else(|| format!("There is no saved secret named \"{name}\"."))?;
        out.push_str(&value);
        found = true;
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok((out, found))
}

/// The text with each secret placeholder shown as dots, for logs and the UI.
pub fn mask_secrets(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{secret:") {
        out.push_str(&rest[..start]);
        match rest[start..].find("}}") {
            Some(end) => {
                out.push_str("•••");
                rest = &rest[start + end + 2..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_masks() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_454), (2026, 1, 1));
        assert_eq!(mask_secrets("user {{secret:pw}} end"), "user ••• end");
        assert_eq!(clip("abcdef", 3), "abc…");
    }
}
