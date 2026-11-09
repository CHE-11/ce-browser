//! Local screen recording: a tab, a window, or a monitor, with microphone, system audio, and a
//! camera file kept beside the MP4 so the bubble can move after the take.

mod audio;
mod capture;
mod draw;
mod encode;
mod library;
mod overlay;
mod targets;

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

pub use draw::{Stroke, StrokePoint};
pub use library::{RecordingEntry, RecordingUpdate};
pub use targets::{RecordingTarget, Target};

const FPS: u32 = 30;
const FRAME_HNS: i64 = 10_000_000 / FPS as i64;

const IDLE: u8 = 0;
const COUNTDOWN: u8 = 1;
const RECORDING: u8 = 2;
const PAUSED: u8 = 3;
const FAILED: u8 = 4;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CameraLayout {
    pub x: f32,
    pub y: f32,
    pub size: f32,
}

impl Default for CameraLayout {
    fn default() -> Self {
        Self { x: 0.04, y: 0.62, size: 0.18 }
    }
}

impl CameraLayout {
    pub fn clamped(self) -> Self {
        Self {
            x: self.x.clamp(-0.25, 1.0),
            y: self.y.clamp(-0.25, 1.0),
            size: if self.size == 0.0 { 0.18 } else { self.size.clamp(0.08, 0.5) },
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRequest {
    pub source: String,
    pub target: String,
    pub window: String,
    pub tab_id: u32,
    pub title: String,
    pub microphone: bool,
    pub system_audio: bool,
    pub camera: bool,
    pub countdown: bool,
    #[serde(default)]
    pub microphone_device_id: String,
    /// `720`, `1080`, `1440`, or `native`.
    #[serde(default = "default_resolution")]
    pub resolution: String,
    /// `bottom-left`, `bottom-right`, `top-left`, or `top-right`.
    #[serde(default = "default_corner")]
    pub camera_corner: String,
    /// `system`, `hidden`, or `circle`.
    #[serde(default = "default_cursor")]
    pub cursor: String,
    /// `off`, `1.5`, or `2`.
    #[serde(default = "default_focus")]
    pub focus: String,
}

fn default_corner() -> String {
    "bottom-left".into()
}

fn default_cursor() -> String {
    "system".into()
}

fn default_focus() -> String {
    "off".into()
}

fn default_resolution() -> String {
    "1440".into()
}

const CURSOR_SYSTEM: u8 = 0;
const CURSOR_HIDDEN: u8 = 1;
const CURSOR_CIRCLE: u8 = 2;

fn cursor_code(mode: &str) -> u8 {
    match mode {
        "hidden" => CURSOR_HIDDEN,
        "circle" => CURSOR_CIRCLE,
        _ => CURSOR_SYSTEM,
    }
}

fn focus_code(mode: &str) -> u8 {
    match mode {
        "1.5" => 1,
        "2" => 2,
        _ => 0,
    }
}

fn focus_zoom(mode: u8) -> f32 {
    match mode {
        1 => 1.5,
        2 => 2.0,
        _ => 1.0,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenRecordingStatus {
    pub phase: String,
    pub elapsed_ms: u64,
    pub countdown: u32,
    pub id: String,
    pub error: Option<String>,
    pub path: Option<String>,
    pub camera: bool,
    pub microphone: bool,
    pub mic_muted: bool,
    /// Microphone loudness, from 0 to 1.
    pub mic_level: f32,
    pub drawing: bool,
    pub strokes: u32,
    pub tool: String,
    pub color: String,
    pub owner_window: String,
}

struct Control {
    stop: AtomicBool,
    paused: AtomicBool,
    mic_muted: AtomicBool,
    mic_level: AtomicU32,
    drawing: AtomicBool,
    camera_on: AtomicBool,
    cursor: AtomicU8,
    focus: AtomicU8,
    elapsed_ms: AtomicU64,
    countdown: AtomicU64,
    phase: AtomicU8,
    strokes: Mutex<Vec<Stroke>>,
    tool: Mutex<String>,
    color: Mutex<String>,
    camera: Mutex<CameraLayout>,
    error: Mutex<Option<String>>,
    path: Mutex<Option<String>>,
    target: Target,
    id: String,
    dir: PathBuf,
    owner_window: String,
    title: String,
    microphone: bool,
    system_audio: bool,
    source: String,
    max_long: u32,
}

struct Live {
    control: Arc<Control>,
}

static SESSION: Mutex<Option<Live>> = Mutex::new(None);
static LAST: Mutex<Option<Result<library::RecordingEntry, String>>> = Mutex::new(None);

/// `720`, `1080`, `1440`, or `native`. Anything else is 1440p.
pub fn long_side_limit(resolution: &str) -> u32 {
    match resolution {
        "720" => 1280,
        "1080" => 1920,
        "native" => 3840,
        _ => 2560,
    }
}

/// Long side kept at the capture size, up to `max_long`. H.264 wants even dimensions.
pub fn output_size(width: u32, height: u32, max_long: u32) -> (u32, u32) {
    let max_long = max_long.clamp(640, 3840);
    let long = width.max(height).max(1);
    let (mut w, mut h) = if long <= max_long {
        (width.max(2), height.max(2))
    } else {
        let scale = f64::from(max_long) / f64::from(long);
        ((f64::from(width) * scale).round() as u32, (f64::from(height) * scale).round() as u32)
    };
    w = (w.max(2) / 2) * 2;
    h = (h.max(2) / 2) * 2;
    (w, h)
}

pub fn list_targets(skip_hwnd: isize) -> Vec<RecordingTarget> {
    targets::list(skip_hwnd)
}

pub fn start(app: &AppHandle, request: StartRequest) -> Result<ScreenRecordingStatus, String> {
    if SESSION.lock().expect("recording").is_some() {
        return Err("A recording is already running.".into());
    }
    let target = targets::resolve(app, &request)?;
    let id = library::new_id();
    let dir = library::prepare(app, &id)?;
    let control = Arc::new(Control {
        stop: AtomicBool::new(false),
        paused: AtomicBool::new(false),
        mic_muted: AtomicBool::new(false),
        mic_level: AtomicU32::new(0),
        drawing: AtomicBool::new(false),
        camera_on: AtomicBool::new(request.camera),
        cursor: AtomicU8::new(cursor_code(&request.cursor)),
        focus: AtomicU8::new(focus_code(&request.focus)),
        elapsed_ms: AtomicU64::new(0),
        countdown: AtomicU64::new(if request.countdown { 3 } else { 0 }),
        phase: AtomicU8::new(if request.countdown { COUNTDOWN } else { RECORDING }),
        strokes: Mutex::new(Vec::new()),
        tool: Mutex::new("pen".into()),
        color: Mutex::new("#ef4444".into()),
        camera: Mutex::new(CameraLayout::default()),
        error: Mutex::new(None),
        path: Mutex::new(None),
        target: target.clone(),
        id: id.clone(),
        dir,
        owner_window: request.window.clone(),
        title: if request.title.trim().is_empty() { target.label.clone() } else { request.title.trim().to_string() },
        microphone: request.microphone,
        system_audio: request.system_audio,
        source: request.source.clone(),
        max_long: long_side_limit(&request.resolution),
    });
    if let Err(error) = overlay::open(app, &target, request.camera) {
        let _ = std::fs::remove_dir_all(&control.dir);
        return Err(error);
    }
    if request.camera {
        let _ = overlay::move_camera(app, &target, &request.camera_corner);
    }
    *SESSION.lock().expect("recording") = Some(Live { control: control.clone() });
    let worker = app.clone();
    let device = request.microphone_device_id.clone();
    let countdown = request.countdown;
    if let Err(error) = thread::Builder::new()
        .name("recording".into())
        .spawn(move || run(worker, control, countdown, device))
    {
        let dir = SESSION.lock().expect("recording").as_ref().map(|live| live.control.dir.clone());
        *SESSION.lock().expect("recording") = None;
        overlay::close(app);
        if let Some(dir) = dir {
            let _ = std::fs::remove_dir_all(dir);
        }
        return Err(error.to_string());
    }
    snapshot_now()
}

pub fn pause() -> Result<ScreenRecordingStatus, String> {
    with_control(|control| {
        control.paused.store(true, Ordering::Relaxed);
        control.phase.store(PAUSED, Ordering::Relaxed);
    })?;
    Ok(snapshot_now()?)
}

pub fn resume() -> Result<ScreenRecordingStatus, String> {
    with_control(|control| {
        control.paused.store(false, Ordering::Relaxed);
        control.phase.store(RECORDING, Ordering::Relaxed);
    })?;
    Ok(snapshot_now()?)
}

pub fn stop() -> Result<library::RecordingEntry, String> {
    {
        let session = SESSION.lock().expect("recording");
        let Some(live) = session.as_ref() else {
            return Err("No recording is running.".into());
        };
        live.control.stop.store(true, Ordering::Relaxed);
    }
    let started = Instant::now();
    while SESSION.lock().expect("recording").is_some() {
        if started.elapsed() > Duration::from_secs(20) {
            return Err("The recording did not stop.".into());
        }
        thread::sleep(Duration::from_millis(40));
    }
    match LAST.lock().expect("recording").clone() {
        Some(result) => result,
        None => Err("The recording did not stop.".into()),
    }
}

pub fn set_mic_muted(muted: bool) -> Result<ScreenRecordingStatus, String> {
    with_control(|control| control.mic_muted.store(muted, Ordering::Relaxed))?;
    snapshot_now()
}

pub fn set_controls_mode(app: &AppHandle, mode: &str) -> Result<(), String> {
    overlay::set_controls_mode(app, mode)
}

pub fn set_camera_corner(app: &AppHandle, corner: &str) -> Result<(), String> {
    let target = with_control(|control| control.target.clone())?;
    let corner = match corner {
        "bottom-right" | "top-left" | "top-right" => corner,
        _ => "bottom-left",
    };
    overlay::move_camera(app, &target, corner)
}

pub fn set_cursor(mode: &str) -> Result<(), String> {
    with_control(|control| control.cursor.store(cursor_code(mode), Ordering::Relaxed))?;
    Ok(())
}

pub fn set_focus(mode: &str) -> Result<(), String> {
    with_control(|control| control.focus.store(focus_code(mode), Ordering::Relaxed))?;
    Ok(())
}

pub fn set_camera(app: &AppHandle, on: bool) -> Result<ScreenRecordingStatus, String> {
    let target = with_control(|control| {
        control.camera_on.store(on, Ordering::Relaxed);
        control.target.clone()
    })?;
    if on {
        overlay::show_camera(app, &target)?;
    } else {
        overlay::hide_camera(app);
    }
    let status = snapshot_now()?;
    let _ = app.emit("screen-recording", &status);
    Ok(status)
}

pub fn set_drawing(app: &AppHandle, on: bool) -> Result<ScreenRecordingStatus, String> {
    let target = with_control(|control| {
        control.drawing.store(on, Ordering::Relaxed);
        control.target.clone()
    })?;
    if on {
        overlay::show_draw(app, &target);
    } else {
        overlay::hide_draw(app);
    }
    let status = snapshot_now()?;
    let _ = app.emit("screen-recording", &status);
    Ok(status)
}

pub fn set_style(app: &AppHandle, tool: String, color: String) -> Result<ScreenRecordingStatus, String> {
    with_control(|control| {
        let tool = match tool.as_str() {
            "arrow" | "rectangle" | "highlighter" | "eraser" | "pen" => tool,
            _ => "pen".into(),
        };
        *control.tool.lock().expect("tool") = tool;
        if color.len() == 7 && color.starts_with('#') {
            *control.color.lock().expect("color") = color;
        }
    })?;
    let status = snapshot_now()?;
    let _ = app.emit("screen-recording", &status);
    Ok(status)
}

pub fn erase(points: Vec<draw::StrokePoint>) -> Result<(), String> {
    if points.is_empty() {
        return Ok(());
    }
    with_control(|control| {
        let mut strokes = control.strokes.lock().expect("strokes");
        strokes.retain(|stroke| !stroke.draft && !points.iter().any(|point| draw::touches(stroke, *point, 0.02)));
    })?;
    Ok(())
}

pub fn add_stroke(stroke: Stroke) -> Result<(), String> {
    let Some(stroke) = draw::sanitize(stroke) else {
        return Ok(());
    };
    with_control(|control| {
        let mut strokes = control.strokes.lock().expect("strokes");
        if strokes.last().is_some_and(|item| item.draft) {
            strokes.pop();
        }
        if strokes.len() >= 200 {
            strokes.remove(0);
        }
        strokes.push(stroke);
    })?;
    Ok(())
}

pub fn set_draft(stroke: Option<Stroke>) -> Result<(), String> {
    with_control(|control| {
        let mut strokes = control.strokes.lock().expect("strokes");
        if strokes.last().is_some_and(|item| item.draft) {
            strokes.pop();
        }
        if let Some(mut stroke) = stroke.and_then(draw::sanitize) {
            stroke.draft = true;
            strokes.push(stroke);
        }
    })?;
    Ok(())
}

pub fn clear_strokes(app: &AppHandle) -> Result<(), String> {
    with_control(|control| {
        control.strokes.lock().expect("strokes").clear();
    })?;
    let status = snapshot_now()?;
    let _ = app.emit("screen-recording", &status);
    Ok(())
}

pub fn undo_stroke(app: &AppHandle) -> Result<(), String> {
    with_control(|control| {
        let mut strokes = control.strokes.lock().expect("strokes");
        let draft = strokes.last().is_some_and(|item| item.draft).then(|| strokes.pop()).flatten();
        strokes.pop();
        if let Some(draft) = draft {
            strokes.push(draft);
        }
    })?;
    let status = snapshot_now()?;
    let _ = app.emit("screen-recording", &status);
    Ok(())
}

pub fn append_camera(bytes: &[u8]) -> Result<(), String> {
    let dir = with_control(|control| control.dir.clone())?;
    let path = library::camera_file(&dir);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("Couldn't save the camera. {error}"))?;
    file.write_all(bytes).map_err(|error| format!("Couldn't save the camera. {error}"))?;
    Ok(())
}

pub fn status() -> ScreenRecordingStatus {
    SESSION
        .lock()
        .ok()
        .and_then(|session| session.as_ref().map(|live| snapshot(&live.control)))
        .unwrap_or_else(idle_status)
}

pub fn library_list(app: &AppHandle) -> Vec<RecordingEntry> {
    library::list(app)
}

pub fn library_update(app: &AppHandle, patch: RecordingUpdate) -> Result<RecordingEntry, String> {
    library::update(app, patch)
}

pub fn library_delete(app: &AppHandle, id: &str) -> Result<(), String> {
    library::delete(app, id)
}

pub fn library_export(app: &AppHandle, id: &str, path: &std::path::Path) -> Result<String, String> {
    library::export(app, id, path)
}

pub fn media_url(app: &AppHandle, id: &str, kind: &str) -> Result<String, String> {
    library::media_url(app, id, kind)
}

pub fn begin_export(path: &std::path::Path) -> Result<(), String> {
    library::begin_export(path)
}

pub fn append_export(bytes: &[u8]) -> Result<(), String> {
    library::append_export(bytes)
}

pub fn finish_export() -> Result<(), String> {
    library::finish_export()
}

fn with_control<T>(change: impl FnOnce(&Control) -> T) -> Result<T, String> {
    let session = SESSION.lock().expect("recording");
    let live = session.as_ref().ok_or("No recording is running.")?;
    Ok(change(&live.control))
}

fn snapshot_now() -> Result<ScreenRecordingStatus, String> {
    let session = SESSION.lock().expect("recording");
    let live = session.as_ref().ok_or("No recording is running.")?;
    Ok(snapshot(&live.control))
}

fn snapshot(control: &Control) -> ScreenRecordingStatus {
    ScreenRecordingStatus {
        phase: phase_name(control.phase.load(Ordering::Relaxed)),
        elapsed_ms: control.elapsed_ms.load(Ordering::Relaxed),
        countdown: control.countdown.load(Ordering::Relaxed) as u32,
        id: control.id.clone(),
        error: control.error.lock().expect("error").clone(),
        path: control.path.lock().expect("path").clone(),
        camera: control.camera_on.load(Ordering::Relaxed),
        microphone: control.microphone,
        mic_muted: control.mic_muted.load(Ordering::Relaxed),
        mic_level: control.mic_level.load(Ordering::Relaxed) as f32 / 1000.0,
        drawing: control.drawing.load(Ordering::Relaxed),
        strokes: control.strokes.lock().expect("strokes").iter().filter(|stroke| !stroke.draft).count() as u32,
        tool: control.tool.lock().expect("tool").clone(),
        color: control.color.lock().expect("color").clone(),
        owner_window: control.owner_window.clone(),
    }
}

fn idle_status() -> ScreenRecordingStatus {
    ScreenRecordingStatus {
        phase: "idle".into(),
        elapsed_ms: 0,
        countdown: 0,
        id: String::new(),
        error: None,
        path: None,
        camera: false,
        microphone: false,
        mic_muted: false,
        mic_level: 0.0,
        drawing: false,
        strokes: 0,
        tool: "pen".into(),
        color: "#ef4444".into(),
        owner_window: String::new(),
    }
}

fn phase_name(phase: u8) -> String {
    match phase {
        COUNTDOWN => "countdown",
        RECORDING => "recording",
        PAUSED => "paused",
        FAILED => "error",
        _ => "idle",
    }
    .into()
}

fn publish(app: &AppHandle, control: &Control) {
    let _ = app.emit("screen-recording", snapshot(control));
}

fn run(app: AppHandle, control: Arc<Control>, countdown: bool, device_id: String) {
    let audio = audio::Mix::start(control.microphone, control.system_audio, &device_id);
    if countdown && !wait_countdown(&app, &control, &audio) {
        cancel(&app, &control);
        return;
    }
    control.phase.store(RECORDING, Ordering::Relaxed);
    control.countdown.store(0, Ordering::Relaxed);
    publish(&app, &control);
    let mut grabber = match capture::Grabber::open(&control.target, control.max_long) {
        Ok(grabber) => grabber,
        Err(error) => {
            fail(&app, &control, error);
            return;
        }
    };
    grabber.set_cursor_visible(control.cursor.load(Ordering::Relaxed) == CURSOR_SYSTEM);
    let mut system_cursor = control.cursor.load(Ordering::Relaxed) == CURSOR_SYSTEM;
    let mut aim: Option<(f32, f32)> = None;
    let mut primed = None;
    let ready_at = Instant::now();
    while primed.is_none() && ready_at.elapsed() < Duration::from_secs(3) {
        if control.stop.load(Ordering::Relaxed) {
            cancel(&app, &control);
            return;
        }
        match grabber.frame() {
            Ok(Some(frame)) => primed = Some(frame.to_vec()),
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(error) => {
                fail(&app, &control, error);
                return;
            }
        }
    }
    let Some(first) = primed else {
        fail(&app, &control, "The recording has no frames.".into());
        return;
    };
    let Some((width, height)) = grabber.size() else {
        fail(&app, &control, "The recording has no frames.".into());
        return;
    };
    let video_path = library::video_file(&control.dir);
    let writer = match encode::Writer::open(&video_path, width, height) {
        Ok(writer) => writer,
        Err(error) => {
            fail(&app, &control, error);
            return;
        }
    };
    let (width, height) = writer.size();
    audio.discard();
    let opening = present(&first, width, height, &control, &mut aim);
    if let Err(error) = writer.video(&opening, 0, FRAME_HNS) {
        fail(&app, &control, error);
        return;
    }
    // Each later frame is stamped with the recording clock. A slow encode used to pack
    // whatever frames it managed into a 30 fps timeline, so a 13s take played back in about 3s.
    let mut last_pts: i64 = 0;
    let mut misses = 0u32;
    let mut last_emit = Instant::now();
    let mut last_follow = Instant::now();
    let clock_started = Instant::now();
    let mut paused_for = Duration::ZERO;
    let mut pause_began: Option<Instant> = None;
    let note_time = |paused_for: Duration, pause_began: Option<Instant>| {
        let mut held = paused_for;
        if let Some(began) = pause_began {
            held += began.elapsed();
        }
        let elapsed = clock_started.elapsed().saturating_sub(held).as_millis() as u64;
        control.elapsed_ms.store(elapsed, Ordering::Relaxed);
        elapsed
    };
    note_time(paused_for, pause_began);
    loop {
        if control.stop.load(Ordering::Relaxed) || !targets::alive(&control.target) {
            break;
        }
        let drawing = control.drawing.load(Ordering::Relaxed);
        let show_system = control.cursor.load(Ordering::Relaxed) == CURSOR_SYSTEM;
        if show_system != system_cursor {
            grabber.set_cursor_visible(show_system);
            system_cursor = show_system;
        }
        if last_follow.elapsed() > Duration::from_millis(400) {
            if drawing {
                overlay::follow_draw(&app, &control.target);
            }
            if let Some(layout) = overlay::camera_layout(&app, &control.target) {
                *control.camera.lock().expect("camera") = layout;
            }
            last_follow = Instant::now();
        }
        if control.paused.load(Ordering::Relaxed) {
            if pause_began.is_none() {
                pause_began = Some(Instant::now());
            }
            let _ = grabber.frame();
            audio.discard();
            control.mic_level.store(audio.mic_level(), Ordering::Relaxed);
            control.phase.store(PAUSED, Ordering::Relaxed);
            if last_emit.elapsed() > Duration::from_millis(200) {
                publish(&app, &control);
                last_emit = Instant::now();
            }
            note_time(paused_for, pause_began);
            thread::sleep(Duration::from_millis(40));
            continue;
        }
        if let Some(began) = pause_began.take() {
            paused_for += began.elapsed();
        }
        let elapsed_ms = note_time(paused_for, None);
        let elapsed_hns = (elapsed_ms as i64).saturating_mul(10_000);
        control.phase.store(RECORDING, Ordering::Relaxed);
        if elapsed_hns < last_pts.saturating_add(FRAME_HNS) {
            let wait = (last_pts + FRAME_HNS - elapsed_hns).max(0) as u64;
            if last_emit.elapsed() > Duration::from_millis(200) {
                publish(&app, &control);
                last_emit = Instant::now();
            }
            thread::sleep(Duration::from_nanos(wait.saturating_mul(100).min(33_000_000)));
            continue;
        }
        let frame = match grabber.frame() {
            Ok(Some(frame)) => {
                misses = 0;
                frame.to_vec()
            }
            Ok(None) => {
                misses += 1;
                if misses > 90 {
                    break;
                }
                Vec::new()
            }
            Err(_) => {
                misses += 1;
                if misses > 90 {
                    break;
                }
                Vec::new()
            }
        };
        if frame.is_empty() {
            thread::sleep(Duration::from_millis(10));
            continue;
        }
        let frame = present(&frame, width, height, &control, &mut aim);
        let gap = (elapsed_hns - last_pts).max(FRAME_HNS);
        if writer.video(&frame, elapsed_hns, gap).is_err() {
            break;
        }
        let samples = ((gap.saturating_mul(48_000) + 5_000_000) / 10_000_000).max(1) as usize;
        let pcm = audio.pull(samples, !control.mic_muted.load(Ordering::Relaxed));
        let _ = writer.audio(&pcm, last_pts);
        last_pts = elapsed_hns;
        control.mic_level.store(audio.mic_level(), Ordering::Relaxed);
        if last_emit.elapsed() > Duration::from_millis(200) {
            publish(&app, &control);
            last_emit = Instant::now();
        }
    }
    audio.discard();
    let elapsed = note_time(paused_for, pause_began);
    let saved = writer.finish();
    if saved.is_err() || elapsed < 40 {
        fail(&app, &control, saved.err().unwrap_or_else(|| "The recording has no frames.".into()));
        return;
    }
    if let Some(layout) = overlay::camera_layout(&app, &control.target) {
        *control.camera.lock().expect("camera") = layout;
    }
    let camera_path = library::camera_file(&control.dir);
    // The camera window flushes its last chunk after it sees the idle phase, and that command
    // only works while this session is still the current one.
    control.phase.store(IDLE, Ordering::Relaxed);
    publish(&app, &control);
    thread::sleep(Duration::from_millis(450));
    let camera_path = camera_path.is_file().then(|| camera_path.to_string_lossy().into_owned()).unwrap_or_default();
    let entry = library::RecordingEntry {
        id: control.id.clone(),
        title: control.title.clone(),
        created_at: SystemTime::now().duration_since(UNIX_EPOCH).map(|time| time.as_millis() as u64).unwrap_or(0),
        duration_ms: elapsed,
        source: control.source.clone(),
        source_label: control.target.label.clone(),
        width,
        height,
        video_path: video_path.to_string_lossy().into_owned(),
        camera_path,
        camera: control.camera.lock().expect("camera").clamped(),
        trim_start_ms: 0,
        trim_end_ms: elapsed,
        has_mic: control.microphone,
        has_system_audio: control.system_audio,
    };
    *control.path.lock().expect("path") = Some(entry.video_path.clone());
    *control.error.lock().expect("error") = None;
    library::insert(&app, entry.clone());
    overlay::close(&app);
    *LAST.lock().expect("recording") = Some(Ok(entry));
    clear_session(&control);
    publish(&app, &control);
}

fn fit(frame: &[u8], width: u32, height: u32) -> Vec<u8> {
    let need = (width as usize) * (height as usize) * 4;
    if frame.len() == need {
        frame.to_vec()
    } else {
        let mut sized = vec![0u8; need];
        let copy = frame.len().min(need);
        sized[..copy].copy_from_slice(&frame[..copy]);
        sized
    }
}

/// Strokes, focus zoom, and the circle pointer, in that order. The picture stays the same size.
fn present(frame: &[u8], width: u32, height: u32, control: &Control, aim: &mut Option<(f32, f32)>) -> Vec<u8> {
    let mut frame = fit(frame, width, height);
    {
        let strokes = control.strokes.lock().expect("strokes");
        draw::paint(&mut frame, width, height, &strokes);
    }
    let cursor_mode = control.cursor.load(Ordering::Relaxed);
    let zoom = focus_zoom(control.focus.load(Ordering::Relaxed));
    let cursor = targets::cursor_in_frame(&control.target, width, height);
    if zoom > 1.0 {
        if let Some((x, y)) = cursor {
            let (sx, sy) = match *aim {
                Some((ax, ay)) => (ax + (x - ax) * 0.28, ay + (y - ay) * 0.28),
                None => (x, y),
            };
            *aim = Some((sx, sy));
            let (zoomed, mark) = capture::zoom_around(&frame, width, height, sx, sy, x, y, zoom);
            frame = zoomed;
            if cursor_mode == CURSOR_CIRCLE {
                if let Some((mx, my)) = mark {
                    draw::cursor_mark(&mut frame, width, height, mx, my);
                }
            }
            return frame;
        }
    }
    *aim = None;
    if cursor_mode == CURSOR_CIRCLE {
        if let Some((x, y)) = cursor {
            if (0.0..width as f32).contains(&x) && (0.0..height as f32).contains(&y) {
                draw::cursor_mark(&mut frame, width, height, x.round() as i32, y.round() as i32);
            }
        }
    }
    frame
}

fn wait_countdown(app: &AppHandle, control: &Arc<Control>, audio: &audio::Mix) -> bool {
    for second in (1..=3).rev() {
        control.countdown.store(second, Ordering::Relaxed);
        control.phase.store(COUNTDOWN, Ordering::Relaxed);
        for _ in 0..10 {
            if control.stop.load(Ordering::Relaxed) {
                return false;
            }
            control.mic_level.store(audio.mic_level(), Ordering::Relaxed);
            publish(app, control);
            thread::sleep(Duration::from_millis(100));
        }
    }
    true
}

fn cancel(app: &AppHandle, control: &Arc<Control>) {
    overlay::close(app);
    let _ = std::fs::remove_dir_all(&control.dir);
    control.phase.store(IDLE, Ordering::Relaxed);
    *control.error.lock().expect("error") = None;
    *LAST.lock().expect("recording") = Some(Err("cancelled".into()));
    let mut status = snapshot(control);
    status.phase = "idle".into();
    status.id.clear();
    status.path = None;
    clear_session(control);
    let _ = app.emit("screen-recording", status);
}

fn fail(app: &AppHandle, control: &Arc<Control>, error: String) {
    overlay::close(app);
    let _ = std::fs::remove_dir_all(&control.dir);
    *control.error.lock().expect("error") = Some(error.clone());
    control.phase.store(FAILED, Ordering::Relaxed);
    *LAST.lock().expect("recording") = Some(Err(error));
    clear_session(control);
    publish(app, control);
}

#[cfg(test)]
mod tests {
    use super::output_size;

    #[test]
    fn a_4k_frame_fits_on_a_1440p_long_side() {
        assert_eq!(output_size(3840, 2160, 2560), (2560, 1440));
        assert_eq!(output_size(2560, 1440, 2560), (2560, 1440));
        assert_eq!(output_size(2560, 1440, 1920), (1920, 1080));
        assert_eq!(output_size(1919, 1000, 2560).0 % 2, 0);
        assert_eq!(output_size(1919, 1000, 2560).1 % 2, 0);
    }

    #[test]
    fn focus_zoom_keeps_the_frame_size_and_a_mark_near_the_pointer() {
        let width = 40u32;
        let height = 20u32;
        let mut frame = vec![0u8; (width * height * 4) as usize];
        let index = ((10 * width + 20) * 4) as usize;
        frame[index] = 255;
        let (zoomed, mark) = super::capture::zoom_around(&frame, width, height, 20.0, 10.0, 20.0, 10.0, 2.0);
        assert_eq!(zoomed.len(), frame.len());
        let (mx, my) = mark.expect("pointer stays in the zoom");
        assert!((mx - 20).abs() <= 2);
        assert!((my - 10).abs() <= 2);
    }
}

fn clear_session(control: &Arc<Control>) {
    let mut session = SESSION.lock().expect("recording");
    if session.as_ref().is_some_and(|live| Arc::ptr_eq(&live.control, control)) {
        *session = None;
    }
}
