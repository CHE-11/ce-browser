//! Every native webview, where it sits and what it shows.

use serde::Serialize;
use tauri::{AppHandle, Manager, Webview};

/// Popups park their windows at -20000. Anything this far off is not on any monitor.
const PARKED_AT: i32 = -10000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugWebview {
    pub label: String,
    /// Label of the native window that hosts it.
    pub window: String,
    /// `ui`, `tab`, `popup`, `extpopup`, `devtools`, `agent`, `peek`, `script`, `debug` or `other`.
    pub kind: String,
    pub tab_id: Option<u32>,
    pub url: String,
    /// Physical px inside the host window.
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Physical px on the virtual screen.
    pub screen_x: i32,
    pub screen_y: i32,
    /// Moved off screen or outside its window, such as a background tab or an idle popup.
    pub parked: bool,
    pub window_visible: bool,
    /// The debug outline is drawn around it now.
    pub outlined: bool,
}

pub fn kind_of(label: &str) -> &'static str {
    const PREFIXES: [(&str, &str); 10] = [
        ("ui-", "ui"),
        ("tab-", "tab"),
        ("popup-", "popup"),
        ("extpopup-", "extpopup"),
        ("devtools-", "devtools"),
        ("inspect-", "devtools"),
        ("agent-", "agent"),
        ("peek-", "peek"),
        ("script-", "script"),
        ("debug-", "debug"),
    ];
    if label == "ui" {
        return "ui";
    }
    PREFIXES
        .iter()
        .find(|(prefix, _)| label.starts_with(prefix))
        .map(|(_, kind)| *kind)
        .unwrap_or("other")
}

pub fn tab_id(label: &str) -> Option<u32> {
    label.strip_prefix("tab-")?.parse().ok()
}

fn rank(kind: &str) -> usize {
    [
        "ui", "tab", "devtools", "agent", "peek", "popup", "extpopup", "script", "debug",
    ]
    .iter()
    .position(|item| *item == kind)
    .unwrap_or(usize::MAX)
}

/// `screen` and `local` are the webview's top-left corner, `size` its size and `client` the
/// host window's client size.
fn is_parked(screen: (i32, i32), local: (i32, i32), size: (u32, u32), client: (u32, u32)) -> bool {
    if screen.0 <= PARKED_AT || screen.1 <= PARKED_AT || size.0 == 0 || size.1 == 0 {
        return true;
    }
    let right = local.0.saturating_add(size.0 as i32);
    let bottom = local.1.saturating_add(size.1 as i32);
    right <= 0 || bottom <= 0 || local.0 >= client.0 as i32 || local.1 >= client.1 as i32
}

fn describe(webview: &Webview) -> Option<DebugWebview> {
    let window = webview.window();
    let label = webview.label().to_string();
    let origin = window.inner_position().ok()?;
    let client = window.inner_size().ok()?;
    let at = webview.position().ok()?;
    let size = webview.size().ok()?;
    let screen = (origin.x + at.x, origin.y + at.y);
    let kind = kind_of(&label);
    Some(DebugWebview {
        tab_id: tab_id(&label),
        kind: kind.to_string(),
        url: webview.url().map(|url| url.to_string()).unwrap_or_default(),
        x: at.x,
        y: at.y,
        width: size.width,
        height: size.height,
        screen_x: screen.0,
        screen_y: screen.1,
        parked: is_parked(
            screen,
            (at.x, at.y),
            (size.width, size.height),
            (client.width, client.height),
        ),
        window_visible: window.is_visible().unwrap_or(false),
        outlined: false,
        window: window.label().to_string(),
        label,
    })
}

pub fn list(app: &AppHandle) -> Vec<DebugWebview> {
    let mut rows: Vec<DebugWebview> = app.webviews().values().filter_map(describe).collect();
    rows.sort_by(|left, right| {
        rank(&left.kind)
            .cmp(&rank(&right.kind))
            .then_with(|| left.tab_id.cmp(&right.tab_id))
            .then_with(|| left.label.cmp(&right.label))
    });
    rows
}

#[cfg(test)]
mod tests {
    use super::{is_parked, kind_of, tab_id};

    #[test]
    fn labels_map_to_kinds() {
        assert_eq!(kind_of("ui"), "ui");
        assert_eq!(kind_of("ui-2"), "ui");
        assert_eq!(kind_of("tab-14"), "tab");
        assert_eq!(kind_of("popup-main-0"), "popup");
        assert_eq!(kind_of("extpopup-main"), "extpopup");
        assert_eq!(kind_of("devtools-main"), "devtools");
        assert_eq!(kind_of("agent-main"), "agent");
        assert_eq!(kind_of("peek-frame-1"), "peek");
        assert_eq!(kind_of("script-3"), "script");
        assert_eq!(kind_of("debug-main"), "debug");
        assert_eq!(kind_of("uikit"), "other");
    }

    #[test]
    fn only_tab_labels_have_a_tab_id() {
        assert_eq!(tab_id("tab-7"), Some(7));
        assert_eq!(tab_id("tab-x"), None);
        assert_eq!(tab_id("ui"), None);
    }

    #[test]
    fn parked_views_are_off_screen_or_outside_their_window() {
        let client = (1200, 800);
        assert!(!is_parked((100, 140), (0, 40), (1200, 760), client));
        assert!(is_parked((-20000, -20000), (0, 0), (900, 900), client));
        assert!(is_parked((100, 140), (1300, 40), (1200, 760), client));
        assert!(is_parked((100, 140), (-1200, 40), (1200, 760), client));
        assert!(is_parked((100, 140), (0, 40), (0, 760), client));
    }
}
