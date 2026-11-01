use std::time::Duration;

use tauri::AppHandle;

use crate::{tabs, webview2};

const TICK: Duration = Duration::from_secs(1);
const SLEEP_EVERY: u32 = 30;

/// Reloads tabs on a timer and suspends background tabs that have been off screen long enough.
pub fn init(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let mut ticks = 0u32;
        loop {
            std::thread::sleep(TICK);
            ticks += 1;
            refresh_due(&app);
            crate::system::apply_launch(&app);
            if ticks.is_multiple_of(5) {
                crate::system::refresh_tray(&app);
            }
            if crate::system::saver_active(&app) || ticks.is_multiple_of(SLEEP_EVERY) {
                for id in tabs::sleep_candidates(&app) {
                    let _ = tabs::sleep_tab(&app, id);
                }
            }
            crate::system::sync_frames(&app);
        }
    });
}

fn refresh_due(app: &AppHandle) {
    for (id, watch) in tabs::refresh_due(app) {
        tabs::stamp_refresh(app, id);
        if watch {
            if let Some(webview) = tabs::page_view(app, id) {
                let mark = webview2::evaluate(
                    &webview,
                    r#"(function(){var text=document.body?document.body.innerText:"";var h=0;for(var i=0;i<text.length;i+=17)h=(h*33+text.charCodeAt(i))>>>0;return text.length+":"+h})()"#,
                    Duration::from_secs(2),
                    "Couldn't read the page.",
                );
                if let Ok(value) = mark {
                    let text = value.as_str().unwrap_or("").to_string();
                    if tabs::note_content(app, id, &text) {
                        tabs::emit_tabs(app);
                    }
                }
            }
        }
        let _ = tabs::reload(app, id);
    }
}
