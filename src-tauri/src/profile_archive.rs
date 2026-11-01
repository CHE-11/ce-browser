use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::AppHandle;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::{files, paths, profiles};

const MAGIC: &[u8; 4] = b"CEPF";
const VERSION: u8 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const MAX_UNPACKED: u64 = 512 * 1024 * 1024;
const MAX_ENTRIES: usize = 20_000;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedProfile {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    format: u32,
    name: String,
    exported_at: u64,
}

pub fn export_archive(app: &AppHandle, path: &Path, password: &str) -> Result<(), String> {
    if password.is_empty() {
        return Err("Enter a password.".into());
    }
    let dir = paths::data_dir(app);
    let name = profiles::list(app)
        .into_iter()
        .find(|profile| profile.current)
        .map(|profile| profile.name)
        .unwrap_or_else(|| "Profile".into());
    let packed = pack_profile(&dir, &name)?;
    let sealed = seal(&packed, password)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, sealed).map_err(|error| format!("Could not write {}: {error}", path.display()))
}

pub fn import_archive(
    app: &AppHandle,
    path: &Path,
    password: &str,
) -> Result<ImportedProfile, String> {
    if password.is_empty() {
        return Err("Enter a password.".into());
    }
    let sealed =
        fs::read(path).map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let packed = open_sealed(&sealed, password)?;
    let temp = std::env::temp_dir().join(format!("ce-profile-{}", now_ms()));
    fs::create_dir_all(&temp).map_err(|error| error.to_string())?;
    let unpacked = unpack_archive(&packed, &temp);
    let result = unpacked.and_then(|_| import_unpacked(app, &temp));
    let _ = fs::remove_dir_all(&temp);
    result
}

fn import_unpacked(app: &AppHandle, temp: &Path) -> Result<ImportedProfile, String> {
    let manifest = read_manifest(&temp.join("manifest.json"))?;
    validate_profile_json(&temp.join("settings.json"), "settings")?;
    validate_profile_json(&temp.join("bookmarks.json"), "bookmarks")?;
    let id = profiles::create_closed(app, &manifest.name)?;
    let dest = paths::profile_data_dir(app, &id);
    if let Err(error) = copy_profile_files(temp, &dest) {
        let _ = profiles::delete(app, &id);
        return Err(error);
    }
    let name = profiles::list(app)
        .into_iter()
        .find(|profile| profile.id == id)
        .map(|profile| profile.name)
        .unwrap_or(manifest.name);
    Ok(ImportedProfile { id, name })
}

fn pack_profile(dir: &Path, name: &str) -> Result<Vec<u8>, String> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        let manifest = Manifest {
            format: 1,
            name: clean_name(name),
            exported_at: now_ms(),
        };
        write_zip_file(
            &mut zip,
            "manifest.json",
            &serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?,
            options,
        )?;
        for name in profiles::PROFILE_FILES {
            let path = dir.join(name);
            if !path.is_file() {
                continue;
            }
            let mut bytes = fs::read(&path)
                .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
            if *name == "settings.json" {
                bytes = strip_sync_settings(&bytes)?;
            }
            write_zip_file(&mut zip, name, &bytes, options)?;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                if entry.path().is_file()
                    && name.starts_with("window-icon")
                    && name.ends_with(".ico")
                {
                    let bytes = fs::read(entry.path()).map_err(|error| error.to_string())?;
                    write_zip_file(&mut zip, &name, &bytes, options)?;
                }
            }
        }
        for name in profiles::PROFILE_DIRS {
            let folder = dir.join(name);
            if folder.is_dir() {
                add_tree(&mut zip, &folder, name, options)?;
            }
        }
        zip.finish().map_err(|error| error.to_string())?;
    }
    Ok(cursor.into_inner())
}

fn write_zip_file<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    name: &str,
    bytes: &[u8],
    options: SimpleFileOptions,
) -> Result<(), String> {
    zip.start_file(name, options)
        .map_err(|error| error.to_string())?;
    zip.write_all(bytes).map_err(|error| error.to_string())
}

fn add_tree<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    dir: &Path,
    prefix: &str,
    options: SimpleFileOptions,
) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|error| format!("Could not read {}: {error}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "." || name == ".." || name.contains('/') || name.contains('\\') {
            continue;
        }
        let stored = format!("{prefix}/{name}");
        if path.is_dir() {
            add_tree(zip, &path, &stored, options)?;
        } else if path.is_file() {
            let bytes = fs::read(&path)
                .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
            write_zip_file(zip, &stored, &bytes, options)?;
        }
    }
    Ok(())
}

fn unpack_archive(bytes: &[u8], dest: &Path) -> Result<(), String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| "This profile file is damaged.".to_string())?;
    if archive.len() > MAX_ENTRIES {
        return Err("This profile file has too many files.".into());
    }
    let mut total = 0u64;
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|_| "This profile file is damaged.".to_string())?;
        let name = file.name().replace('\\', "/");
        if !safe_zip_path(&name) {
            return Err("This profile file contains an unsafe path.".into());
        }
        if file.is_dir() {
            fs::create_dir_all(dest.join(&name)).map_err(|error| error.to_string())?;
            continue;
        }
        let size = file.size();
        total = total.saturating_add(size);
        if total > MAX_UNPACKED {
            return Err("This profile file is too large.".into());
        }
        let target = dest.join(&name);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut output = File::create(&target).map_err(|error| error.to_string())?;
        let mut limited = file.take(MAX_UNPACKED);
        std::io::copy(&mut limited, &mut output).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn copy_profile_files(from: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|error| error.to_string())?;
    for name in profiles::PROFILE_FILES {
        let source = from.join(name);
        if !source.is_file() {
            continue;
        }
        let mut bytes = fs::read(&source).map_err(|error| error.to_string())?;
        if *name == "settings.json" {
            bytes = strip_sync_settings(&bytes)?;
        }
        fs::write(dest.join(name), bytes)
            .map_err(|error| format!("Could not write {}: {error}", name))?;
    }
    if let Ok(entries) = fs::read_dir(from) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if entry.path().is_file() && name.starts_with("window-icon") && name.ends_with(".ico") {
                fs::copy(entry.path(), dest.join(name.as_ref()))
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    for name in profiles::PROFILE_DIRS {
        let folder = from.join(name);
        if folder.is_dir() {
            files::copy_dir(&folder, &dest.join(name))?;
        }
    }
    Ok(())
}

fn read_manifest(path: &Path) -> Result<Manifest, String> {
    let text =
        fs::read_to_string(path).map_err(|_| "This profile file has no manifest.".to_string())?;
    let manifest: Manifest = serde_json::from_str(&text)
        .map_err(|_| "This profile file has no manifest.".to_string())?;
    if manifest.format != 1 {
        return Err("This profile file is from a newer version of ce-browser.".into());
    }
    Ok(Manifest {
        format: 1,
        name: clean_name(&manifest.name),
        exported_at: manifest.exported_at,
    })
}

fn validate_profile_json(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| format!("The {label} in this profile file are damaged."))?;
    if !value.is_object() {
        return Err(format!("The {label} in this profile file are damaged."));
    }
    if label == "bookmarks" {
        let _: crate::bookmarks::BookmarkTree = serde_json::from_value(value)
            .map_err(|_| "The bookmarks in this profile file are damaged.".to_string())?;
    }
    Ok(())
}

pub(crate) fn strip_sync_settings(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut value: Value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(_) => return Ok(bytes.to_vec()),
    };
    if let Some(object) = value.as_object_mut() {
        object.insert("syncFolder".into(), Value::String(String::new()));
        for key in [
            "syncRevision",
            "syncBookmarksRevision",
            "syncExtensionsRevision",
            "syncSettingsRevision",
            "syncHistoryRevision",
        ] {
            object.insert(key.into(), Value::from(0));
        }
    }
    serde_json::to_vec_pretty(&value).map_err(|error| error.to_string())
}

fn clean_name(name: &str) -> String {
    let trimmed = name.trim();
    let name = if trimmed.is_empty() {
        "Imported"
    } else {
        trimmed
    };
    name.chars().take(64).collect()
}

pub(crate) fn safe_zip_path(name: &str) -> bool {
    if name.is_empty() || name.starts_with('/') || name.starts_with('\\') {
        return false;
    }
    if name.contains(':') {
        return false;
    }
    !name
        .split(['/', '\\'])
        .any(|part| part.is_empty() || part == "." || part == "..")
}

pub(crate) fn seal(plain: &[u8], password: &str) -> Result<Vec<u8>, String> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let key = derive_key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| "Could not encrypt the profile.".to_string())?;
    let encrypted = cipher
        .encrypt(Nonce::from_slice(&nonce), plain)
        .map_err(|_| "Could not encrypt the profile.".to_string())?;
    let mut out = Vec::with_capacity(4 + 1 + SALT_LEN + NONCE_LEN + encrypted.len());
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&encrypted);
    Ok(out)
}

pub(crate) fn open_sealed(bytes: &[u8], password: &str) -> Result<Vec<u8>, String> {
    if bytes.len() < 4 + 1 + SALT_LEN + NONCE_LEN + 16 || bytes.get(..4) != Some(MAGIC) {
        return Err("This is not a ce-browser profile file.".into());
    }
    if bytes[4] != VERSION {
        return Err("This profile file is from a newer version of ce-browser.".into());
    }
    let salt = &bytes[5..5 + SALT_LEN];
    let nonce = &bytes[5 + SALT_LEN..5 + SALT_LEN + NONCE_LEN];
    let encrypted = &bytes[5 + SALT_LEN + NONCE_LEN..];
    let key = derive_key(password, salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| "That password did not open this profile file.".to_string())?;
    cipher
        .decrypt(Nonce::from_slice(nonce), encrypted)
        .map_err(|_| "That password did not open this profile file.".to_string())
}

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let params = Params::new(19_456, 2, 1, Some(32)).map_err(|error| error.to_string())?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|_| "Could not derive a key from that password.".to_string())?;
    Ok(key)
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

    #[test]
    fn a_sealed_profile_opens_with_the_same_password() {
        let sealed = seal(b"profile-bytes", "correct horse").unwrap();
        let opened = open_sealed(&sealed, "correct horse").unwrap();
        assert_eq!(opened, b"profile-bytes");
        assert!(open_sealed(&sealed, "wrong").is_err());
        assert!(open_sealed(b"not a profile", "correct horse").is_err());
    }

    #[test]
    fn export_clears_the_sync_folder_and_keeps_the_other_settings() {
        let stripped = strip_sync_settings(
            br#"{"syncFolder":"C:/sync","theme":"zinc","syncRevision":4,"assistantKey":"secret"}"#,
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&stripped).unwrap();
        assert_eq!(value["syncFolder"], "");
        assert_eq!(value["syncRevision"], 0);
        assert_eq!(value["theme"], "zinc");
        assert_eq!(value["assistantKey"], "secret");
    }

    #[test]
    fn zip_paths_cannot_escape_the_profile() {
        assert!(safe_zip_path("settings.json"));
        assert!(safe_zip_path("extensions/id/manifest.json"));
        assert!(!safe_zip_path("../settings.json"));
        assert!(!safe_zip_path("/etc/passwd"));
        assert!(!safe_zip_path("extensions/../../outside.json"));
        assert!(!safe_zip_path("C:/outside.json"));
    }
}
