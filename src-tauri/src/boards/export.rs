//! Writing pins and folders back to disk, so a folder exported here imports as the same tree.

use std::fs;
use std::path::{Path, PathBuf};

use tauri::AppHandle;

use super::model::{BoardsSnapshot, Pin};
use super::{snapshot, storage};
use crate::files;

/// File name for a pin's original: its file name, or its title with the stored extension.
pub fn file_name_for(pin: &Pin) -> String {
    let ext = if pin.ext.is_empty() {
        "png"
    } else {
        pin.ext.as_str()
    };
    let base = if !pin.file_name.trim().is_empty() {
        pin.file_name.trim().to_string()
    } else if !pin.title.trim().is_empty() {
        pin.title.trim().chars().take(120).collect()
    } else {
        pin.id.clone()
    };
    let name = files::sanitize_file_name(&base);
    let has_ext = name
        .rsplit_once('.')
        .is_some_and(|(_, current)| current.eq_ignore_ascii_case(ext));
    if has_ext {
        name
    } else {
        format!("{name}.{ext}")
    }
}

/// Writes one pin into `dir`. Links without a file become `.url` shortcuts.
fn write_pin(app: &AppHandle, pin: &Pin, dir: &Path) -> Result<(), String> {
    if let Some(source) = storage::original_path(app, pin) {
        let target = files::unique_path(dir, &file_name_for(pin));
        fs::copy(&source, &target)
            .map(|_| ())
            .map_err(|error| format!("Couldn't export {}. {error}", pin.title))
    } else {
        let url = if pin.link_url.is_empty() {
            &pin.page_url
        } else {
            &pin.link_url
        };
        if url.is_empty() {
            return Ok(());
        }
        let title = if pin.title.trim().is_empty() {
            "Link"
        } else {
            pin.title.trim()
        };
        let target = files::unique_path(dir, &format!("{}.url", files::sanitize_file_name(title)));
        fs::write(&target, format!("[InternetShortcut]\r\nURL={url}\r\n"))
            .map_err(|error| format!("Couldn't export {}. {error}", pin.title))
    }
}

fn ensure(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| format!("Couldn't create {}. {error}", dir.display()))
}

/// Writes the pins into `dir`. Returns how many were written.
pub fn export_pins(app: &AppHandle, ids: &[String], dir: &Path) -> Result<usize, String> {
    ensure(dir)?;
    let boards = snapshot(app);
    let mut written = 0;
    let mut first_error = None;
    for pin in boards.pins.iter().filter(|pin| ids.contains(&pin.id)) {
        match write_pin(app, pin, dir) {
            Ok(()) => written += 1,
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    if written == 0 {
        if let Some(error) = first_error {
            return Err(error);
        }
    }
    Ok(written)
}

/// Writes the folder as `dir/<name>`, with a subdirectory for each subfolder. Returns the
/// directory made.
pub fn export_folder(app: &AppHandle, id: &str, dir: &Path) -> Result<PathBuf, String> {
    let boards = snapshot(app);
    let folder = boards
        .folders
        .iter()
        .find(|folder| folder.id == id)
        .ok_or("That folder no longer exists.")?;
    ensure(dir)?;
    let target = files::unique_path(dir, &folder.name);
    write_folder(app, &boards, id, &target)?;
    Ok(target)
}

fn write_folder(
    app: &AppHandle,
    boards: &BoardsSnapshot,
    id: &str,
    dir: &Path,
) -> Result<(), String> {
    ensure(dir)?;
    for pin in boards
        .pins
        .iter()
        .filter(|pin| !pin.in_trash() && pin.folder_ids.iter().any(|folder| folder == id))
    {
        write_pin(app, pin, dir)?;
    }
    for child in boards
        .folders
        .iter()
        .filter(|folder| folder.parent_id.as_deref() == Some(id))
    {
        write_folder(
            app,
            boards,
            &child.id,
            &files::unique_path(dir, &child.name),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_names_keep_the_stored_extension() {
        let pin = Pin {
            title: "Sunset".into(),
            ext: "jpg".into(),
            ..Pin::default()
        };
        assert_eq!(file_name_for(&pin), "Sunset.jpg");
        let pin = Pin {
            file_name: "report.PDF".into(),
            ext: "pdf".into(),
            ..Pin::default()
        };
        assert_eq!(file_name_for(&pin), "report.PDF");
        let pin = Pin {
            title: "a/b: c".into(),
            ext: "png".into(),
            ..Pin::default()
        };
        assert_eq!(file_name_for(&pin), "a_b_ c.png");
    }
}
