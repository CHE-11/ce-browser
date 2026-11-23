//! Boards: an asset library of pins saved from web pages and files, kept in nested folders.
//! `ce://boards` shows them.
//!
//! `boards.json` in the profile folder holds the folders and pins. Pin files live in `boards/`
//! (see `storage`), and the `ceboard` protocol serves them to the boards page (see `serve`).

mod capture_pins;
mod clipboard;
mod eagle;
mod export;
mod import;
mod model;
mod palette;
mod serve;
mod storage;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter};

use crate::{capture, files, paths};

pub use capture_pins::{pin_element, pin_images, pin_link, pin_page, pin_region, save_menu_image};
pub use clipboard::paste_clipboard;
pub use eagle::import_eagle_library;
pub use export::{export_folder, export_pins};
pub use import::{BoardImportItem, BoardImportResult, cancel_import, import_items, start_import};
pub use model::{BoardFolder, BoardsSnapshot, FolderPatch, Pin, PinPatch};
pub use serve::respond as serve;

use model::valid_id;

const FILE: &str = "boards.json";

/// Every change reads, edits and writes `boards.json` while holding this lock.
static LOCK: Mutex<()> = Mutex::new(());

fn store_path(app: &AppHandle) -> PathBuf {
    paths::data_file(app, FILE)
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Call while holding `LOCK`. A repaired or migrated file is written back, so ids made during
/// repair stay the same on the next read.
fn read(app: &AppHandle) -> BoardsSnapshot {
    let path = store_path(app);
    let stored: BoardsSnapshot = files::read_json(&path);
    let boards = stored.clone().normalize();
    if boards != stored && path.exists() {
        let _ = files::write_json(&path, &boards);
    }
    boards
}

pub fn snapshot(app: &AppHandle) -> BoardsSnapshot {
    let _guard = lock();
    read(app)
}

fn change<T>(
    app: &AppHandle,
    edit: impl FnOnce(&mut BoardsSnapshot) -> Result<T, String>,
) -> Result<T, String> {
    let (result, snapshot) = {
        let _guard = lock();
        let mut snapshot = read(app);
        let result = edit(&mut snapshot)?;
        files::write_json(&store_path(app), &snapshot)?;
        (result, snapshot)
    };
    let _ = app.emit("boards-changed", &snapshot);
    crate::sync::touch();
    Ok(result)
}

/// Starts filling in data older pins lack.
pub fn init(app: &AppHandle) {
    storage::start_backfill(app.clone());
}

pub fn create_folder(
    app: &AppHandle,
    name: &str,
    parent: Option<&str>,
) -> Result<BoardFolder, String> {
    change(app, |boards| boards.create_folder(name, parent))
}

pub fn rename_folder(app: &AppHandle, id: &str, name: &str) -> Result<(), String> {
    change(app, |boards| boards.rename_folder(id, name))
}

pub fn move_folder(app: &AppHandle, id: &str, parent: Option<&str>) -> Result<(), String> {
    change(app, |boards| boards.move_folder(id, parent))
}

pub fn update_folder(app: &AppHandle, id: &str, patch: &FolderPatch) -> Result<(), String> {
    change(app, |boards| boards.update_folder(id, patch))
}

/// Removes the folder tree. Returns how many pins went to the trash.
pub fn delete_folder(app: &AppHandle, id: &str) -> Result<usize, String> {
    change(app, |boards| boards.delete_folder(id))
}

pub fn duplicate_folder(app: &AppHandle, id: &str) -> Result<BoardFolder, String> {
    change(app, |boards| boards.duplicate_folder(id))
}

/// The folder with this name, made at the top level when there is none.
pub fn folder_named(app: &AppHandle, name: &str) -> Result<String, String> {
    change(app, |boards| {
        let lower = name.trim().to_lowercase();
        if let Some(folder) = boards
            .folders
            .iter()
            .find(|folder| folder.name.to_lowercase() == lower)
        {
            return Ok(folder.id.clone());
        }
        Ok(boards.create_folder(name, None)?.id)
    })
}

pub fn update_pin(app: &AppHandle, id: &str, patch: &PinPatch) -> Result<(), String> {
    change(app, |boards| boards.update_pin(id, patch))
}

pub fn add_pins_to_folder(app: &AppHandle, ids: &[String], folder: &str) -> Result<(), String> {
    change(app, |boards| boards.add_to_folder(ids, folder))
}

pub fn remove_pins_from_folder(
    app: &AppHandle,
    ids: &[String],
    folder: &str,
) -> Result<(), String> {
    change(app, |boards| boards.remove_from_folder(ids, folder))
}

/// `to` of `UNCATEGORIZED` or empty takes the pins out of every folder.
pub fn move_pins(
    app: &AppHandle,
    ids: &[String],
    from: Option<&str>,
    to: &str,
) -> Result<(), String> {
    let to = (!to.is_empty() && to != model::UNCATEGORIZED).then_some(to);
    change(app, |boards| boards.move_pins(ids, from, to))
}

pub fn add_pin_tags(app: &AppHandle, ids: &[String], tags: &[String]) -> Result<(), String> {
    change(app, |boards| {
        boards.add_tags(ids, tags);
        Ok(())
    })
}

pub fn remove_pin_tags(app: &AppHandle, ids: &[String], tags: &[String]) -> Result<(), String> {
    change(app, |boards| {
        boards.remove_tags(ids, tags);
        Ok(())
    })
}

pub fn rename_tag(app: &AppHandle, from: &str, to: &str) -> Result<(), String> {
    change(app, |boards| boards.rename_tag(from, to))
}

pub fn delete_tag(app: &AppHandle, name: &str) -> Result<(), String> {
    change(app, |boards| {
        boards.delete_tag(name);
        Ok(())
    })
}

pub fn trash_pins(app: &AppHandle, ids: &[String]) -> Result<(), String> {
    change(app, |boards| {
        boards.trash_pins(ids);
        Ok(())
    })
}

pub fn restore_pins(app: &AppHandle, ids: &[String]) -> Result<(), String> {
    change(app, |boards| {
        boards.restore_pins(ids);
        Ok(())
    })
}

pub fn delete_pins_forever(app: &AppHandle, ids: &[String]) -> Result<(), String> {
    let removed = change(app, |boards| Ok(boards.delete_pins(ids)))?;
    storage::remove_files(app, &removed);
    Ok(())
}

pub fn empty_trash(app: &AppHandle) -> Result<usize, String> {
    let removed = change(app, |boards| Ok(boards.empty_trash()))?;
    storage::remove_files(app, &removed);
    Ok(removed.len())
}

pub fn mark_used(app: &AppHandle, ids: &[String]) -> Result<(), String> {
    change(app, |boards| {
        boards.mark_used(ids);
        Ok(())
    })
}

fn find_pin(app: &AppHandle, id: &str) -> Result<Pin, String> {
    if !valid_id(id) {
        return Err("That pin no longer exists.".into());
    }
    snapshot(app)
        .pins
        .into_iter()
        .find(|pin| pin.id == id)
        .ok_or_else(|| "That pin no longer exists.".to_string())
}

fn pin_original(app: &AppHandle, id: &str) -> Result<(Pin, PathBuf), String> {
    let pin = find_pin(app, id)?;
    let path = storage::original_path(app, &pin).ok_or("This pin has no saved file.")?;
    Ok((pin, path))
}

/// PNG bytes of the pin's image, converting other formats.
fn png_bytes(app: &AppHandle, id: &str) -> Result<Vec<u8>, String> {
    let (_, path) = pin_original(app, id)?;
    let bytes = std::fs::read(&path).map_err(|_| "This pin has no image.".to_string())?;
    if storage::image_ext(&bytes) == Some("png") {
        return Ok(bytes);
    }
    capture::encode_png(&storage::decode_image(&bytes)?)
}

pub fn copy_image(app: &AppHandle, id: &str) -> Result<(), String> {
    capture::copy_edited_png(&png_bytes(app, id)?)
}

/// Saves the original. A `.png` path for another format saves a converted PNG.
pub fn save_image_as(app: &AppHandle, id: &str, path: &Path) -> Result<(), String> {
    let (pin, source) = pin_original(app, id)?;
    let wants_png = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"));
    if wants_png && pin.ext != "png" {
        return capture::save_edited_png(&png_bytes(app, id)?, path);
    }
    copy_to(&source, path)
}

pub fn save_file_as(app: &AppHandle, id: &str, path: &Path) -> Result<(), String> {
    let (_, source) = pin_original(app, id)?;
    copy_to(&source, path)
}

fn copy_to(source: &Path, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Couldn't save the file. {error}"))?;
    }
    std::fs::copy(source, path)
        .map(|_| ())
        .map_err(|error| format!("Couldn't save the file. {error}"))
}

/// Copies the original to a temp name with its own name and extension, then opens it.
pub fn open_pin_file(app: &AppHandle, id: &str) -> Result<(), String> {
    let (pin, source) = pin_original(app, id)?;
    let dir = std::env::temp_dir().join("ce-browser-boards").join(id);
    std::fs::create_dir_all(&dir).map_err(|error| format!("Couldn't open the file. {error}"))?;
    let path = dir.join(export::file_name_for(&pin));
    std::fs::copy(&source, &path).map_err(|error| format!("Couldn't open the file. {error}"))?;
    let _ = mark_used(app, &[id.to_string()]);
    files::open_with_default(&path)
}

/// Shows the stored original in Explorer.
pub fn reveal_pin_file(app: &AppHandle, id: &str) -> Result<(), String> {
    let (_, source) = pin_original(app, id)?;
    files::reveal_path(&source)
}
