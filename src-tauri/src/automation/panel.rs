//! The agent panel: a docked column beside the pages that shows agent runs.

use tauri::webview::Color;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl};

use crate::{paths, settings, tabs, webview2};

pub const PANEL_WIDTH: f64 = 380.0;

/// Opens or closes the panel in `window`. `open` forces a state. Returns whether it is open.
pub fn toggle(app: &AppHandle, window: &str, open: Option<bool>) -> Result<bool, String> {
    let is_open = tabs::agent_panel_width(app, window) > 0.0;
    let want = open.unwrap_or(!is_open);
    if want {
        ensure_view(app, window)?;
        tabs::set_agent_panel(app, window, PANEL_WIDTH);
        if let Some(view) = app.get_webview(&tabs::agent_panel_label(window)) {
            let _ = view.set_focus();
        }
    } else {
        tabs::set_agent_panel(app, window, 0.0);
    }
    Ok(want)
}

fn ensure_view(app: &AppHandle, window: &str) -> Result<(), String> {
    let label = tabs::agent_panel_label(window);
    if app.get_webview(&label).is_some() {
        return Ok(());
    }
    let host = app
        .get_window(window)
        .ok_or("The browser window is missing.")?;
    let (r, g, b) = settings::popup_surface(app);
    let url = paths::internal_url(app, "agent-panel");
    let builder = tabs::base_builder(app, &label, WebviewUrl::External(url))
        .background_color(Color(r, g, b, 255))
        .focused(false);
    let view = host
        .add_child(
            builder,
            LogicalPosition::new(-10000.0, -10000.0),
            LogicalSize::new(PANEL_WIDTH, 400.0),
        )
        .map_err(|error| error.to_string())?;
    webview2::attach_panel_hooks(app, &view);
    Ok(())
}
