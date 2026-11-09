//! `recordings.json` and the files next to it, plus a loopback server so a `<video>` can seek.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::files;

use super::encode;
use super::CameraLayout;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RecordingEntry {
    pub id: String,
    pub title: String,
    pub created_at: u64,
    pub duration_ms: u64,
    pub source: String,
    pub source_label: String,
    pub width: u32,
    pub height: u32,
    pub video_path: String,
    pub camera_path: String,
    pub camera: CameraLayout,
    pub trim_start_ms: u64,
    pub trim_end_ms: u64,
    pub has_mic: bool,
    pub has_system_audio: bool,
}

impl Default for RecordingEntry {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            created_at: 0,
            duration_ms: 0,
            source: String::new(),
            source_label: String::new(),
            width: 0,
            height: 0,
            video_path: String::new(),
            camera_path: String::new(),
            camera: CameraLayout::default(),
            trim_start_ms: 0,
            trim_end_ms: 0,
            has_mic: false,
            has_system_audio: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Catalog {
    items: Vec<RecordingEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingUpdate {
    pub id: String,
    pub title: Option<String>,
    pub trim_start_ms: Option<u64>,
    pub trim_end_ms: Option<u64>,
    pub camera: Option<CameraLayout>,
}

pub fn root(app: &AppHandle) -> PathBuf {
    crate::paths::data_dir(app).join("recordings")
}

pub fn new_id() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let millis = SystemTime::now().duration_since(UNIX_EPOCH).map(|time| time.as_millis()).unwrap_or(0);
    format!("{millis:x}{n:x}")
}

pub fn prepare(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    let dir = root(app).join(id);
    fs::create_dir_all(&dir).map_err(|error| format!("Couldn't start the recording. {error}"))?;
    Ok(dir)
}

pub fn video_file(dir: &Path) -> PathBuf {
    dir.join("video.mp4")
}

pub fn camera_file(dir: &Path) -> PathBuf {
    dir.join("camera.webm")
}

pub fn list(app: &AppHandle) -> Vec<RecordingEntry> {
    load(app).items.into_iter().filter(|item| Path::new(&item.video_path).is_file() || file_in(app, &item.id, "video.mp4").is_some_and(|path| path.is_file())).collect()
}

pub fn insert(app: &AppHandle, entry: RecordingEntry) {
    mutate(app, |catalog| {
        catalog.items.retain(|item| item.id != entry.id);
        catalog.items.insert(0, entry);
    });
}

pub fn update(app: &AppHandle, patch: RecordingUpdate) -> Result<RecordingEntry, String> {
    let mut updated = None;
    mutate(app, |catalog| {
        let Some(item) = catalog.items.iter_mut().find(|item| item.id == patch.id) else {
            return;
        };
        if let Some(title) = patch.title.as_ref() {
            let title = title.trim();
            if !title.is_empty() {
                item.title = title.chars().take(180).collect();
            }
        }
        if let Some(start) = patch.trim_start_ms {
            item.trim_start_ms = start.min(item.duration_ms);
        }
        if let Some(end) = patch.trim_end_ms {
            item.trim_end_ms = end.min(item.duration_ms).max(item.trim_start_ms);
        }
        if item.trim_end_ms > 0 && item.trim_end_ms <= item.trim_start_ms {
            item.trim_end_ms = item.duration_ms;
        }
        if let Some(camera) = patch.camera {
            item.camera = camera.clamped();
        }
        updated = Some(item.clone());
    });
    updated.ok_or_else(|| "That recording is no longer here.".into())
}

pub fn delete(app: &AppHandle, id: &str) -> Result<(), String> {
    if !safe_id(id) {
        return Err("That recording is no longer here.".into());
    }
    let mut folder = None;
    mutate(app, |catalog| {
        if let Some(index) = catalog.items.iter().position(|item| item.id == id) {
            let item = catalog.items.remove(index);
            folder = Some(PathBuf::from(item.video_path).parent().map(Path::to_path_buf).unwrap_or_else(|| root(app).join(id)));
        }
    });
    if let Some(folder) = folder {
        let root = root(app);
        if folder.starts_with(&root) {
            let _ = fs::remove_dir_all(folder);
        }
    }
    Ok(())
}

pub fn export(app: &AppHandle, id: &str, dest: &Path) -> Result<String, String> {
    if !dest.is_absolute() {
        return Err("Choose a full file path.".into());
    }
    let item = list(app).into_iter().find(|item| item.id == id).ok_or("That recording is no longer here.")?;
    let src = PathBuf::from(&item.video_path);
    if !src.is_file() {
        return Err("The video file is missing.".into());
    }
    let end = if item.trim_end_ms == 0 { item.duration_ms } else { item.trim_end_ms };
    encode::export_video(&src, dest, item.trim_start_ms, end, item.duration_ms)?;
    Ok(dest.to_string_lossy().into_owned())
}

pub fn media_url(app: &AppHandle, id: &str, kind: &str) -> Result<String, String> {
    let name = match kind {
        "camera" => "camera.webm",
        _ => "video.mp4",
    };
    if !safe_id(id) {
        return Err("That recording is no longer here.".into());
    }
    let port = ensure_server(app)?;
    Ok(format!("http://127.0.0.1:{port}/{id}/{name}"))
}

static PORT: Mutex<Option<u16>> = Mutex::new(None);
static EXPORT: Mutex<Option<File>> = Mutex::new(None);
static CATALOG: Mutex<()> = Mutex::new(());

pub fn begin_export(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("Choose a full file path.".into());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("Couldn't export the recording. {error}"))?;
    }
    let file = File::create(path).map_err(|error| format!("Couldn't export the recording. {error}"))?;
    *EXPORT.lock().expect("export") = Some(file);
    Ok(())
}

pub fn append_export(bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut guard = EXPORT.lock().expect("export");
    let file = guard.as_mut().ok_or("Start an export first.")?;
    file.write_all(bytes).map_err(|error| format!("Couldn't export the recording. {error}"))
}

pub fn finish_export() -> Result<(), String> {
    EXPORT.lock().expect("export").take();
    Ok(())
}

fn load(app: &AppHandle) -> Catalog {
    let _guard = CATALOG.lock().expect("recordings");
    files::read_json(&catalog_path(app))
}

fn mutate(app: &AppHandle, change: impl FnOnce(&mut Catalog)) {
    let _guard = CATALOG.lock().expect("recordings");
    let path = catalog_path(app);
    let mut catalog: Catalog = files::read_json(&path);
    change(&mut catalog);
    let _ = files::write_json(&path, &catalog);
}

fn catalog_path(app: &AppHandle) -> PathBuf {
    root(app).join("recordings.json")
}

fn file_in(app: &AppHandle, id: &str, name: &str) -> Option<PathBuf> {
    safe_id(id).then(|| root(app).join(id).join(name))
}

pub fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_hexdigit())
}

fn ensure_server(app: &AppHandle) -> Result<u16, String> {
    let mut slot = PORT.lock().expect("recording http");
    if let Some(port) = *slot {
        return Ok(port);
    }
    let server = Server::http("127.0.0.1:0").map_err(|error| format!("Couldn't play the recording. {error}"))?;
    let port = server.server_addr().to_ip().map(|addr| addr.port()).unwrap_or(0);
    if port == 0 {
        return Err("Couldn't play the recording.".into());
    }
    *slot = Some(port);
    let app = app.clone();
    thread::Builder::new()
        .name("recording-http".into())
        .spawn(move || {
            for request in server.incoming_requests() {
                let app = app.clone();
                thread::spawn(move || respond(&app, request));
            }
        })
        .map_err(|error| format!("Couldn't play the recording. {error}"))?;
    Ok(port)
}

fn respond(app: &AppHandle, request: Request) {
    if request.method() == &Method::Options {
        let _ = request.respond(cors(Response::empty(204)));
        return;
    }
    if request.method() != &Method::Get && request.method() != &Method::Head {
        let _ = request.respond(cors(Response::empty(405)));
        return;
    }
    let Some((id, name)) = split_path(request.url()) else {
        let _ = request.respond(cors(Response::empty(404)));
        return;
    };
    let Some(path) = contained(app, &id, &name) else {
        let _ = request.respond(cors(Response::empty(404)));
        return;
    };
    let Ok(file) = File::open(&path) else {
        let _ = request.respond(cors(Response::empty(404)));
        return;
    };
    let Ok(meta) = file.metadata() else {
        let _ = request.respond(cors(Response::empty(404)));
        return;
    };
    let length = meta.len();
    let content = if name.ends_with(".webm") { "video/webm" } else { "video/mp4" };
    if let Some((start, end)) = range_of(header(&request, "Range").as_deref(), length) {
        let mut file = file;
        if file.seek(SeekFrom::Start(start)).is_err() {
            let _ = request.respond(cors(Response::empty(416)));
            return;
        }
        let mut body = vec![0u8; (end - start + 1) as usize];
        if file.read_exact(&mut body).is_err() {
            let _ = request.respond(cors(Response::empty(416)));
            return;
        }
        let response = Response::from_data(body)
            .with_status_code(StatusCode(206))
            .with_header(header_line("Content-Type", content))
            .with_header(header_line("Accept-Ranges", "bytes"))
            .with_header(header_line("Content-Range", &format!("bytes {start}-{end}/{length}")));
        let _ = request.respond(cors(response));
        return;
    }
    let response = Response::from_file(file)
        .with_status_code(200)
        .with_header(header_line("Content-Type", content))
        .with_header(header_line("Accept-Ranges", "bytes"));
    let _ = request.respond(cors(response));
}

fn contained(app: &AppHandle, id: &str, name: &str) -> Option<PathBuf> {
    if name != "video.mp4" && name != "camera.webm" {
        return None;
    }
    let path = file_in(app, id, name)?;
    let root = fs::canonicalize(root(app)).ok()?;
    let file = fs::canonicalize(&path).ok()?;
    file.starts_with(root).then_some(file)
}

fn split_path(url: &str) -> Option<(String, String)> {
    let path = url.split('?').next().unwrap_or(url);
    let mut parts = path.split('/').filter(|part| !part.is_empty());
    let id = parts.next()?.to_string();
    let name = parts.next()?.to_string();
    parts.next().is_none().then_some((id, name))
}

fn range_of(header: Option<&str>, length: u64) -> Option<(u64, u64)> {
    let header = header?;
    let value = header.trim().strip_prefix("bytes=")?;
    if length == 0 {
        return None;
    }
    let (start, end) = value.split_once('-')?;
    let start = if start.is_empty() { 0 } else { start.parse().ok()? };
    let end = if end.is_empty() { length - 1 } else { end.parse::<u64>().ok()?.min(length - 1) };
    (start <= end && start < length).then_some((start, end))
}

fn header(request: &Request, name: &str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|header| header.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str().to_string())
}

fn header_line(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("header")
}

fn cors<R: std::io::Read>(response: Response<R>) -> Response<R> {
    response
        .with_header(header_line("Access-Control-Allow-Origin", "*"))
        .with_header(header_line("Access-Control-Allow-Headers", "Range"))
        .with_header(header_line("Access-Control-Expose-Headers", "Content-Range, Accept-Ranges, Content-Length"))
}

#[cfg(test)]
mod tests {
    use super::{Catalog, RecordingEntry, safe_id};

    #[test]
    fn an_older_catalog_keeps_missing_fields() {
        let catalog: Catalog = serde_json::from_str(r#"{"items":[{"id":"ab","title":"Hi"}]}"#).expect("catalog");
        let item = &catalog.items[0];
        assert_eq!(item.title, "Hi");
        assert_eq!(item.trim_start_ms, 0);
        assert!(item.camera_path.is_empty());
        assert_eq!(item.camera.x, RecordingEntry::default().camera.x);
    }

    #[test]
    fn ids_are_hex_only() {
        assert!(safe_id("ab12"));
        assert!(!safe_id("../video"));
        assert!(!safe_id(""));
    }
}
