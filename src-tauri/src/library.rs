//! `ce://files`: a folder listing and a text preview for JSON, CSV, Markdown, and logs.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::accelerator;

pub const PREVIEW_LIMIT: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub directory: bool,
    pub size: u64,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BrowseResult {
    #[serde(rename_all = "camelCase")]
    Directory {
        path: String,
        parent: Option<String>,
        entries: Vec<FileEntry>,
    },
    #[serde(rename_all = "camelCase")]
    File {
        path: String,
        parent: Option<String>,
        name: String,
        kind: String,
        text: String,
        too_big: bool,
        size: u64,
    },
}

pub fn browse(path: &Path) -> Result<BrowseResult, String> {
    if path.is_dir() {
        return Ok(BrowseResult::Directory {
            path: path.to_string_lossy().into_owned(),
            parent: parent_of(path),
            entries: list_dir(path)?,
        });
    }
    if !path.is_file() {
        return Err(format!("That path does not exist: {}", path.display()));
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let kind = accelerator::preview_kind(&name).to_string();
    let size = fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    let (text, too_big) = if kind == "other" {
        (String::new(), false)
    } else if size > PREVIEW_LIMIT {
        (String::new(), true)
    } else {
        let bytes =
            fs::read(path).map_err(|error| format!("Couldn't read {}. {error}", path.display()))?;
        let text = String::from_utf8(bytes).map_err(|_| "This file is not text.".to_string())?;
        (text, false)
    };
    Ok(BrowseResult::File {
        path: path.to_string_lossy().into_owned(),
        parent: parent_of(path),
        name,
        kind,
        text,
        too_big,
        size,
    })
}

fn list_dir(path: &Path) -> Result<Vec<FileEntry>, String> {
    let mut entries = Vec::new();
    let read =
        fs::read_dir(path).map_err(|error| format!("Couldn't open {}. {error}", path.display()))?;
    for entry in read.flatten() {
        let file_path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let directory = file_path.is_dir();
        let size = if directory {
            0
        } else {
            entry.metadata().map(|meta| meta.len()).unwrap_or(0)
        };
        entries.push(FileEntry {
            kind: if directory {
                "folder".into()
            } else {
                accelerator::preview_kind(&name).into()
            },
            name,
            path: file_path.to_string_lossy().into_owned(),
            directory,
            size,
        });
    }
    entries.sort_by(|left, right| {
        right.directory.cmp(&left.directory).then_with(|| {
            left.name
                .to_ascii_lowercase()
                .cmp(&right.name.to_ascii_lowercase())
        })
    });
    Ok(entries)
}

fn parent_of(path: &Path) -> Option<String> {
    path.parent()
        .filter(|parent| *parent != path && parent.exists())
        .map(PathBuf::from)
        .map(|parent| parent.to_string_lossy().into_owned())
}
