use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::{files, paths};

const FILE: &str = "styles.css";
const MAX_BYTES: usize = 256 * 1024;

/// Starter file. The rules stay inside a comment until the user uncomments them.
const TEMPLATE: &str = "\
/* ce-browser user styles for this profile.
   This file is styles.css, next to settings.json.
   Uncomment the block below to override the theme chosen in Settings.
   950 is the page, 900 is panels and menus, 800 is fields, 100 is text.
   The browser loads this file when it opens. Saving from Settings applies it immediately.
*/

/*
html[data-theme][data-mode] {
  --app-zinc-950: #09090b;
  --app-zinc-900: #18181b;
  --app-zinc-800: #27272a;
  --app-zinc-700: #3f3f46;
  --app-zinc-600: #52525b;
  --app-zinc-500: #71717a;
  --app-zinc-400: #a1a1aa;
  --app-zinc-300: #d4d4d8;
  --app-zinc-200: #e4e4e7;
  --app-zinc-100: #f4f4f5;
  --app-zinc-50: #fafafa;

  --app-sky-300: #7dd3fc;
  --app-sky-400: #38bdf8;
  --app-sky-500: #0ea5e9;
  --app-sky-600: #0284c7;
  --app-sky-800: #075985;
  --app-sky-950: #082f49;

  --app-violet-100: #ede9fe;
  --app-violet-200: #ddd6fe;
  --app-violet-300: #c4b5fd;
  --app-violet-500: #8b5cf6;
  --app-violet-600: #7c3aed;
  --app-violet-700: #6d28d9;
  --app-violet-800: #5b21b6;
  --app-violet-900: #4c1d95;
  --app-violet-950: #2e1065;

  --app-emerald-300: #6ee7b7;
  --app-emerald-400: #34d399;
  --app-emerald-500: #10b981;
  --app-emerald-600: #059669;
  --app-emerald-950: #022c22;

  --app-amber-200: #fde68a;
  --app-amber-300: #fcd34d;
  --app-amber-400: #fbbf24;
  --app-amber-900: #78350f;
  --app-amber-950: #451a03;

  --app-red-300: #fca5a5;
  --app-red-400: #f87171;
  --app-red-500: #ef4444;
  --app-red-600: #dc2626;
  --app-red-900: #7f1d1d;
  --app-red-950: #450a0a;
}
*/
";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserStyles {
    pub css: String,
    pub path: String,
}

pub fn template() -> &'static str {
    TEMPLATE
}

pub fn ensure(app: &AppHandle) -> Result<(), String> {
    let path = file_path(app);
    if path.is_file() {
        return Ok(());
    }
    write_text(&path, TEMPLATE)
}

pub fn get(app: &AppHandle) -> Result<UserStyles, String> {
    ensure(app)?;
    read(app)
}

pub fn save(app: &AppHandle, css: &str) -> Result<UserStyles, String> {
    if css.len() > MAX_BYTES {
        return Err("That stylesheet is too large.".into());
    }
    let path = file_path(app);
    write_text(&path, css)?;
    let styles = read(app)?;
    publish(app, &styles);
    Ok(styles)
}

/// Reads the file again and tells every open view to apply it.
pub fn reload(app: &AppHandle) -> Result<UserStyles, String> {
    ensure(app)?;
    let styles = read(app)?;
    publish(app, &styles);
    Ok(styles)
}

pub fn show(app: &AppHandle) -> Result<(), String> {
    ensure(app)?;
    files::reveal_path(&file_path(app))
}

fn read(app: &AppHandle) -> Result<UserStyles, String> {
    let path = file_path(app);
    let css = fs::read_to_string(&path).map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    Ok(UserStyles {
        css,
        path: path.to_string_lossy().into_owned(),
    })
}

fn publish(app: &AppHandle, styles: &UserStyles) {
    let _ = app.emit("user-styles-changed", styles);
}

fn file_path(app: &AppHandle) -> PathBuf {
    paths::data_file(app, FILE)
}

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    }
    let temp = path.with_extension("css.tmp");
    fs::write(&temp, text).map_err(|error| format!("Could not write {}: {error}", temp.display()))?;
    fs::rename(&temp, path).map_err(|error| format!("Could not write {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::TEMPLATE;

    #[test]
    fn starter_css_is_commented_out() {
        assert!(TEMPLATE.contains("--app-zinc-950"));
        assert!(TEMPLATE.contains("html[data-theme][data-mode]"));
        let mut comment = false;
        for line in TEMPLATE.lines() {
            let mut rest = line.trim();
            while !rest.is_empty() {
                if comment {
                    let Some(end) = rest.find("*/") else {
                        break;
                    };
                    comment = false;
                    rest = rest[end + 2..].trim_start();
                } else if let Some(start) = rest.find("/*") {
                    assert_eq!(start, 0, "active css before a comment: {line}");
                    comment = true;
                    rest = rest[start + 2..].trim_start();
                } else {
                    panic!("active css outside a comment: {line}");
                }
            }
        }
        assert!(!comment, "the starter file leaves a comment open");
    }
}
