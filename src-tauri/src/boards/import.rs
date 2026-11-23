//! Adding files, images, links and whole folder trees to boards.
//!
//! Originals are stored as they are. A dropped or picked folder becomes a folder of the same
//! name, with a subfolder for each directory inside it. Importing the same tree again fills the
//! same folders, and files already in a folder (same hash) are skipped.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use super::capture_pins::draft;
use super::model::{FILE_NAME_LIMIT, MIME_LIMIT, Pin, UNCATEGORIZED, clean_ext, clip_text, new_id};
use super::{change, snapshot, storage};

/// Largest file read from disk.
pub const MAX_PATH_BYTES: u64 = 1024 * 1024 * 1024;
/// Largest file sent as base64 from a drop or paste.
pub const MAX_DATA_BYTES: u64 = 200 * 1024 * 1024;
const MAX_DEPTH: usize = 32;
const COMMIT_EVERY: usize = 40;
const COMMIT_INTERVAL: Duration = Duration::from_millis(800);
const PROGRESS_INTERVAL: Duration = Duration::from_millis(120);
const SKIPPED_NAMES: [&str; 4] = ["thumbs.db", "desktop.ini", ".ds_store", "ehthumbs.db"];

/// One file, image or link dropped, pasted or picked on `ce://boards`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BoardImportItem {
    pub name: String,
    pub mime: String,
    /// Set for a link. Empty when `data` or `path` carries a file.
    pub url: String,
    /// Base64 file bytes from a drop or paste.
    pub data: String,
    /// Path from the file picker. Read on this machine; not a remote address.
    pub path: String,
    /// Folder path inside the target ("a/b") for files dropped as part of a folder.
    pub relative_dir: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardImportResult {
    pub pins: Vec<Pin>,
    /// Files skipped because the same file is already in that folder.
    pub duplicates: usize,
    pub failed: usize,
}

/// Sent as `board-import-progress` while a folder import runs.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardImportProgress {
    pub job_id: String,
    pub total: usize,
    pub done: usize,
    pub imported: usize,
    pub duplicates: usize,
    pub failed: usize,
    /// File being read, or a step such as "Reading folders".
    pub current: String,
    pub finished: bool,
    pub cancelled: bool,
    /// First error, when a file couldn't be added.
    pub error: String,
    /// Folder made for the first picked folder, to open when the import ends.
    pub folder_id: Option<String>,
}

static JOBS: Mutex<Option<HashMap<String, Arc<AtomicBool>>>> = Mutex::new(None);

fn jobs() -> std::sync::MutexGuard<'static, Option<HashMap<String, Arc<AtomicBool>>>> {
    JOBS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn cancel_import(job: &str) {
    if let Some(flag) = jobs().as_ref().and_then(|jobs| jobs.get(job)) {
        flag.store(true, Ordering::Relaxed);
    }
}

fn safe_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name).trim();
    let cleaned: String = base
        .chars()
        .filter(|character| !matches!(character, '\0' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .take(FILE_NAME_LIMIT)
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() || cleaned == ".." {
        "file".into()
    } else {
        cleaned
    }
}

fn ext_of(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, ext)| clean_ext(ext))
        .unwrap_or_default()
}

fn import_name(item: &BoardImportItem) -> String {
    if !item.name.trim().is_empty() {
        return safe_file_name(&item.name);
    }
    if !item.path.trim().is_empty() {
        return Path::new(item.path.trim())
            .file_name()
            .and_then(|name| name.to_str())
            .map(safe_file_name)
            .unwrap_or_else(|| "file".into());
    }
    "file".into()
}

fn title_from_name(name: &str) -> String {
    let name = safe_file_name(name);
    name.rsplit_once('.')
        .map(|(stem, _)| stem.trim())
        .filter(|stem| !stem.is_empty())
        .unwrap_or(name.as_str())
        .to_string()
}

fn looks_like_image(name: &str, mime: &str) -> bool {
    let mime = mime.trim().to_ascii_lowercase();
    if mime == "image/svg+xml" {
        return false;
    }
    if mime.starts_with("image/") {
        return true;
    }
    matches!(
        ext_of(name).as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico"
    )
}

fn http_url(raw: &str) -> Option<String> {
    let url = raw.trim();
    if url.len() > 2000 || url.chars().any(char::is_whitespace) {
        return None;
    }
    if url.starts_with("https://") || url.starts_with("http://") {
        Some(url.to_string())
    } else {
        None
    }
}

fn item_is_empty(item: &BoardImportItem) -> bool {
    item.url.trim().is_empty() && item.data.trim().is_empty() && item.path.trim().is_empty()
}

fn decode_data(item: &BoardImportItem) -> Result<Vec<u8>, String> {
    let data = item.data.trim();
    if data.len() as u64 > MAX_DATA_BYTES / 3 * 4 + 8 {
        return Err(format!("{} is larger than 200 MB.", import_name(item)));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| "Couldn't read that file.".to_string())?;
    if bytes.is_empty() {
        return Err(format!("{} is empty.", import_name(item)));
    }
    Ok(bytes)
}

/// Stores decodable image bytes as an image pin. False when the bytes are not an image.
fn try_image(app: &AppHandle, pin: &mut Pin, bytes: &[u8], name_ext: &str) -> Result<bool, String> {
    let Ok(image) = storage::decode_image(bytes) else {
        return Ok(false);
    };
    let ext = storage::image_ext(bytes)
        .map(str::to_string)
        .unwrap_or_else(|| {
            if name_ext.is_empty() {
                "png".into()
            } else {
                name_ext.to_string()
            }
        });
    let stored = (|| {
        storage::write_original(app, &pin.id, &ext, bytes)?;
        storage::write_thumb(app, &pin.id, &image)
    })();
    match stored {
        Ok(colors) => pin.palette = colors,
        Err(error) => {
            storage::remove_new_files(app, &pin.id, &ext);
            return Err(error);
        }
    }
    pin.kind = "image".into();
    pin.has_image = true;
    pin.width = image.width();
    pin.height = image.height();
    pin.mime = storage::mime_for_ext(&ext).into();
    pin.ext = ext;
    pin.size = bytes.len() as u64;
    pin.hash = storage::hash_bytes(bytes);
    Ok(true)
}

/// Fills in a file pin whose original is stored, and asks the shell for a thumbnail.
fn finish_file(app: &AppHandle, pin: &mut Pin, ext: &str, size: u64, hash: String, mime: &str) {
    pin.kind = "file".into();
    pin.ext = ext.into();
    pin.size = size;
    pin.hash = hash;
    pin.mime = if mime.is_empty() {
        storage::mime_for_ext(ext).into()
    } else {
        clip_text(mime, MIME_LIMIT)
    };
    if let Some(path) = storage::original_path(app, pin) {
        if let Some(image) = storage::shell_thumbnail(&path) {
            if let Ok(colors) = storage::write_thumb(app, &pin.id, &image) {
                pin.has_thumb = true;
                pin.width = image.width();
                pin.height = image.height();
                pin.palette = colors;
            }
        }
    }
}

/// Stores the item's file and returns its pin. The pin is not saved to `boards.json` yet.
pub(super) fn prepare(app: &AppHandle, item: &BoardImportItem) -> Result<Pin, String> {
    let has_path = !item.path.trim().is_empty();
    let has_data = !item.data.trim().is_empty();
    if !has_path && !has_data {
        let url = http_url(&item.url)
            .ok_or_else(|| "That link needs to start with http:// or https://.".to_string())?;
        return Ok(draft("link", &url, &url, &url, ""));
    }
    let name = import_name(item);
    let mime = item.mime.trim();
    let name_ext = ext_of(&name);
    let mut pin = draft("file", &title_from_name(&name), "", "", "");
    pin.file_name = clip_text(&name, FILE_NAME_LIMIT);
    let file_ext = if name_ext.is_empty() {
        "bin".to_string()
    } else {
        name_ext.clone()
    };

    if has_path {
        let path = PathBuf::from(item.path.trim());
        let meta = fs::metadata(&path).map_err(|error| format!("Couldn't read {name}. {error}"))?;
        if !meta.is_file() {
            return Err(format!("{name} is not a file."));
        }
        if meta.len() == 0 {
            return Err(format!("{name} is empty."));
        }
        if meta.len() > MAX_PATH_BYTES {
            return Err(format!("{name} is larger than 1 GB."));
        }
        pin.file_modified_at = storage::modified_ms(&path);
        if looks_like_image(&name, mime) && meta.len() <= storage::MAX_DECODE_BYTES {
            let bytes =
                fs::read(&path).map_err(|error| format!("Couldn't read {name}. {error}"))?;
            if try_image(app, &mut pin, &bytes, &name_ext)? {
                return Ok(pin);
            }
        }
        let (size, hash) =
            storage::copy_original(app, &pin.id, &file_ext, &path).inspect_err(|_| {
                storage::remove_new_files(app, &pin.id, &file_ext);
            })?;
        finish_file(app, &mut pin, &file_ext, size, hash, mime);
        return Ok(pin);
    }

    let bytes = decode_data(item)?;
    if looks_like_image(&name, mime) && try_image(app, &mut pin, &bytes, &name_ext)? {
        return Ok(pin);
    }
    storage::write_original(app, &pin.id, &file_ext, &bytes).inspect_err(|_| {
        storage::remove_new_files(app, &pin.id, &file_ext);
    })?;
    let hash = storage::hash_bytes(&bytes);
    finish_file(app, &mut pin, &file_ext, bytes.len() as u64, hash, mime);
    Ok(pin)
}

/// Saves prepared pins. `base` is a folder id or `UNCATEGORIZED`. Pins whose file is already
/// in their folder are left out and their files removed.
fn commit(
    app: &AppHandle,
    base: &str,
    batch: Vec<(Pin, String)>,
) -> Result<(Vec<Pin>, usize), String> {
    if batch.is_empty() {
        return Ok((Vec::new(), 0));
    }
    let result = change(app, |boards| {
        let base = boards.target_for_save(base)?;
        let mut saved: Vec<Pin> = Vec::new();
        let mut duplicates = Vec::new();
        for (mut pin, relative) in batch {
            let target = if relative.trim().is_empty() {
                base.clone()
            } else {
                boards.folder_path(base.as_deref(), &relative)
            };
            let repeated = !pin.hash.is_empty()
                && (boards.has_duplicate(&pin.hash, target.as_deref())
                    || saved.iter().any(|other| {
                        other.hash == pin.hash && other.folder_ids.first() == target.as_ref()
                    }));
            if repeated {
                duplicates.push(pin);
                continue;
            }
            pin.folder_ids = target.into_iter().collect();
            saved.push(pin);
        }
        boards.add_pins(saved.clone());
        boards.last_folder_id = base;
        Ok((saved, duplicates))
    });
    match result {
        Ok((saved, duplicates)) => {
            for pin in &duplicates {
                storage::remove_new_files(app, &pin.id, &pin.ext);
            }
            Ok((saved, duplicates.len()))
        }
        Err(error) => Err(error),
    }
}

/// Saves files, images and links. An empty `folder` uses the last folder; `UNCATEGORIZED`
/// saves to no folder. Fails only when nothing could be saved.
pub fn import_items(
    app: &AppHandle,
    folder: &str,
    items: &[BoardImportItem],
) -> Result<BoardImportResult, String> {
    storage::init_com();
    let base = snapshot(app)
        .target_for_save(folder)?
        .unwrap_or_else(|| UNCATEGORIZED.to_string());
    let mut prepared = Vec::new();
    let mut failed = 0;
    let mut first_error = None;
    for item in items.iter().filter(|item| !item_is_empty(item)) {
        match prepare(app, item) {
            Ok(pin) => prepared.push((pin, item.relative_dir.clone())),
            Err(error) => {
                failed += 1;
                first_error.get_or_insert(error);
            }
        }
    }
    let files: Vec<(String, String)> = prepared
        .iter()
        .map(|(pin, _)| (pin.id.clone(), pin.ext.clone()))
        .collect();
    let (pins, duplicates) = commit(app, &base, prepared).inspect_err(|_| {
        for (id, ext) in &files {
            storage::remove_new_files(app, id, ext);
        }
    })?;
    if pins.is_empty() && duplicates == 0 {
        return Err(first_error.unwrap_or_else(|| "Add a file or a link.".into()));
    }
    Ok(BoardImportResult {
        pins,
        duplicates,
        failed,
    })
}

/// Files and directories found under the picked paths.
#[derive(Default)]
struct Walked {
    /// File path and the folder path it goes to ("" for the target itself).
    files: Vec<(PathBuf, String)>,
    /// Every directory as a folder path, parents first, so empty ones are made too.
    dirs: Vec<String>,
}

fn skipped(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const HIDDEN: u32 = 0x2;
    const SYSTEM: u32 = 0x4;
    const REPARSE_POINT: u32 = 0x400;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if name.starts_with('.') || SKIPPED_NAMES.contains(&name.as_str()) {
        return true;
    }
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            meta.file_type().is_symlink()
                || meta.file_attributes() & (HIDDEN | SYSTEM | REPARSE_POINT) != 0
        }
        Err(_) => true,
    }
}

fn walk_dir(dir: &Path, relative: &str, depth: usize, out: &mut Walked, cancel: &AtomicBool) {
    if depth > MAX_DEPTH || cancel.load(Ordering::Relaxed) {
        return;
    }
    out.dirs.push(relative.to_string());
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    entries.sort_by_key(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
    });
    let mut subdirs = Vec::new();
    for path in entries {
        if skipped(&path) {
            continue;
        }
        if path.is_dir() {
            subdirs.push(path);
        } else if path.is_file() {
            out.files.push((path, relative.to_string()));
        }
    }
    for path in subdirs {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        walk_dir(&path, &format!("{relative}/{name}"), depth + 1, out, cancel);
    }
}

fn walk(paths: &[String], cancel: &AtomicBool) -> Walked {
    let mut out = Walked::default();
    for raw in paths {
        let path = PathBuf::from(raw.trim());
        if path.is_dir() {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Imported".into());
            walk_dir(&path, &name, 0, &mut out, cancel);
        } else if path.is_file() {
            out.files.push((path, String::new()));
        }
    }
    out
}

/// Imports files and folder trees from disk on a worker thread. Progress arrives as
/// `board-import-progress`. Returns the job id for `cancel_import`.
pub fn start_import(app: &AppHandle, paths: Vec<String>, folder: &str) -> Result<String, String> {
    if paths.iter().all(|path| path.trim().is_empty()) {
        return Err("Choose a file or folder.".into());
    }
    let base = snapshot(app)
        .target_for_save(folder)?
        .unwrap_or_else(|| UNCATEGORIZED.to_string());
    Ok(spawn_job(app, move |app, job, cancel| {
        run_job(app, job, &paths, &base, cancel)
    }))
}

/// Runs `work` on a worker thread with a cancel flag `cancel_import` can set. Returns the job id.
pub(super) fn spawn_job(
    app: &AppHandle,
    work: impl FnOnce(&AppHandle, &str, &AtomicBool) + Send + 'static,
) -> String {
    let job = new_id("j");
    let cancel = Arc::new(AtomicBool::new(false));
    jobs()
        .get_or_insert_with(HashMap::new)
        .insert(job.clone(), cancel.clone());
    let app = app.clone();
    let id = job.clone();
    std::thread::spawn(move || {
        storage::init_com();
        work(&app, &id, &cancel);
        if let Some(jobs) = jobs().as_mut() {
            jobs.remove(&id);
        }
    });
    job
}

pub(super) fn emit_progress(app: &AppHandle, progress: &BoardImportProgress) {
    let _ = app.emit("board-import-progress", progress);
}

fn run_job(app: &AppHandle, job: &str, paths: &[String], base: &str, cancel: &AtomicBool) {
    let mut progress = BoardImportProgress {
        job_id: job.to_string(),
        current: "Reading folders".into(),
        ..BoardImportProgress::default()
    };
    let emit = |progress: &BoardImportProgress| emit_progress(app, progress);
    emit(&progress);

    let walked = walk(paths, cancel);
    progress.total = walked.files.len();

    if !walked.dirs.is_empty() {
        let first = walked.dirs[0].clone();
        let dirs = walked.dirs.clone();
        let made = change(app, |boards| {
            let base = boards.target_for_save(base)?;
            let mut first_id = None;
            for dir in &dirs {
                let id = boards.folder_path(base.as_deref(), dir);
                if dir == &first {
                    first_id = id;
                }
            }
            Ok(first_id)
        });
        match made {
            Ok(id) => progress.folder_id = id,
            Err(error) => {
                progress.error = error;
                progress.finished = true;
                emit(&progress);
                return;
            }
        }
    }

    let mut pending: Vec<(Pin, String)> = Vec::new();
    let mut last_commit = Instant::now();
    let mut last_emit = Instant::now();
    let flush = |pending: &mut Vec<(Pin, String)>, progress: &mut BoardImportProgress| {
        let batch = std::mem::take(pending);
        let files: Vec<(String, String)> = batch
            .iter()
            .map(|(pin, _)| (pin.id.clone(), pin.ext.clone()))
            .collect();
        match commit(app, base, batch) {
            Ok((saved, duplicates)) => {
                progress.imported += saved.len();
                progress.duplicates += duplicates;
            }
            Err(error) => {
                for (id, ext) in &files {
                    storage::remove_new_files(app, id, ext);
                }
                progress.failed += files.len();
                if progress.error.is_empty() {
                    progress.error = error;
                }
            }
        }
    };

    for (path, relative) in &walked.files {
        if cancel.load(Ordering::Relaxed) {
            progress.cancelled = true;
            break;
        }
        progress.current = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let item = BoardImportItem {
            path: path.to_string_lossy().into_owned(),
            ..BoardImportItem::default()
        };
        match prepare(app, &item) {
            Ok(pin) => pending.push((pin, relative.clone())),
            Err(error) => {
                progress.failed += 1;
                if progress.error.is_empty() {
                    progress.error = error;
                }
            }
        }
        progress.done += 1;
        if pending.len() >= COMMIT_EVERY
            || (!pending.is_empty() && last_commit.elapsed() >= COMMIT_INTERVAL)
        {
            flush(&mut pending, &mut progress);
            last_commit = Instant::now();
        }
        if last_emit.elapsed() >= PROGRESS_INTERVAL {
            emit(&progress);
            last_emit = Instant::now();
        }
    }
    flush(&mut pending, &mut progress);
    progress.current.clear();
    progress.finished = true;
    emit(&progress);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_tell_images_from_files_and_links() {
        assert!(looks_like_image("shot.PNG", ""));
        assert!(looks_like_image("x", "image/webp"));
        assert!(!looks_like_image("notes.txt", "text/plain"));
        assert!(!looks_like_image("icon.svg", "image/svg+xml"));
        assert_eq!(
            http_url("https://example.com/a"),
            Some("https://example.com/a".into())
        );
        assert_eq!(http_url("notes.txt"), None);
        assert_eq!(safe_file_name("../..\\report.pdf"), "report.pdf");
        assert_eq!(title_from_name("Holiday photo.JPG"), "Holiday photo");
        assert_eq!(ext_of("archive.tar.GZ"), "gz");
    }

    #[test]
    fn walking_a_folder_keeps_its_structure_and_skips_hidden_files() {
        let root = std::env::temp_dir().join(format!("ce-boards-walk-{}", new_id("t")));
        let top = root.join("Trip");
        fs::create_dir_all(top.join("Day 1").join("Empty")).expect("dirs");
        fs::create_dir_all(top.join(".git")).expect("hidden dir");
        fs::write(top.join("cover.png"), b"x").expect("file");
        fs::write(top.join("Day 1").join("a.jpg"), b"x").expect("file");
        fs::write(top.join("Thumbs.db"), b"x").expect("file");
        fs::write(top.join(".git").join("config"), b"x").expect("file");
        let walked = walk(
            &[top.to_string_lossy().into_owned()],
            &AtomicBool::new(false),
        );
        let relatives: Vec<&str> = walked
            .files
            .iter()
            .map(|(_, relative)| relative.as_str())
            .collect();
        assert_eq!(relatives, vec!["Trip", "Trip/Day 1"]);
        assert_eq!(walked.dirs, vec!["Trip", "Trip/Day 1", "Trip/Day 1/Empty"]);
        let _ = fs::remove_dir_all(&root);
    }
}
