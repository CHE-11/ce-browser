use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::{files, paths, settings, tabs};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Ui,
    Page,
    Popup,
    /// The peek frame and the page inside it.
    Peek,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    NewTab,
    CloseTab,
    ReopenTab,
    NextTab,
    PreviousTab,
    SelectTab(usize),
    LastTab,
    FocusAddress,
    Reload,
    Back,
    Forward,
    Bookmark,
    ToggleBookmarksBar,
    Downloads,
    Find,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    DevTools,
    ClosePopup,
    ClosePeek,
    Workspace(usize),
    CommandPalette,
    /// The first half of a two-key shortcut. The key is swallowed.
    ChordWait,
}

const VK_TAB: u32 = 0x09;
const VK_ESCAPE: u32 = 0x1B;
const VK_PRIOR: u32 = 0x21;
const VK_NEXT: u32 = 0x22;
const VK_LEFT: u32 = 0x25;
const VK_RIGHT: u32 = 0x27;
const VK_NUMPAD0: u32 = 0x60;
const VK_ADD: u32 = 0x6B;
const VK_SUBTRACT: u32 = 0x6D;
const VK_F4: u32 = 0x73;
const VK_F5: u32 = 0x74;
const VK_F6: u32 = 0x75;
const VK_F12: u32 = 0x7B;
const VK_OEM_PLUS: u32 = 0xBB;
const VK_OEM_MINUS: u32 = 0xBD;

pub fn action_for(key: u32, ctrl: bool, shift: bool, alt: bool, target: Target) -> Option<Action> {
    if target == Target::Popup && key == VK_ESCAPE && !ctrl && !alt {
        return Some(Action::ClosePopup);
    }
    if target == Target::Peek && key == VK_ESCAPE && !ctrl && !alt && !shift {
        return Some(Action::ClosePeek);
    }
    let token = chord_token(key, ctrl, shift, alt)?;
    if let Some(action) = binding_action(&token) {
        return Some(action);
    }
    None
}

/// WebView2 raises accelerator keys on the main thread; the work runs off it.
pub fn dispatch(app: &AppHandle, action: Action) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = run(&app, action) {
            eprintln!("shortcut {action:?}: {error}");
        }
    });
}

pub fn run(app: &AppHandle, action: Action) -> Result<(), String> {
    let active = tabs::active_id(app);
    let window = tabs::focused_window(app);
    match action {
        Action::NewTab => tabs::create_tab(app, None, false, None).map(|_| ()),
        Action::CloseTab => tabs::close_selected(app),
        Action::ReopenTab => tabs::reopen_closed(app).map(|_| ()),
        Action::NextTab => tabs::cycle(app, 1),
        Action::PreviousTab => tabs::cycle(app, -1),
        Action::SelectTab(index) => tabs::select_index(app, Some(index)),
        Action::LastTab => tabs::select_index(app, None),
        Action::FocusAddress => {
            tabs::close_window_popup(app, &window);
            tabs::send_command(app, &window, "focus-address");
            Ok(())
        }
        Action::Reload => match active {
            Some(id) => tabs::reload(app, id),
            None => Ok(()),
        },
        Action::Back => {
            if let Some(id) = active {
                tabs::go_back(app, id);
            }
            Ok(())
        }
        Action::Forward => {
            if let Some(id) = active {
                tabs::go_forward(app, id);
            }
            Ok(())
        }
        Action::Bookmark => {
            tabs::close_window_popup(app, &window);
            tabs::send_command(app, &window, "bookmark");
            Ok(())
        }
        Action::Downloads => {
            tabs::close_window_popup(app, &window);
            tabs::send_command(app, &window, "downloads");
            Ok(())
        }
        Action::Find => {
            tabs::close_window_popup(app, &window);
            tabs::send_command(app, &window, "find");
            Ok(())
        }
        Action::ToggleBookmarksBar => settings::update(app, |settings| {
            settings.show_bookmarks_bar = !settings.show_bookmarks_bar
        })
        .map(|_| ()),
        Action::ZoomIn | Action::ZoomOut | Action::ZoomReset => {
            let change = match action {
                Action::ZoomIn => "in",
                Action::ZoomOut => "out",
                _ => "reset",
            };
            match active {
                Some(id) => tabs::zoom(app, id, change).map(|_| ()),
                None => Ok(()),
            }
        }
        Action::DevTools => {
            if let Some(id) = active {
                tabs::open_devtools(app, id);
            }
            Ok(())
        }
        Action::ClosePopup => {
            tabs::close_window_popup(app, &window);
            Ok(())
        }
        Action::ClosePeek => {
            crate::peek::close(app, &window);
            Ok(())
        }
        Action::Workspace(index) => tabs::switch_workspace_index(app, index),
        Action::CommandPalette => {
            tabs::close_window_popup(app, &window);
            tabs::send_command(app, &window, "palette");
            Ok(())
        }
        Action::ChordWait => Ok(()),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutBinding {
    pub action: String,
    pub label: String,
    /// One token, or two for a chord: `["ctrl+k", "t"]`. Empty unbinds the action.
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct ShortcutFile {
    bindings: Vec<ShortcutBinding>,
}

struct ShortcutState {
    bindings: Mutex<Vec<ShortcutBinding>>,
}

const FILE: &str = "shortcuts.json";

pub fn init(app: &AppHandle) {
    let loaded: ShortcutFile = files::read_json(&paths::data_file(app, FILE));
    store_bindings(&loaded.bindings);
    app.manage(ShortcutState {
        bindings: Mutex::new(effective(&loaded.bindings)),
    });
}

pub fn list(app: &AppHandle) -> Vec<ShortcutBinding> {
    if let Some(state) = app.try_state::<ShortcutState>() {
        return state.bindings.lock().expect("shortcuts").clone();
    }
    defaults()
}

pub fn save(
    app: &AppHandle,
    bindings: Vec<ShortcutBinding>,
) -> Result<Vec<ShortcutBinding>, String> {
    let bindings = normalize_saved(&bindings);
    files::write_json(
        &paths::data_file(app, FILE),
        &ShortcutFile {
            bindings: bindings.clone(),
        },
    )?;
    let shown = effective(&bindings);
    store_bindings(&bindings);
    if let Some(state) = app.try_state::<ShortcutState>() {
        *state.bindings.lock().expect("shortcuts") = shown.clone();
    }
    let _ = app.emit("shortcuts-changed", &shown);
    Ok(shown)
}

pub fn reset_action(app: &AppHandle, action: &str) -> Result<Vec<ShortcutBinding>, String> {
    let mut saved = saved_or_empty(app);
    saved.retain(|binding| binding.action != action);
    save(app, saved)
}

pub fn reset_all(app: &AppHandle) -> Result<Vec<ShortcutBinding>, String> {
    save(app, Vec::new())
}

fn saved_or_empty(app: &AppHandle) -> Vec<ShortcutBinding> {
    let loaded: ShortcutFile = files::read_json(&paths::data_file(app, FILE));
    loaded.bindings
}

/// Saved rows win. An action missing from the file keeps its default. An empty `keys` list unbinds it.
pub fn effective(saved: &[ShortcutBinding]) -> Vec<ShortcutBinding> {
    let mut rows = Vec::new();
    for binding in saved {
        if binding.keys.is_empty() {
            continue;
        }
        rows.push(binding.clone());
    }
    for binding in defaults() {
        if !saved.iter().any(|item| item.action == binding.action) {
            rows.push(binding);
        }
    }
    rows
}

fn normalize_saved(bindings: &[ShortcutBinding]) -> Vec<ShortcutBinding> {
    let known: Vec<String> = defaults()
        .into_iter()
        .map(|binding| binding.action)
        .collect();
    let mut rows = Vec::new();
    for binding in bindings {
        if !known.iter().any(|action| action == &binding.action)
            || rows
                .iter()
                .any(|item: &ShortcutBinding| item.action == binding.action)
        {
            continue;
        }
        let keys: Vec<String> = binding
            .keys
            .iter()
            .map(|key| key.trim().to_ascii_lowercase())
            .filter(|key| !key.is_empty() && key.len() <= 40)
            .take(2)
            .collect();
        let label = defaults()
            .into_iter()
            .find(|item| item.action == binding.action)
            .map(|item| item.label)
            .unwrap_or_else(|| binding.action.clone());
        rows.push(ShortcutBinding {
            action: binding.action.clone(),
            label,
            keys,
        });
    }
    rows
}

fn binding_action(token: &str) -> Option<Action> {
    let bindings = live_bindings();
    let pending = take_pending();
    if let Some(first) = pending {
        if let Some(binding) = bindings.iter().find(|binding| {
            binding.keys.len() == 2 && binding.keys[0] == first && binding.keys[1] == token
        }) {
            return action_from(&binding.action);
        }
    }
    let exact = bindings
        .iter()
        .find(|binding| binding.keys.len() == 1 && binding.keys[0] == token);
    let prefix = bindings
        .iter()
        .any(|binding| binding.keys.len() == 2 && binding.keys[0] == token);
    if prefix && exact.is_none() {
        set_pending(token);
        return Some(Action::ChordWait);
    }
    exact.and_then(|binding| action_from(&binding.action))
}

static BINDINGS: Mutex<Vec<ShortcutBinding>> = Mutex::new(Vec::new());

fn live_bindings() -> Vec<ShortcutBinding> {
    let guard = BINDINGS.lock().expect("shortcuts");
    if guard.is_empty() {
        defaults()
    } else {
        guard.clone()
    }
}

fn store_bindings(bindings: &[ShortcutBinding]) {
    *BINDINGS.lock().expect("shortcuts") = effective(bindings)
        .into_iter()
        .filter(|binding| !binding.keys.is_empty())
        .collect();
}

fn take_pending() -> Option<String> {
    PENDING
        .lock()
        .expect("chord")
        .take()
        .and_then(|(token, at)| (at.elapsed() < std::time::Duration::from_secs(1)).then_some(token))
}

fn set_pending(token: &str) {
    *PENDING.lock().expect("chord") = Some((token.to_string(), Instant::now()));
}

static PENDING: Mutex<Option<(String, Instant)>> = Mutex::new(None);

fn chord_token(key: u32, ctrl: bool, shift: bool, alt: bool) -> Option<String> {
    let name = key_name(key)?;
    let mut parts = Vec::new();
    if ctrl {
        parts.push("ctrl");
    }
    if shift {
        parts.push("shift");
    }
    if alt {
        parts.push("alt");
    }
    parts.push(name);
    Some(parts.join("+"))
}

fn key_name(key: u32) -> Option<&'static str> {
    match key {
        0x30..=0x39 => digit(key - 0x30),
        0x41..=0x5A => letter(key - 0x41),
        0x70..=0x7B => function_key(key - 0x70),
        VK_TAB => Some("tab"),
        VK_LEFT => Some("left"),
        VK_RIGHT => Some("right"),
        VK_OEM_PLUS | VK_ADD => Some("plus"),
        VK_OEM_MINUS | VK_SUBTRACT => Some("minus"),
        VK_PRIOR => Some("pageup"),
        VK_NEXT => Some("pagedown"),
        _ => None,
    }
}

fn digit(index: u32) -> Option<&'static str> {
    Some(["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"][index as usize])
}

fn letter(index: u32) -> Option<&'static str> {
    const LETTERS: [&str; 26] = [
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r",
        "s", "t", "u", "v", "w", "x", "y", "z",
    ];
    Some(LETTERS[index as usize])
}

fn function_key(index: u32) -> Option<&'static str> {
    const KEYS: [&str; 12] = [
        "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12",
    ];
    KEYS.get(index as usize).copied()
}

fn action_from(action: &str) -> Option<Action> {
    if let Some(rest) = action.strip_prefix("select-tab-") {
        return rest.parse().ok().map(Action::SelectTab);
    }
    if let Some(rest) = action.strip_prefix("workspace-") {
        return rest.parse().ok().map(Action::Workspace);
    }
    Some(match action {
        "new-tab" | "new-window" => Action::NewTab,
        "close-tab" | "close-f4" => Action::CloseTab,
        "reopen-tab" => Action::ReopenTab,
        "next-tab" | "next-page" => Action::NextTab,
        "previous-tab" | "previous-page" => Action::PreviousTab,
        "last-tab" => Action::LastTab,
        "focus-address" | "alt-focus" | "focus-f6" => Action::FocusAddress,
        "reload" | "reload-key" => Action::Reload,
        "back" => Action::Back,
        "forward" => Action::Forward,
        "bookmark" => Action::Bookmark,
        "toggle-bookmarks-bar" => Action::ToggleBookmarksBar,
        "downloads" => Action::Downloads,
        "find" => Action::Find,
        "zoom-in" | "zoom-in-shift" => Action::ZoomIn,
        "zoom-out" => Action::ZoomOut,
        "zoom-reset" => Action::ZoomReset,
        "devtools" => Action::DevTools,
        "command-palette" => Action::CommandPalette,
        _ => return None,
    })
}

fn bind(action: &str, label: &str, keys: &str) -> ShortcutBinding {
    ShortcutBinding {
        action: action.into(),
        label: label.into(),
        keys: keys.split_whitespace().map(|key| key.to_string()).collect(),
    }
}

pub fn defaults() -> Vec<ShortcutBinding> {
    let mut rows = vec![
        bind("command-palette", "Command palette", "ctrl+k"),
        bind("new-tab", "New tab", "ctrl+t"),
        bind("close-tab", "Close tab", "ctrl+w"),
        bind("reopen-tab", "Reopen closed tab", "ctrl+shift+t"),
        bind("next-tab", "Next tab", "ctrl+tab"),
        bind("previous-tab", "Previous tab", "ctrl+shift+tab"),
        bind("last-tab", "Last tab", "ctrl+9"),
        bind("focus-address", "Focus address bar", "ctrl+l"),
        bind("alt-focus", "Focus address bar", "alt+d"),
        bind("focus-f6", "Focus address bar", "f6"),
        bind("reload", "Reload", "ctrl+r"),
        bind("reload-key", "Reload", "f5"),
        bind("back", "Back", "alt+left"),
        bind("forward", "Forward", "alt+right"),
        bind("bookmark", "Bookmark this page", "ctrl+d"),
        bind(
            "toggle-bookmarks-bar",
            "Show or hide bookmarks bar",
            "ctrl+shift+b",
        ),
        bind("downloads", "Downloads", "ctrl+j"),
        bind("find", "Find in page", "ctrl+f"),
        bind("zoom-in", "Zoom in", "ctrl+plus"),
        bind("zoom-out", "Zoom out", "ctrl+minus"),
        bind("zoom-reset", "Reset zoom", "ctrl+0"),
        bind("devtools", "Developer tools", "f12"),
        bind("new-window", "New tab", "ctrl+n"),
        bind("close-f4", "Close tab", "ctrl+f4"),
        bind("next-page", "Next tab", "ctrl+pagedown"),
        bind("previous-page", "Previous tab", "ctrl+pageup"),
        bind("zoom-in-shift", "Zoom in", "ctrl+shift+plus"),
    ];
    for index in 0..8 {
        rows.push(bind(
            &format!("select-tab-{index}"),
            &format!("Select tab {}", index + 1),
            &format!("ctrl+{}", index + 1),
        ));
    }
    for index in 0..9 {
        rows.push(bind(
            &format!("workspace-{index}"),
            &format!("Workspace {}", index + 1),
            &format!("ctrl+alt+{}", index + 1),
        ));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::{ShortcutBinding, effective};

    #[test]
    fn a_missing_action_keeps_the_default() {
        let saved = vec![ShortcutBinding {
            action: "new-tab".into(),
            label: "New tab".into(),
            keys: vec!["ctrl+shift+n".into()],
        }];
        let rows = effective(&saved);
        assert!(
            rows.iter()
                .any(|row| row.action == "new-tab" && row.keys == vec!["ctrl+shift+n".to_string()])
        );
        assert!(
            rows.iter().any(
                |row| row.action == "command-palette" && row.keys == vec!["ctrl+k".to_string()]
            )
        );
    }

    #[test]
    fn an_empty_binding_unbinds_the_action() {
        let saved = vec![ShortcutBinding {
            action: "find".into(),
            label: "Find".into(),
            keys: vec![],
        }];
        let rows = effective(&saved);
        assert!(rows.iter().all(|row| row.action != "find"));
    }
}
