//! Boards: pins saved from web pages, kept on boards inside nested folders. `ce://boards` shows them.
//!
//! `boards.json` in the profile folder holds the folders, boards and pins. Each pin with a picture
//! has `boards/<pin id>.png` and a small `boards/<pin id>.thumb.jpg` for the grid.

use std::collections::{HashMap, HashSet};
use std::fs;
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
const PIN_KINDS: [&str; 5] = ["page", "region", "element", "link", "image"];
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
    !id.is_empty() && id.chars().all(|character| character.is_ascii_alphanumeric())
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
        let folder_ids: HashSet<String> = self.folders.iter().map(|folder| folder.id.clone()).collect();
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
        if let Some(last) = self.last_board_id.clone().filter(|id| self.board_exists(id)) {
            return Ok(last);
        }
        if let Some(first) = self.boards.first() {
            return Ok(first.id.clone());
        }
        Ok(self.unsorted_board())
    }

    fn add_pin(&mut self, pin: Pin) {
        if let Some(board) = self.boards.iter_mut().find(|board| board.id == pin.board_id) {
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

    pub fn create_folder(&mut self, name: &str, parent: Option<&str>) -> Result<BoardFolder, String> {
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
            .filter(|board| board.folder_id.as_ref().is_some_and(|folder| folders.contains(folder)))
            .map(|board| board.id.clone())
            .collect();
        self.folders.retain(|folder| !folders.contains(&folder.id));
        self.boards.retain(|board| !boards.contains(&board.id));
        if self.last_board_id.as_ref().is_some_and(|board| boards.contains(board)) {
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
            if !self.pins.iter().any(|item| item.id == pin && item.board_id == id) {
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
            if board.cover_pin_id.as_ref().is_some_and(|pin| removed.contains(pin)) {
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
    }
}

pub fn create_folder(app: &AppHandle, name: &str, parent: Option<&str>) -> Result<BoardFolder, String> {
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
    let crop_height = ((f64::from(THUMB_MAX_HEIGHT) / scale).floor() as u32).clamp(1, height.max(1));
    let cropped = image::imageops::crop_imm(image, 0, 0, width, crop_height).to_image();
    let target_height = ((f64::from(crop_height) * scale).round() as u32).max(1);
    let resized = image::imageops::resize(&cropped, target_width, target_height, FilterType::Triangle);
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
    fs::create_dir_all(&dir).map_err(|error| format!("Couldn't create {}. {error}", dir.display()))?;
    let png = capture::encode_png(image)?;
    fs::write(image_path(app, id), png).map_err(|error| format!("Couldn't save the image. {error}"))?;
    fs::write(thumb_path(app, id), thumbnail_jpeg(image)?)
        .map_err(|error| format!("Couldn't save the thumbnail. {error}"))
}

struct PinDraft {
    kind: &'static str,
    title: String,
    page_url: String,
    link_url: String,
    text: String,
}

fn save_pin(app: &AppHandle, board_id: &str, draft: PinDraft, image: Option<&RgbaImage>) -> Result<Pin, String> {
    let id = new_id("p");
    if let Some(image) = image {
        if let Err(error) = write_image(app, &id, image) {
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
    let draft = PinDraft {
        kind: "page",
        title,
        page_url: url.clone(),
        link_url: url,
        text: String::new(),
    };
    save_pin(app, board_id, draft, Some(&image))
}

pub fn pin_region(app: &AppHandle, id: u32, board_id: &str) -> Result<Pin, String> {
    let (title, url) = tab_page(app, id);
    let image = capture::region_image(app, id)?;
    let draft = PinDraft {
        kind: "region",
        title,
        page_url: url.clone(),
        link_url: url,
        text: String::new(),
    };
    save_pin(app, board_id, draft, Some(&image))
}

pub fn pin_element(app: &AppHandle, id: u32, board_id: &str) -> Result<Pin, String> {
    let (title, url) = tab_page(app, id);
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let value = webview2::evaluate(&webview, ELEMENT_SCRIPT, Duration::from_secs(150), PICK_FAILURE)?;
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
    let draft = PinDraft {
        kind: "element",
        title: if heading.is_empty() { title } else { heading },
        page_url: url.clone(),
        link_url: if href.is_empty() { url } else { href },
        text: text_of(&value, "text"),
    };
    save_pin(app, board_id, draft, Some(&image))
}

pub fn pin_link(app: &AppHandle, id: u32, board_id: &str) -> Result<Pin, String> {
    let (tab_title, url) = tab_page(app, id);
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let value = webview2::evaluate(&webview, LINK_SCRIPT, Duration::from_secs(10), PICK_FAILURE)?;
    let preview = text_of(&value, "image");
    let image = (!preview.is_empty())
        .then(|| page_menu::image_bytes(&preview).and_then(|bytes| decode_image(&bytes)).ok())
        .flatten();
    let image = match image {
        Some(image) => image,
        None => capture::page_image(app, id, false, false)?.0,
    };
    let title = text_of(&value, "title");
    let link = text_of(&value, "url");
    let draft = PinDraft {
        kind: "link",
        title: if title.is_empty() { tab_title } else { title },
        page_url: url.clone(),
        link_url: if link.is_empty() { url } else { link },
        text: text_of(&value, "description"),
    };
    save_pin(app, board_id, draft, Some(&image))
}

/// Downloads each image and pins it. Fails only when none could be saved.
pub fn pin_images(app: &AppHandle, id: u32, board_id: &str, urls: &[String]) -> Result<Vec<Pin>, String> {
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
                let draft = PinDraft {
                    kind: "image",
                    title: title.clone(),
                    page_url: url.clone(),
                    link_url: source.to_string(),
                    text: String::new(),
                };
                save_pin(app, &board, draft, Some(&image))
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
            let image = image::load_from_memory_with_format(&png, ImageFormat::Png).ok()?.to_rgba8();
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
        requestAnimationFrame(function () { resolve(value); });
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
        let inner = boards.create_folder("Inner", Some(&outer.id)).expect("inner");
        let kept = boards.create_board("Kept", None).expect("kept");
        let nested = boards.create_board("Nested", Some(&inner.id)).expect("nested");
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
        boards.folders.push(BoardFolder { id: "a".into(), name: "A".into(), parent_id: Some("b".into()), created_at: 0 });
        boards.folders.push(BoardFolder { id: "b".into(), name: "B".into(), parent_id: Some("a".into()), created_at: 0 });
        boards.folders.push(BoardFolder { id: "c".into(), name: "C".into(), parent_id: Some("gone".into()), created_at: 0 });
        boards.boards.push(Board { id: "x".into(), name: "X".into(), folder_id: Some("gone".into()), ..Board::default() });
        boards.pins.push(Pin { id: "p".into(), board_id: "gone".into(), kind: "hologram".into(), ..Pin::default() });
        boards.last_board_id = Some("gone".into());
        let boards = boards.normalize();
        let parent = |id: &str| boards.folders.iter().find(|folder| folder.id == id).and_then(|folder| folder.parent_id.clone());
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
    fn thumbnails_of_tall_images_keep_the_top() {
        let image = RgbaImage::from_pixel(1000, 10000, Rgba([0, 0, 0, 0]));
        let jpeg = thumbnail_jpeg(&image).expect("thumb");
        let thumb = image::load_from_memory(&jpeg).expect("decode");
        assert_eq!(thumb.width(), THUMB_WIDTH);
        assert!(thumb.height() <= THUMB_MAX_HEIGHT);
    }
}
