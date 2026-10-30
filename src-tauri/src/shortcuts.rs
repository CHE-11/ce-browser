use tauri::AppHandle;

use crate::{settings, tabs};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Ui,
    Page,
    Popup,
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
    let letter = |character: char| key == character as u32;
    if target == Target::Popup && key == VK_ESCAPE && !ctrl && !alt {
        return Some(Action::ClosePopup);
    }
    if ctrl && !alt {
        if shift {
            return match key {
                _ if letter('T') => Some(Action::ReopenTab),
                _ if letter('B') => Some(Action::ToggleBookmarksBar),
                VK_TAB => Some(Action::PreviousTab),
                VK_OEM_PLUS | VK_ADD => Some(Action::ZoomIn),
                _ => None,
            };
        }
        return match key {
            _ if letter('T') || letter('N') => Some(Action::NewTab),
            _ if letter('W') => Some(Action::CloseTab),
            _ if letter('L') => Some(Action::FocusAddress),
            _ if letter('R') => Some(Action::Reload),
            _ if letter('D') => Some(Action::Bookmark),
            _ if letter('J') => Some(Action::Downloads),
            _ if letter('F') => Some(Action::Find),
            _ if (0x31..=0x38).contains(&key) => Some(Action::SelectTab((key - 0x31) as usize)),
            0x39 => Some(Action::LastTab),
            0x30 | VK_NUMPAD0 => Some(Action::ZoomReset),
            VK_TAB | VK_NEXT => Some(Action::NextTab),
            VK_PRIOR => Some(Action::PreviousTab),
            VK_F4 => Some(Action::CloseTab),
            VK_OEM_PLUS | VK_ADD => Some(Action::ZoomIn),
            VK_OEM_MINUS | VK_SUBTRACT => Some(Action::ZoomOut),
            _ => None,
        };
    }
    if alt && !ctrl && !shift {
        return match key {
            VK_LEFT => Some(Action::Back),
            VK_RIGHT => Some(Action::Forward),
            _ if letter('D') => Some(Action::FocusAddress),
            _ => None,
        };
    }
    if !ctrl && !alt && !shift {
        return match key {
            VK_F5 => Some(Action::Reload),
            VK_F6 => Some(Action::FocusAddress),
            VK_F12 => Some(Action::DevTools),
            _ => None,
        };
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
    match action {
        Action::NewTab => tabs::create_tab(app, None, false, None).map(|_| ()),
        Action::CloseTab => tabs::close_selected(app),
        Action::ReopenTab => tabs::reopen_closed(app).map(|_| ()),
        Action::NextTab => tabs::cycle(app, 1),
        Action::PreviousTab => tabs::cycle(app, -1),
        Action::SelectTab(index) => tabs::select_index(app, Some(index)),
        Action::LastTab => tabs::select_index(app, None),
        Action::FocusAddress => {
            tabs::close_popup(app);
            tabs::send_command(app, "focus-address");
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
            tabs::close_popup(app);
            tabs::send_command(app, "bookmark");
            Ok(())
        }
        Action::Downloads => {
            tabs::close_popup(app);
            tabs::send_command(app, "downloads");
            Ok(())
        }
        Action::Find => {
            tabs::close_popup(app);
            tabs::send_command(app, "find");
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
            tabs::close_popup(app);
            Ok(())
        }
    }
}
