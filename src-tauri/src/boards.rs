//! Boards: pins saved from web pages, kept on boards inside nested folders. `ce://boards` shows them.
//!
//! `boards.json` in the profile folder holds the folders, boards and pins. Each pin with a picture
//! has `boards/<pin id>.png` and a small `boards/<pin id>.thumb.jpg` for the grid.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::{capture, files, page_menu, paths, tabs, undo, webview2};

const FILE: &str = "boards.json";
const IMAGE_DIR: &str = "boards";
const UNSORTED: &str = "Unsorted";
const THUMB_WIDTH: u32 = 480;
/// Thumbnails of very tall captures keep only the top of the image.
const THUMB_MAX_HEIGHT: u32 = 1200;
const TITLE_LIMIT: usize = 300;
const TEXT_LIMIT: usize = 2000;
const NAME_LIMIT: usize = 120;
const NOTE_LIMIT: usize = 5000;
/// Images saved by one "Images on this page" request.
const MAX_IMAGES: usize = 60;
/// Files, images and links added together from the boards page.
const MAX_IMPORTS: usize = 40;
const MAX_IMPORT_BYTES: u64 = 20 * 1024 * 1024;
const FILE_NAME_LIMIT: usize = 240;
const MIME_LIMIT: usize = 200;
const PIN_KINDS: [&str; 6] = ["page", "region", "element", "link", "image", "file"];
const PICK_FAILURE: &str = "Couldn't read this page.";

/// Every change reads, edits and writes `boards.json` while holding this lock.
static LOCK: Mutex<()> = Mutex::new(());
static COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BoardFolder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Board {
    pub id: String,
    pub name: String,
    pub folder_id: Option<String>,
    pub cover_pin_id: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Pin {
    pub id: String,
    pub board_id: String,
    pub kind: String,
    pub title: String,
    pub page_url: String,
    pub link_url: String,
    pub text: String,
    pub note: String,
    pub has_image: bool,
    pub width: u32,
    pub height: u32,
    pub created_at: u64,
    /// Original name for a file pin, and for an image added from a file.
    pub file_name: String,
    pub mime: String,
}

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
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BoardsSnapshot {
    pub folders: Vec<BoardFolder>,
    pub boards: Vec<Board>,
    /// Newest first.
    pub pins: Vec<Pin>,
    pub last_board_id: Option<String>,
}

fn new_id(prefix: &str) -> String {
    format!(
        "{prefix}{:x}{:x}",
        tabs::now_ms(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

/// Pin ids name files, so only plain letters and digits are accepted from the UI.
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

fn clip_text(text: &str, limit: usize) -> String {
    text.trim().chars().take(limit).collect()
}

fn clean_name(name: &str, fallback: &str) -> String {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let name = clip_text(&name, NAME_LIMIT);
    if name.is_empty() {
        fallback.to_string()
    } else {
        name
    }
}

impl BoardsSnapshot {
    /// Repairs references an older or hand-edited file may contain. A current file is unchanged.
    pub fn normalize(mut self) -> Self {
        let mut seen = HashSet::new();
        self.folders.retain_mut(|folder| {
            if folder.id.is_empty() {
                folder.id = new_id("f");
            }
            if folder.name.trim().is_empty() {
                folder.name = "Untitled folder".into();
            }
            seen.insert(folder.id.clone())
        });
        let folder_ids: HashSet<String> = self
            .folders
            .iter()
            .map(|folder| folder.id.clone())
            .collect();
        for folder in &mut self.folders {
            if folder
                .parent_id
                .as_ref()
                .is_some_and(|parent| parent == &folder.id || !folder_ids.contains(parent))
            {
                folder.parent_id = None;
            }
        }
        self.break_folder_cycles();

        let mut seen = HashSet::new();
        self.boards.retain_mut(|board| {
            if board.id.is_empty() {
                board.id = new_id("b");
            }
            if board.name.trim().is_empty() {
                board.name = "Untitled board".into();
            }
            if board
                .folder_id
                .as_ref()
                .is_some_and(|folder| !folder_ids.contains(folder))
            {
                board.folder_id = None;
            }
            seen.insert(board.id.clone())
        });

        let board_ids: HashSet<String> = self.boards.iter().map(|board| board.id.clone()).collect();
        let mut seen = HashSet::new();
        let mut orphans = false;
        self.pins.retain_mut(|pin| {
            if pin.id.is_empty() {
                pin.id = new_id("p");
            }
            if !PIN_KINDS.contains(&pin.kind.as_str()) {
                pin.kind = "page".into();
            }
            if !board_ids.contains(&pin.board_id) {
                orphans = true;
            }
            seen.insert(pin.id.clone())
        });
        if orphans {
            let unsorted = self.unsorted_board();
            for pin in &mut self.pins {
                if !board_ids.contains(&pin.board_id) {
                    pin.board_id = unsorted.clone();
                }
            }
        }

        let pin_boards: HashMap<&str, &str> = self
            .pins
            .iter()
            .map(|pin| (pin.id.as_str(), pin.board_id.as_str()))
            .collect();
        for board in &mut self.boards {
            if board
                .cover_pin_id
                .as_deref()
                .is_some_and(|pin| pin_boards.get(pin) != Some(&board.id.as_str()))
            {
                board.cover_pin_id = None;
            }
        }
        if self
            .last_board_id
            .as_ref()
            .is_some_and(|board| !self.boards.iter().any(|item| &item.id == board))
        {
            self.last_board_id = None;
        }
        self
    }

    /// A folder whose parents lead back to itself moves to the top level.
    fn break_folder_cycles(&mut self) {
        let mut parents: HashMap<String, Option<String>> = self
            .folders
            .iter()
            .map(|folder| (folder.id.clone(), folder.parent_id.clone()))
            .collect();
        for folder in &mut self.folders {
            let mut visited = HashSet::new();
            let mut cursor = parents.get(&folder.id).cloned().flatten();
            while let Some(id) = cursor {
                if id == folder.id {
                    folder.parent_id = None;
                    parents.insert(folder.id.clone(), None);
                    break;
                }
                if !visited.insert(id.clone()) {
                    break;
                }
                cursor = parents.get(&id).cloned().flatten();
            }
        }
    }

    fn folder_exists(&self, id: &str) -> bool {
        self.folders.iter().any(|folder| folder.id == id)
    }

    fn board_exists(&self, id: &str) -> bool {
        self.boards.iter().any(|board| board.id == id)
    }

    fn check_folder(&self, folder: Option<&str>) -> Result<(), String> {
        match folder {
            Some(id) if !self.folder_exists(id) => Err("That folder no longer exists.".into()),
            _ => Ok(()),
        }
    }

    /// `id` and every folder inside it.
    fn folder_tree(&self, id: &str) -> HashSet<String> {
        let mut found = HashSet::from([id.to_string()]);
        loop {
            let before = found.len();
            for folder in &self.folders {
                if folder
                    .parent_id
                    .as_ref()
                    .is_some_and(|parent| found.contains(parent))
                {
                    found.insert(folder.id.clone());
                }
            }
            if found.len() == before {
                return found;
            }
        }
    }

    /// The top-level "Unsorted" board, created when it is missing.
    fn unsorted_board(&mut self) -> String {
        if let Some(board) = self
            .boards
            .iter()
            .find(|board| board.folder_id.is_none() && board.name == UNSORTED)
        {
            return board.id.clone();
        }
        let now = tabs::now_ms();
        let board = Board {
            id: new_id("b"),
            name: UNSORTED.into(),
            folder_id: None,
            cover_pin_id: None,
            created_at: now,
            updated_at: now,
        };
        let id = board.id.clone();
        self.boards.push(board);
        id
    }

    /// Board a new pin goes to. An empty request means the last board used, then any board.
    fn board_for_save(&mut self, requested: &str) -> Result<String, String> {
        if !requested.is_empty() {
            return if self.board_exists(requested) {
                Ok(requested.to_string())
            } else {
                Err("That board no longer exists.".into())
            };
        }
        if let Some(last) = self
            .last_board_id
            .clone()
            .filter(|id| self.board_exists(id))
        {
            return Ok(last);
        }
        if let Some(first) = self.boards.first() {
            return Ok(first.id.clone());
        }
        Ok(self.unsorted_board())
    }

    fn add_pin(&mut self, pin: Pin) {
        if let Some(board) = self
            .boards
            .iter_mut()
            .find(|board| board.id == pin.board_id)
        {
            board.updated_at = pin.created_at;
        }
        self.last_board_id = Some(pin.board_id.clone());
        self.pins.insert(0, pin);
    }

    pub fn board_name(&self, id: &str) -> Option<&str> {
        self.boards
            .iter()
            .find(|board| board.id == id)
            .map(|board| board.name.as_str())
    }

    pub fn create_folder(
        &mut self,
        name: &str,
        parent: Option<&str>,
    ) -> Result<BoardFolder, String> {
        self.check_folder(parent)?;
        let folder = BoardFolder {
            id: new_id("f"),
            name: clean_name(name, "New folder"),
            parent_id: parent.map(str::to_string),
            created_at: tabs::now_ms(),
        };
        self.folders.push(folder.clone());
        Ok(folder)
    }

    pub fn rename_folder(&mut self, id: &str, name: &str) -> Result<(), String> {
        let folder = self
            .folders
            .iter_mut()
            .find(|folder| folder.id == id)
            .ok_or("That folder no longer exists.")?;
        folder.name = clean_name(name, &folder.name);
        Ok(())
    }

    pub fn move_folder(&mut self, id: &str, parent: Option<&str>) -> Result<(), String> {
        if !self.folder_exists(id) {
            return Err("That folder no longer exists.".into());
        }
        self.check_folder(parent)?;
        if parent.is_some_and(|parent| self.folder_tree(id).contains(parent)) {
            return Err("A folder can't go inside itself.".into());
        }
        if let Some(folder) = self.folders.iter_mut().find(|folder| folder.id == id) {
            folder.parent_id = parent.map(str::to_string);
        }
        Ok(())
    }

    /// Removes the folder with every folder, board and pin inside it. Returns the removed pin ids.
    pub fn delete_folder(&mut self, id: &str) -> Result<Vec<String>, String> {
        if !self.folder_exists(id) {
            return Err("That folder no longer exists.".into());
        }
        let folders = self.folder_tree(id);
        let boards: HashSet<String> = self
            .boards
            .iter()
            .filter(|board| {
                board
                    .folder_id
                    .as_ref()
                    .is_some_and(|folder| folders.contains(folder))
            })
            .map(|board| board.id.clone())
            .collect();
        self.folders.retain(|folder| !folders.contains(&folder.id));
        self.boards.retain(|board| !boards.contains(&board.id));
        if self
            .last_board_id
            .as_ref()
            .is_some_and(|board| boards.contains(board))
        {
            self.last_board_id = None;
        }
        Ok(self.remove_pins_where(|pin| boards.contains(&pin.board_id)))
    }

    pub fn create_board(&mut self, name: &str, folder: Option<&str>) -> Result<Board, String> {
        self.check_folder(folder)?;
        let now = tabs::now_ms();
        let board = Board {
            id: new_id("b"),
            name: clean_name(name, "New board"),
            folder_id: folder.map(str::to_string),
            cover_pin_id: None,
            created_at: now,
            updated_at: now,
        };
        self.boards.push(board.clone());
        Ok(board)
    }

    fn board_mut(&mut self, id: &str) -> Result<&mut Board, String> {
        self.boards
            .iter_mut()
            .find(|board| board.id == id)
            .ok_or_else(|| "That board no longer exists.".to_string())
    }

    pub fn rename_board(&mut self, id: &str, name: &str) -> Result<(), String> {
        let board = self.board_mut(id)?;
        board.name = clean_name(name, &board.name);
        Ok(())
    }

    pub fn move_board(&mut self, id: &str, folder: Option<&str>) -> Result<(), String> {
        self.check_folder(folder)?;
        self.board_mut(id)?.folder_id = folder.map(str::to_string);
        Ok(())
    }

    pub fn set_cover(&mut self, id: &str, pin: Option<&str>) -> Result<(), String> {
        if let Some(pin) = pin {
            if !self
                .pins
                .iter()
                .any(|item| item.id == pin && item.board_id == id)
            {
                return Err("That pin is not on this board.".into());
            }
        }
        self.board_mut(id)?.cover_pin_id = pin.map(str::to_string);
        Ok(())
    }

    /// Removes the board and its pins. Returns the removed pin ids.
    pub fn delete_board(&mut self, id: &str) -> Result<Vec<String>, String> {
        if !self.board_exists(id) {
            return Err("That board no longer exists.".into());
        }
        self.boards.retain(|board| board.id != id);
        if self.last_board_id.as_deref() == Some(id) {
            self.last_board_id = None;
        }
        Ok(self.remove_pins_where(|pin| pin.board_id == id))
    }

    pub fn update_pin(&mut self, id: &str, title: &str, note: &str) -> Result<(), String> {
        let pin = self
            .pins
            .iter_mut()
            .find(|pin| pin.id == id)
            .ok_or("That pin no longer exists.")?;
        pin.title = clip_text(title, TITLE_LIMIT);
        pin.note = clip_text(note, NOTE_LIMIT);
        Ok(())
    }

    pub fn move_pins(&mut self, ids: &[String], board: &str) -> Result<(), String> {
        if !self.board_exists(board) {
            return Err("That board no longer exists.".into());
        }
        let ids: HashSet<&String> = ids.iter().collect();
        let now = tabs::now_ms();
        let mut moved = HashSet::new();
        for pin in &mut self.pins {
            if ids.contains(&pin.id) && pin.board_id != board {
                moved.insert(pin.board_id.clone());
                pin.board_id = board.to_string();
            }
        }
        for item in &mut self.boards {
            if item.id == board {
                item.updated_at = now;
            }
            if moved.contains(&item.id) {
                item.cover_pin_id = None;
            }
        }
        Ok(())
    }

    pub fn delete_pins(&mut self, ids: &[String]) -> Vec<String> {
        let ids: HashSet<&String> = ids.iter().collect();
        self.remove_pins_where(|pin| ids.contains(&pin.id))
    }

    fn remove_pins_where(&mut self, remove: impl Fn(&Pin) -> bool) -> Vec<String> {
        let mut removed = Vec::new();
        self.pins.retain(|pin| {
            if remove(pin) {
                removed.push(pin.id.clone());
                false
            } else {
                true
            }
        });
        for board in &mut self.boards {
            if board
                .cover_pin_id
                .as_ref()
                .is_some_and(|pin| removed.contains(pin))
            {
                board.cover_pin_id = None;
            }
        }
        removed
    }
}

fn store_path(app: &AppHandle) -> PathBuf {
    paths::data_file(app, FILE)
}

fn image_dir(app: &AppHandle) -> PathBuf {
    paths::data_file(app, IMAGE_DIR)
}

fn image_path(app: &AppHandle, id: &str) -> PathBuf {
    image_dir(app).join(format!("{id}.png"))
}

fn thumb_path(app: &AppHandle, id: &str) -> PathBuf {
    image_dir(app).join(format!("{id}.thumb.jpg"))
}

fn file_path(app: &AppHandle, id: &str) -> PathBuf {
    image_dir(app).join(format!("{id}.bin"))
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Call while holding `LOCK`. A repaired file is written back, so ids made during repair (such as
/// the "Unsorted" board) stay the same on the next read.
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

fn remove_images(app: &AppHandle, ids: &[String]) {
    for id in ids.iter().filter(|id| valid_id(id)) {
        let _ = fs::remove_file(image_path(app, id));
        let _ = fs::remove_file(thumb_path(app, id));
        let _ = fs::remove_file(file_path(app, id));
    }
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

pub fn delete_folder(app: &AppHandle, id: &str) -> Result<(), String> {
    let removed = change(app, |boards| boards.delete_folder(id))?;
    remove_images(app, &removed);
    Ok(())
}

pub fn create_board(app: &AppHandle, name: &str, folder: Option<&str>) -> Result<Board, String> {
    change(app, |boards| boards.create_board(name, folder))
}

pub fn rename_board(app: &AppHandle, id: &str, name: &str) -> Result<(), String> {
    change(app, |boards| boards.rename_board(id, name))
}

pub fn move_board(app: &AppHandle, id: &str, folder: Option<&str>) -> Result<(), String> {
    change(app, |boards| boards.move_board(id, folder))
}

pub fn set_cover(app: &AppHandle, id: &str, pin: Option<&str>) -> Result<(), String> {
    change(app, |boards| boards.set_cover(id, pin))
}

pub fn delete_board(app: &AppHandle, id: &str) -> Result<(), String> {
    let removed = change(app, |boards| boards.delete_board(id))?;
    remove_images(app, &removed);
    Ok(())
}

pub fn update_pin(app: &AppHandle, id: &str, title: &str, note: &str) -> Result<(), String> {
    change(app, |boards| boards.update_pin(id, title, note))
}

pub fn move_pins(app: &AppHandle, ids: &[String], board: &str) -> Result<(), String> {
    change(app, |boards| boards.move_pins(ids, board))
}

pub fn delete_pins(app: &AppHandle, ids: &[String]) -> Result<(), String> {
    let removed = change(app, |boards| Ok(boards.delete_pins(ids)))?;
    remove_images(app, &removed);
    Ok(())
}

/// JPEG for the grid: at most `THUMB_WIDTH` wide, only the top of very tall images, and
/// transparent pixels drawn on white.
pub fn thumbnail_jpeg(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let (width, height) = image.dimensions();
    let width = width.max(1);
    let target_width = width.min(THUMB_WIDTH);
    let scale = f64::from(target_width) / f64::from(width);
    let crop_height =
        ((f64::from(THUMB_MAX_HEIGHT) / scale).floor() as u32).clamp(1, height.max(1));
    let cropped = image::imageops::crop_imm(image, 0, 0, width, crop_height).to_image();
    let target_height = ((f64::from(crop_height) * scale).round() as u32).max(1);
    let resized =
        image::imageops::resize(&cropped, target_width, target_height, FilterType::Triangle);
    let flat = RgbImage::from_fn(resized.width(), resized.height(), |x, y| {
        let Rgba([red, green, blue, alpha]) = *resized.get_pixel(x, y);
        let alpha = u32::from(alpha);
        let blend = |channel: u8| ((u32::from(channel) * alpha + 255 * (255 - alpha)) / 255) as u8;
        Rgb([blend(red), blend(green), blend(blue)])
    });
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 82)
        .encode_image(&flat)
        .map_err(|error| format!("Couldn't make a thumbnail. {error}"))?;
    Ok(out)
}

fn write_image(app: &AppHandle, id: &str, image: &RgbaImage) -> Result<(), String> {
    let dir = image_dir(app);
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Couldn't create {}. {error}", dir.display()))?;
    let png = capture::encode_png(image)?;
    fs::write(image_path(app, id), png)
        .map_err(|error| format!("Couldn't save the image. {error}"))?;
    fs::write(thumb_path(app, id), thumbnail_jpeg(image)?)
        .map_err(|error| format!("Couldn't save the thumbnail. {error}"))
}

fn write_file(app: &AppHandle, id: &str, bytes: &[u8]) -> Result<(), String> {
    let dir = image_dir(app);
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Couldn't create {}. {error}", dir.display()))?;
    fs::write(file_path(app, id), bytes).map_err(|error| format!("Couldn't save the file. {error}"))
}

struct PinDraft {
    kind: &'static str,
    title: String,
    page_url: String,
    link_url: String,
    text: String,
    file_name: String,
    mime: String,
}

fn pin_draft(
    kind: &'static str,
    title: String,
    page_url: String,
    link_url: String,
    text: String,
) -> PinDraft {
    PinDraft {
        kind,
        title,
        page_url,
        link_url,
        text,
        file_name: String::new(),
        mime: String::new(),
    }
}

fn save_pin(
    app: &AppHandle,
    board_id: &str,
    draft: PinDraft,
    image: Option<&RgbaImage>,
    file_bytes: Option<&[u8]>,
) -> Result<Pin, String> {
    let id = new_id("p");
    if let Some(image) = image {
        if let Err(error) = write_image(app, &id, image) {
            remove_images(app, std::slice::from_ref(&id));
            return Err(error);
        }
    }
    if let Some(bytes) = file_bytes {
        if let Err(error) = write_file(app, &id, bytes) {
            remove_images(app, std::slice::from_ref(&id));
            return Err(error);
        }
    }
    let pin = Pin {
        id: id.clone(),
        board_id: String::new(),
        kind: draft.kind.into(),
        title: clip_text(&draft.title, TITLE_LIMIT),
        page_url: draft.page_url,
        link_url: draft.link_url,
        text: clip_text(&draft.text, TEXT_LIMIT),
        note: String::new(),
        has_image: image.is_some(),
        width: image.map(RgbaImage::width).unwrap_or(0),
        height: image.map(RgbaImage::height).unwrap_or(0),
        created_at: tabs::now_ms(),
        file_name: clip_text(&draft.file_name, FILE_NAME_LIMIT),
        mime: clip_text(&draft.mime, MIME_LIMIT),
    };
    let result = change(app, |boards| {
        let mut pin = pin;
        pin.board_id = boards.board_for_save(board_id)?;
        boards.add_pin(pin.clone());
        Ok(pin)
    });
    if result.is_err() {
        remove_images(app, &[id]);
    }
    result
}

/// Title and address of tab `id`.
fn tab_page(app: &AppHandle, id: u32) -> (String, String) {
    tabs::snapshot(app)
        .tabs
        .into_iter()
        .find(|tab| tab.id == id)
        .map(|tab| (tab.title, tab.url))
        .unwrap_or_default()
}

fn decode_image(bytes: &[u8]) -> Result<RgbaImage, String> {
    image::load_from_memory(bytes)
        .map(|image| image.to_rgba8())
        .map_err(|error| format!("That image format isn't supported. {error}"))
}

fn text_of(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

pub fn pin_page(app: &AppHandle, id: u32, board_id: &str, full_page: bool) -> Result<Pin, String> {
    let (title, url) = tab_page(app, id);
    let (image, _) = capture::page_image(app, id, full_page, full_page)?;
    let draft = pin_draft("page", title, url.clone(), url, String::new());
    save_pin(app, board_id, draft, Some(&image), None)
}

pub fn pin_region(app: &AppHandle, id: u32, board_id: &str) -> Result<Pin, String> {
    let (title, url) = tab_page(app, id);
    let image = capture::region_image(app, id)?;
    let draft = pin_draft("region", title, url.clone(), url, String::new());
    save_pin(app, board_id, draft, Some(&image), None)
}

pub fn pin_element(app: &AppHandle, id: u32, board_id: &str) -> Result<Pin, String> {
    let (title, url) = tab_page(app, id);
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let value = webview2::evaluate(
        &webview,
        ELEMENT_SCRIPT,
        Duration::from_secs(150),
        PICK_FAILURE,
    )?;
    let number = |key: &str| value.get(key).and_then(|item| item.as_f64());
    let (Some(x), Some(y), Some(width), Some(height)) =
        (number("x"), number("y"), number("width"), number("height"))
    else {
        return Err("The capture was cancelled.".into());
    };
    if width < 1.0 || height < 1.0 {
        return Err("That section has no size.".into());
    }
    let image = capture::clip_image(app, id, x, y, width, height)?;
    let heading = text_of(&value, "heading");
    let href = text_of(&value, "href");
    let draft = pin_draft(
        "element",
        if heading.is_empty() { title } else { heading },
        url.clone(),
        if href.is_empty() { url } else { href },
        text_of(&value, "text"),
    );
    save_pin(app, board_id, draft, Some(&image), None)
}

pub fn pin_link(app: &AppHandle, id: u32, board_id: &str) -> Result<Pin, String> {
    let (tab_title, url) = tab_page(app, id);
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let value = webview2::evaluate(&webview, LINK_SCRIPT, Duration::from_secs(10), PICK_FAILURE)?;
    let preview = text_of(&value, "image");
    let image = (!preview.is_empty())
        .then(|| {
            page_menu::image_bytes(&preview)
                .and_then(|bytes| decode_image(&bytes))
                .ok()
        })
        .flatten();
    let image = match image {
        Some(image) => image,
        None => capture::page_image(app, id, false, false)?.0,
    };
    let title = text_of(&value, "title");
    let link = text_of(&value, "url");
    let draft = pin_draft(
        "link",
        if title.is_empty() { tab_title } else { title },
        url.clone(),
        if link.is_empty() { url } else { link },
        text_of(&value, "description"),
    );
    save_pin(app, board_id, draft, Some(&image), None)
}

/// Downloads each image and pins it. Fails only when none could be saved.
pub fn pin_images(
    app: &AppHandle,
    id: u32,
    board_id: &str,
    urls: &[String],
) -> Result<Vec<Pin>, String> {
    let (title, url) = tab_page(app, id);
    let mut seen = HashSet::new();
    let mut board = board_id.to_string();
    let mut pins = Vec::new();
    let mut first_error = None;
    for source in urls
        .iter()
        .map(|source| source.trim())
        .filter(|source| !source.is_empty() && seen.insert(source.to_string()))
        .take(MAX_IMAGES)
    {
        let saved = page_menu::image_bytes(source)
            .and_then(|bytes| decode_image(&bytes))
            .and_then(|image| {
                let draft = pin_draft(
                    "image",
                    title.clone(),
                    url.clone(),
                    source.to_string(),
                    String::new(),
                );
                save_pin(app, &board, draft, Some(&image), None)
            });
        match saved {
            Ok(pin) => {
                board = pin.board_id.clone();
                pins.push(pin);
            }
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    if pins.is_empty() {
        return Err(first_error.unwrap_or_else(|| "Choose at least one image.".into()));
    }
    Ok(pins)
}

/// "Save image to board" in the page menu. Saves to the last board used. Runs on a worker thread.
pub fn save_menu_image(app: &AppHandle, id: u32, source: &str) {
    let window = tabs::tab_window_label(app, id).unwrap_or_else(|| tabs::WINDOW.to_string());
    let message = match pin_images(app, id, "", &[source.to_string()]) {
        Ok(pins) => {
            let boards = snapshot(app);
            let name = pins
                .first()
                .and_then(|pin| boards.board_name(&pin.board_id))
                .unwrap_or(UNSORTED);
            format!("Saved to {name}")
        }
        Err(error) => error,
    };
    undo::notice(app, &window, &message);
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn read_png(app: &AppHandle, id: &str) -> Result<Vec<u8>, String> {
    if !valid_id(id) {
        return Err("That pin no longer exists.".into());
    }
    fs::read(image_path(app, id)).map_err(|_| "This pin has no image.".to_string())
}

/// Thumbnail data URLs by pin id. Pins without an image are left out. A missing thumbnail is
/// made again from the full image.
pub fn thumbnails(app: &AppHandle, ids: &[String]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for id in ids.iter().filter(|id| valid_id(id)) {
        let path = thumb_path(app, id);
        let bytes = fs::read(&path).ok().or_else(|| {
            let png = fs::read(image_path(app, id)).ok()?;
            let image = image::load_from_memory_with_format(&png, ImageFormat::Png)
                .ok()?
                .to_rgba8();
            let jpeg = thumbnail_jpeg(&image).ok()?;
            let _ = fs::write(&path, &jpeg);
            Some(jpeg)
        });
        if let Some(bytes) = bytes {
            out.insert(id.clone(), data_url("image/jpeg", &bytes));
        }
    }
    out
}

pub fn image_data_url(app: &AppHandle, id: &str) -> Result<String, String> {
    Ok(data_url("image/png", &read_png(app, id)?))
}

pub fn copy_image(app: &AppHandle, id: &str) -> Result<(), String> {
    capture::copy_edited_png(&read_png(app, id)?)
}

pub fn save_image_as(app: &AppHandle, id: &str, path: &Path) -> Result<(), String> {
    capture::save_edited_png(&read_png(app, id)?, path)
}

fn extension_of(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default()
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
    let stem = name
        .rsplit_once('.')
        .map(|(stem, _)| stem.trim())
        .filter(|stem| !stem.is_empty())
        .unwrap_or(name.as_str());
    stem.to_string()
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
        extension_of(name).as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
    )
}

fn image_mime(name: &str, mime: &str) -> String {
    let mime = mime.trim();
    if mime.to_ascii_lowercase().starts_with("image/") {
        return mime.to_string();
    }
    match extension_of(name).as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    }
    .to_string()
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

fn read_import_bytes(item: &BoardImportItem) -> Result<Vec<u8>, String> {
    if !item.path.trim().is_empty() {
        let path = Path::new(item.path.trim());
        let meta =
            fs::metadata(path).map_err(|error| format!("Couldn't read that file. {error}"))?;
        if !meta.is_file() {
            return Err("That path is not a file.".into());
        }
        if meta.len() > MAX_IMPORT_BYTES {
            return Err("That file is larger than 20 MB.".into());
        }
        if meta.len() == 0 {
            return Err("That file is empty.".into());
        }
        return fs::read(path).map_err(|error| format!("Couldn't read that file. {error}"));
    }
    let data = item.data.trim();
    if data.is_empty() {
        return Err("That file is empty.".into());
    }
    if data.len() > (MAX_IMPORT_BYTES as usize) / 3 * 4 + 8 {
        return Err("That file is larger than 20 MB.".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| "Couldn't read that file.".to_string())?;
    if bytes.is_empty() {
        return Err("That file is empty.".into());
    }
    if bytes.len() as u64 > MAX_IMPORT_BYTES {
        return Err("That file is larger than 20 MB.".into());
    }
    Ok(bytes)
}

fn import_one(app: &AppHandle, board_id: &str, item: &BoardImportItem) -> Result<Pin, String> {
    let has_bytes = !item.data.trim().is_empty() || !item.path.trim().is_empty();
    if !has_bytes {
        let url = http_url(&item.url)
            .ok_or_else(|| "That link needs to start with http:// or https://.".to_string())?;
        return save_pin(
            app,
            board_id,
            pin_draft("link", url.clone(), url.clone(), url, String::new()),
            None,
            None,
        );
    }
    let bytes = read_import_bytes(item)?;
    let name = import_name(item);
    let mime = item.mime.trim().to_string();
    if looks_like_image(&name, &mime) {
        if let Ok(image) = decode_image(&bytes) {
            let mut draft = pin_draft(
                "image",
                title_from_name(&name),
                String::new(),
                String::new(),
                String::new(),
            );
            draft.file_name = name;
            draft.mime = image_mime(&draft.file_name, &mime);
            return save_pin(app, board_id, draft, Some(&image), None);
        }
    }
    let mut draft = pin_draft(
        "file",
        title_from_name(&name),
        String::new(),
        String::new(),
        String::new(),
    );
    draft.file_name = name;
    draft.mime = if mime.is_empty() {
        "application/octet-stream".into()
    } else {
        mime
    };
    save_pin(app, board_id, draft, None, Some(&bytes))
}

/// Saves files, images and links onto a board. An empty `board_id` uses the last board.
/// Fails only when nothing could be saved. At most `MAX_IMPORTS` items are kept.
pub fn import_items(
    app: &AppHandle,
    board_id: &str,
    items: &[BoardImportItem],
) -> Result<Vec<Pin>, String> {
    let mut board = board_id.to_string();
    let mut pins = Vec::new();
    let mut first_error = None;
    for item in items
        .iter()
        .filter(|item| !item_is_empty(item))
        .take(MAX_IMPORTS)
    {
        match import_one(app, &board, item) {
            Ok(pin) => {
                board = pin.board_id.clone();
                pins.push(pin);
            }
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    if pins.is_empty() {
        return Err(first_error.unwrap_or_else(|| "Add a file or a link.".into()));
    }
    Ok(pins)
}

/// Saves whatever is on the clipboard onto a board. An image wins over a link copied with it.
/// An empty `board_id` uses the last board.
pub fn paste_clipboard(app: &AppHandle, board_id: &str) -> Result<Vec<Pin>, String> {
    import_items(app, board_id, &clipboard_import_items()?)
}

fn clipboard_import_items() -> Result<Vec<BoardImportItem>, String> {
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
        RegisterClipboardFormatW,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use windows::Win32::System::Ole::{CF_DIB, CF_HDROP, CF_UNICODETEXT};
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
    use windows::core::w;

    unsafe {
        OpenClipboard(None).map_err(|_| "The clipboard is busy. Try again.".to_string())?;
        let result = (|| {
            if format_ready(u32::from(CF_HDROP.0)) {
                let files = hdrop_files();
                if !files.is_empty() {
                    return Ok(files);
                }
            }
            let png_format = RegisterClipboardFormatW(w!("PNG"));
            if png_format != 0
                && format_ready(png_format)
                && let Some(bytes) = clipboard_bytes(png_format)
                && let Ok(item) = image_item(bytes)
            {
                return Ok(vec![item]);
            }
            for format in [u32::from(CF_DIB.0), 17u32] {
                if !format_ready(format) {
                    continue;
                }
                if let Some(bytes) = clipboard_bytes(format)
                    && let Ok(png) = png_from_dib(&bytes)
                    && let Ok(item) = image_item(png)
                {
                    return Ok(vec![item]);
                }
            }
            if format_ready(u32::from(CF_UNICODETEXT.0))
                && let Some(bytes) = clipboard_bytes(u32::from(CF_UNICODETEXT.0))
            {
                let links = links_in_text(&clipboard_text(&bytes));
                if !links.is_empty() {
                    return Ok(links.into_iter().take(MAX_IMPORTS).map(link_item).collect());
                }
            }
            Err("Copy an image, file, or link first.".into())
        })();
        let _ = CloseClipboard();
        return result;
    }

    fn format_ready(format: u32) -> bool {
        unsafe { IsClipboardFormatAvailable(format).is_ok() }
    }

    fn clipboard_bytes(format: u32) -> Option<Vec<u8>> {
        unsafe {
            let handle = GetClipboardData(format).ok()?;
            let memory = windows::Win32::Foundation::HGLOBAL(handle.0);
            let pointer = GlobalLock(memory);
            if pointer.is_null() {
                return None;
            }
            let size = GlobalSize(memory);
            let bytes = std::slice::from_raw_parts(pointer.cast::<u8>(), size).to_vec();
            let _ = GlobalUnlock(memory);
            Some(bytes)
        }
    }

    fn hdrop_files() -> Vec<BoardImportItem> {
        let Some(handle) = (unsafe { GetClipboardData(u32::from(CF_HDROP.0)).ok() }) else {
            return Vec::new();
        };
        let drop = HDROP(handle.0);
        let count = unsafe { DragQueryFileW(drop, 0xFFFF_FFFF, None) };
        let mut items = Vec::new();
        for index in 0..count {
            let length = unsafe { DragQueryFileW(drop, index, None) } as usize;
            if length == 0 {
                continue;
            }
            let mut buffer = vec![0u16; length + 1];
            unsafe { DragQueryFileW(drop, index, Some(&mut buffer)) };
            let path = PathBuf::from(String::from_utf16_lossy(&buffer[..length]));
            if path.is_file() {
                items.push(BoardImportItem {
                    path: path.to_string_lossy().into_owned(),
                    ..BoardImportItem::default()
                });
            }
            if items.len() == MAX_IMPORTS {
                break;
            }
        }
        items
    }
}

fn image_item(png: Vec<u8>) -> Result<BoardImportItem, String> {
    if png.is_empty() {
        return Err("That image is empty.".into());
    }
    if png.len() as u64 > MAX_IMPORT_BYTES {
        return Err("That image is larger than 20 MB.".into());
    }
    // A PNG copied from another app is already encoded. A bitmap is converted before this.
    if image::load_from_memory(&png).is_err() {
        return Err("That image format isn't supported.".into());
    }
    Ok(BoardImportItem {
        name: "Pasted image.png".into(),
        mime: "image/png".into(),
        data: base64::engine::general_purpose::STANDARD.encode(png),
        ..BoardImportItem::default()
    })
}

fn link_item(url: String) -> BoardImportItem {
    BoardImportItem {
        url,
        ..BoardImportItem::default()
    }
}

fn clipboard_text(bytes: &[u8]) -> String {
    let wide: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    String::from_utf16_lossy(&wide)
}

fn links_in_text(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for token in text.split_whitespace() {
        let token = token.trim_matches(|character: char| {
            matches!(
                character,
                '<' | '>' | '"' | '\'' | '(' | ')' | ',' | ';' | '.'
            )
        });
        let lower = token.to_ascii_lowercase();
        if (lower.starts_with("http://") || lower.starts_with("https://"))
            && !urls.iter().any(|url| url == token)
        {
            urls.push(token.to_string());
        }
    }
    urls
}

/// Turns a device-independent bitmap from the clipboard into a PNG.
fn png_from_dib(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Graphics::Gdi::BITMAPINFOHEADER;

    let header_size = size_of::<BITMAPINFOHEADER>();
    if bytes.len() < header_size {
        return Err("That image isn't a format the clipboard can paste.".into());
    }
    let header = unsafe { bytes.as_ptr().cast::<BITMAPINFOHEADER>().read_unaligned() };
    let declared = header.biSize as usize;
    if declared < header_size || declared > bytes.len() {
        return Err("That image isn't a format the clipboard can paste.".into());
    }
    let width = header.biWidth.unsigned_abs();
    let top_down = header.biHeight < 0;
    let height = header.biHeight.unsigned_abs();
    if width == 0 || height == 0 || width > 16_000 || height > 16_000 {
        return Err("That image isn't a format the clipboard can paste.".into());
    }
    let bpp = header.biBitCount;
    if bpp != 24 && bpp != 32 {
        return Err("That image isn't a format the clipboard can paste.".into());
    }
    // BI_RGB is 0. BI_BITFIELDS is 3 and, for a 40-byte header, the masks follow it.
    let pixels_at = if header.biCompression == 3 && declared == header_size {
        declared + 12
    } else if header.biCompression == 0 || header.biCompression == 3 {
        declared
    } else {
        return Err("That image isn't a format the clipboard can paste.".into());
    };
    let channels = bpp as usize / 8;
    let stride = (width as usize * channels).div_ceil(4) * 4;
    let needed = pixels_at + stride * height as usize;
    if bytes.len() < needed {
        return Err("That image isn't a format the clipboard can paste.".into());
    }
    let mut image = RgbaImage::new(width, height);
    for y in 0..height {
        let source_y = if top_down { y } else { height - 1 - y };
        let row = pixels_at + source_y as usize * stride;
        for x in 0..width {
            let index = row + x as usize * channels;
            let blue = bytes[index];
            let green = bytes[index + 1];
            let red = bytes[index + 2];
            let alpha = if channels == 4 { bytes[index + 3] } else { 255 };
            image.put_pixel(x, y, Rgba([red, green, blue, alpha]));
        }
    }
    if channels == 4 && image.pixels().all(|pixel| pixel[3] == 0) {
        for pixel in image.pixels_mut() {
            pixel[3] = 255;
        }
    }
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|error| format!("Couldn't read the copied image. {error}"))?;
    Ok(png)
}

fn read_pin_file(app: &AppHandle, id: &str) -> Result<(String, Vec<u8>), String> {
    if !valid_id(id) {
        return Err("That pin no longer exists.".into());
    }
    let boards = snapshot(app);
    let pin = boards
        .pins
        .iter()
        .find(|pin| pin.id == id)
        .ok_or("That pin no longer exists.")?;
    if pin.kind != "file" {
        return Err("This pin is not a file.".into());
    }
    let name = pin.file_name.clone();
    let bytes = fs::read(file_path(app, id)).map_err(|_| "This pin has no file.".to_string())?;
    Ok((name, bytes))
}

/// Copies the file to a temp name with its original extension, then opens it.
pub fn open_pin_file(app: &AppHandle, id: &str) -> Result<(), String> {
    let (file_name, bytes) = read_pin_file(app, id)?;
    let dir = std::env::temp_dir().join("ce-browser-boards").join(id);
    fs::create_dir_all(&dir).map_err(|error| format!("Couldn't open the file. {error}"))?;
    let path = dir.join(safe_file_name(&file_name));
    fs::write(&path, bytes).map_err(|error| format!("Couldn't open the file. {error}"))?;
    files::open_with_default(&path)
}

pub fn save_file_as(app: &AppHandle, id: &str, path: &Path) -> Result<(), String> {
    let (_, bytes) = read_pin_file(app, id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("Couldn't save the file. {error}"))?;
    }
    fs::write(path, bytes).map_err(|error| format!("Couldn't save the file. {error}"))
}

/// Outlines the element under the mouse. Click picks it, Alt+wheel or `[` / `]` picks the
/// parent or child, Esc cancels. Resolves with the element's rect in document CSS pixels.
const ELEMENT_SCRIPT: &str = r##"(function () {
  return new Promise(function (resolve) {
    var root = document.documentElement;
    var font = "font:12px/1.4 'Segoe UI',system-ui,sans-serif;";
    var box = document.createElement("div");
    box.style.cssText = "position:fixed;z-index:2147483647;pointer-events:none;display:none;border:2px solid #38bdf8;background:rgba(56,189,248,.18);border-radius:3px;box-sizing:border-box";
    var label = document.createElement("div");
    label.style.cssText = "position:fixed;z-index:2147483647;pointer-events:none;display:none;background:#0c4a6e;color:#e0f2fe;padding:2px 6px;border-radius:4px;white-space:nowrap;" + font;
    var hint = document.createElement("div");
    hint.textContent = "Click a section to save it \u00b7 Alt+scroll or [ ] to grow or shrink \u00b7 Esc to cancel";
    hint.style.cssText = "position:fixed;left:50%;top:12px;transform:translateX(-50%);z-index:2147483647;pointer-events:none;background:rgba(9,9,11,.92);color:#fafafa;padding:6px 12px;border-radius:8px;box-shadow:0 4px 16px rgba(0,0,0,.4);" + font;
    root.appendChild(box);
    root.appendChild(label);
    root.appendChild(hint);
    var previousCursor = root.style.cursor;
    root.style.cursor = "crosshair";
    var hovered = null;
    var current = null;
    var children = [];
    var timer = setTimeout(function () { finish(null); }, 140000);

    function show(element) {
      current = element;
      if (!element) {
        box.style.display = "none";
        label.style.display = "none";
        return;
      }
      var rect = element.getBoundingClientRect();
      box.style.display = "block";
      box.style.left = rect.left + "px";
      box.style.top = rect.top + "px";
      box.style.width = rect.width + "px";
      box.style.height = rect.height + "px";
      var name = element.tagName.toLowerCase() + (element.id ? "#" + element.id : "");
      label.textContent = name + "  " + Math.round(rect.width) + " \u00d7 " + Math.round(rect.height);
      label.style.display = "block";
      label.style.left = Math.max(0, rect.left) + "px";
      label.style.top = (rect.top > 24 ? rect.top - 22 : rect.top + 2) + "px";
    }
    function grow() {
      if (current && current.parentElement && current.parentElement !== root) {
        children.push(current);
        show(current.parentElement);
      }
    }
    function shrink() {
      if (children.length) show(children.pop());
    }
    function onMove(event) {
      var element = document.elementFromPoint(event.clientX, event.clientY);
      if (!element || element === root) return;
      if (element !== hovered) {
        hovered = element;
        children = [];
        show(element);
      }
    }
    function onWheel(event) {
      if (!event.altKey) return;
      event.preventDefault();
      event.stopPropagation();
      if (event.deltaY < 0) grow();
      else shrink();
    }
    function block(event) {
      event.preventDefault();
      event.stopPropagation();
    }
    function onClick(event) {
      block(event);
      if (!current) return;
      var element = current;
      var rect = element.getBoundingClientRect();
      var heading = element.matches("h1,h2,h3,h4") ? element : element.querySelector("h1,h2,h3,h4");
      var link = element.closest("a[href]") || element.querySelector("a[href]");
      finish({
        x: rect.left + window.scrollX,
        y: rect.top + window.scrollY,
        width: rect.width,
        height: rect.height,
        text: (element.innerText || "").replace(/\n{3,}/g, "\n\n").trim().slice(0, 2000),
        heading: heading ? (heading.innerText || "").trim().slice(0, 300) : "",
        href: link ? link.href : ""
      });
    }
    function onKey(event) {
      if (event.key === "Escape") {
        block(event);
        finish(null);
      } else if (event.key === "[" || event.key === "ArrowUp") {
        block(event);
        grow();
      } else if (event.key === "]" || event.key === "ArrowDown") {
        block(event);
        shrink();
      }
    }
    function onScroll() {
      show(current);
    }
    function finish(value) {
      clearTimeout(timer);
      document.removeEventListener("mousemove", onMove, true);
      document.removeEventListener("wheel", onWheel, { capture: true });
      document.removeEventListener("mousedown", block, true);
      document.removeEventListener("mouseup", block, true);
      document.removeEventListener("click", onClick, true);
      document.removeEventListener("keydown", onKey, true);
      window.removeEventListener("scroll", onScroll, true);
      box.remove();
      label.remove();
      hint.remove();
      root.style.cursor = previousCursor;
      requestAnimationFrame(function () {
        requestAnimationFrame(function () {
          setTimeout(function () { resolve(value); }, 40);
        });
      });
    }
    document.addEventListener("mousemove", onMove, true);
    document.addEventListener("wheel", onWheel, { capture: true, passive: false });
    document.addEventListener("mousedown", block, true);
    document.addEventListener("mouseup", block, true);
    document.addEventListener("click", onClick, true);
    document.addEventListener("keydown", onKey, true);
    window.addEventListener("scroll", onScroll, true);
  });
})()"##;

/// Title, description, preview image and canonical address for a link card.
const LINK_SCRIPT: &str = r#"(function () {
  function meta(selector) {
    var element = document.querySelector(selector);
    return element ? (element.getAttribute("content") || "").trim() : "";
  }
  function absolute(value) {
    try { return value ? new URL(value, location.href).href : ""; } catch (error) { return ""; }
  }
  var canonical = document.querySelector('link[rel="canonical"]');
  var image = meta('meta[property="og:image"]') || meta('meta[property="og:image:url"]') ||
    meta('meta[name="twitter:image"]') || meta('meta[name="twitter:image:src"]');
  return {
    title: meta('meta[property="og:title"]') || meta('meta[name="twitter:title"]') || document.title || "",
    description: meta('meta[property="og:description"]') || meta('meta[name="description"]') || meta('meta[name="twitter:description"]'),
    image: absolute(image),
    url: absolute(canonical ? canonical.getAttribute("href") : "") || location.href
  };
})()"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> BoardsSnapshot {
        let mut boards = BoardsSnapshot::default();
        let outer = boards.create_folder("Outer", None).expect("outer");
        let inner = boards
            .create_folder("Inner", Some(&outer.id))
            .expect("inner");
        let kept = boards.create_board("Kept", None).expect("kept");
        let nested = boards
            .create_board("Nested", Some(&inner.id))
            .expect("nested");
        for (id, board) in [("p1", &kept.id), ("p2", &nested.id), ("p3", &nested.id)] {
            boards.add_pin(Pin {
                id: id.into(),
                board_id: board.clone(),
                kind: "page".into(),
                ..Pin::default()
            });
        }
        boards
    }

    #[test]
    fn deleting_a_folder_removes_everything_inside_it() {
        let mut boards = sample();
        let outer = boards.folders[0].id.clone();
        let mut removed = boards.delete_folder(&outer).expect("delete");
        removed.sort();
        assert_eq!(removed, vec!["p2".to_string(), "p3".to_string()]);
        assert!(boards.folders.is_empty());
        assert_eq!(boards.boards.len(), 1);
        assert_eq!(boards.pins.len(), 1);
        assert_eq!(boards.last_board_id, None);
    }

    #[test]
    fn a_folder_cannot_move_inside_itself() {
        let mut boards = sample();
        let outer = boards.folders[0].id.clone();
        let inner = boards.folders[1].id.clone();
        assert!(boards.move_folder(&outer, Some(&inner)).is_err());
        assert!(boards.move_folder(&outer, Some(&outer)).is_err());
        assert!(boards.move_folder(&inner, None).is_ok());
        assert!(boards.move_folder(&outer, Some(&inner)).is_ok());
    }

    #[test]
    fn loading_repairs_missing_references() {
        let mut boards = BoardsSnapshot::default();
        boards.folders.push(BoardFolder {
            id: "a".into(),
            name: "A".into(),
            parent_id: Some("b".into()),
            created_at: 0,
        });
        boards.folders.push(BoardFolder {
            id: "b".into(),
            name: "B".into(),
            parent_id: Some("a".into()),
            created_at: 0,
        });
        boards.folders.push(BoardFolder {
            id: "c".into(),
            name: "C".into(),
            parent_id: Some("gone".into()),
            created_at: 0,
        });
        boards.boards.push(Board {
            id: "x".into(),
            name: "X".into(),
            folder_id: Some("gone".into()),
            ..Board::default()
        });
        boards.pins.push(Pin {
            id: "p".into(),
            board_id: "gone".into(),
            kind: "hologram".into(),
            ..Pin::default()
        });
        boards.last_board_id = Some("gone".into());
        let boards = boards.normalize();
        let parent = |id: &str| {
            boards
                .folders
                .iter()
                .find(|folder| folder.id == id)
                .and_then(|folder| folder.parent_id.clone())
        };
        assert!(parent("a").is_none() || parent("b").is_none());
        assert_eq!(parent("c"), None);
        assert_eq!(boards.boards[0].folder_id, None);
        let pin = &boards.pins[0];
        assert_eq!(pin.kind, "page");
        assert_eq!(boards.board_name(&pin.board_id), Some(UNSORTED));
        assert_eq!(boards.last_board_id, None);
    }

    #[test]
    fn a_current_file_loads_unchanged() {
        let boards = sample();
        assert_eq!(boards.clone().normalize(), boards);
    }

    #[test]
    fn an_older_file_with_missing_keys_still_loads() {
        let boards: BoardsSnapshot = serde_json::from_str(
            r#"{ "boards": [{ "id": "b1", "name": "Ideas" }], "pins": [{ "id": "p1", "boardId": "b1" }], "retired": true }"#,
        )
        .expect("parse");
        let boards = boards.normalize();
        assert!(boards.folders.is_empty());
        assert_eq!(boards.pins[0].kind, "page");
        assert_eq!(boards.pins[0].board_id, "b1");
        assert_eq!(boards.pins[0].file_name, "");
        assert_eq!(boards.pins[0].mime, "");
        assert_eq!(boards.boards[0].folder_id, None);
    }

    #[test]
    fn saving_without_a_board_uses_the_last_one_or_makes_unsorted() {
        let mut empty = BoardsSnapshot::default();
        let id = empty.board_for_save("").expect("board");
        assert_eq!(empty.board_name(&id), Some(UNSORTED));
        let mut boards = sample();
        let last = boards.last_board_id.clone().expect("last");
        assert_eq!(boards.board_for_save("").expect("board"), last);
        assert!(boards.board_for_save("missing").is_err());
    }

    #[test]
    fn a_file_pin_keeps_its_kind_when_new_fields_are_missing() {
        let boards: BoardsSnapshot = serde_json::from_str(
            r#"{ "boards": [{ "id": "b1", "name": "Ideas" }], "pins": [{ "id": "p1", "boardId": "b1", "kind": "file" }] }"#,
        )
        .expect("parse");
        let boards = boards.normalize();
        assert_eq!(boards.pins[0].kind, "file");
        assert_eq!(boards.pins[0].file_name, "");
        assert_eq!(boards.pins[0].mime, "");
    }

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
    }

    #[test]
    fn clipboard_text_keeps_links_and_a_bitmap_becomes_a_png() {
        assert_eq!(
            links_in_text("see https://example.com/a, and https://example.com/a again"),
            vec!["https://example.com/a".to_string()]
        );
        assert!(links_in_text("just a note").is_empty());
        let mut bytes = vec![0u8; 40 + 8];
        bytes[0] = 40;
        bytes[4] = 2;
        bytes[8] = 1;
        bytes[12] = 1;
        bytes[14] = 32;
        bytes[40] = 255;
        bytes[41] = 0;
        bytes[42] = 0;
        bytes[43] = 255;
        bytes[44] = 0;
        bytes[45] = 255;
        bytes[46] = 0;
        bytes[47] = 255;
        let png = png_from_dib(&bytes).expect("png");
        let image = image::load_from_memory(&png).expect("decode").to_rgba8();
        assert_eq!(image.dimensions(), (2, 1));
        assert_eq!(image.get_pixel(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(image.get_pixel(1, 0).0, [0, 255, 0, 255]);
    }

    #[test]
    fn thumbnails_of_tall_images_keep_the_top() {
        let image = RgbaImage::from_pixel(1000, 10000, Rgba([0, 0, 0, 0]));
        let jpeg = thumbnail_jpeg(&image).expect("thumb");
        let thumb = image::load_from_memory(&jpeg).expect("decode");
        assert_eq!(thumb.width(), THUMB_WIDTH);
        assert!(thumb.height() <= THUMB_MAX_HEIGHT);
    }
}
