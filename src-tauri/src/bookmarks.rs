use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::paths;

const FILE: &str = "bookmarks.json";
pub const BAR: &str = "bar";
pub const OTHER: &str = "other";

/// A folder has `children` (possibly empty). A link has a `url`. A separator has neither.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BookmarkNode {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
    pub children: Option<Vec<BookmarkNode>>,
    pub separator: bool,
    pub added_at: u64,
}

impl BookmarkNode {
    fn folder(id: &str, title: &str) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            url: None,
            children: Some(Vec::new()),
            separator: false,
            added_at: now_ms(),
        }
    }

    fn is_folder(&self) -> bool {
        !self.separator && self.url.is_none()
    }

    fn find(&self, id: &str) -> Option<&BookmarkNode> {
        if self.id == id {
            return Some(self);
        }
        self.children
            .as_ref()?
            .iter()
            .find_map(|child| child.find(id))
    }

    fn find_mut(&mut self, id: &str) -> Option<&mut BookmarkNode> {
        if self.id == id {
            return Some(self);
        }
        self.children
            .as_mut()?
            .iter_mut()
            .find_map(|child| child.find_mut(id))
    }

    fn take(&mut self, id: &str) -> Option<BookmarkNode> {
        let children = self.children.as_mut()?;
        if let Some(index) = children.iter().position(|child| child.id == id) {
            return Some(children.remove(index));
        }
        children.iter_mut().find_map(|child| child.take(id))
    }

    /// Folder that holds `id`, and the position inside it.
    fn parent_of(&self, id: &str) -> Option<(String, usize)> {
        let children = self.children.as_ref()?;
        if let Some(index) = children.iter().position(|child| child.id == id) {
            return Some((self.id.clone(), index));
        }
        children.iter().find_map(|child| child.parent_of(id))
    }

    /// Links in this folder and every folder inside it, in order.
    fn links(&self, out: &mut Vec<(String, String)>) {
        if let Some(url) = self.url.as_ref().filter(|_| !self.separator) {
            out.push((self.title.clone(), url.clone()));
        }
        if let Some(children) = self.children.as_ref() {
            for child in children {
                child.links(out);
            }
        }
    }

    fn find_url(&self, url: &str) -> Option<&BookmarkNode> {
        if self.url.as_deref() == Some(url) {
            return Some(self);
        }
        self.children
            .as_ref()?
            .iter()
            .find_map(|child| child.find_url(url))
    }

    /// Older files may contain a folder with `children: null`.
    fn repair(&mut self) {
        if self.id.is_empty() {
            self.id = new_id();
        }
        if self.separator {
            self.url = None;
            self.children = None;
        } else if self.url.is_none() && self.children.is_none() {
            self.children = Some(Vec::new());
        }
        if let Some(children) = self.children.as_mut() {
            children.iter_mut().for_each(BookmarkNode::repair);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BookmarkTree {
    pub bar: BookmarkNode,
    pub other: BookmarkNode,
}

impl Default for BookmarkTree {
    fn default() -> Self {
        Self {
            bar: BookmarkNode::folder(BAR, "Bookmarks bar"),
            other: BookmarkNode::folder(OTHER, "Other bookmarks"),
        }
    }
}

impl BookmarkTree {
    fn roots(&self) -> [&BookmarkNode; 2] {
        [&self.bar, &self.other]
    }

    fn find(&self, id: &str) -> Option<&BookmarkNode> {
        self.roots().into_iter().find_map(|root| root.find(id))
    }

    fn find_mut(&mut self, id: &str) -> Option<&mut BookmarkNode> {
        if let Some(found) = self.bar.find_mut(id) {
            return Some(found);
        }
        self.other.find_mut(id)
    }

    fn take(&mut self, id: &str) -> Option<BookmarkNode> {
        self.bar.take(id).or_else(|| self.other.take(id))
    }

    fn insert(
        &mut self,
        parent_id: &str,
        index: Option<usize>,
        node: BookmarkNode,
    ) -> Result<(), String> {
        let parent = self
            .find_mut(parent_id)
            .ok_or("That folder no longer exists.")?;
        let children = parent
            .children
            .as_mut()
            .ok_or("Bookmarks can only go inside folders.")?;
        let index = index.unwrap_or(children.len()).min(children.len());
        children.insert(index, node);
        Ok(())
    }

    fn normalize(mut self) -> Self {
        self.bar.id = BAR.into();
        self.other.id = OTHER.into();
        self.bar.url = None;
        self.other.url = None;
        if self.bar.title.is_empty() {
            self.bar.title = "Bookmarks bar".into();
        }
        if self.other.title.is_empty() {
            self.other.title = "Other bookmarks".into();
        }
        self.bar.repair();
        self.other.repair();
        self
    }
}

pub struct Bookmarks(Mutex<BookmarkTree>);

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn new_id() -> String {
    format!(
        "b{:x}{:x}",
        now_ms(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

pub fn init(app: &AppHandle) {
    crate::sync_conflicts::note(app, &paths::data_file(app, FILE));
    let tree: BookmarkTree = crate::sync_conflicts::read_json(app, &paths::data_file(app, FILE));
    app.manage(Bookmarks(Mutex::new(tree.normalize())));
}

pub fn reload(app: &AppHandle) {
    let tree: BookmarkTree = crate::sync_conflicts::read_json(app, &paths::data_file(app, FILE));
    let tree = tree.normalize();
    *app.state::<Bookmarks>().0.lock().expect("bookmarks lock") = tree.clone();
    let _ = app.emit("bookmarks-changed", &tree);
}

pub fn tree(app: &AppHandle) -> BookmarkTree {
    app.state::<Bookmarks>()
        .0
        .lock()
        .expect("bookmarks lock")
        .clone()
}

fn change<T>(
    app: &AppHandle,
    edit: impl FnOnce(&mut BookmarkTree) -> Result<T, String>,
) -> Result<T, String> {
    let (result, snapshot) = {
        let state = app.state::<Bookmarks>();
        let mut tree = state.0.lock().expect("bookmarks lock");
        let result = edit(&mut tree)?;
        (result, tree.clone())
    };
    crate::sync_conflicts::store(app, &paths::data_file(app, FILE), &snapshot)?;
    let _ = app.emit("bookmarks-changed", &snapshot);
    crate::sync::touch();
    Ok(result)
}

pub fn replace(app: &AppHandle, tree: BookmarkTree) -> Result<(), String> {
    let tree = tree.normalize();
    crate::sync_conflicts::store(app, &paths::data_file(app, FILE), &tree)?;
    *app.state::<Bookmarks>().0.lock().expect("bookmarks lock") = tree.clone();
    let _ = app.emit("bookmarks-changed", &tree);
    Ok(())
}

pub fn add(
    app: &AppHandle,
    parent_id: &str,
    title: &str,
    url: Option<String>,
    index: Option<usize>,
    separator: bool,
) -> Result<BookmarkNode, String> {
    let node = if separator {
        BookmarkNode {
            id: new_id(),
            title: String::new(),
            url: None,
            children: None,
            separator: true,
            added_at: now_ms(),
        }
    } else {
        let url = url
            .map(|url| url.trim().to_string())
            .filter(|url| !url.is_empty());
        let title = match (title.trim(), &url) {
            ("", Some(url)) => url.clone(),
            ("", None) => "New folder".to_string(),
            (title, _) => title.to_string(),
        };
        BookmarkNode {
            id: new_id(),
            title,
            children: if url.is_none() {
                Some(Vec::new())
            } else {
                None
            },
            url,
            separator: false,
            added_at: now_ms(),
        }
    };
    let created = node.clone();
    change(app, |tree| tree.insert(parent_id, index, node))?;
    Ok(created)
}

pub fn update(app: &AppHandle, id: &str, title: &str, url: Option<String>) -> Result<(), String> {
    change(app, |tree| {
        let node = tree.find_mut(id).ok_or("That bookmark no longer exists.")?;
        if node.separator {
            node.title = title.trim().to_string();
            node.url = None;
            node.children = None;
            return Ok(());
        }
        node.title = title.trim().to_string();
        if !node.is_folder() {
            let url = url
                .map(|url| url.trim().to_string())
                .filter(|url| !url.is_empty());
            node.url = Some(url.ok_or("A bookmark needs an address.")?);
            if node.title.is_empty() {
                node.title = node.url.clone().unwrap_or_default();
            }
        }
        Ok(())
    })
}

pub fn move_to(
    app: &AppHandle,
    id: &str,
    parent_id: &str,
    index: Option<usize>,
) -> Result<(), String> {
    if id == BAR || id == OTHER {
        return Err("The bookmarks bar and Other bookmarks cannot be moved.".into());
    }
    change(app, |tree| {
        let node = tree.find(id).ok_or("That bookmark no longer exists.")?;
        if node.find(parent_id).is_some() {
            return Err("A folder cannot be moved into itself.".into());
        }
        let parent = tree
            .find(parent_id)
            .ok_or("That folder no longer exists.")?;
        let siblings = parent
            .children
            .as_ref()
            .ok_or("Bookmarks can only go inside folders.")?;
        let current = siblings.iter().position(|child| child.id == id);
        let node = tree.take(id).ok_or("That bookmark no longer exists.")?;
        let index = match (index, current) {
            (Some(index), Some(current)) if current < index => Some(index - 1),
            (index, _) => index,
        };
        tree.insert(parent_id, index, node)
    })
}

/// Removes a bookmark and returns it with its folder and position, so it can be put back.
pub fn delete_returning(
    app: &AppHandle,
    id: &str,
) -> Result<(BookmarkNode, String, usize), String> {
    if id == BAR || id == OTHER {
        return Err("The bookmarks bar and Other bookmarks cannot be deleted.".into());
    }
    change(app, |tree| {
        let (parent, index) = tree
            .roots()
            .into_iter()
            .find_map(|root| root.parent_of(id))
            .ok_or("That bookmark no longer exists.")?;
        let node = tree.take(id).ok_or("That bookmark no longer exists.")?;
        Ok((node, parent, index))
    })
}

/// Puts a deleted bookmark back. A folder that is gone too sends it to Other bookmarks.
pub fn restore(
    app: &AppHandle,
    node: BookmarkNode,
    parent: &str,
    index: usize,
) -> Result<(), String> {
    change(app, |tree| {
        if tree.find(&node.id).is_some() {
            return Ok(());
        }
        let parent = if tree.find(parent).is_some_and(BookmarkNode::is_folder) {
            parent.to_string()
        } else {
            OTHER.to_string()
        };
        tree.insert(&parent, Some(index), node)
    })
}

/// A new folder under Other bookmarks holding these links. Returns the folder.
pub fn add_folder_of_links(
    app: &AppHandle,
    title: &str,
    links: &[(String, String)],
) -> Result<BookmarkNode, String> {
    let title = match title.trim() {
        "" => "Saved tabs".to_string(),
        title => title.to_string(),
    };
    let mut folder = BookmarkNode::folder(&new_id(), &title);
    folder.children = Some(
        links
            .iter()
            .filter(|(_, url)| !url.trim().is_empty())
            .map(|(title, url)| BookmarkNode {
                id: new_id(),
                title: if title.trim().is_empty() {
                    url.clone()
                } else {
                    title.trim().to_string()
                },
                url: Some(url.trim().to_string()),
                children: None,
                separator: false,
                added_at: now_ms(),
            })
            .collect(),
    );
    let created = folder.clone();
    change(app, |tree| tree.insert(OTHER, None, folder))?;
    Ok(created)
}

/// A folder's title and the links inside it, including links in its subfolders.
pub fn folder_links(app: &AppHandle, id: &str) -> Result<(String, Vec<(String, String)>), String> {
    let tree = tree(app);
    let folder = tree.find(id).ok_or("That folder no longer exists.")?;
    if !folder.is_folder() {
        return Err("Only a folder can open as a group.".into());
    }
    let mut links = Vec::new();
    folder.links(&mut links);
    Ok((folder.title.clone(), links))
}

pub fn find_by_url(app: &AppHandle, url: &str) -> Option<BookmarkNode> {
    let tree = tree(app);
    tree.roots()
        .into_iter()
        .find_map(|root| root.find_url(url))
        .cloned()
}

pub fn export_html(app: &AppHandle, path: &Path) -> Result<(), String> {
    let tree = tree(app);
    let mut out = String::from(
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n<TITLE>Bookmarks</TITLE>\n<H1>Bookmarks</H1>\n<DL><p>\n",
    );
    write_folder(&mut out, &tree.bar, 1, true);
    for child in tree.other.children.iter().flatten() {
        write_node(&mut out, child, 1);
    }
    out.push_str("</DL><p>\n");
    fs::write(path, out).map_err(|error| format!("Could not write {}: {error}", path.display()))
}

fn write_folder(out: &mut String, folder: &BookmarkNode, depth: usize, toolbar: bool) {
    let indent = "    ".repeat(depth);
    let toolbar = if toolbar {
        " PERSONAL_TOOLBAR_FOLDER=\"true\""
    } else {
        ""
    };
    out.push_str(&format!(
        "{indent}<DT><H3 ADD_DATE=\"{}\"{toolbar}>{}</H3>\n{indent}<DL><p>\n",
        folder.added_at / 1000,
        escape(&folder.title)
    ));
    for child in folder.children.iter().flatten() {
        write_node(out, child, depth + 1);
    }
    out.push_str(&format!("{indent}</DL><p>\n"));
}

fn write_node(out: &mut String, node: &BookmarkNode, depth: usize) {
    if node.separator {
        let indent = "    ".repeat(depth);
        let title = node.title.trim();
        if title.is_empty() {
            out.push_str(&format!("{indent}<HR>\n"));
        } else {
            out.push_str(&format!("{indent}<HR TITLE=\"{}\">\n", escape(title)));
        }
        return;
    }
    match &node.url {
        Some(url) => out.push_str(&format!(
            "{}<DT><A HREF=\"{}\" ADD_DATE=\"{}\">{}</A>\n",
            "    ".repeat(depth),
            escape(url),
            node.added_at / 1000,
            escape(&node.title)
        )),
        None => write_folder(out, node, depth, false),
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&amp;", "&")
}

/// Imports a Netscape bookmark file (Chrome, Edge, Firefox export) into an "Imported" folder
/// under Other bookmarks. The exporting browser's toolbar folder goes onto the bookmarks bar.
pub fn import_html(app: &AppHandle, path: &Path) -> Result<usize, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let (imported, toolbar, count) = parse_html(&text);
    if count == 0 {
        return Err("No bookmarks were found in that file.".into());
    }
    change(app, |tree| {
        if let Some(bar) = tree.bar.children.as_mut() {
            bar.extend(toolbar);
        }
        if imported
            .children
            .as_ref()
            .is_some_and(|children| !children.is_empty())
        {
            tree.insert(OTHER, None, imported)?;
        }
        Ok(())
    })?;
    Ok(count)
}

struct Frame {
    node: BookmarkNode,
    toolbar: bool,
}

fn parse_html(text: &str) -> (BookmarkNode, Vec<BookmarkNode>, usize) {
    let mut root = BookmarkNode::folder(&new_id(), "Imported");
    let mut toolbar_items = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    let mut pending: Option<Frame> = None;
    let mut count = 0;
    let lower = text.to_ascii_lowercase();
    let mut cursor = 0;

    while let Some(offset) = lower[cursor..].find('<') {
        let start = cursor + offset;
        let Some(end_offset) = lower[start..].find('>') else {
            break;
        };
        let end = start + end_offset;
        let tag = &lower[start + 1..end];
        let raw_tag = &text[start + 1..end];
        cursor = end + 1;

        if tag.starts_with("h3") {
            let close = lower[cursor..]
                .find("</h3>")
                .map(|index| cursor + index)
                .unwrap_or(cursor);
            let title = unescape(text[cursor..close].trim());
            let mut node = BookmarkNode::folder(&new_id(), &title);
            node.added_at = attribute(raw_tag, "add_date")
                .and_then(|value| value.parse::<u64>().ok())
                .map(|seconds| seconds * 1000)
                .unwrap_or(node.added_at);
            pending = Some(Frame {
                node,
                toolbar: tag.contains("personal_toolbar_folder"),
            });
            cursor = close;
        } else if tag.starts_with("dl") {
            if let Some(frame) = pending.take() {
                stack.push(frame);
            }
        } else if tag.starts_with("/dl") {
            if let Some(frame) = stack.pop() {
                let parent = stack
                    .last_mut()
                    .map(|parent| &mut parent.node)
                    .unwrap_or(&mut root);
                if frame.toolbar {
                    toolbar_items.extend(frame.node.children.unwrap_or_default());
                } else if let Some(children) = parent.children.as_mut() {
                    children.push(frame.node);
                }
            }
        } else if tag == "hr" || tag.starts_with("hr ") || tag.starts_with("hr/") {
            let node = BookmarkNode {
                id: new_id(),
                title: attribute(raw_tag, "title").unwrap_or_default(),
                url: None,
                children: None,
                separator: true,
                added_at: now_ms(),
            };
            count += 1;
            let parent = stack
                .last_mut()
                .map(|parent| &mut parent.node)
                .unwrap_or(&mut root);
            if let Some(children) = parent.children.as_mut() {
                children.push(node);
            }
        } else if tag.starts_with("a ") || tag == "a" {
            let close = lower[cursor..]
                .find("</a>")
                .map(|index| cursor + index)
                .unwrap_or(cursor);
            let title = unescape(text[cursor..close].trim());
            cursor = close;
            let Some(href) = attribute(raw_tag, "href") else {
                continue;
            };
            let href = unescape(&href);
            if href.starts_with("javascript:") || href.starts_with("place:") {
                continue;
            }
            let node = BookmarkNode {
                id: new_id(),
                title: if title.is_empty() {
                    href.clone()
                } else {
                    title
                },
                url: Some(href),
                children: None,
                separator: false,
                added_at: attribute(raw_tag, "add_date")
                    .and_then(|value| value.parse::<u64>().ok())
                    .map(|seconds| seconds * 1000)
                    .unwrap_or_else(now_ms),
            };
            count += 1;
            let parent = stack
                .last_mut()
                .map(|parent| &mut parent.node)
                .unwrap_or(&mut root);
            if let Some(children) = parent.children.as_mut() {
                children.push(node);
            }
        }
    }
    while let Some(frame) = stack.pop() {
        let parent = stack
            .last_mut()
            .map(|parent| &mut parent.node)
            .unwrap_or(&mut root);
        if let Some(children) = parent.children.as_mut() {
            children.push(frame.node);
        }
    }
    (root, toolbar_items, count)
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let key = format!("{name}=");
    let mut search = 0;
    while let Some(found) = lower[search..].find(&key) {
        let index = search + found;
        let boundary = index == 0 || lower.as_bytes()[index - 1].is_ascii_whitespace();
        search = index + key.len();
        if !boundary {
            continue;
        }
        let rest = &tag[index + key.len()..];
        return Some(match rest.chars().next() {
            Some(quote @ ('"' | '\'')) => rest[1..]
                .split(quote)
                .next()
                .unwrap_or_default()
                .to_string(),
            _ => rest
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string(),
        });
    }
    None
}
