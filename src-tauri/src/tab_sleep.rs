use std::time::Duration;

use tauri::AppHandle;

use crate::tabs;

const CHECK_EVERY: Duration = Duration::from_secs(30);

/// Suspends background tabs that have been off screen longer than the setting allows.
pub fn init(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(CHECK_EVERY);
            for id in tabs::sleep_candidates(&app) {
                let _ = tabs::sleep_tab(&app, id);
            }
        }
    });
}
