use std::ffi::{OsStr, c_void};
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[link(name = "shell32")]
unsafe extern "system" {
    fn ShellExecuteW(
        hwnd: *mut c_void,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show_cmd: i32,
    ) -> isize;
}

pub fn reveal_path(path: &Path) -> Result<(), String> {
    let mut command = Command::new("explorer.exe");
    if path.is_file() {
        command.arg("/select,").arg(path);
    } else if path.exists() {
        command.arg(path);
    } else {
        let parent = path
            .parent()
            .filter(|parent| parent.exists())
            .ok_or_else(|| format!("Path does not exist: {}", path.display()))?;
        command.arg(parent);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Could not show {} in Explorer: {error}", path.display()))?;
    Ok(())
}

pub fn open_with_default(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("File does not exist: {}", path.display()));
    }
    let file = wide(path.as_os_str());
    let operation = wide(OsStr::new("open"));
    let code = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if code > 32 {
        return Ok(());
    }
    Err(format!("Could not open {} (error {code}).", path.display()))
}

/// Returns `dir/name`, or `dir/name (n).ext` when that file already exists.
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let name = sanitize_file_name(name);
    let candidate = dir.join(&name);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(&name);
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.clone());
    let extension = path
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    (1..)
        .map(|index| dir.join(format!("{stem} ({index}){extension}")))
        .find(|candidate| !candidate.exists())
        .unwrap_or(candidate)
}

pub fn sanitize_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            character if character.is_control() => '_',
            character => character,
        })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches('.').to_string();
    if trimmed.is_empty() {
        "download".to_string()
    } else {
        trimmed
    }
}

pub fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to)
        .map_err(|error| format!("Could not create {}: {error}", to.display()))?;
    let entries = fs::read_dir(from)
        .map_err(|error| format!("Could not read {}: {error}", from.display()))?;
    for entry in entries.flatten() {
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            copy_dir(&source, &target)?;
        } else {
            fs::copy(&source, &target)
                .map_err(|error| format!("Could not copy {}: {error}", source.display()))?;
        }
    }
    Ok(())
}

/// A file that exists but no longer parses is kept as `<name>.json.bak` so the next save
/// does not silently replace the user's data.
pub fn read_json<T: serde::de::DeserializeOwned + Default>(path: &Path) -> T {
    let Ok(text) = fs::read_to_string(path) else {
        return T::default();
    };
    match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(_) => {
            let _ = fs::copy(path, path.with_extension("json.bak"));
            T::default()
        }
    }
}

pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, text)
        .map_err(|error| format!("Could not write {}: {error}", temp.display()))?;
    fs::rename(&temp, path).map_err(|error| format!("Could not write {}: {error}", path.display()))
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}
