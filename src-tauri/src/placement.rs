//! Where this profile last left a browser window, remembered per computer.
//!
//! Computers that share a profile have different monitors. `window.json` keeps a
//! frame for each computer name, and a save replaces only this computer's entry.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Window};

use crate::{files, paths};

const FILE: &str = "window.json";
const SAVE_DELAY: Duration = Duration::from_millis(800);
const MIN_WIDTH: u32 = 640;
const MIN_HEIGHT: u32 = 420;
const MAX_EDGE: u32 = 16_384;
/// Top strip that has to meet a monitor before the saved frame is kept.
const GRAB_HEIGHT: u32 = 32;
const GRAB_WIDTH: i64 = 64;
const DEFAULT_WIDTH: f64 = 1280.0;
const DEFAULT_HEIGHT: f64 = 840.0;
/// Windows parks a minimized window near this coordinate.
const MINIMIZED_SENTINEL: i32 = -32_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WindowFile {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

impl Default for WindowFile {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            maximized: false,
        }
    }
}

/// `window.json`. `computers` is keyed by the Windows computer name.
///
/// Files written before computer names are one frame at the top level. That
/// frame is used while `computers` is still empty. A save writes `computers`
/// only and replaces this computer's entry, leaving every other computer's
/// frame as it was.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct WindowStore {
    computers: BTreeMap<String, WindowFile>,
    #[serde(skip_serializing)]
    x: i32,
    #[serde(skip_serializing)]
    y: i32,
    #[serde(skip_serializing)]
    width: u32,
    #[serde(skip_serializing)]
    height: u32,
    #[serde(skip_serializing)]
    maximized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Frame {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WorkArea {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

struct PlacementState {
    tx: Mutex<Option<Sender<()>>>,
    current: Mutex<Option<WindowFile>>,
    suppress: Mutex<bool>,
}

/// Move and resize events arrive in a stream. The saver thread writes once that stream
/// has been quiet for `SAVE_DELAY`.
pub fn init(app: &AppHandle) {
    let (tx, rx) = mpsc::channel::<()>();
    let saver_app = app.clone();
    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            loop {
                match rx.recv_timeout(SAVE_DELAY) {
                    Ok(()) => continue,
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            let _ = save_now(&saver_app);
        }
    });
    app.manage(PlacementState {
        tx: Mutex::new(Some(tx)),
        current: Mutex::new(None),
        suppress: Mutex::new(false),
    });
}

/// The frame saved for this computer, when it has a real size.
///
/// An older file has a single frame and no computer name. This computer uses
/// that frame when `computers` has no entry for it yet.
pub fn saved(app: &AppHandle) -> Option<WindowFile> {
    let path = paths::data_file(app, FILE);
    if !path.is_file() {
        return None;
    }
    let store: WindowStore = files::read_json(&path);
    frame_for(&store, &crate::downloads::computer_name())
}

/// Places `window` on the saved monitor. A monitor that is no longer connected
/// gets the frame centered on the primary work area instead.
pub fn apply(app: &AppHandle, window: &Window, file: WindowFile) {
    let frame = fit(
        Frame {
            x: file.x,
            y: file.y,
            width: file.width,
            height: file.height,
        },
        &work_areas(app),
    );
    set_suppress(app, true);
    let _ = window.set_position(PhysicalPosition::new(frame.x, frame.y));
    let _ = window.set_size(PhysicalSize::new(frame.width, frame.height));
    if file.maximized {
        let _ = window.maximize();
    }
    if let Some(state) = app.try_state::<PlacementState>() {
        *state.current.lock().expect("placement lock") = Some(WindowFile {
            x: frame.x,
            y: frame.y,
            width: frame.width,
            height: frame.height,
            maximized: file.maximized,
        });
    }
    set_suppress(app, false);
}

/// Remembers this browser window. A minimized or fullscreen frame is ignored so it
/// cannot replace the windowed one. While maximized, only the flag changes.
pub fn note(window: &Window) {
    let app = window.app_handle();
    let Some(state) = app.try_state::<PlacementState>() else {
        return;
    };
    if *state.suppress.lock().expect("placement lock") {
        return;
    }
    if window.is_minimized().unwrap_or(false) || window.is_fullscreen().unwrap_or(false) {
        return;
    }
    let maximized = window.is_maximized().unwrap_or(false);
    // Read the window before taking the placement lock. These getters run on the
    // main thread and can re-enter this handler.
    let normal = if maximized {
        None
    } else {
        match (window.outer_position(), window.inner_size()) {
            (Ok(position), Ok(size))
                if position.x > MINIMIZED_SENTINEL
                    && position.y > MINIMIZED_SENTINEL
                    && size.width > 0
                    && size.height > 0 =>
            {
                Some((position, size))
            }
            _ => return,
        }
    };
    let fallback = if maximized {
        Some(fallback_frame(window))
    } else {
        None
    };
    let mut current = state.current.lock().expect("placement lock");
    if let Some((position, size)) = normal {
        *current = Some(WindowFile {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
            maximized: false,
        });
    } else if maximized {
        let restored = (*current).or_else(|| fallback);
        if let Some(restored) = restored {
            *current = Some(WindowFile {
                maximized: true,
                ..restored
            });
        }
    }
    drop(current);
    schedule(&app);
}

/// Writes the remembered frame immediately and stops the debounced saver.
pub fn flush(app: &AppHandle) {
    let _ = save_now(app);
    if let Some(state) = app.try_state::<PlacementState>() {
        state.tx.lock().expect("placement lock").take();
    }
}

fn schedule(app: &AppHandle) {
    let Some(state) = app.try_state::<PlacementState>() else {
        return;
    };
    if let Some(tx) = state.tx.lock().expect("placement lock").as_ref() {
        let _ = tx.send(());
    }
}

fn save_now(app: &AppHandle) -> Result<(), String> {
    let Some(state) = app.try_state::<PlacementState>() else {
        return Ok(());
    };
    let Some(frame) = *state.current.lock().expect("placement lock") else {
        return Ok(());
    };
    // Read again so a frame another computer saved since startup stays in the file.
    let path = paths::data_file(app, FILE);
    let mut store: WindowStore = files::read_json(&path);
    remember(&mut store, &crate::downloads::computer_name(), frame);
    files::write_json(&path, &store)
}

/// This computer's frame. A shared older file still applies when nobody has a named entry.
fn frame_for(store: &WindowStore, computer: &str) -> Option<WindowFile> {
    if let Some(frame) = find_frame(store, computer) {
        return real_frame(frame);
    }
    if store.computers.is_empty() {
        return real_frame(store.legacy_frame());
    }
    None
}

fn find_frame(store: &WindowStore, computer: &str) -> Option<WindowFile> {
    store
        .computers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(computer))
        .map(|(_, frame)| *frame)
}

/// Inserts `frame` under `computer`. A name that differs only by case is the same computer.
fn remember(store: &mut WindowStore, computer: &str, frame: WindowFile) {
    let keys: Vec<String> = store
        .computers
        .keys()
        .filter(|name| name.eq_ignore_ascii_case(computer))
        .cloned()
        .collect();
    let key = keys
        .first()
        .cloned()
        .unwrap_or_else(|| computer.to_string());
    for extra in keys.iter().skip(1) {
        store.computers.remove(extra);
    }
    store.computers.insert(key, frame);
}

fn real_frame(frame: WindowFile) -> Option<WindowFile> {
    if frame.width == 0 || frame.height == 0 {
        None
    } else {
        Some(frame)
    }
}

impl WindowStore {
    fn legacy_frame(&self) -> WindowFile {
        WindowFile {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            maximized: self.maximized,
        }
    }
}

fn set_suppress(app: &AppHandle, suppress: bool) {
    if let Some(state) = app.try_state::<PlacementState>() {
        *state.suppress.lock().expect("placement lock") = suppress;
    }
}

fn work_areas(app: &AppHandle) -> Vec<WorkArea> {
    app.available_monitors()
        .unwrap_or_default()
        .into_iter()
        .map(|monitor| {
            let area = monitor.work_area();
            WorkArea {
                x: area.position.x,
                y: area.position.y,
                width: area.size.width,
                height: area.size.height,
            }
        })
        .collect()
}

/// A normal frame on the monitor the window is already maximized on.
fn fallback_frame(window: &Window) -> WindowFile {
    let Some(monitor) = window.current_monitor().ok().flatten() else {
        return WindowFile {
            x: 0,
            y: 0,
            width: DEFAULT_WIDTH as u32,
            height: DEFAULT_HEIGHT as u32,
            maximized: true,
        };
    };
    let area = monitor.work_area();
    let scale = monitor.scale_factor();
    let width = (DEFAULT_WIDTH * scale).round() as u32;
    let height = (DEFAULT_HEIGHT * scale).round() as u32;
    let width = width.clamp(1, area.size.width.max(1));
    let height = height.clamp(1, area.size.height.max(1));
    WindowFile {
        x: area.position.x + (area.size.width.saturating_sub(width) as i32) / 2,
        y: area.position.y + (area.size.height.saturating_sub(height) as i32) / 2,
        width,
        height,
        maximized: true,
    }
}

/// Keeps `saved` when enough of its top edge still meets a connected monitor.
/// Otherwise the frame is centered on the primary work area, shrunk to fit,
/// and never taken below 640×420 unless that area itself is smaller.
fn fit(saved: Frame, monitors: &[WorkArea]) -> Frame {
    let saved = clamp_size(saved);
    if monitors.is_empty() || on_screen(&saved, monitors) {
        return saved;
    }
    center_on(saved, monitors[primary_index(monitors)])
}

fn clamp_size(frame: Frame) -> Frame {
    Frame {
        x: frame.x,
        y: frame.y,
        width: frame.width.clamp(MIN_WIDTH, MAX_EDGE),
        height: frame.height.clamp(MIN_HEIGHT, MAX_EDGE),
    }
}

fn on_screen(frame: &Frame, monitors: &[WorkArea]) -> bool {
    let band = WorkArea {
        x: frame.x,
        y: frame.y,
        width: frame.width,
        height: GRAB_HEIGHT.min(frame.height).max(1),
    };
    monitors.iter().any(|monitor| {
        let (width, height) = overlap(&band, monitor);
        width >= GRAB_WIDTH && height > 0
    })
}

fn primary_index(monitors: &[WorkArea]) -> usize {
    monitors
        .iter()
        .enumerate()
        .min_by_key(|(_, area)| area.x.unsigned_abs() as u64 + area.y.unsigned_abs() as u64)
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn center_on(frame: Frame, area: WorkArea) -> Frame {
    let width = fit_edge(frame.width, area.width, MIN_WIDTH);
    let height = fit_edge(frame.height, area.height, MIN_HEIGHT);
    Frame {
        x: area.x + (area.width.saturating_sub(width) / 2) as i32,
        y: area.y + (area.height.saturating_sub(height) / 2) as i32,
        width,
        height,
    }
}

fn fit_edge(wanted: u32, available: u32, min: u32) -> u32 {
    let available = available.max(1);
    let floor = min.min(available);
    wanted.clamp(floor, available)
}

fn overlap(a: &WorkArea, b: &WorkArea) -> (i64, i64) {
    let ax2 = i64::from(a.x) + i64::from(a.width);
    let ay2 = i64::from(a.y) + i64::from(a.height);
    let bx2 = i64::from(b.x) + i64::from(b.width);
    let by2 = i64::from(b.y) + i64::from(b.height);
    let width = (ax2.min(bx2) - i64::from(a.x).max(i64::from(b.x))).max(0);
    let height = (ay2.min(by2) - i64::from(a.y).max(i64::from(b.y))).max(0);
    (width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(x: i32, y: i32, width: u32, height: u32) -> WorkArea {
        WorkArea {
            x,
            y,
            width,
            height,
        }
    }

    fn frame(x: i32, y: i32, width: u32, height: u32) -> Frame {
        Frame {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn keeps_a_frame_on_the_monitor_it_was_on() {
        let monitors = [area(0, 0, 1920, 1080), area(1920, 0, 1920, 1080)];
        let saved = frame(2100, 80, 1280, 800);
        assert_eq!(fit(saved, &monitors), saved);
    }

    #[test]
    fn keeps_a_frame_that_only_just_meets_the_top_edge() {
        let monitors = [area(0, 0, 1920, 1080)];
        let saved = frame(1920 - GRAB_WIDTH as i32, 40, 900, 700);
        assert_eq!(fit(saved, &monitors), saved);
    }

    #[test]
    fn recenters_when_the_monitor_is_gone() {
        let monitors = [area(0, 0, 1920, 1080)];
        assert_eq!(
            fit(frame(4000, 100, 1280, 840), &monitors),
            frame(320, 120, 1280, 840)
        );
    }

    #[test]
    fn a_missing_monitor_uses_the_primary_work_area() {
        let monitors = [area(1920, 0, 1920, 1080), area(0, 0, 1920, 1080)];
        assert_eq!(
            fit(frame(-3000, 40, 1000, 700), &monitors),
            frame(460, 190, 1000, 700)
        );
    }

    #[test]
    fn shrinks_to_the_work_area_when_the_frame_does_not_fit() {
        let monitors = [area(0, 0, 1280, 800)];
        assert_eq!(
            fit(frame(5000, 5000, 2000, 1500), &monitors),
            frame(0, 0, 1280, 800)
        );
    }

    #[test]
    fn uses_the_work_area_when_it_is_smaller_than_the_minimum() {
        let monitors = [area(0, 0, 400, 300)];
        assert_eq!(
            fit(frame(5000, 5000, 1280, 840), &monitors),
            frame(0, 0, 400, 300)
        );
    }

    #[test]
    fn raises_a_tiny_frame_to_the_minimum() {
        let monitors = [area(0, 0, 1920, 1080)];
        assert_eq!(
            fit(frame(10, 20, 100, 100), &monitors),
            frame(10, 20, MIN_WIDTH, MIN_HEIGHT)
        );
    }

    #[test]
    fn ignores_a_sliver_of_overlap() {
        let monitors = [area(0, 0, 1920, 1080)];
        let saved = frame(1920 - (GRAB_WIDTH as i32 - 1), 100, 800, 600);
        assert_eq!(fit(saved, &monitors), frame(560, 240, 800, 600));
    }

    #[test]
    fn keeps_the_position_when_no_monitor_is_connected() {
        assert_eq!(
            fit(frame(100, 200, 100, 100), &[]),
            frame(100, 200, MIN_WIDTH, MIN_HEIGHT)
        );
    }

    #[test]
    fn caps_a_corrupt_size_that_is_still_on_a_monitor() {
        let monitors = [area(0, 0, 1920, 1080)];
        assert_eq!(
            fit(frame(40, 40, 1_000_000, 1_000_000), &monitors),
            frame(40, 40, MAX_EDGE, MAX_EDGE)
        );
    }

    #[test]
    fn loads_an_older_file_and_ignores_unknown_fields() {
        let file: WindowFile =
            serde_json::from_str(r#"{"x":10,"y":20,"width":800,"height":600,"extra":true}"#)
                .unwrap();
        assert_eq!(
            file,
            WindowFile {
                x: 10,
                y: 20,
                width: 800,
                height: 600,
                maximized: false,
            }
        );
        assert_eq!(
            serde_json::from_str::<WindowFile>("{}").unwrap(),
            WindowFile::default()
        );
    }

    fn placed(x: i32, y: i32, width: u32, height: u32, maximized: bool) -> WindowFile {
        WindowFile {
            x,
            y,
            width,
            height,
            maximized,
        }
    }

    #[test]
    fn an_older_file_is_used_until_a_computer_has_its_own_frame() {
        let store: WindowStore = serde_json::from_str(
            r#"{"x":10,"y":20,"width":800,"height":600,"maximized":true,"extra":true}"#,
        )
        .unwrap();
        let frame = placed(10, 20, 800, 600, true);
        assert_eq!(frame_for(&store, "ALPHA"), Some(frame));
        assert_eq!(frame_for(&store, "BETA"), Some(frame));
        assert_eq!(
            serde_json::from_str::<WindowStore>("{}").unwrap(),
            WindowStore::default()
        );
        assert_eq!(frame_for(&WindowStore::default(), "ALPHA"), None);
    }

    #[test]
    fn each_computer_keeps_its_own_frame() {
        let text = r#"{"extra":true,"computers":{"ALPHA":{"x":10,"y":20,"width":800,"height":600,"maximized":false,"nope":1},"BETA":{"x":1920,"y":40,"width":1400,"height":900,"maximized":true}}}"#;
        let store: WindowStore = serde_json::from_str(text).unwrap();
        assert_eq!(
            frame_for(&store, "alpha"),
            Some(placed(10, 20, 800, 600, false))
        );
        assert_eq!(
            frame_for(&store, "BETA"),
            Some(placed(1920, 40, 1400, 900, true))
        );
        assert_eq!(frame_for(&store, "GAMMA"), None);
    }

    #[test]
    fn a_named_file_ignores_the_old_shared_frame() {
        let store: WindowStore = serde_json::from_str(
            r#"{"x":1,"y":2,"width":900,"height":700,"computers":{"ALPHA":{"x":10,"y":20,"width":800,"height":600,"maximized":false}}}"#,
        )
        .unwrap();
        assert_eq!(
            frame_for(&store, "ALPHA"),
            Some(placed(10, 20, 800, 600, false))
        );
        assert_eq!(frame_for(&store, "BETA"), None);
    }

    #[test]
    fn saving_one_computer_leaves_the_other_and_drops_the_old_frame() {
        let mut store: WindowStore = serde_json::from_str(
            r#"{"x":1,"y":2,"width":900,"height":700,"computers":{"BETA":{"x":1920,"y":40,"width":1400,"height":900,"maximized":true}}}"#,
        )
        .unwrap();
        let alpha = placed(30, 40, 1000, 800, false);
        remember(&mut store, "alpha", alpha);
        let written = serde_json::to_value(&store).unwrap();
        assert!(written.get("x").is_none());
        assert!(written.get("width").is_none());
        let restored: WindowStore = serde_json::from_value(written).unwrap();
        assert_eq!(frame_for(&restored, "ALPHA"), Some(alpha));
        assert_eq!(
            frame_for(&restored, "BETA"),
            Some(placed(1920, 40, 1400, 900, true))
        );
        assert_eq!(restored.computers.len(), 2);
    }

    #[test]
    fn a_current_file_round_trips_without_changing_frames() {
        let text = r#"{"computers":{"ALPHA":{"x":10,"y":20,"width":800,"height":600,"maximized":false},"BETA":{"x":1920,"y":40,"width":1400,"height":900,"maximized":true}}}"#;
        let store: WindowStore = serde_json::from_str(text).unwrap();
        let again: WindowStore =
            serde_json::from_str(&serde_json::to_string(&store).unwrap()).unwrap();
        assert_eq!(store, again);
        assert_eq!(frame_for(&again, "ALPHA"), frame_for(&store, "ALPHA"));
        assert_eq!(frame_for(&again, "BETA"), frame_for(&store, "BETA"));
    }

    #[test]
    fn remembering_a_computer_overwrites_only_that_computer() {
        let mut store = WindowStore::default();
        remember(&mut store, "Alpha", placed(1, 2, 800, 600, false));
        remember(&mut store, "BETA", placed(3, 4, 900, 700, true));
        remember(&mut store, "ALPHA", placed(5, 6, 1000, 800, false));
        assert_eq!(store.computers.len(), 2);
        assert_eq!(
            frame_for(&store, "alpha"),
            Some(placed(5, 6, 1000, 800, false))
        );
        assert_eq!(
            frame_for(&store, "beta"),
            Some(placed(3, 4, 900, 700, true))
        );
        assert_eq!(
            store.computers.keys().next().map(String::as_str),
            Some("Alpha")
        );
    }

    #[test]
    fn a_zero_size_frame_is_ignored() {
        let mut store = WindowStore::default();
        remember(&mut store, "ALPHA", WindowFile::default());
        assert_eq!(frame_for(&store, "ALPHA"), None);
        assert_eq!(frame_for(&store, "BETA"), None);
    }
}
