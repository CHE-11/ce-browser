//! Windows and monitors the setup popover can record, plus the HWND a take actually captures.

use serde::Serialize;
use tauri::{AppHandle, Manager};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::core::BOOL;
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GetCursorPos, GetWindowLongPtrW, GetWindowRect, GetWindowTextW, IsWindow, IsWindowVisible,
    WS_EX_TOOLWINDOW,
};

use crate::tabs;

use super::StartRequest;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingTarget {
    pub id: String,
    pub kind: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct Target {
    #[allow(dead_code)]
    pub kind: String,
    pub hwnd: isize,
    pub monitor: isize,
    pub label: String,
}

#[derive(Debug, Clone, Copy)]
pub struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub fn list(skip_hwnd: isize) -> Vec<RecordingTarget> {
    let mut found: Vec<RecordingTarget> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(enum_window), LPARAM(&mut found as *mut Vec<RecordingTarget> as isize));
        let _ = EnumDisplayMonitors(None, None, Some(enum_monitor), LPARAM(&mut found as *mut Vec<RecordingTarget> as isize));
    }
    found.retain(|target| target.kind != "window" || target.id != skip_hwnd.to_string());
    found
}

pub fn resolve(app: &AppHandle, request: &StartRequest) -> Result<Target, String> {
    match request.source.as_str() {
        "tab" => {
            let webview = tabs::page_view(app, request.tab_id).ok_or("This page is not loaded yet.")?;
            let hwnd = crate::webview2::host_hwnd(&webview)?;
            Ok(Target {
                kind: "tab".into(),
                hwnd,
                monitor: 0,
                label: fallback(&request.title, "Tab"),
            })
        }
        "browser" => {
            let label = if request.window.is_empty() { tabs::WINDOW.to_string() } else { request.window.clone() };
            let window = app.get_window(&label).ok_or("The browser window is missing.")?;
            let hwnd = window.hwnd().map_err(|error| error.to_string())?;
            Ok(Target {
                kind: "browser".into(),
                hwnd: hwnd.0 as isize,
                monitor: 0,
                label: "This window".into(),
            })
        }
        "window" => {
            let hwnd = request.target.parse::<isize>().map_err(|_| "Choose a window to record.".to_string())?;
            if hwnd == 0 || !unsafe { IsWindow(Some(HWND(hwnd as *mut core::ffi::c_void))).as_bool() } {
                return Err("That window is no longer open.".into());
            }
            Ok(Target {
                kind: "window".into(),
                hwnd,
                monitor: 0,
                label: window_title(hwnd).unwrap_or_else(|| "Window".into()),
            })
        }
        "display" => {
            let monitor = request.target.parse::<isize>().map_err(|_| "Choose a screen to record.".to_string())?;
            if monitor == 0 {
                return Err("Choose a screen to record.".into());
            }
            Ok(Target {
                kind: "display".into(),
                hwnd: 0,
                monitor,
                label: monitor_label(monitor).unwrap_or_else(|| "Screen".into()),
            })
        }
        _ => Err("Choose what to record.".into()),
    }
}

pub fn screen_rect(target: &Target) -> Option<ScreenRect> {
    if target.monitor != 0 {
        return monitor_rect(target.monitor);
    }
    if target.hwnd == 0 {
        return None;
    }
    let hwnd = HWND(target.hwnd as *mut core::ffi::c_void);
    if !unsafe { IsWindow(Some(hwnd)).as_bool() } {
        return None;
    }
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect).ok()? };
    let width = rect.right.saturating_sub(rect.left);
    let height = rect.bottom.saturating_sub(rect.top);
    if width < 40 || height < 40 {
        return None;
    }
    Some(ScreenRect { x: rect.left, y: rect.top, width, height })
}

/// Pointer position in the frame, in pixels. It can sit outside the picture.
pub fn cursor_in_frame(target: &Target, width: u32, height: u32) -> Option<(f32, f32)> {
    let rect = screen_rect(target)?;
    if rect.width <= 0 || rect.height <= 0 || width == 0 || height == 0 {
        return None;
    }
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point).ok()? };
    let x = (point.x - rect.x) as f32 / rect.width as f32 * width as f32;
    let y = (point.y - rect.y) as f32 / rect.height as f32 * height as f32;
    Some((x, y))
}

pub fn alive(target: &Target) -> bool {
    if target.monitor != 0 {
        return monitor_rect(target.monitor).is_some();
    }
    target.hwnd != 0 && unsafe { IsWindow(Some(HWND(target.hwnd as *mut core::ffi::c_void))).as_bool() }
}

unsafe extern "system" fn enum_window(hwnd: HWND, data: LPARAM) -> BOOL {
    let found = unsafe { &mut *(data.0 as *mut Vec<RecordingTarget>) };
    if !unsafe { IsWindowVisible(hwnd).as_bool() } {
        return BOOL(1);
    }
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    if style & (WS_EX_TOOLWINDOW.0 as isize) != 0 {
        return BOOL(1);
    }
    let Some(title) = window_title(hwnd.0 as isize) else {
        return BOOL(1);
    };
    if title.starts_with("Recording") || title.starts_with("ce-browser outline") {
        return BOOL(1);
    }
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect).is_err() } {
        return BOOL(1);
    }
    if rect.right - rect.left < 80 || rect.bottom - rect.top < 80 {
        return BOOL(1);
    }
    found.push(RecordingTarget {
        id: (hwnd.0 as isize).to_string(),
        kind: "window".into(),
        label: title,
    });
    BOOL(1)
}

unsafe extern "system" fn enum_monitor(monitor: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
    let found = unsafe { &mut *(data.0 as *mut Vec<RecordingTarget>) };
    let label = monitor_label(monitor.0 as isize).unwrap_or_else(|| "Screen".into());
    found.push(RecordingTarget {
        id: (monitor.0 as isize).to_string(),
        kind: "display".into(),
        label,
    });
    BOOL(1)
}

fn window_title(hwnd: isize) -> Option<String> {
    let mut buffer = [0u16; 256];
    let length = unsafe { GetWindowTextW(HWND(hwnd as *mut core::ffi::c_void), &mut buffer) };
    if length <= 0 {
        return None;
    }
    let title = String::from_utf16_lossy(&buffer[..length as usize]).trim().to_string();
    (!title.is_empty()).then_some(title)
}

fn monitor_label(monitor: isize) -> Option<String> {
    let rect = monitor_rect(monitor)?;
    Some(format!("Screen {}×{}", rect.width, rect.height))
}

fn monitor_rect(monitor: isize) -> Option<ScreenRect> {
    if monitor == 0 {
        return None;
    }
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..MONITORINFO::default()
    };
    let ok = unsafe { GetMonitorInfoW(HMONITOR(monitor as *mut core::ffi::c_void), &mut info).as_bool() };
    if !ok {
        return None;
    }
    let rect = info.rcMonitor;
    let width = rect.right.saturating_sub(rect.left);
    let height = rect.bottom.saturating_sub(rect.top);
    (width > 0 && height > 0).then_some(ScreenRect { x: rect.left, y: rect.top, width, height })
}

fn fallback(title: &str, default: &str) -> String {
    let title = title.trim();
    if title.is_empty() { default.into() } else { title.to_string() }
}
