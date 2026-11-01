use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

use serde::Serialize;
use tauri::window::{Effect, EffectsBuilder};
use tauri::{AppHandle, Manager};

use crate::settings;

/// Mica needs Windows 11. Acrylic works from Windows 10 version 1809.
const MICA_BUILD: u32 = 22_000;
const ACRYLIC_BUILD: u32 = 17_763;

/// Browser windows created transparent. Only these can show a backdrop.
static TRANSPARENT: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

#[repr(C)]
struct OsVersionInfo {
    size: u32,
    major: u32,
    minor: u32,
    build: u32,
    platform: u32,
    service_pack: [u16; 128],
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn RtlGetVersion(info: *mut OsVersionInfo) -> i32;
}

/// The Windows build number, such as 26100. `GetVersionEx` reports an old version to apps
/// without a manifest, so this asks ntdll.
pub fn windows_build() -> u32 {
    static BUILD: LazyLock<u32> = LazyLock::new(|| {
        let mut info = OsVersionInfo {
            size: std::mem::size_of::<OsVersionInfo>() as u32,
            major: 0,
            minor: 0,
            build: 0,
            platform: 0,
            service_pack: [0; 128],
        };
        if unsafe { RtlGetVersion(&mut info) } == 0 { info.build } else { 0 }
    });
    *BUILD
}

pub fn supported(kind: &str) -> bool {
    match kind {
        "mica" => windows_build() >= MICA_BUILD,
        "acrylic" => windows_build() >= ACRYLIC_BUILD,
        _ => true,
    }
}

/// The backdrop settings ask for, when this Windows can draw it.
pub fn wanted(app: &AppHandle) -> Option<&'static str> {
    match settings::get(app).window_backdrop.as_str() {
        "mica" if supported("mica") => Some("mica"),
        "acrylic" if supported("acrylic") => Some("acrylic"),
        _ => None,
    }
}

pub fn note_transparent(label: &str) {
    TRANSPARENT.lock().expect("backdrop").insert(label.to_string());
}

pub fn forget(label: &str) {
    TRANSPARENT.lock().expect("backdrop").remove(label);
}

/// The window shows a backdrop now, so its own surfaces must stay see-through.
pub fn active(app: &AppHandle, label: &str) -> bool {
    wanted(app).is_some() && TRANSPARENT.lock().expect("backdrop").contains(label)
}

/// Turns the backdrop on or off for every browser window that can show one.
pub fn apply_all(app: &AppHandle) {
    let effect = match wanted(app) {
        Some("mica") => Some(Effect::Mica),
        Some("acrylic") => Some(Effect::Acrylic),
        _ => None,
    };
    let labels: Vec<String> = TRANSPARENT.lock().expect("backdrop").iter().cloned().collect();
    for label in labels {
        let Some(window) = app.get_window(&label) else {
            continue;
        };
        let result = match effect.clone() {
            Some(effect) => window.set_effects(EffectsBuilder::new().effect(effect).build()),
            None => window.set_effects(None),
        };
        if let Err(error) = result {
            eprintln!("window backdrop: {error}");
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackdropSupport {
    pub mica: bool,
    pub acrylic: bool,
    /// The setting asks for a backdrop the open window was not created for. A restart shows it.
    pub restart_needed: bool,
}

pub fn support(app: &AppHandle) -> BackdropSupport {
    let wanted = wanted(app).is_some();
    let ready = TRANSPARENT.lock().expect("backdrop").contains(crate::tabs::WINDOW);
    BackdropSupport {
        mica: supported("mica"),
        acrylic: supported("acrylic"),
        restart_needed: wanted && !ready,
    }
}
