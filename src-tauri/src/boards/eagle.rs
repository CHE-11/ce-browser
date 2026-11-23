//! Importing an Eagle library (`<name>.library`): its folder tree from `metadata.json`, and each
//! item's file, name, tags, notes, address, rating, colors and folders from
//! `images/<id>.info/metadata.json`. Importing the same library again fills the same folders
//! and skips files already there.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Deserialize;
use tauri::AppHandle;

use super::import::{BoardImportItem, BoardImportProgress, emit_progress, prepare, spawn_job};
use super::model::{
    FOLDER_COLORS, NOTE_LIMIT, PaletteColor, Pin, TITLE_LIMIT, UNCATEGORIZED, clean_tags, clip_text,
};
use super::{change, snapshot, storage};

const COMMIT_EVERY: usize = 40;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(120);

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct EagleLibrary {
    folders: Vec<EagleFolder>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct EagleFolder {
    id: String,
    name: String,
    description: String,
    icon_color: String,
    children: Vec<EagleFolder>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct EagleItem {
    name: String,
    ext: String,
    tags: Vec<String>,
    folders: Vec<String>,
    is_deleted: bool,
    url: String,
    annotation: String,
    star: Option<u8>,
    btime: Option<u64>,
    mtime: Option<u64>,
    palettes: Vec<EaglePalette>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct EaglePalette {
    color: Vec<u8>,
    ratio: f32,
}

/// The `.library` folder for `path`: the folder itself, or the one holding a picked
/// `metadata.json`.
fn library_root(path: &Path) -> Option<PathBuf> {
    let root = if path.is_file() { path.parent()? } else { path };
    (root.join("metadata.json").is_file() && root.join("images").is_dir())
        .then(|| root.to_path_buf())
}

/// Starts importing the library at `path` into `folder` (empty or `UNCATEGORIZED` for the top
/// level). Progress arrives as `board-import-progress`.
pub fn import_eagle_library(app: &AppHandle, path: &str, folder: &str) -> Result<String, String> {
    let root = library_root(Path::new(path.trim()))
        .ok_or("That isn't an Eagle library. Choose the folder that ends in .library.")?;
    let text = fs::read_to_string(root.join("metadata.json"))
        .map_err(|error| format!("Couldn't read the library. {error}"))?;
    let library: EagleLibrary = serde_json::from_str(&text)
        .map_err(|error| format!("Couldn't read the library. {error}"))?;
    let base = if folder.trim().is_empty() || folder == UNCATEGORIZED {
        None
    } else {
        snapshot(app).target_for_save(folder)?
    };
    Ok(spawn_job(app, move |app, job, cancel| {
        run(app, job, &root, library, base, cancel)
    }))
}

fn color_of(icon_color: &str) -> String {
    let color = icon_color.trim().to_ascii_lowercase();
    if FOLDER_COLORS.contains(&color.as_str()) {
        color
    } else {
        String::new()
    }
}

/// Makes (or finds by name) a folder for each Eagle folder. Returns Eagle id to folder id.
fn make_folders(
    app: &AppHandle,
    folders: &[EagleFolder],
    base: Option<String>,
) -> Result<HashMap<String, String>, String> {
    change(app, |boards| {
        let mut ids = HashMap::new();
        let mut stack: Vec<(&EagleFolder, Option<String>)> = folders
            .iter()
            .map(|folder| (folder, base.clone()))
            .collect();
        while let Some((folder, parent)) = stack.pop() {
            let Some(id) =
                boards.folder_path(parent.as_deref(), &folder.name.replace(['/', '\\'], "-"))
            else {
                continue;
            };
            if let Some(made) = boards.folders.iter_mut().find(|item| item.id == id) {
                if made.description.is_empty() {
                    made.description = clip_text(&folder.description, 2000);
                }
                if made.color.is_empty() {
                    made.color = color_of(&folder.icon_color);
                }
            }
            ids.insert(folder.id.clone(), id.clone());
            for child in &folder.children {
                stack.push((child, Some(id.clone())));
            }
        }
        Ok(ids)
    })
}

/// Item folders under `images/`, sorted so the import order is stable.
fn item_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(root.join("images"))
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.is_dir()
                        && path
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("info"))
                })
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    dirs
}

/// The item's file in its `.info` folder: `<name>.<ext>`, else any file that is not the
/// thumbnail or the metadata.
fn item_file(dir: &Path, item: &EagleItem) -> Option<PathBuf> {
    let named = dir.join(format!("{}.{}", item.name, item.ext));
    if named.is_file() {
        return Some(named);
    }
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            path.is_file() && name != "metadata.json" && !name.contains("_thumbnail.")
        })
}

fn item_thumbnail(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .to_lowercase()
                    .contains("_thumbnail.")
            })
        })
}

fn palette_of(item: &EagleItem) -> Vec<PaletteColor> {
    let total: f32 = item
        .palettes
        .iter()
        .map(|palette| palette.ratio.max(0.0))
        .sum();
    if total <= 0.0 {
        return Vec::new();
    }
    item.palettes
        .iter()
        .filter(|palette| palette.color.len() >= 3)
        .map(|palette| PaletteColor {
            color: format!(
                "#{:02x}{:02x}{:02x}",
                palette.color[0], palette.color[1], palette.color[2]
            ),
            ratio: ((palette.ratio.max(0.0) / total) * 1000.0).round() / 1000.0,
        })
        .take(8)
        .collect()
}

/// A pin for one Eagle item, with its file stored. None when the item has nothing to import.
fn item_pin(
    app: &AppHandle,
    dir: &Path,
    item: &EagleItem,
    folders: &HashMap<String, String>,
) -> Result<Option<Pin>, String> {
    let thumbnail = item_thumbnail(dir);
    let is_bookmark = item.ext.eq_ignore_ascii_case("url");
    let mut pin = if is_bookmark {
        let Some(thumbnail) = &thumbnail else {
            return Ok(None);
        };
        let mut pin = prepare(
            app,
            &BoardImportItem {
                path: thumbnail.to_string_lossy().into_owned(),
                ..BoardImportItem::default()
            },
        )?;
        pin.kind = "link".into();
        pin.file_name.clear();
        pin
    } else {
        let Some(file) = item_file(dir, item) else {
            return Ok(None);
        };
        let mut pin = prepare(
            app,
            &BoardImportItem {
                path: file.to_string_lossy().into_owned(),
                name: format!("{}.{}", item.name, item.ext),
                ..BoardImportItem::default()
            },
        )?;
        if !pin.has_image && !pin.has_thumb {
            if let Some(image) = thumbnail
                .as_ref()
                .and_then(|path| fs::read(path).ok())
                .and_then(|bytes| storage::decode_image(&bytes).ok())
            {
                if let Ok(colors) = storage::write_thumb(app, &pin.id, &image) {
                    pin.has_thumb = true;
                    pin.width = image.width();
                    pin.height = image.height();
                    pin.palette = colors;
                }
            }
        }
        pin
    };
    pin.title = clip_text(&item.name, TITLE_LIMIT);
    pin.note = clip_text(&item.annotation, NOTE_LIMIT);
    pin.tags = clean_tags(&item.tags);
    pin.rating = item.star.unwrap_or(0).min(5);
    // Eagle writes "http://" for items without an address.
    if let Some(url) = url::Url::parse(item.url.trim())
        .ok()
        .filter(|url| url.host_str().is_some_and(|host| !host.is_empty()))
    {
        pin.page_url = url.to_string();
        pin.link_url = url.to_string();
    }
    let palette = palette_of(item);
    if !palette.is_empty() {
        pin.palette = palette;
    }
    if let Some(btime) = item.btime.filter(|time| *time > 0) {
        pin.created_at = btime;
    }
    if let Some(mtime) = item.mtime.filter(|time| *time > 0) {
        pin.file_modified_at = mtime;
    }
    pin.folder_ids = item
        .folders
        .iter()
        .filter_map(|folder| folders.get(folder).cloned())
        .collect();
    if item.is_deleted {
        pin.deleted_at = Some(crate::tabs::now_ms());
    }
    Ok(Some(pin))
}

/// Saves the pins, leaving out any whose file is already in each of its folders.
fn commit(
    app: &AppHandle,
    base: &Option<String>,
    batch: Vec<Pin>,
    progress: &mut BoardImportProgress,
) {
    if batch.is_empty() {
        return;
    }
    let result = change(app, |boards| {
        let mut saved = Vec::new();
        let mut repeated = Vec::new();
        for mut pin in batch {
            if pin.folder_ids.is_empty() {
                pin.folder_ids = base.clone().into_iter().collect();
            }
            let duplicate = !pin.hash.is_empty()
                && if pin.folder_ids.is_empty() {
                    boards.has_duplicate(&pin.hash, None)
                } else {
                    pin.folder_ids
                        .iter()
                        .all(|folder| boards.has_duplicate(&pin.hash, Some(folder)))
                };
            if duplicate {
                repeated.push(pin);
            } else {
                saved.push(pin);
            }
        }
        let count = saved.len();
        let last = boards.last_folder_id.clone();
        boards.add_pins(saved);
        boards.last_folder_id = last;
        Ok((count, repeated))
    });
    match result {
        Ok((count, repeated)) => {
            progress.imported += count;
            progress.duplicates += repeated.len();
            for pin in &repeated {
                storage::remove_new_files(app, &pin.id, &pin.ext);
            }
        }
        Err(error) => {
            progress.failed += 1;
            if progress.error.is_empty() {
                progress.error = error;
            }
        }
    }
}

fn run(
    app: &AppHandle,
    job: &str,
    root: &Path,
    library: EagleLibrary,
    base: Option<String>,
    cancel: &AtomicBool,
) {
    let mut progress = BoardImportProgress {
        job_id: job.to_string(),
        current: "Reading the library".into(),
        ..BoardImportProgress::default()
    };
    emit_progress(app, &progress);
    let folders = match make_folders(app, &library.folders, base.clone()) {
        Ok(folders) => folders,
        Err(error) => {
            progress.error = error;
            progress.finished = true;
            emit_progress(app, &progress);
            return;
        }
    };
    progress.folder_id = base.clone();
    let dirs = item_dirs(root);
    progress.total = dirs.len();
    let mut pending = Vec::new();
    let mut last_emit = Instant::now();
    for dir in &dirs {
        if cancel.load(Ordering::Relaxed) {
            progress.cancelled = true;
            break;
        }
        let parsed = fs::read_to_string(dir.join("metadata.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<EagleItem>(&text).ok());
        match parsed {
            Some(item) => {
                progress.current = format!("{}.{}", item.name, item.ext);
                match item_pin(app, dir, &item, &folders) {
                    Ok(Some(pin)) => pending.push(pin),
                    Ok(None) => {}
                    Err(error) => {
                        progress.failed += 1;
                        if progress.error.is_empty() {
                            progress.error = error;
                        }
                    }
                }
            }
            None => progress.failed += 1,
        }
        progress.done += 1;
        if pending.len() >= COMMIT_EVERY {
            commit(app, &base, std::mem::take(&mut pending), &mut progress);
        }
        if last_emit.elapsed() >= PROGRESS_INTERVAL {
            emit_progress(app, &progress);
            last_emit = Instant::now();
        }
    }
    commit(app, &base, pending, &mut progress);
    progress.current.clear();
    progress.finished = true;
    emit_progress(app, &progress);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eagle_metadata_reads_folders_items_and_palettes() {
        let library: EagleLibrary = serde_json::from_str(
            r#"{ "folders": [{ "id": "A", "name": "Archives", "iconColor": "red",
                 "children": [{ "id": "B", "name": "Job Hunt", "children": [] }] }],
                 "smartFolders": [], "applicationVersion": "4.0" }"#,
        )
        .expect("library");
        assert_eq!(library.folders[0].children[0].name, "Job Hunt");
        assert_eq!(color_of(&library.folders[0].icon_color), "red");
        assert_eq!(color_of("chartreuse"), "");
        let item: EagleItem = serde_json::from_str(
            r#"{ "id": "X", "name": "Clipboard", "ext": "png", "tags": ["Applications"],
                 "folders": ["B"], "isDeleted": false, "url": "http://", "annotation": "",
                 "star": 4, "palettes": [{ "color": [255, 0, 0], "ratio": 75, "$$hashKey": "1" },
                 { "color": [0, 0, 255], "ratio": 25 }] }"#,
        )
        .expect("item");
        assert_eq!(item.star, Some(4));
        let palette = palette_of(&item);
        assert_eq!(palette[0].color, "#ff0000");
        assert!((palette[0].ratio - 0.75).abs() < 0.001);
    }
}
