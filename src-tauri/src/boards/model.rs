//! Folders and pins kept in `boards.json`, and the repairs applied when it loads.
//!
//! Folders nest and hold pins directly. A pin can be in several folders, or in none
//! ("Uncategorized"). Files written before folders held pins had boards inside folders; each
//! board loads as a folder with the same id.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::tabs;

/// Name of the board older builds made for pins saved without a board.
pub const UNSORTED: &str = "Unsorted";
/// Save target for pins that belong to no folder.
pub const UNCATEGORIZED: &str = "uncategorized";
pub const PIN_KINDS: [&str; 6] = ["page", "region", "element", "link", "image", "file"];
pub const FOLDER_COLORS: [&str; 9] = [
    "gray", "red", "orange", "yellow", "green", "aqua", "blue", "purple", "pink",
];
pub const PIN_SORTS: [&str; 8] = [
    "newest", "oldest", "title", "kind", "size", "rating", "dimensions", "extension",
];

pub const TITLE_LIMIT: usize = 300;
pub const TEXT_LIMIT: usize = 2000;
pub const NOTE_LIMIT: usize = 5000;
pub const FILE_NAME_LIMIT: usize = 240;
pub const MIME_LIMIT: usize = 200;
const NAME_LIMIT: usize = 120;
const DESCRIPTION_LIMIT: usize = 2000;
const URL_LIMIT: usize = 2000;
const TAG_LIMIT: usize = 60;
const TAGS_PER_PIN: usize = 100;
const ICON_LIMIT: usize = 40;
const EXT_LIMIT: usize = 12;

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn new_id(prefix: &str) -> String {
    format!(
        "{prefix}{:x}{:x}",
        tabs::now_ms(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

/// Pin ids name files, so only plain letters and digits are accepted from the UI.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

pub fn clip_text(text: &str, limit: usize) -> String {
    text.trim().chars().take(limit).collect()
}

pub fn clean_name(name: &str, fallback: &str) -> String {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let name = clip_text(&name, NAME_LIMIT);
    if name.is_empty() {
        fallback.to_string()
    } else {
        name
    }
}

/// Lowercase letters and digits only, so the extension can name a file.
pub fn clean_ext(ext: &str) -> String {
    let ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    if ext.len() > EXT_LIMIT || !ext.chars().all(|character| character.is_ascii_alphanumeric()) {
        return String::new();
    }
    ext
}

/// Trimmed, single-spaced, unique ignoring case, in the order given.
pub fn clean_tags(tags: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for tag in tags {
        let tag = clip_text(&tag.split_whitespace().collect::<Vec<_>>().join(" "), TAG_LIMIT);
        if tag.is_empty() || !seen.insert(tag.to_lowercase()) {
            continue;
        }
        out.push(tag);
        if out.len() == TAGS_PER_PIN {
            break;
        }
    }
    out
}

fn clean_icon(icon: &str) -> String {
    let icon = icon.trim();
    if icon.len() > ICON_LIMIT
        || !icon
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return String::new();
    }
    icon.to_string()
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BoardFolder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub description: String,
    /// One of `FOLDER_COLORS`, or empty for the default.
    pub color: String,
    /// A lucide icon name, or empty for the folder icon.
    pub icon: String,
    pub cover_pin_id: Option<String>,
    /// One of `PIN_SORTS`, or empty to use the page's sort.
    pub sort: String,
    /// The folder view also lists pins in its subfolders.
    pub show_subfolders: bool,
}

/// A board from a file written before boards became folders. Only read.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LegacyBoard {
    pub id: String,
    pub name: String,
    pub folder_id: Option<String>,
    pub cover_pin_id: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PaletteColor {
    /// `#rrggbb`.
    pub color: String,
    /// Share of the image, 0 to 1.
    pub ratio: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Pin {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub page_url: String,
    pub link_url: String,
    pub text: String,
    pub note: String,
    /// The pin has a decodable image (`<id>.<ext>` or `<id>.png`) and a thumbnail.
    pub has_image: bool,
    /// A file pin whose thumbnail came from the Windows shell.
    pub has_thumb: bool,
    pub width: u32,
    pub height: u32,
    pub created_at: u64,
    /// Original name for a file pin, and for an image added from a file.
    pub file_name: String,
    pub mime: String,
    /// Empty means Uncategorized.
    pub folder_ids: Vec<String>,
    pub tags: Vec<String>,
    /// 0 to 5 stars.
    pub rating: u8,
    pub palette: Vec<PaletteColor>,
    /// Extension of the stored original, lowercase without the dot.
    pub ext: String,
    /// Bytes of the stored original.
    pub size: u64,
    /// SHA-256 prefix of the stored original. Same hash, same file.
    pub hash: String,
    pub file_modified_at: u64,
    pub last_opened_at: u64,
    /// Set while the pin is in the trash.
    pub deleted_at: Option<u64>,
    /// Board of a pin written before boards became folders. Only read.
    #[serde(skip_serializing)]
    pub board_id: String,
}

impl Pin {
    pub fn in_trash(&self) -> bool {
        self.deleted_at.is_some()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BoardsSnapshot {
    pub folders: Vec<BoardFolder>,
    /// Newest first.
    pub pins: Vec<Pin>,
    /// Folder the toolbar and the page menu save to. None saves to Uncategorized.
    #[serde(alias = "lastBoardId")]
    pub last_folder_id: Option<String>,
    #[serde(rename = "boards", skip_serializing)]
    pub legacy_boards: Vec<LegacyBoard>,
}

/// Changes to a folder. Fields left out stay as they are.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FolderPatch {
    pub name: Option<String>,
    pub description: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub sort: Option<String>,
    pub show_subfolders: Option<bool>,
    /// An empty id clears the cover.
    pub cover_pin_id: Option<String>,
}

/// Changes to a pin. Fields left out stay as they are.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PinPatch {
    pub title: Option<String>,
    pub note: Option<String>,
    pub link_url: Option<String>,
    pub rating: Option<u8>,
    pub tags: Option<Vec<String>>,
}

impl BoardsSnapshot {
    /// Repairs references an older or hand-edited file may contain, and turns boards into
    /// folders. A current file is unchanged.
    pub fn normalize(mut self) -> Self {
        self.migrate_boards();

        let mut seen = HashSet::new();
        self.folders.retain_mut(|folder| {
            if folder.id.is_empty() {
                folder.id = new_id("f");
            }
            if folder.name.trim().is_empty() {
                folder.name = "Untitled folder".into();
            }
            if !folder.color.is_empty() && !FOLDER_COLORS.contains(&folder.color.as_str()) {
                folder.color.clear();
            }
            if !folder.sort.is_empty() && !PIN_SORTS.contains(&folder.sort.as_str()) {
                folder.sort.clear();
            }
            let icon = clean_icon(&folder.icon);
            if icon != folder.icon {
                folder.icon = icon;
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
        self.pins.retain_mut(|pin| {
            if pin.id.is_empty() {
                pin.id = new_id("p");
            }
            if !PIN_KINDS.contains(&pin.kind.as_str()) {
                pin.kind = "page".into();
            }
            if pin
                .folder_ids
                .iter()
                .any(|folder| !folder_ids.contains(folder))
                || has_duplicates(&pin.folder_ids)
            {
                let mut kept = HashSet::new();
                pin.folder_ids
                    .retain(|folder| folder_ids.contains(folder) && kept.insert(folder.clone()));
            }
            pin.rating = pin.rating.min(5);
            let tags = clean_tags(&pin.tags);
            if tags != pin.tags {
                pin.tags = tags;
            }
            let ext = clean_ext(&pin.ext);
            if ext != pin.ext {
                pin.ext = ext;
            }
            seen.insert(pin.id.clone())
        });

        let live: HashSet<&str> = self
            .pins
            .iter()
            .filter(|pin| !pin.in_trash())
            .map(|pin| pin.id.as_str())
            .collect();
        let mut stale = Vec::new();
        for (index, folder) in self.folders.iter().enumerate() {
            if folder
                .cover_pin_id
                .as_deref()
                .is_some_and(|pin| !live.contains(pin))
            {
                stale.push(index);
            }
        }
        for index in stale {
            self.folders[index].cover_pin_id = None;
        }
        if self
            .last_folder_id
            .as_ref()
            .is_some_and(|folder| !folder_ids.contains(folder))
        {
            self.last_folder_id = None;
        }
        self
    }

    /// Each board becomes a folder with the same id inside the board's folder. Pins on a board
    /// join that folder. The top-level "Unsorted" board is dropped and its pins become
    /// Uncategorized.
    fn migrate_boards(&mut self) {
        let boards = std::mem::take(&mut self.legacy_boards);
        let mut unsorted = HashSet::new();
        let mut taken: HashSet<String> = self
            .folders
            .iter()
            .map(|folder| folder.id.clone())
            .collect();
        for board in boards {
            if board.folder_id.is_none() && board.name == UNSORTED {
                unsorted.insert(board.id);
                continue;
            }
            if board.id.is_empty() || !taken.insert(board.id.clone()) {
                continue;
            }
            self.folders.push(BoardFolder {
                id: board.id,
                name: board.name,
                parent_id: board.folder_id,
                created_at: board.created_at,
                updated_at: board.updated_at,
                cover_pin_id: board.cover_pin_id,
                ..BoardFolder::default()
            });
        }
        for pin in &mut self.pins {
            let board = std::mem::take(&mut pin.board_id);
            if !board.is_empty() && pin.folder_ids.is_empty() && !unsorted.contains(&board) {
                pin.folder_ids.push(board);
            }
        }
        if self
            .last_folder_id
            .as_ref()
            .is_some_and(|folder| unsorted.contains(folder))
        {
            self.last_folder_id = None;
        }
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

    pub fn folder_exists(&self, id: &str) -> bool {
        self.folders.iter().any(|folder| folder.id == id)
    }

    fn check_folder(&self, folder: Option<&str>) -> Result<(), String> {
        match folder {
            Some(id) if !self.folder_exists(id) => Err("That folder no longer exists.".into()),
            _ => Ok(()),
        }
    }

    fn folder_mut(&mut self, id: &str) -> Result<&mut BoardFolder, String> {
        self.folders
            .iter_mut()
            .find(|folder| folder.id == id)
            .ok_or_else(|| "That folder no longer exists.".to_string())
    }

    pub fn folder_name(&self, id: &str) -> Option<&str> {
        self.folders
            .iter()
            .find(|folder| folder.id == id)
            .map(|folder| folder.name.as_str())
    }

    /// "Folder / Sub folder".
    pub fn folder_label(&self, id: &str) -> Option<String> {
        let mut names = Vec::new();
        let mut seen = HashSet::new();
        let mut cursor = Some(id.to_string());
        while let Some(current) = cursor {
            if !seen.insert(current.clone()) {
                break;
            }
            let folder = self.folders.iter().find(|folder| folder.id == current)?;
            names.push(folder.name.clone());
            cursor = folder.parent_id.clone();
        }
        names.reverse();
        Some(names.join(" / "))
    }

    /// Where a pin saved to `folder_ids` is shown, for notices.
    pub fn place_name(&self, folder_ids: &[String]) -> String {
        folder_ids
            .first()
            .and_then(|id| self.folder_label(id))
            .unwrap_or_else(|| "Uncategorized".into())
    }

    /// `id` and every folder inside it.
    pub fn folder_tree(&self, id: &str) -> HashSet<String> {
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

    /// Folder a new pin goes to. Empty means the last folder used; `UNCATEGORIZED` means none.
    pub fn target_for_save(&self, requested: &str) -> Result<Option<String>, String> {
        let requested = requested.trim();
        if requested == UNCATEGORIZED {
            return Ok(None);
        }
        if !requested.is_empty() {
            return if self.folder_exists(requested) {
                Ok(Some(requested.to_string()))
            } else {
                Err("That folder no longer exists.".into())
            };
        }
        Ok(self
            .last_folder_id
            .clone()
            .filter(|id| self.folder_exists(id)))
    }

    /// Adds new pins in front, newest first. The first pin's folder becomes the last one used.
    pub fn add_pins(&mut self, pins: Vec<Pin>) {
        let Some(first) = pins.first() else {
            return;
        };
        self.last_folder_id = first.folder_ids.first().cloned();
        let now = first.created_at;
        let touched: HashSet<String> = pins
            .iter()
            .flat_map(|pin| pin.folder_ids.iter().cloned())
            .collect();
        for folder in &mut self.folders {
            if touched.contains(&folder.id) {
                folder.updated_at = now;
            }
        }
        let mut pins = pins;
        pins.reverse();
        for pin in pins {
            self.pins.insert(0, pin);
        }
    }

    pub fn create_folder(
        &mut self,
        name: &str,
        parent: Option<&str>,
    ) -> Result<BoardFolder, String> {
        self.check_folder(parent)?;
        let now = tabs::now_ms();
        let folder = BoardFolder {
            id: new_id("f"),
            name: clean_name(name, "New folder"),
            parent_id: parent.map(str::to_string),
            created_at: now,
            updated_at: now,
            ..BoardFolder::default()
        };
        self.folders.push(folder.clone());
        Ok(folder)
    }

    /// The folder at `relative` ("a/b/c") under `base`, made where missing. Names match
    /// ignoring case, so importing the same tree again fills the same folders.
    pub fn folder_path(&mut self, base: Option<&str>, relative: &str) -> Option<String> {
        let mut parent = base.map(str::to_string);
        for segment in relative
            .split(['/', '\\'])
            .map(str::trim)
            .filter(|segment| !segment.is_empty() && *segment != "." && *segment != "..")
        {
            let name = clean_name(segment, "Folder");
            let lower = name.to_lowercase();
            let existing = self
                .folders
                .iter()
                .find(|folder| folder.parent_id == parent && folder.name.to_lowercase() == lower)
                .map(|folder| folder.id.clone());
            let id = match existing {
                Some(id) => id,
                None => {
                    let now = tabs::now_ms();
                    let folder = BoardFolder {
                        id: new_id("f"),
                        name,
                        parent_id: parent.clone(),
                        created_at: now,
                        updated_at: now,
                        ..BoardFolder::default()
                    };
                    let id = folder.id.clone();
                    self.folders.push(folder);
                    id
                }
            };
            parent = Some(id);
        }
        parent
    }

    pub fn rename_folder(&mut self, id: &str, name: &str) -> Result<(), String> {
        let folder = self.folder_mut(id)?;
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
        self.folder_mut(id)?.parent_id = parent.map(str::to_string);
        Ok(())
    }

    pub fn update_folder(&mut self, id: &str, patch: &FolderPatch) -> Result<(), String> {
        let cover = patch.cover_pin_id.as_deref().map(str::trim);
        if let Some(pin) = cover.filter(|pin| !pin.is_empty()) {
            if !self
                .pins
                .iter()
                .any(|item| item.id == pin && !item.in_trash())
            {
                return Err("That pin no longer exists.".into());
            }
        }
        let folder = self.folder_mut(id)?;
        if let Some(name) = &patch.name {
            folder.name = clean_name(name, &folder.name);
        }
        if let Some(description) = &patch.description {
            folder.description = clip_text(description, DESCRIPTION_LIMIT);
        }
        if let Some(color) = &patch.color {
            let color = color.trim();
            folder.color = if FOLDER_COLORS.contains(&color) {
                color.to_string()
            } else {
                String::new()
            };
        }
        if let Some(icon) = &patch.icon {
            folder.icon = clean_icon(icon);
        }
        if let Some(sort) = &patch.sort {
            let sort = sort.trim();
            folder.sort = if PIN_SORTS.contains(&sort) {
                sort.to_string()
            } else {
                String::new()
            };
        }
        if let Some(show) = patch.show_subfolders {
            folder.show_subfolders = show;
        }
        if let Some(cover) = cover {
            folder.cover_pin_id = (!cover.is_empty()).then(|| cover.to_string());
        }
        folder.updated_at = tabs::now_ms();
        Ok(())
    }

    /// Removes the folder and every folder inside it. Pins left in no folder go to the trash.
    pub fn delete_folder(&mut self, id: &str) -> Result<usize, String> {
        if !self.folder_exists(id) {
            return Err("That folder no longer exists.".into());
        }
        let folders = self.folder_tree(id);
        self.folders.retain(|folder| !folders.contains(&folder.id));
        let now = tabs::now_ms();
        let mut trashed = 0;
        for pin in &mut self.pins {
            let before = pin.folder_ids.len();
            pin.folder_ids.retain(|folder| !folders.contains(folder));
            if before > 0 && pin.folder_ids.is_empty() && !pin.in_trash() {
                pin.deleted_at = Some(now);
                trashed += 1;
            }
        }
        if self
            .last_folder_id
            .as_ref()
            .is_some_and(|folder| folders.contains(folder))
        {
            self.last_folder_id = None;
        }
        Ok(trashed)
    }

    /// Copies the folder and its subfolders next to it. Pins join the copies too; their files
    /// are shared, not copied.
    pub fn duplicate_folder(&mut self, id: &str) -> Result<BoardFolder, String> {
        let source = self
            .folders
            .iter()
            .find(|folder| folder.id == id)
            .cloned()
            .ok_or("That folder no longer exists.")?;
        let tree = self.folder_tree(id);
        let now = tabs::now_ms();
        let mut ids: HashMap<String, String> = HashMap::new();
        for folder_id in &tree {
            ids.insert(folder_id.clone(), new_id("f"));
        }
        let copies: Vec<BoardFolder> = self
            .folders
            .iter()
            .filter(|folder| tree.contains(&folder.id))
            .map(|folder| {
                let mut copy = folder.clone();
                copy.id = ids[&folder.id].clone();
                if folder.id == source.id {
                    copy.name = clean_name(&format!("{} copy", folder.name), "Folder copy");
                } else {
                    copy.parent_id = folder.parent_id.as_ref().and_then(|parent| ids.get(parent)).cloned();
                }
                copy.created_at = now;
                copy.updated_at = now;
                copy
            })
            .collect();
        let root = copies
            .iter()
            .find(|folder| folder.id == ids[&source.id])
            .cloned()
            .ok_or("That folder no longer exists.")?;
        self.folders.extend(copies);
        for pin in &mut self.pins {
            let extra: Vec<String> = pin
                .folder_ids
                .iter()
                .filter_map(|folder| ids.get(folder).cloned())
                .collect();
            pin.folder_ids.extend(extra);
        }
        Ok(root)
    }

    fn pin_mut(&mut self, id: &str) -> Result<&mut Pin, String> {
        self.pins
            .iter_mut()
            .find(|pin| pin.id == id)
            .ok_or_else(|| "That pin no longer exists.".to_string())
    }

    pub fn update_pin(&mut self, id: &str, patch: &PinPatch) -> Result<(), String> {
        let pin = self.pin_mut(id)?;
        if let Some(title) = &patch.title {
            pin.title = clip_text(title, TITLE_LIMIT);
        }
        if let Some(note) = &patch.note {
            pin.note = clip_text(note, NOTE_LIMIT);
        }
        if let Some(link) = &patch.link_url {
            pin.link_url = clip_text(link, URL_LIMIT);
        }
        if let Some(rating) = patch.rating {
            pin.rating = rating.min(5);
        }
        if let Some(tags) = &patch.tags {
            pin.tags = clean_tags(tags);
        }
        Ok(())
    }

    fn each_pin(&mut self, ids: &[String], mut edit: impl FnMut(&mut Pin)) {
        let ids: HashSet<&String> = ids.iter().collect();
        for pin in &mut self.pins {
            if ids.contains(&pin.id) {
                edit(pin);
            }
        }
    }

    fn touch_folder(&mut self, id: &str) {
        let now = tabs::now_ms();
        if let Some(folder) = self.folders.iter_mut().find(|folder| folder.id == id) {
            folder.updated_at = now;
        }
    }

    pub fn add_to_folder(&mut self, ids: &[String], folder: &str) -> Result<(), String> {
        self.check_folder(Some(folder))?;
        self.each_pin(ids, |pin| {
            if !pin.folder_ids.iter().any(|item| item == folder) {
                pin.folder_ids.push(folder.to_string());
            }
        });
        self.touch_folder(folder);
        Ok(())
    }

    pub fn remove_from_folder(&mut self, ids: &[String], folder: &str) -> Result<(), String> {
        self.each_pin(ids, |pin| pin.folder_ids.retain(|item| item != folder));
        Ok(())
    }

    /// Takes the pins out of `from` (every folder when None) and puts them in `to`
    /// (Uncategorized when None).
    pub fn move_pins(
        &mut self,
        ids: &[String],
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<(), String> {
        self.check_folder(to)?;
        self.each_pin(ids, |pin| {
            match from {
                Some(from) => pin.folder_ids.retain(|item| item != from),
                None => pin.folder_ids.clear(),
            }
            if let Some(to) = to {
                if !pin.folder_ids.iter().any(|item| item == to) {
                    pin.folder_ids.push(to.to_string());
                }
            }
        });
        if let Some(to) = to {
            self.touch_folder(to);
        }
        Ok(())
    }

    pub fn add_tags(&mut self, ids: &[String], tags: &[String]) {
        let tags = clean_tags(tags);
        self.each_pin(ids, |pin| {
            let mut next = pin.tags.clone();
            next.extend(tags.iter().cloned());
            pin.tags = clean_tags(&next);
        });
    }

    pub fn remove_tags(&mut self, ids: &[String], tags: &[String]) {
        let remove: HashSet<String> = tags.iter().map(|tag| tag.trim().to_lowercase()).collect();
        self.each_pin(ids, |pin| {
            pin.tags.retain(|tag| !remove.contains(&tag.to_lowercase()))
        });
    }

    /// Renames a tag on every pin. A name that already exists merges the two.
    pub fn rename_tag(&mut self, from: &str, to: &str) -> Result<(), String> {
        let from = from.trim().to_lowercase();
        let to = clean_tags(&[to.to_string()]);
        let Some(to) = to.first() else {
            return Err("Enter a tag name.".into());
        };
        for pin in &mut self.pins {
            if pin.tags.iter().any(|tag| tag.to_lowercase() == from) {
                let next: Vec<String> = pin
                    .tags
                    .iter()
                    .map(|tag| {
                        if tag.to_lowercase() == from {
                            to.clone()
                        } else {
                            tag.clone()
                        }
                    })
                    .collect();
                pin.tags = clean_tags(&next);
            }
        }
        Ok(())
    }

    pub fn delete_tag(&mut self, name: &str) {
        let name = name.trim().to_lowercase();
        for pin in &mut self.pins {
            pin.tags.retain(|tag| tag.to_lowercase() != name);
        }
    }

    pub fn trash_pins(&mut self, ids: &[String]) {
        let now = tabs::now_ms();
        self.each_pin(ids, |pin| {
            if pin.deleted_at.is_none() {
                pin.deleted_at = Some(now);
            }
        });
        let gone: HashSet<&String> = ids.iter().collect();
        for folder in &mut self.folders {
            if folder.cover_pin_id.as_ref().is_some_and(|pin| gone.contains(pin)) {
                folder.cover_pin_id = None;
            }
        }
    }

    pub fn restore_pins(&mut self, ids: &[String]) {
        self.each_pin(ids, |pin| pin.deleted_at = None);
    }

    pub fn mark_used(&mut self, ids: &[String]) {
        let now = tabs::now_ms();
        self.each_pin(ids, |pin| pin.last_opened_at = now);
    }

    /// Deletes pins for good. Returns them so their files can be removed.
    pub fn delete_pins(&mut self, ids: &[String]) -> Vec<Pin> {
        let ids: HashSet<&String> = ids.iter().collect();
        self.remove_pins_where(|pin| ids.contains(&pin.id))
    }

    pub fn empty_trash(&mut self) -> Vec<Pin> {
        self.remove_pins_where(Pin::in_trash)
    }

    fn remove_pins_where(&mut self, remove: impl Fn(&Pin) -> bool) -> Vec<Pin> {
        let mut removed = Vec::new();
        self.pins.retain(|pin| {
            if remove(pin) {
                removed.push(pin.clone());
                false
            } else {
                true
            }
        });
        let gone: HashSet<&str> = removed.iter().map(|pin| pin.id.as_str()).collect();
        for folder in &mut self.folders {
            if folder
                .cover_pin_id
                .as_deref()
                .is_some_and(|pin| gone.contains(pin))
            {
                folder.cover_pin_id = None;
            }
        }
        removed
    }

    /// True when a live pin with this hash is already in the folder (or Uncategorized).
    pub fn has_duplicate(&self, hash: &str, folder: Option<&str>) -> bool {
        !hash.is_empty()
            && self.pins.iter().any(|pin| {
                !pin.in_trash()
                    && pin.hash == hash
                    && match folder {
                        Some(folder) => pin.folder_ids.iter().any(|item| item == folder),
                        None => pin.folder_ids.is_empty(),
                    }
            })
    }
}

fn has_duplicates(values: &[String]) -> bool {
    let mut seen = HashSet::new();
    values.iter().any(|value| !seen.insert(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> BoardsSnapshot {
        let mut boards = BoardsSnapshot::default();
        let outer = boards.create_folder("Outer", None).expect("outer");
        let inner = boards
            .create_folder("Inner", Some(&outer.id))
            .expect("inner");
        let kept = boards.create_folder("Kept", None).expect("kept");
        let pins = [("p1", &kept.id), ("p2", &inner.id), ("p3", &inner.id)]
            .into_iter()
            .map(|(id, folder)| Pin {
                id: id.into(),
                folder_ids: vec![folder.clone()],
                kind: "page".into(),
                ..Pin::default()
            })
            .collect();
        boards.add_pins(pins);
        boards
    }

    fn folder_id(boards: &BoardsSnapshot, name: &str) -> String {
        boards
            .folders
            .iter()
            .find(|folder| folder.name == name)
            .map(|folder| folder.id.clone())
            .expect("folder")
    }

    #[test]
    fn deleting_a_folder_trashes_pins_left_in_no_folder() {
        let mut boards = sample();
        let outer = folder_id(&boards, "Outer");
        let trashed = boards.delete_folder(&outer).expect("delete");
        assert_eq!(trashed, 2);
        assert_eq!(boards.folders.len(), 1);
        assert_eq!(boards.pins.len(), 3);
        assert!(boards.pins.iter().filter(|pin| pin.in_trash()).count() == 2);
        boards.restore_pins(&["p2".into()]);
        let restored = boards.pins.iter().find(|pin| pin.id == "p2").expect("p2");
        assert!(!restored.in_trash());
        assert!(restored.folder_ids.is_empty());
        let removed = boards.empty_trash();
        assert_eq!(removed.len(), 1);
        assert_eq!(boards.pins.len(), 2);
    }

    #[test]
    fn a_pin_in_another_folder_survives_a_folder_delete() {
        let mut boards = sample();
        let kept = folder_id(&boards, "Kept");
        let inner = folder_id(&boards, "Inner");
        boards.add_to_folder(&["p2".into()], &kept).expect("add");
        boards.delete_folder(&inner).expect("delete");
        let pin = boards.pins.iter().find(|pin| pin.id == "p2").expect("p2");
        assert!(!pin.in_trash());
        assert_eq!(pin.folder_ids, vec![kept]);
    }

    #[test]
    fn a_folder_cannot_move_inside_itself() {
        let mut boards = sample();
        let outer = folder_id(&boards, "Outer");
        let inner = folder_id(&boards, "Inner");
        assert!(boards.move_folder(&outer, Some(&inner)).is_err());
        assert!(boards.move_folder(&outer, Some(&outer)).is_err());
        assert!(boards.move_folder(&inner, None).is_ok());
        assert!(boards.move_folder(&outer, Some(&inner)).is_ok());
    }

    #[test]
    fn saved_files_from_the_board_release_load_as_folders() {
        let default: BoardsSnapshot = serde_json::from_str(
            r#"{
              "folders": [],
              "boards": [
                { "id": "b1a199e0324b0", "name": "Clothing", "folderId": null, "coverPinId": null, "createdAt": 1793582969419, "updatedAt": 1794616130430 },
                { "id": "b1a1d77fbe3d1", "name": "Funny", "folderId": null, "coverPinId": null, "createdAt": 1794616835645, "updatedAt": 1794617462946 }
              ],
              "pins": [
                {
                  "id": "p1a1d781103a3", "boardId": "b1a1d77fbe3d1", "kind": "image",
                  "title": "Jack Raines on X", "pageUrl": "https://x.com/Jack_Raines/status/2120544470642901409?s=12",
                  "linkUrl": "https://pbs.twimg.com/media/HW2v9WxXAAEW-12?format=webp&name=large",
                  "text": "", "note": "", "hasImage": true, "width": 986, "height": 900,
                  "createdAt": 1794616923058, "fileName": "", "mime": ""
                }
              ],
              "lastBoardId": "b1a1d77fbe3d1"
            }"#,
        )
        .expect("parse default");
        let default = default.normalize();
        let names: Vec<&str> = default.folders.iter().map(|folder| folder.name.as_str()).collect();
        assert_eq!(names, vec!["Clothing", "Funny"]);
        assert_eq!(default.folders[1].created_at, 1794616835645);
        let pin = &default.pins[0];
        assert_eq!(pin.folder_ids, vec!["b1a1d77fbe3d1".to_string()]);
        assert!(pin.has_image);
        assert_eq!((pin.width, pin.height), (986, 900));
        assert_eq!(pin.deleted_at, None);
        assert_eq!(default.last_folder_id.as_deref(), Some("b1a1d77fbe3d1"));
        assert_eq!(default.clone().normalize(), default);

        let media: BoardsSnapshot = serde_json::from_str(
            r#"{
              "folders": [],
              "boards": [{ "id": "b1a1a25158831", "name": "Unsorted", "folderId": null, "coverPinId": null, "createdAt": 1793724602499, "updatedAt": 1793724623983 }],
              "pins": [
                { "id": "p1a1a251ac5b2", "boardId": "b1a1a25158831", "kind": "page", "title": "YouTube", "pageUrl": "https://www.youtube.com/shorts/p9-B9k02C1M", "linkUrl": "https://www.youtube.com/shorts/p9-B9k02C1M", "text": "", "note": "", "hasImage": true, "width": 1540, "height": 920, "createdAt": 1793724623983, "fileName": "", "mime": "" },
                { "id": "p1a1a25158700", "boardId": "b1a1a25158831", "kind": "page", "title": "YouTube", "pageUrl": "https://www.youtube.com/shorts/p9-B9k02C1M", "linkUrl": "https://www.youtube.com/shorts/p9-B9k02C1M", "text": "", "note": "", "hasImage": true, "width": 1540, "height": 920, "createdAt": 1793724602499, "fileName": "", "mime": "" }
              ],
              "lastBoardId": "b1a1a25158831"
            }"#,
        )
        .expect("parse media");
        let media = media.normalize();
        assert!(media.folders.is_empty());
        assert_eq!(media.pins.len(), 2);
        assert!(media.pins.iter().all(|pin| pin.folder_ids.is_empty() && pin.has_image));
        assert_eq!(media.last_folder_id, None);
        let written = serde_json::to_string(&media).expect("write");
        let again: BoardsSnapshot = serde_json::from_str(&written).expect("read back");
        assert_eq!(again.normalize(), media);
    }

    #[test]
    fn an_old_file_with_boards_loads_as_folders() {
        let boards: BoardsSnapshot = serde_json::from_str(
            r#"{
              "folders": [{ "id": "f1", "name": "Work" }],
              "boards": [
                { "id": "b1", "name": "Ideas", "folderId": "f1", "coverPinId": "p1" },
                { "id": "b2", "name": "Unsorted" },
                { "id": "b3", "name": "Top" }
              ],
              "pins": [
                { "id": "p1", "boardId": "b1", "kind": "image" },
                { "id": "p2", "boardId": "b2" },
                { "id": "p3", "boardId": "gone" }
              ],
              "lastBoardId": "b2",
              "retired": true
            }"#,
        )
        .expect("parse");
        let boards = boards.normalize();
        assert!(boards.legacy_boards.is_empty());
        assert_eq!(boards.folders.len(), 3);
        let ideas = boards.folders.iter().find(|folder| folder.id == "b1").expect("b1");
        assert_eq!(ideas.parent_id.as_deref(), Some("f1"));
        assert_eq!(ideas.cover_pin_id.as_deref(), Some("p1"));
        assert!(boards.folders.iter().any(|folder| folder.id == "b3" && folder.parent_id.is_none()));
        assert!(!boards.folders.iter().any(|folder| folder.name == UNSORTED));
        assert_eq!(boards.pins[0].folder_ids, vec!["b1".to_string()]);
        assert!(boards.pins[1].folder_ids.is_empty());
        assert!(boards.pins[2].folder_ids.is_empty());
        assert_eq!(boards.last_folder_id, None);
        let written = serde_json::to_value(&boards).expect("write");
        assert!(written.get("boards").is_none());
        assert!(written["pins"][0].get("boardId").is_none());
        assert_eq!(boards.clone().normalize(), boards);
    }

    #[test]
    fn the_last_board_key_is_still_read() {
        let boards: BoardsSnapshot = serde_json::from_str(
            r#"{ "boards": [{ "id": "b1", "name": "Ideas" }], "pins": [], "lastBoardId": "b1" }"#,
        )
        .expect("parse");
        assert_eq!(boards.normalize().last_folder_id.as_deref(), Some("b1"));
    }

    #[test]
    fn loading_repairs_missing_references() {
        let mut boards = BoardsSnapshot::default();
        for (id, parent) in [("a", "b"), ("b", "a"), ("c", "gone")] {
            boards.folders.push(BoardFolder {
                id: id.into(),
                name: id.to_uppercase(),
                parent_id: Some(parent.into()),
                color: "chartreuse".into(),
                ..BoardFolder::default()
            });
        }
        boards.pins.push(Pin {
            id: "p".into(),
            folder_ids: vec!["gone".into(), "c".into(), "c".into()],
            kind: "hologram".into(),
            rating: 9,
            tags: vec![" Red ".into(), "red".into(), "".into()],
            ext: ".PNG".into(),
            ..Pin::default()
        });
        boards.last_folder_id = Some("gone".into());
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
        assert!(boards.folders.iter().all(|folder| folder.color.is_empty()));
        let pin = &boards.pins[0];
        assert_eq!(pin.kind, "page");
        assert_eq!(pin.folder_ids, vec!["c".to_string()]);
        assert_eq!(pin.rating, 5);
        assert_eq!(pin.tags, vec!["Red".to_string()]);
        assert_eq!(pin.ext, "png");
        assert_eq!(boards.last_folder_id, None);
    }

    #[test]
    fn a_current_file_loads_unchanged() {
        let boards = sample();
        assert_eq!(boards.clone().normalize(), boards);
    }

    #[test]
    fn saving_without_a_folder_uses_the_last_one_or_uncategorized() {
        let empty = BoardsSnapshot::default();
        assert_eq!(empty.target_for_save("").expect("target"), None);
        let boards = sample();
        let last = boards.last_folder_id.clone().expect("last");
        assert_eq!(boards.target_for_save("").expect("target"), Some(last));
        assert_eq!(boards.target_for_save(UNCATEGORIZED).expect("target"), None);
        assert!(boards.target_for_save("missing").is_err());
    }

    #[test]
    fn folder_paths_are_found_again_by_name() {
        let mut boards = BoardsSnapshot::default();
        let first = boards.folder_path(None, "Photos/2024\\Trips").expect("path");
        let second = boards.folder_path(None, "photos/2024/trips").expect("path");
        assert_eq!(first, second);
        assert_eq!(boards.folders.len(), 3);
        assert_eq!(boards.folder_label(&first).as_deref(), Some("Photos / 2024 / Trips"));
        assert_eq!(boards.folder_path(None, ""), None);
    }

    #[test]
    fn tags_rename_merges_and_moves_keep_other_folders() {
        let mut boards = sample();
        boards.add_tags(&["p1".into(), "p2".into()], &["Blue".into(), "red".into()]);
        boards.add_tags(&["p1".into()], &["Navy".into()]);
        boards.rename_tag("navy", "blue").expect("rename");
        let p1 = boards.pins.iter().find(|pin| pin.id == "p1").expect("p1");
        assert_eq!(p1.tags, vec!["Blue".to_string(), "red".to_string()]);
        let kept = folder_id(&boards, "Kept");
        let inner = folder_id(&boards, "Inner");
        boards.add_to_folder(&["p1".into()], &inner).expect("add");
        boards.move_pins(&["p1".into()], Some(&kept), None).expect("move");
        let p1 = boards.pins.iter().find(|pin| pin.id == "p1").expect("p1");
        assert_eq!(p1.folder_ids, vec![inner.clone()]);
        boards.move_pins(&["p1".into()], None, None).expect("move");
        let p1 = boards.pins.iter().find(|pin| pin.id == "p1").expect("p1");
        assert!(p1.folder_ids.is_empty());
    }

    #[test]
    fn duplicating_a_folder_copies_the_tree_and_shares_pins() {
        let mut boards = sample();
        let outer = folder_id(&boards, "Outer");
        let copy = boards.duplicate_folder(&outer).expect("duplicate");
        assert_eq!(copy.name, "Outer copy");
        assert_eq!(boards.folders.len(), 5);
        let p2 = boards.pins.iter().find(|pin| pin.id == "p2").expect("p2");
        assert_eq!(p2.folder_ids.len(), 2);
        assert_eq!(boards.clone().normalize(), boards);
    }
}
