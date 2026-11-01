use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rand::RngCore;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

use crate::{bookmarks, files, history, paths, profiles, settings};

const WATCHED: &[&str] = &[
    "bookmarks.json",
    "history.json",
    "settings.json",
    "extensions.json",
    "profiles.json",
];

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflict {
    pub id: String,
    pub path: String,
    pub label: String,
    pub time: u64,
    pub summary: String,
    pub can_merge: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncStamp {
    revision: u64,
    machine_id: String,
    base_revision: u64,
}

impl Default for SyncStamp {
    fn default() -> Self {
        Self {
            revision: 0,
            machine_id: String::new(),
            base_revision: 0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LedgerRow {
    hash: String,
    revision: u64,
    machine_id: String,
    base_revision: u64,
    #[serde(default)]
    inflight: bool,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct LedgerFile {
    files: BTreeMap<String, LedgerRow>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConflictItem {
    id: String,
    /// Profile file this conflict is about, relative to the data root.
    path: String,
    /// Cloud conflict copy, relative to the data root. Empty when this computer detected the edit.
    cloud: String,
    ours: Value,
    theirs: Value,
    base: Value,
    time: u64,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ConflictStore {
    items: Vec<ConflictItem>,
}

struct DiskDoc {
    bytes: Vec<u8>,
    content: Value,
    stamp: SyncStamp,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryDoc {
    items: Vec<history::HistoryItem>,
}

static PENDING: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn gate() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|error| error.into_inner())
}

/// A reload that did not finish should not keep blocking the next save.
pub fn prepare(app: &AppHandle) {
    let _guard = gate();
    let mut ledger = load_ledger(app);
    let mut changed = false;
    for row in ledger.files.values_mut() {
        if row.inflight {
            row.inflight = false;
            changed = true;
        }
    }
    if changed {
        let _ = save_ledger(app, &ledger);
    }
    pending().clear();
}

/// Records the file currently on disk as this computer's base. A later edit from that base is not a conflict.
pub fn note(app: &AppHandle, path: &Path) {
    let Some(key) = relative_key(app, path) else {
        return;
    };
    let _guard = gate();
    let mut ledger = load_ledger(app);
    if ledger.files.contains_key(&key) {
        return;
    }
    if let Some(disk) = read_doc(path) {
        let machine = machine_id(app);
        save_base(app, &key, &disk.bytes);
        ledger.files.insert(key, row_from(&disk, &machine));
        let _ = save_ledger(app, &ledger);
    }
}

/// The sync folder changed. The files now on disk are the base, and earlier conflicts no longer apply.
pub fn rebase(app: &AppHandle) {
    let _guard = gate();
    let mut ledger = LedgerFile::default();
    let machine = machine_id(app);
    for path in watched_paths(app) {
        let Some(key) = relative_key(app, &path) else {
            continue;
        };
        if let Some(disk) = read_doc(&path) {
            save_base(app, &key, &disk.bytes);
            ledger.files.insert(key, row_from(&disk, &machine));
        }
    }
    let _ = save_ledger(app, &ledger);
    let _ = save_conflicts(app, &ConflictStore::default());
    pending().clear();
    publish(app, &[]);
}

/// Writes a watched profile file, or holds both copies when another computer changed it first.
pub fn store(app: &AppHandle, path: &Path, value: &impl Serialize) -> Result<(), String> {
    let Some(key) = relative_key(app, path) else {
        return files::write_json(path, value);
    };
    let content = content_of(value)?;
    let new_hash = content_hash(&content);
    let _guard = gate();
    let syncing = paths::storage_root().is_some();
    let machine = machine_id(app);
    let mut ledger = load_ledger(app);
    let mut conflicts = load_conflicts(app);
    let disk = read_doc(path);

    if let Some(item) = conflicts
        .items
        .iter_mut()
        .find(|item| item.path == key && item.cloud.is_empty())
    {
        if syncing {
            item.ours = content;
            item.time = now_ms();
            if let Some(disk) = &disk {
                item.theirs = disk.content.clone();
            }
            save_conflicts(app, &conflicts)?;
            publish(app, &conflicts.items);
            return Ok(());
        }
        conflicts
            .items
            .retain(|item| item.path != key || !item.cloud.is_empty());
        save_conflicts(app, &conflicts)?;
    }

    if syncing && ledger.files.get(&key).is_some_and(|row| row.inflight) {
        hold(
            &mut conflicts,
            app,
            &key,
            "",
            &content,
            disk.as_ref(),
            &ledger,
        )?;
        if let Some(row) = ledger.files.get_mut(&key) {
            row.inflight = false;
        }
        let _ = save_ledger(app, &ledger);
        pending().retain(|item| item != &key);
        save_conflicts(app, &conflicts)?;
        publish(app, &conflicts.items);
        return Ok(());
    }

    let disk_hash = disk.as_ref().map(|doc| content_hash(&doc.content));
    if disk_hash.as_deref() == Some(new_hash.as_str()) {
        if let Some(disk) = disk {
            ledger.files.insert(key, row_from(&disk, &machine));
            let _ = save_ledger(app, &ledger);
        }
        return Ok(());
    }

    let row = ledger.files.get(&key).cloned();
    let disk_rev = disk.as_ref().map(|doc| doc.stamp.revision).unwrap_or(0);
    let diverged = syncing
        && disk.is_some()
        && match &row {
            Some(row) => {
                disk_hash.as_deref() != Some(row.hash.as_str()) && disk_rev >= row.revision
            }
            None => true,
        };
    if diverged {
        hold(
            &mut conflicts,
            app,
            &key,
            "",
            &content,
            disk.as_ref(),
            &ledger,
        )?;
        save_conflicts(app, &conflicts)?;
        publish(app, &conflicts.items);
        return Ok(());
    }

    let parent = row.as_ref().map(|row| row.revision).unwrap_or(disk_rev);
    let stamp = SyncStamp {
        revision: parent.saturating_add(1).max(1),
        machine_id: machine,
        base_revision: disk_rev,
    };
    let bytes = write_stamped(path, &content, &stamp)?;
    save_base(app, &key, &bytes);
    ledger.files.insert(
        key,
        LedgerRow {
            hash: new_hash,
            revision: stamp.revision,
            machine_id: stamp.machine_id,
            base_revision: stamp.base_revision,
            inflight: false,
        },
    );
    save_ledger(app, &ledger)?;
    Ok(())
}

fn hold(
    conflicts: &mut ConflictStore,
    app: &AppHandle,
    key: &str,
    cloud: &str,
    ours: &Value,
    disk: Option<&DiskDoc>,
    ledger: &LedgerFile,
) -> Result<(), String> {
    let id = if cloud.is_empty() {
        format!("stamp:{key}")
    } else {
        format!("cloud:{cloud}")
    };
    let theirs = disk.map(|doc| doc.content.clone()).unwrap_or(Value::Null);
    let base = read_base_value(app, key).unwrap_or_else(|| {
        ledger
            .files
            .get(key)
            .and_then(|_| read_base_value(app, key))
            .unwrap_or(Value::Null)
    });
    if let Some(item) = conflicts.items.iter_mut().find(|item| item.id == id) {
        item.ours = ours.clone();
        item.theirs = theirs;
        item.time = now_ms();
        return Ok(());
    }
    conflicts.items.push(ConflictItem {
        id,
        path: key.to_string(),
        cloud: cloud.to_string(),
        ours: ours.clone(),
        theirs,
        base,
        time: now_ms(),
    });
    Ok(())
}

/// The copy this computer is keeping while a conflict is open. Profile loading uses it so a held
/// edit is still what this window shows.
/// Reads a profile file. While a conflict is open, this returns the copy this computer is keeping.
pub fn read_json<T: DeserializeOwned + Default>(app: &AppHandle, path: &Path) -> T {
    if let Some(value) = local_document(app, path)
        && let Ok(parsed) = serde_json::from_value(value)
    {
        return parsed;
    }
    files::read_json(path)
}

pub fn local_document(app: &AppHandle, path: &Path) -> Option<Value> {
    let key = relative_key(app, path)?;
    let _guard = gate();
    load_conflicts(app)
        .items
        .into_iter()
        .find(|item| item.path == key && item.cloud.is_empty())
        .map(|item| item.ours)
}

pub fn list(app: &AppHandle) -> Vec<SyncConflict> {
    let _guard = gate();
    views(&load_conflicts(app).items)
}

pub fn scan(app: &AppHandle) {
    if paths::storage_root().is_none() {
        return;
    }
    let _guard = gate();
    let mut ledger = load_ledger(app);
    let mut conflicts = load_conflicts(app);
    let machine = machine_id(app);
    let mut changed = false;
    for path in watched_paths(app) {
        let Some(key) = relative_key(app, &path) else {
            continue;
        };
        if conflicts
            .items
            .iter()
            .any(|item| item.path == key && item.cloud.is_empty())
        {
            if let Some(disk) = read_doc(&path)
                && let Some(item) = conflicts
                    .items
                    .iter_mut()
                    .find(|item| item.path == key && item.cloud.is_empty())
                && content_hash(&disk.content) != content_hash(&item.theirs)
            {
                item.theirs = disk.content;
                item.time = now_ms();
                changed = true;
            }
            continue;
        }
        let Some(disk) = read_doc(&path) else {
            continue;
        };
        let Some(row) = ledger.files.get(&key).cloned() else {
            save_base(app, &key, &disk.bytes);
            ledger.files.insert(key, row_from(&disk, &machine));
            changed = true;
            continue;
        };
        if row.inflight {
            continue;
        }
        let hash = content_hash(&disk.content);
        if hash == row.hash {
            if disk.stamp.revision > row.revision {
                if let Some(slot) = ledger.files.get_mut(&key) {
                    slot.revision = disk.stamp.revision;
                    slot.machine_id = disk.stamp.machine_id.clone();
                    slot.base_revision = disk.stamp.base_revision;
                }
                changed = true;
            } else if disk.stamp.revision < row.revision {
                let stamp = SyncStamp {
                    revision: row.revision,
                    machine_id: row.machine_id.clone(),
                    base_revision: row.base_revision,
                };
                if let Ok(bytes) = write_stamped(&path, &disk.content, &stamp) {
                    save_base(app, &key, &bytes);
                }
            }
            continue;
        }
        if disk.stamp.revision > row.revision {
            if let Some(slot) = ledger.files.get_mut(&key) {
                slot.inflight = true;
            }
            let mut waiting = pending();
            if !waiting.iter().any(|item| item == &key) {
                waiting.push(key);
            }
            changed = true;
            continue;
        }
        if disk.stamp.revision < row.revision {
            if let Some(bytes) = read_base(app, &key) {
                let _ = write_bytes(&path, &bytes);
            } else {
                save_base(app, &key, &disk.bytes);
                ledger.files.insert(key, row_from(&disk, &machine));
                changed = true;
            }
            continue;
        }
        let ours = read_base_value(app, &key).unwrap_or_else(|| disk.content.clone());
        if content_hash(&ours) == hash {
            ledger.files.insert(key, row_from(&disk, &machine));
            changed = true;
            continue;
        }
        let _ = hold(&mut conflicts, app, &key, "", &ours, Some(&disk), &ledger);
        changed = true;
    }
    let root = paths::data_root(app);
    let copies = cloud_copies(&root);
    let mut live_clouds = BTreeSet::new();
    for (copy, original) in copies {
        live_clouds.insert(copy.clone());
        if conflicts.items.iter().any(|item| item.cloud == copy) {
            continue;
        }
        let Some(disk) = read_doc(&resolve_relative(&root, &copy)) else {
            continue;
        };
        let ours = read_doc(&resolve_relative(&root, &original))
            .map(|doc| doc.content)
            .unwrap_or(Value::Null);
        let base = read_base_value(app, &original).unwrap_or(Value::Null);
        conflicts.items.push(ConflictItem {
            id: format!("cloud:{copy}"),
            path: original,
            cloud: copy,
            ours,
            theirs: disk.content,
            base,
            time: now_ms(),
        });
        changed = true;
    }
    let before = conflicts.items.len();
    conflicts
        .items
        .retain(|item| item.cloud.is_empty() || live_clouds.contains(&item.cloud));
    changed |= conflicts.items.len() != before;
    if changed {
        let _ = save_ledger(app, &ledger);
        let _ = save_conflicts(app, &conflicts);
        publish(app, &conflicts.items);
    }
}

pub fn apply_pending(app: &AppHandle) {
    let waiting = {
        let _guard = gate();
        pending().clone()
    };
    for key in waiting {
        let conflicted = {
            let _guard = gate();
            load_conflicts(app)
                .items
                .iter()
                .any(|item| item.path == key && item.cloud.is_empty())
        };
        if conflicted {
            finish_reload(app, &key, true);
            continue;
        }
        reload_document(app, &key);
        let conflicted = {
            let _guard = gate();
            load_conflicts(app)
                .items
                .iter()
                .any(|item| item.path == key && item.cloud.is_empty())
        };
        finish_reload(app, &key, conflicted);
        if !conflicted && key.ends_with("extensions.json") {
            let _ = crate::extensions::sync_on_startup(app);
        }
    }
}

fn finish_reload(app: &AppHandle, key: &str, conflicted: bool) {
    let _guard = gate();
    let mut ledger = load_ledger(app);
    if conflicted {
        if let Some(row) = ledger.files.get_mut(key) {
            row.inflight = false;
        }
    } else {
        let path = resolve_relative(&paths::data_root(app), key);
        if let Some(disk) = read_doc(&path) {
            let machine = machine_id(app);
            save_base(app, &key, &disk.bytes);
            ledger
                .files
                .insert(key.to_string(), row_from(&disk, &machine));
        } else if let Some(row) = ledger.files.get_mut(key) {
            row.inflight = false;
        }
    }
    let _ = save_ledger(app, &ledger);
    pending().retain(|item| item != key);
}

pub fn resolve(app: &AppHandle, id: &str, choice: &str) -> Result<(), String> {
    let current = settings::get(app);
    let machine = machine_id(app);
    let (key, cloud, written) = {
        let _guard = gate();
        let mut conflicts = load_conflicts(app);
        let index = conflicts
            .items
            .iter()
            .position(|item| item.id == id)
            .ok_or("That conflict is already resolved.")?;
        let item = conflicts.items[index].clone();
        if !matches!(choice, "mine" | "theirs" | "merge") {
            return Err("Choose which copy to keep.".into());
        }
        if choice == "merge" && !can_merge(&item.path) {
            return Err("Choose one copy for this file.".into());
        }
        let root = paths::data_root(app);
        let target = resolve_relative(&root, &item.path);
        if choice == "mine" && !item.cloud.is_empty() {
            let _ = fs::remove_file(resolve_relative(&root, &item.cloud));
            if let Some(disk) = read_doc(&target) {
                let mut ledger = load_ledger(app);
                save_base(app, &item.path, &disk.bytes);
                ledger
                    .files
                    .insert(item.path.clone(), row_from(&disk, &machine));
                save_ledger(app, &ledger)?;
            }
            conflicts.items.remove(index);
            save_conflicts(app, &conflicts)?;
            publish(app, &conflicts.items);
            return Ok(());
        }
        let mut chosen = match choice {
            "mine" => item.ours.clone(),
            "theirs" => item.theirs.clone(),
            "merge" => merge_documents(&item.path, &item.base, &item.ours, &item.theirs)?,
            _ => return Err("Choose which copy to keep.".into()),
        };
        if item.path.ends_with("settings.json") {
            preserve_local(&mut chosen, &current);
        }
        let mut ledger = load_ledger(app);
        let disk_rev = read_doc(&target).map(|doc| doc.stamp.revision).unwrap_or(0);
        let parent = ledger
            .files
            .get(&item.path)
            .map(|row| row.revision)
            .unwrap_or(0)
            .max(disk_rev);
        let stamp = SyncStamp {
            revision: parent.saturating_add(1).max(1),
            machine_id: machine.clone(),
            base_revision: disk_rev,
        };
        let bytes = write_stamped(&target, &chosen, &stamp)?;
        save_base(app, &item.path, &bytes);
        ledger.files.insert(
            item.path.clone(),
            LedgerRow {
                hash: content_hash(&chosen),
                revision: stamp.revision,
                machine_id: stamp.machine_id,
                base_revision: stamp.base_revision,
                inflight: false,
            },
        );
        if !item.cloud.is_empty() {
            let _ = fs::remove_file(resolve_relative(&root, &item.cloud));
        }
        conflicts.items.remove(index);
        save_ledger(app, &ledger)?;
        save_conflicts(app, &conflicts)?;
        publish(app, &conflicts.items);
        (item.path, item.cloud, true)
    };
    let _ = (cloud, written);
    reload_document(app, &key);
    Ok(())
}

fn reload_document(app: &AppHandle, relative: &str) {
    match file_name(relative) {
        "history.json" => history::reload(app),
        "bookmarks.json" => bookmarks::reload(app),
        "settings.json" => settings::reload(app),
        "extensions.json" => crate::extensions::adopt_saved(app),
        "profiles.json" => {
            let _ = app.emit("profiles-changed", profiles::list(app));
        }
        _ => {}
    }
}

fn preserve_local(value: &mut Value, settings: &settings::BrowserSettings) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    object.insert(
        "syncFolder".into(),
        Value::String(settings.sync_folder.clone()),
    );
    object.insert("syncRevision".into(), Value::from(settings.sync_revision));
    object.insert(
        "syncBookmarksRevision".into(),
        Value::from(settings.sync_bookmarks_revision),
    );
    object.insert(
        "syncExtensionsRevision".into(),
        Value::from(settings.sync_extensions_revision),
    );
    object.insert(
        "syncSettingsRevision".into(),
        Value::from(settings.sync_settings_revision),
    );
    object.insert(
        "syncHistoryRevision".into(),
        Value::from(settings.sync_history_revision),
    );
    object.insert(
        "assistantKey".into(),
        Value::String(settings.assistant_key.clone()),
    );
    object.insert(
        "downloadDir".into(),
        Value::String(settings.download_dir.clone()),
    );
}

fn merge_documents(
    path: &str,
    base: &Value,
    ours: &Value,
    theirs: &Value,
) -> Result<Value, String> {
    if path.ends_with("history.json") {
        let merged = merge_history_items(&items_of(base), &items_of(ours), &items_of(theirs));
        return serde_json::to_value(HistoryDoc { items: merged })
            .map_err(|error| error.to_string());
    }
    if path.ends_with("bookmarks.json") {
        let base_tree: bookmarks::BookmarkTree =
            serde_json::from_value(base.clone()).unwrap_or_default();
        let our_tree: bookmarks::BookmarkTree =
            serde_json::from_value(ours.clone()).unwrap_or_default();
        let their_tree: bookmarks::BookmarkTree =
            serde_json::from_value(theirs.clone()).unwrap_or_default();
        let merged = merge_bookmark_trees(&base_tree, &our_tree, &their_tree);
        return serde_json::to_value(merged).map_err(|error| error.to_string());
    }
    Err("Choose one copy for this file.".into())
}

pub(crate) fn merge_history_items(
    base: &[history::HistoryItem],
    ours: &[history::HistoryItem],
    theirs: &[history::HistoryItem],
) -> Vec<history::HistoryItem> {
    fn keyed(items: &[history::HistoryItem]) -> BTreeMap<String, history::HistoryItem> {
        let mut map = BTreeMap::new();
        for item in items {
            map.insert(paths::pin_key(&item.url), item.clone());
        }
        map
    }
    let base = keyed(base);
    let ours = keyed(ours);
    let theirs = keyed(theirs);
    let mut keys = BTreeSet::new();
    keys.extend(ours.keys().cloned());
    keys.extend(theirs.keys().cloned());
    let mut next_id = base
        .values()
        .chain(ours.values())
        .chain(theirs.values())
        .map(|item| item.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let mut merged = Vec::new();
    for key in keys {
        let base_item = base.get(&key);
        let our_item = ours.get(&key);
        let their_item = theirs.get(&key);
        let our_count = our_item.map(|item| item.visit_count).unwrap_or(0);
        let their_count = their_item.map(|item| item.visit_count).unwrap_or(0);
        let visit = if let Some(base_item) = base_item {
            let our_delta = if our_item.is_some() {
                our_count.saturating_sub(base_item.visit_count)
            } else {
                0
            };
            let their_delta = if their_item.is_some() {
                their_count.saturating_sub(base_item.visit_count)
            } else {
                0
            };
            base_item
                .visit_count
                .saturating_add(our_delta)
                .saturating_add(their_delta)
        } else {
            our_count.max(their_count)
        };
        let newer = match (our_item, their_item) {
            (Some(our), Some(their)) if our.last_visited >= their.last_visited => our,
            (Some(_), Some(their)) => their,
            (Some(our), None) => our,
            (None, Some(their)) => their,
            (None, None) => continue,
        };
        let id = our_item
            .or(their_item)
            .map(|item| item.id)
            .unwrap_or_else(|| {
                let id = next_id;
                next_id = next_id.saturating_add(1);
                id
            });
        merged.push(history::HistoryItem {
            id,
            url: newer.url.clone(),
            title: newer.title.clone(),
            visit_count: visit.max(1),
            last_visited: newer.last_visited,
            typed_count: our_item
                .map(|item| item.typed_count)
                .unwrap_or(0)
                .max(their_item.map(|item| item.typed_count).unwrap_or(0)),
        });
    }
    merged.sort_by(|left, right| right.last_visited.cmp(&left.last_visited));
    merged.truncate(2000);
    merged
}

pub(crate) fn merge_bookmark_trees(
    base: &bookmarks::BookmarkTree,
    ours: &bookmarks::BookmarkTree,
    theirs: &bookmarks::BookmarkTree,
) -> bookmarks::BookmarkTree {
    bookmarks::BookmarkTree {
        bar: merge_bookmark_node(&base.bar, &ours.bar, &theirs.bar),
        other: merge_bookmark_node(&base.other, &ours.other, &theirs.other),
    }
}

fn merge_bookmark_node(
    base: &bookmarks::BookmarkNode,
    ours: &bookmarks::BookmarkNode,
    theirs: &bookmarks::BookmarkNode,
) -> bookmarks::BookmarkNode {
    let mut node = ours.clone();
    if bookmark_payload_eq(ours, base) && !bookmark_payload_eq(theirs, base) {
        node.title.clone_from(&theirs.title);
        node.url.clone_from(&theirs.url);
        node.separator = theirs.separator;
    }
    let empty = Vec::new();
    node.children = Some(merge_bookmark_children(
        base.children.as_deref().unwrap_or(&empty),
        ours.children.as_deref().unwrap_or(&empty),
        theirs.children.as_deref().unwrap_or(&empty),
    ));
    if node.url.is_some() {
        node.children = None;
    }
    node
}

fn merge_bookmark_children(
    base: &[bookmarks::BookmarkNode],
    ours: &[bookmarks::BookmarkNode],
    theirs: &[bookmarks::BookmarkNode],
) -> Vec<bookmarks::BookmarkNode> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for child in ours {
        seen.insert(child.id.clone());
        let their = theirs.iter().find(|item| item.id == child.id);
        let base_child = base.iter().find(|item| item.id == child.id);
        if let Some(their) = their {
            let both = base_child.is_some_and(|base_child| {
                !bookmark_payload_eq(child, base_child)
                    && !bookmark_payload_eq(their, base_child)
                    && !bookmark_payload_eq(child, their)
            });
            result.push(merge_bookmark_node(
                base_child.unwrap_or(child),
                child,
                their,
            ));
            if both && their.url.is_some() {
                let mut copy = their.clone();
                copy.id = fresh_bookmark_id(&their.id, &seen);
                seen.insert(copy.id.clone());
                copy.children = None;
                result.push(copy);
            }
        } else {
            result.push(child.clone());
        }
    }
    for child in theirs {
        if seen.contains(&child.id) {
            continue;
        }
        seen.insert(child.id.clone());
        result.push(child.clone());
    }
    result
}

fn fresh_bookmark_id(id: &str, seen: &HashSet<String>) -> String {
    let mut candidate = format!("{id}-other");
    let mut index = 2;
    while seen.contains(&candidate) {
        candidate = format!("{id}-other-{index}");
        index += 1;
    }
    candidate
}

fn bookmark_payload_eq(left: &bookmarks::BookmarkNode, right: &bookmarks::BookmarkNode) -> bool {
    left.title == right.title && left.url == right.url && left.separator == right.separator
}

pub(crate) fn summarize(path: &str, ours: &Value, theirs: &Value) -> String {
    if path.ends_with("history.json") {
        let ours = items_of(ours);
        let theirs = items_of(theirs);
        let our_keys: BTreeSet<_> = ours.iter().map(|item| paths::pin_key(&item.url)).collect();
        let their_keys: BTreeSet<_> = theirs
            .iter()
            .map(|item| paths::pin_key(&item.url))
            .collect();
        let only_ours = our_keys.difference(&their_keys).count();
        let only_theirs = their_keys.difference(&our_keys).count();
        return format!("{only_ours} pages only on this computer, {only_theirs} only on the other");
    }
    if path.ends_with("bookmarks.json") {
        let ours = bookmark_links(ours);
        let theirs = bookmark_links(theirs);
        let only_ours = ours.keys().filter(|id| !theirs.contains_key(*id)).count();
        let only_theirs = theirs.keys().filter(|id| !ours.contains_key(*id)).count();
        let edited = ours
            .iter()
            .filter(|(id, payload)| theirs.get(*id).is_some_and(|their| their != *payload))
            .count();
        return format!(
            "{only_ours} bookmarks only on this computer, {only_theirs} only on the other, {edited} edited on both"
        );
    }
    let keys = changed_keys(ours, theirs);
    if keys.is_empty() {
        "The copies match except for sync details".into()
    } else {
        format!("Changed: {}", keys.join(", "))
    }
}

fn bookmark_links(value: &Value) -> BTreeMap<String, String> {
    let Ok(tree) = serde_json::from_value::<bookmarks::BookmarkTree>(value.clone()) else {
        return BTreeMap::new();
    };
    let mut links = BTreeMap::new();
    fn walk(node: &bookmarks::BookmarkNode, links: &mut BTreeMap<String, String>) {
        if node.url.is_some() {
            links.insert(
                node.id.clone(),
                format!("{}|{}", node.title, node.url.as_deref().unwrap_or_default()),
            );
        }
        for child in node.children.as_deref().unwrap_or(&[]) {
            walk(child, links);
        }
    }
    walk(&tree.bar, &mut links);
    walk(&tree.other, &mut links);
    links
}

fn changed_keys(ours: &Value, theirs: &Value) -> Vec<String> {
    let (Some(our_object), Some(their_object)) = (ours.as_object(), theirs.as_object()) else {
        return vec!["the file".into()];
    };
    let mut keys: Vec<String> = our_object
        .keys()
        .chain(their_object.keys())
        .cloned()
        .collect();
    keys.sort();
    keys.dedup();
    keys.retain(|key| key != "syncStamp" && our_object.get(key) != their_object.get(key));
    keys.truncate(8);
    keys
}

fn items_of(value: &Value) -> Vec<history::HistoryItem> {
    serde_json::from_value::<HistoryDoc>(value.clone())
        .map(|doc| doc.items)
        .unwrap_or_default()
}

fn views(items: &[ConflictItem]) -> Vec<SyncConflict> {
    items
        .iter()
        .map(|item| SyncConflict {
            id: item.id.clone(),
            path: item.path.clone(),
            label: label_for(&item.path),
            time: item.time,
            summary: summarize(&item.path, &item.ours, &item.theirs),
            can_merge: can_merge(&item.path),
        })
        .collect()
}

fn publish(app: &AppHandle, items: &[ConflictItem]) {
    let _ = app.emit("sync-conflicts", views(items));
}

fn can_merge(path: &str) -> bool {
    path.ends_with("bookmarks.json") || path.ends_with("history.json")
}

fn label_for(path: &str) -> String {
    let name = match file_name(path) {
        "bookmarks.json" => "Bookmarks",
        "history.json" => "History",
        "settings.json" => "Settings",
        "extensions.json" => "Extensions",
        "profiles.json" => "Profiles",
        other => other,
    };
    match path.rsplit_once('/') {
        Some((dir, _)) => format!("{dir} / {name}"),
        None => name.to_string(),
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn pending() -> std::sync::MutexGuard<'static, Vec<String>> {
    PENDING.lock().unwrap_or_else(|error| error.into_inner())
}

fn relative_key(app: &AppHandle, path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    if !WATCHED.contains(&name) {
        return None;
    }
    let root = paths::data_root(app);
    let relative = path.strip_prefix(&root).ok()?;
    let parts = relative.components().count();
    if name == "profiles.json" {
        if parts != 1 {
            return None;
        }
    } else if parts != 2 {
        return None;
    }
    Some(relative.to_string_lossy().replace('\\', "/"))
}

fn watched_paths(app: &AppHandle) -> Vec<PathBuf> {
    let root = paths::data_root(app);
    let mut paths = Vec::new();
    let registry = root.join("profiles.json");
    if registry.is_file() {
        paths.push(registry);
    }
    let Ok(entries) = fs::read_dir(&root) else {
        return paths;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        for name in WATCHED {
            if *name == "profiles.json" {
                continue;
            }
            let path = entry.path().join(name);
            if path.is_file() {
                paths.push(path);
            }
        }
    }
    paths
}

fn cloud_copies(root: &Path) -> Vec<(String, String)> {
    let mut found = Vec::new();
    collect_cloud(root, root, &mut found);
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                collect_cloud(root, &entry.path(), &mut found);
            }
        }
    }
    found
}

fn collect_cloud(root: &Path, dir: &Path, found: &mut Vec<(String, String)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !is_conflict_name(&name) {
            continue;
        }
        let Some(original) = original_watched(&name) else {
            continue;
        };
        let copy = entry.path();
        let Some(relative) = copy.strip_prefix(root).ok() else {
            continue;
        };
        let relative = relative.to_string_lossy().replace('\\', "/");
        let original_path = if let Some((dir, _)) = relative.rsplit_once('/') {
            format!("{dir}/{original}")
        } else {
            original.to_string()
        };
        found.push((relative, original_path));
    }
}

pub(crate) fn is_conflict_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("sync-conflict")
        || lower.contains("conflicted copy")
        || lower.contains("(conflict")
}

fn original_watched(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    if !lower.ends_with(".json") {
        return None;
    }
    let mut best: Option<&str> = None;
    for watched in WATCHED {
        let stem = watched.trim_end_matches(".json");
        if lower.contains(stem)
            && best.is_none_or(|current| stem.len() > current.trim_end_matches(".json").len())
        {
            best = Some(*watched);
        }
    }
    best
}

fn resolve_relative(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .filter(|part| !part.is_empty())
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

fn content_of(value: &impl Serialize) -> Result<Value, String> {
    let mut value = serde_json::to_value(value).map_err(|error| error.to_string())?;
    if let Some(object) = value.as_object_mut() {
        object.remove("syncStamp");
    }
    Ok(value)
}

fn content_hash(value: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical(value).as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn canonical(value: &Value) -> String {
    let mut out = String::new();
    write_canonical(value, &mut out);
    out
}

fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map
                .keys()
                .filter(|key| key.as_str() != "syncStamp")
                .collect();
            keys.sort();
            out.push('{');
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).unwrap_or_else(|_| "\"\"".into()));
                out.push(':');
                write_canonical(&map[*key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        other => out.push_str(&serde_json::to_string(other).unwrap_or_else(|_| "null".into())),
    }
}

fn read_doc(path: &Path) -> Option<DiskDoc> {
    let bytes = fs::read(path).ok()?;
    let mut value: Value = serde_json::from_slice(&bytes).ok()?;
    let stamp = value
        .as_object_mut()
        .and_then(|object| object.remove("syncStamp"))
        .and_then(|stamp| serde_json::from_value(stamp).ok())
        .unwrap_or_default();
    Some(DiskDoc {
        bytes,
        content: value,
        stamp,
    })
}

fn row_from(disk: &DiskDoc, machine: &str) -> LedgerRow {
    LedgerRow {
        hash: content_hash(&disk.content),
        revision: disk.stamp.revision,
        machine_id: if disk.stamp.machine_id.is_empty() {
            machine.to_string()
        } else {
            disk.stamp.machine_id.clone()
        },
        base_revision: disk.stamp.base_revision,
        inflight: false,
    }
}

fn write_stamped(path: &Path, content: &Value, stamp: &SyncStamp) -> Result<Vec<u8>, String> {
    let mut value = content.clone();
    let Some(object) = value.as_object_mut() else {
        files::write_json(path, content)?;
        return serde_json::to_vec_pretty(content).map_err(|error| error.to_string());
    };
    object.insert(
        "syncStamp".into(),
        serde_json::to_value(stamp).map_err(|error| error.to_string())?,
    );
    files::write_json(path, &value)?;
    serde_json::to_vec_pretty(&value).map_err(|error| error.to_string())
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temp = path.with_extension("json.push");
    fs::write(&temp, bytes).map_err(|error| error.to_string())?;
    fs::rename(&temp, path).map_err(|error| error.to_string())
}

fn ledger_path(app: &AppHandle) -> PathBuf {
    paths::root_data_dir(app).join("sync-ledger.json")
}

fn conflict_path(app: &AppHandle) -> PathBuf {
    paths::root_data_dir(app).join("sync-conflicts.json")
}

fn load_ledger(app: &AppHandle) -> LedgerFile {
    files::read_json(&ledger_path(app))
}

fn save_ledger(app: &AppHandle, ledger: &LedgerFile) -> Result<(), String> {
    files::write_json(&ledger_path(app), ledger)
}

fn load_conflicts(app: &AppHandle) -> ConflictStore {
    files::read_json(&conflict_path(app))
}

fn save_conflicts(app: &AppHandle, conflicts: &ConflictStore) -> Result<(), String> {
    files::write_json(&conflict_path(app), conflicts)
}

fn base_path(app: &AppHandle, key: &str) -> PathBuf {
    let safe = key.replace(['/', '\\'], "__");
    paths::root_data_dir(app).join("sync-base").join(safe)
}

fn save_base(app: &AppHandle, key: &str, bytes: &[u8]) {
    let path = base_path(app, key);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, bytes);
}

fn read_base(app: &AppHandle, key: &str) -> Option<Vec<u8>> {
    fs::read(base_path(app, key)).ok()
}

fn read_base_value(app: &AppHandle, key: &str) -> Option<Value> {
    let bytes = read_base(app, key)?;
    let mut value: Value = serde_json::from_slice(&bytes).ok()?;
    if let Some(object) = value.as_object_mut() {
        object.remove("syncStamp");
    }
    Some(value)
}

fn machine_id(app: &AppHandle) -> String {
    let path = paths::root_data_dir(app).join("machine-id.json");
    if let Ok(text) = fs::read_to_string(&path)
        && let Ok(value) = serde_json::from_str::<Value>(&text)
        && let Some(id) = value.get("id").and_then(|id| id.as_str())
        && !id.is_empty()
    {
        return id.to_string();
    }
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let id = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let _ = files::write_json(&path, &serde_json::json!({ "id": id }));
    id
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bookmarks::BookmarkNode;
    use history::HistoryItem;

    fn item(id: u32, url: &str, title: &str, visits: u32, when: u64) -> HistoryItem {
        HistoryItem {
            id,
            url: url.into(),
            title: title.into(),
            visit_count: visits,
            last_visited: when,
            typed_count: 0,
        }
    }

    #[test]
    fn history_merge_adds_visits_from_both_sides_of_the_base() {
        let base = vec![item(1, "https://example.com/a", "A", 3, 10)];
        let ours = vec![
            item(1, "https://example.com/a", "A later", 4, 30),
            item(2, "https://example.com/ours", "Ours", 1, 40),
        ];
        let theirs = vec![
            item(1, "https://example.com/a/", "A other", 5, 20),
            item(3, "https://example.com/theirs", "Theirs", 2, 50),
        ];
        let merged = merge_history_items(&base, &ours, &theirs);
        let shared = merged.iter().find(|item| item.url.contains("/a")).unwrap();
        assert_eq!(shared.visit_count, 6);
        assert_eq!(shared.title, "A later");
        assert_eq!(merged.len(), 3);
        assert!(merged.iter().any(|item| item.url.contains("ours")));
        assert!(merged.iter().any(|item| item.url.contains("theirs")));
    }

    #[test]
    fn bookmark_merge_keeps_additions_and_both_edits() {
        let link = |id: &str, title: &str| BookmarkNode {
            id: id.into(),
            title: title.into(),
            url: Some(format!("https://example.com/{id}")),
            children: None,
            separator: false,
            added_at: 1,
        };
        let folder = |children: Vec<BookmarkNode>| BookmarkNode {
            id: "bar".into(),
            title: "Bookmarks bar".into(),
            url: None,
            children: Some(children),
            separator: false,
            added_at: 1,
        };
        let other = BookmarkNode {
            id: "other".into(),
            title: "Other bookmarks".into(),
            url: None,
            children: Some(Vec::new()),
            separator: false,
            added_at: 1,
        };
        let base = bookmarks::BookmarkTree {
            bar: folder(vec![link("a", "Alpha")]),
            other: other.clone(),
        };
        let ours = bookmarks::BookmarkTree {
            bar: folder(vec![link("a", "Alpha ours"), link("b", "Beta")]),
            other: other.clone(),
        };
        let theirs = bookmarks::BookmarkTree {
            bar: folder(vec![link("a", "Alpha theirs"), link("c", "Gamma")]),
            other,
        };
        let merged = merge_bookmark_trees(&base, &ours, &theirs);
        let children = merged.bar.children.unwrap();
        let titles: Vec<_> = children.iter().map(|node| node.title.as_str()).collect();
        assert!(titles.contains(&"Alpha ours"));
        assert!(titles.contains(&"Alpha theirs"));
        assert!(titles.contains(&"Beta"));
        assert!(titles.contains(&"Gamma"));
    }

    #[test]
    fn cloud_conflict_names_are_recognized() {
        assert!(is_conflict_name(
            "bookmarks.sync-conflict-20241031-120000-ABCDEF.json"
        ));
        assert!(is_conflict_name(
            "settings (conflicted copy 2024-10-31).json"
        ));
        assert!(is_conflict_name("history (Conflict from laptop).json"));
        assert!(!is_conflict_name("bookmarks.json"));
        assert!(!is_conflict_name("notes.txt"));
    }

    #[test]
    fn a_settings_summary_names_the_changed_fields() {
        let ours = serde_json::json!({"theme": "zinc", "homePage": "ce://newtab"});
        let theirs = serde_json::json!({"theme": "nord", "homePage": "ce://newtab"});
        assert_eq!(
            summarize("Default/settings.json", &ours, &theirs),
            "Changed: theme"
        );
    }
}
