use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::{files, paths, popup, settings, tabs};

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
    ToggleTools,
    Downloads,
    Find,
    FindAll,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    DevTools,
    /// Ctrl+Shift+I. Website tools or the app tools, depending on what is under the pointer.
    Inspect,
    ClosePopup,
    ClosePeek,
    Workspace(usize),
    CommandPalette,
    TabSearch,
    ShortcutSheet,
    SleepTab,
    SleepOtherTabs,
    /// Opens the thumbnail switcher, or moves its highlight by this many tabs.
    TabSwitcher(isize),
    /// The first half of a two-key shortcut. The key is swallowed.
    ChordWait,
    AgentPanel,
    /// Starts recording a flow in the active tab, or stops and saves the recording.
    RecordFlow,
    /// Runs the flow bound to this hotkey slot, 1 to 9.
    Flow(usize),
    /// Dev builds only.
    DebugHud,
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
const VK_OEM_1: u32 = 0xBA;
const VK_OEM_PLUS: u32 = 0xBB;
const VK_OEM_COMMA: u32 = 0xBC;
const VK_OEM_MINUS: u32 = 0xBD;
const VK_OEM_PERIOD: u32 = 0xBE;
const VK_OEM_2: u32 = 0xBF;
const VK_OEM_3: u32 = 0xC0;
const VK_OEM_4: u32 = 0xDB;
const VK_OEM_5: u32 = 0xDC;
const VK_OEM_6: u32 = 0xDD;
const VK_OEM_7: u32 = 0xDE;

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
        Action::NextTab | Action::PreviousTab => {
            let step = if action == Action::NextTab { 1 } else { -1 };
            if popup::kind(app, &window).as_deref() == Some("tab-switcher") {
                popup::emit_to_top(app, &window, "tab-switcher-step", step);
                return Ok(());
            }
            let settings = settings::get(app);
            if settings.tab_cycle_order != "recent" {
                return tabs::cycle(app, step);
            }
            if !settings.tab_switcher {
                return tabs::cycle_recent(app, step);
            }
            show_tab_switcher(app, &window, step);
            Ok(())
        }
        Action::TabSwitcher(_) => {
            // Same panel as the tab-strip grid button. It stays up only while Ctrl is held.
            // A repeat of the shortcut must not toggle that panel shut.
            if popup::kind(app, &window).as_deref() != Some("tab-overview") {
                tabs::send_command(app, &window, "tab-overview");
            }
            Ok(())
        }
        Action::SelectTab(index) => tabs::select_index(app, Some(index)),
        Action::LastTab => tabs::select_index(app, None),
        Action::FocusAddress => {
            popup::close_all(app, &window, false);
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
            popup::close_all(app, &window, false);
            tabs::send_command(app, &window, "bookmark");
            Ok(())
        }
        Action::Downloads => {
            popup::close_all(app, &window, false);
            tabs::send_command(app, &window, "downloads");
            Ok(())
        }
        Action::Find => {
            popup::close_all(app, &window, false);
            tabs::send_command(app, &window, "find");
            Ok(())
        }
        Action::FindAll => {
            popup::close_all(app, &window, false);
            tabs::send_command(app, &window, "find-all");
            Ok(())
        }
        Action::ToggleBookmarksBar => settings::update(app, |settings| {
            settings.show_bookmarks_bar = !settings.show_bookmarks_bar
        })
        .map(|_| ()),
        Action::ToggleTools => settings::update(app, |settings| {
            settings.show_tools_toolbar = !settings.show_tools_toolbar
        })
        .map(|_| ()),
        Action::ZoomIn | Action::ZoomOut | Action::ZoomReset => {
            let change = match action {
                Action::ZoomIn => "in",
                Action::ZoomOut => "out",
                _ => "reset",
            };
            if popup::kind(app, &window).as_deref() == Some("tab-overview") {
                popup::emit_to_top(app, &window, "tab-overview-zoom", change);
                return Ok(());
            }
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
        Action::Inspect => {
            tabs::inspect_hovered(app);
            Ok(())
        }
        Action::ClosePopup => {
            popup::close_top(app, &window);
            Ok(())
        }
        Action::ClosePeek => {
            crate::peek::close(app, &window);
            Ok(())
        }
        Action::Workspace(index) => {
            if !settings::get(app).workspaces_enabled {
                return Ok(());
            }
            tabs::switch_workspace_index(app, index)
        }
        Action::CommandPalette => {
            popup::close_all(app, &window, false);
            tabs::send_command(app, &window, "palette");
            Ok(())
        }
        Action::TabSearch => {
            popup::close_all(app, &window, false);
            tabs::send_command(app, &window, "tab-search");
            Ok(())
        }
        Action::ShortcutSheet => {
            popup::close_all(app, &window, false);
            tabs::send_command(app, &window, "shortcut-sheet");
            Ok(())
        }
        Action::SleepTab => tabs::sleep_selected(app, &window),
        Action::SleepOtherTabs => match active {
            Some(id) => tabs::sleep_other_tabs(app, id),
            None => Ok(()),
        },
        Action::ChordWait => Ok(()),
        Action::AgentPanel => crate::automation::panel::toggle(app, &window, None).map(|_| ()),
        Action::RecordFlow => match active {
            Some(id) => crate::automation::recorder::toggle(app, id),
            None => Ok(()),
        },
        Action::Flow(slot) => {
            crate::automation::scheduler::hotkey(app, slot);
            Ok(())
        }
        Action::DebugHud => {
            if !crate::debug::hud_enabled() {
                return Ok(());
            }
            crate::debug::toggle_hud(app, &window).map(|_| ())
        }
    }
}

/// Opens the thumbnail switcher, or moves the highlight when it is already open.
fn show_tab_switcher(app: &AppHandle, window: &str, step: isize) {
    if popup::kind(app, window).as_deref() == Some("tab-switcher") {
        popup::emit_to_top(app, window, "tab-switcher-step", step);
        return;
    }
    let command = if step > 0 {
        "tab-switcher"
    } else {
        "tab-switcher-back"
    };
    tabs::send_command(app, window, command);
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutBinding {
    pub action: String,
    pub label: String,
    /// One token, or two for a chord: `["ctrl+k", "t"]`. Empty unbinds the action.
    pub keys: Vec<String>,
    /// Heading for the cheat sheet. Filled from the action, so `shortcuts.json` never stores it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub category: String,
}

/// Cheat sheet heading for an action.
fn category_of(action: &str) -> &'static str {
    if action.starts_with("select-tab-") {
        return "Tabs";
    }
    if action.starts_with("workspace-") {
        return "Workspaces";
    }
    if action.starts_with("flow-") || action == "agent-panel" || action == "record-flow" {
        return "Automation";
    }
    match action {
        "new-tab" | "new-window" | "close-tab" | "close-f4" | "reopen-tab" | "next-tab"
        | "previous-tab" | "next-page" | "previous-page" | "last-tab" | "tab-search"
        | "tab-switcher" | "tab-switcher-back" | "sleep-tab" | "sleep-other-tabs" => "Tabs",
        "focus-address"
        | "alt-focus"
        | "focus-f6"
        | "command-palette"
        | "bookmark"
        | "toggle-bookmarks-bar"
        | "toggle-tools" => "Address bar and bookmarks",
        "reload" | "reload-key" | "back" | "forward" | "find" | "find-all" | "zoom-in"
        | "zoom-in-shift" | "zoom-out" | "zoom-reset" | "devtools" | "inspect" | "debug-hud" => {
            "Page"
        }
        _ => "Browser",
    }
}

fn with_category(mut binding: ShortcutBinding) -> ShortcutBinding {
    if let Some(current) = defaults()
        .into_iter()
        .find(|item| item.action == binding.action)
    {
        binding.label = current.label;
    }
    binding.category = category_of(&binding.action).to_string();
    binding
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
    effective(&[])
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
    let saved = migrated(saved);
    let defaults = defaults();
    let mut rows = Vec::new();
    for binding in &saved {
        let unbound_by_default = defaults
            .iter()
            .any(|item| item.action == binding.action && item.keys.is_empty());
        if binding.keys.is_empty() && !unbound_by_default {
            continue;
        }
        rows.push(with_category(binding.clone()));
    }
    for binding in defaults {
        if !saved.iter().any(|item| item.action == binding.action) {
            rows.push(with_category(binding));
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
            category: String::new(),
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
        VK_OEM_1 => Some("semicolon"),
        VK_OEM_COMMA => Some("comma"),
        VK_OEM_PERIOD => Some("period"),
        VK_OEM_2 => Some("slash"),
        VK_OEM_3 => Some("backquote"),
        VK_OEM_4 => Some("bracketleft"),
        VK_OEM_5 => Some("backslash"),
        VK_OEM_6 => Some("bracketright"),
        VK_OEM_7 => Some("quote"),
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
    if let Some(rest) = action.strip_prefix("flow-") {
        return rest
            .parse()
            .ok()
            .filter(|slot| (1..=9).contains(slot))
            .map(Action::Flow);
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
        "toggle-tools" => Action::ToggleTools,
        "downloads" => Action::Downloads,
        "find" => Action::Find,
        "find-all" => Action::FindAll,
        "zoom-in" | "zoom-in-shift" => Action::ZoomIn,
        "zoom-out" => Action::ZoomOut,
        "zoom-reset" => Action::ZoomReset,
        "devtools" => Action::DevTools,
        "inspect" => Action::Inspect,
        "command-palette" => Action::CommandPalette,
        "tab-search" => Action::TabSearch,
        "shortcut-sheet" => Action::ShortcutSheet,
        "sleep-tab" => Action::SleepTab,
        "sleep-other-tabs" => Action::SleepOtherTabs,
        "tab-switcher" => Action::TabSwitcher(1),
        "tab-switcher-back" => Action::TabSwitcher(-1),
        "agent-panel" => Action::AgentPanel,
        "record-flow" => Action::RecordFlow,
        "debug-hud" => Action::DebugHud,
        _ => return None,
    })
}

/// `ctrl+shift+t` used to reopen a closed tab. That chord now shows or hides the tools toolbar.
fn migrated(saved: &[ShortcutBinding]) -> Vec<ShortcutBinding> {
    saved
        .iter()
        .cloned()
        .map(|mut binding| {
            if binding.action == "reopen-tab"
                && binding.keys.len() == 1
                && binding.keys[0] == "ctrl+shift+t"
            {
                binding.keys = vec!["ctrl+shift+z".into()];
            }
            binding
        })
        .collect()
}

fn bind(action: &str, label: &str, keys: &str) -> ShortcutBinding {
    ShortcutBinding {
        action: action.into(),
        label: label.into(),
        keys: keys.split_whitespace().map(|key| key.to_string()).collect(),
        category: String::new(),
    }
}

pub fn defaults() -> Vec<ShortcutBinding> {
    let mut rows = vec![
        bind("command-palette", "Command palette", "ctrl+k"),
        bind(
            "tab-search",
            "Search tabs and recently closed",
            "ctrl+shift+a",
        ),
        bind(
            "shortcut-sheet",
            "Keyboard shortcut cheat sheet",
            "ctrl+slash",
        ),
        bind("sleep-tab", "Sleep selected background tabs", ""),
        bind("sleep-other-tabs", "Sleep other tabs", ""),
        bind("new-tab", "New tab", "ctrl+t"),
        bind("close-tab", "Close tab", "ctrl+w"),
        bind("reopen-tab", "Reopen closed tab", "ctrl+shift+z"),
        bind("next-tab", "Next tab", "ctrl+tab"),
        bind("previous-tab", "Previous tab", "ctrl+shift+tab"),
        bind("tab-switcher", "All tabs", "ctrl+backquote"),
        bind("tab-switcher-back", "All tabs", "ctrl+shift+backquote"),
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
        bind("toggle-tools", "Show or hide tools", "ctrl+shift+t"),
        bind("downloads", "Downloads", "ctrl+j"),
        bind("find", "Find in page", "ctrl+f"),
        bind("find-all", "Find in all tabs", "ctrl+shift+f"),
        bind("zoom-in", "Zoom in", "ctrl+plus"),
        bind("zoom-out", "Zoom out", "ctrl+minus"),
        bind("zoom-reset", "Reset zoom", "ctrl+0"),
        bind("devtools", "Developer tools", "f12"),
        bind("inspect", "Inspect", "ctrl+shift+i"),
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
    rows.push(bind("agent-panel", "Agent panel", "ctrl+shift+y"));
    rows.push(bind("record-flow", "Record a flow, or stop recording", ""));
    for slot in 1..=9 {
        rows.push(bind(
            &format!("flow-{slot}"),
            &format!("Run hotkey flow {slot}"),
            "",
        ));
    }
    rows.push(bind("debug-hud", "Toggle debug HUD", "ctrl+shift+f12"));
    rows
}

#[cfg(test)]
mod tests {
    use super::{
        Action, ShortcutBinding, ShortcutFile, action_from, chord_token, effective, normalize_saved,
    };

    #[test]
    fn slash_and_brackets_have_names() {
        assert_eq!(
            chord_token(0xC0, true, false, false).as_deref(),
            Some("ctrl+backquote")
        );
        assert_eq!(
            chord_token(0xC0, true, true, false).as_deref(),
            Some("ctrl+shift+backquote")
        );
        assert_eq!(
            chord_token(0xBF, true, false, false).as_deref(),
            Some("ctrl+slash")
        );
        assert_eq!(
            chord_token(0xDB, false, false, false).as_deref(),
            Some("bracketleft")
        );
    }

    #[test]
    fn categories_are_shown_but_not_saved() {
        let rows = effective(&[]);
        let sheet = rows
            .iter()
            .find(|row| row.action == "tab-search")
            .expect("tab search");
        assert_eq!(sheet.category, "Tabs");
        let saved = normalize_saved(&rows);
        let json = serde_json::to_string(&ShortcutFile { bindings: saved }).expect("json");
        assert!(!json.contains("category"));
        let old: ShortcutFile = serde_json::from_str(
            r#"{"bindings":[{"action":"find","label":"Find","keys":["ctrl+g"]}]}"#,
        )
        .expect("older file");
        assert_eq!(old.bindings[0].keys, vec!["ctrl+g".to_string()]);
    }

    #[test]
    fn ctrl_backquote_opens_all_tabs() {
        let rows = effective(&[]);
        let forward = rows
            .iter()
            .find(|row| row.action == "tab-switcher")
            .expect("all tabs");
        assert_eq!(forward.label, "All tabs");
        assert_eq!(forward.keys, vec!["ctrl+backquote".to_string()]);
        assert_eq!(forward.category, "Tabs");
        let back = rows
            .iter()
            .find(|row| row.action == "tab-switcher-back")
            .expect("all tabs back");
        assert_eq!(back.label, "All tabs");
        assert_eq!(back.keys, vec!["ctrl+shift+backquote".to_string()]);
    }

    #[test]
    fn ctrl_shift_t_shows_or_hides_tools() {
        let rows = effective(&[]);
        let tools = rows
            .iter()
            .find(|row| row.action == "toggle-tools")
            .expect("tools");
        assert_eq!(tools.keys, vec!["ctrl+shift+t".to_string()]);
        let reopen = rows
            .iter()
            .find(|row| row.action == "reopen-tab")
            .expect("reopen");
        assert_eq!(reopen.keys, vec!["ctrl+shift+z".to_string()]);

        let saved = vec![ShortcutBinding {
            action: "reopen-tab".into(),
            label: "Reopen closed tab".into(),
            keys: vec!["ctrl+shift+t".into()],
            category: String::new(),
        }];
        let rows = effective(&saved);
        assert_eq!(
            rows.iter()
                .find(|row| row.action == "reopen-tab")
                .expect("reopen")
                .keys,
            vec!["ctrl+shift+z".to_string()]
        );
        assert_eq!(
            rows.iter()
                .find(|row| row.action == "toggle-tools")
                .expect("tools")
                .keys,
            vec!["ctrl+shift+t".to_string()]
        );
    }

    #[test]
    fn a_missing_action_keeps_the_default() {
        let saved = vec![ShortcutBinding {
            action: "new-tab".into(),
            label: "New tab".into(),
            keys: vec!["ctrl+shift+n".into()],
            category: String::new(),
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
    fn find_in_all_tabs_defaults_to_ctrl_shift_f() {
        let rows = effective(&[]);
        assert_eq!(
            rows.iter()
                .find(|row| row.action == "find-all")
                .expect("find all")
                .keys,
            vec!["ctrl+shift+f".to_string()]
        );
        assert_eq!(action_from("find-all"), Some(Action::FindAll));
    }

    #[test]
    fn an_empty_binding_unbinds_the_action() {
        let saved = vec![ShortcutBinding {
            action: "find".into(),
            label: "Find".into(),
            keys: vec![],
            category: String::new(),
        }];
        let rows = effective(&saved);
        assert!(rows.iter().all(|row| row.action != "find"));
    }
}
