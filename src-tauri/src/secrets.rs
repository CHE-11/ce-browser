//! API keys for Shopping summaries, encrypted with Windows DPAPI for the current user.
//! Ciphertext lives under local app data, never in the syncable profile folder.

use std::fs;
use std::path::PathBuf;

use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{files, paths, profiles};

const FILE_NAME: &str = "keys.json";
pub const UNREADABLE: &str = "The saved key could not be read on this computer.";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiKeyStatus {
    pub has_key: bool,
    /// Last four characters, when a key could be read. Never the full key.
    pub hint: String,
    /// A matching environment variable is set and no saved key is in use.
    pub from_env: bool,
    /// Ciphertext is present but DPAPI could not decrypt it.
    pub unreadable: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct SecretFile {
    keys: Vec<SecretSlot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SecretSlot {
    provider: String,
    ciphertext: String,
}

pub fn status(app: &AppHandle, provider: &str) -> AiKeyStatus {
    let provider = clean_provider(provider);
    match read_plain(app, &provider) {
        Ok(Some(plain)) => AiKeyStatus {
            has_key: true,
            hint: hint_of(&plain),
            from_env: false,
            unreadable: false,
        },
        Ok(None) => AiKeyStatus {
            has_key: false,
            hint: String::new(),
            from_env: env_key(&provider).is_some(),
            unreadable: false,
        },
        Err(_) => AiKeyStatus {
            has_key: false,
            hint: String::new(),
            from_env: false,
            unreadable: true,
        },
    }
}

pub fn store_key(app: &AppHandle, provider: &str, key: &str) -> Result<(), String> {
    let provider = clean_provider(provider);
    let key = key.trim();
    if key.is_empty() {
        return Err("Enter a key first.".into());
    }
    if key.chars().count() > 512 {
        return Err("That key is too long.".into());
    }
    let ciphertext =
        protect(key).map_err(|_| "Couldn't save the key on this computer.".to_string())?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(ciphertext);
    let mut file = read_file(app);
    file.keys.retain(|slot| slot.provider != provider);
    file.keys.push(SecretSlot {
        provider,
        ciphertext: encoded,
    });
    write_file(app, &file)
}

pub fn clear_key(app: &AppHandle, provider: &str) -> Result<(), String> {
    let provider = clean_provider(provider);
    let mut file = read_file(app);
    file.keys.retain(|slot| slot.provider != provider);
    write_file(app, &file)
}

/// Saved key, or the provider's environment variable when nothing is saved.
/// The value is for the request only. Callers must not log it or return it.
pub fn resolve(app: &AppHandle, provider: &str) -> Result<Option<String>, String> {
    let provider = clean_provider(provider);
    match read_plain(app, &provider) {
        Ok(Some(key)) => Ok(Some(key)),
        Ok(None) => Ok(env_key(&provider)),
        Err(_) => Err(UNREADABLE.to_string()),
    }
}

pub fn env_key(provider: &str) -> Option<String> {
    let names: &[&str] = match provider {
        "openai" => &["OPENAI_API_KEY"],
        "claude" => &["ANTHROPIC_API_KEY", "CLAUDE_API_KEY"],
        _ => return None,
    };
    names.iter().find_map(|name| {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn clean_provider(provider: &str) -> String {
    match provider.trim() {
        "openai" | "claude" | "custom" | "local" => provider.trim().to_string(),
        _ => "local".into(),
    }
}

fn hint_of(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() < 4 {
        return String::new();
    }
    chars[chars.len() - 4..].iter().collect()
}

fn secret_path(app: &AppHandle) -> PathBuf {
    key_path(app, &profiles::active_id())
}

fn key_path(app: &AppHandle, id: &str) -> PathBuf {
    profile_secret_file(app, id, FILE_NAME)
}

fn profile_secret_file(app: &AppHandle, id: &str, file: &str) -> PathBuf {
    let name = if id == profiles::DEFAULT_ID {
        file.to_string()
    } else {
        format!("{id}-{file}")
    };
    paths::root_local_dir(app).join("secrets").join(name)
}

/// Named values automations put into pages as `{{secret:name}}`. Same DPAPI protection as the
/// API keys, in a file of its own so a provider key and a vault entry never share a name.
fn vault_path(app: &AppHandle) -> PathBuf {
    profile_secret_file(app, &profiles::active_id(), VAULT_FILE_NAME)
}

const VAULT_FILE_NAME: &str = "automation-secrets.json";

/// Lowercase letters, digits, `.`, `-` and `_`, up to 64 characters.
pub fn clean_secret_name(raw: &str) -> Option<String> {
    let name = raw.trim().to_ascii_lowercase();
    let valid = (1..=64).contains(&name.len())
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_'));
    valid.then_some(name)
}

pub fn vault_names(app: &AppHandle) -> Vec<String> {
    let file: SecretFile = files::read_json(&vault_path(app));
    let mut names: Vec<String> = file.keys.into_iter().map(|slot| slot.provider).collect();
    names.sort();
    names.dedup();
    names
}

pub fn vault_store(app: &AppHandle, name: &str, value: &str) -> Result<(), String> {
    let name = clean_secret_name(name)
        .ok_or("Use letters, digits, dots, dashes or underscores for the name.")?;
    if value.is_empty() {
        return Err("Enter a value first.".into());
    }
    if value.chars().count() > 4096 {
        return Err("That value is too long.".into());
    }
    let ciphertext =
        protect(value).map_err(|_| "Couldn't save the secret on this computer.".to_string())?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(ciphertext);
    let path = vault_path(app);
    let mut file: SecretFile = files::read_json(&path);
    file.keys.retain(|slot| slot.provider != name);
    file.keys.push(SecretSlot {
        provider: name,
        ciphertext: encoded,
    });
    files::write_json(&path, &file)
}

pub fn vault_clear(app: &AppHandle, name: &str) -> Result<(), String> {
    let Some(name) = clean_secret_name(name) else {
        return Ok(());
    };
    let path = vault_path(app);
    let mut file: SecretFile = files::read_json(&path);
    file.keys.retain(|slot| slot.provider != name);
    files::write_json(&path, &file)
}

/// The plain value of a vault entry. Callers must not log it, return it, or send it to a model.
pub fn vault_resolve(app: &AppHandle, name: &str) -> Result<Option<String>, String> {
    let Some(name) = clean_secret_name(name) else {
        return Ok(None);
    };
    let file: SecretFile = files::read_json(&vault_path(app));
    let Some(slot) = file.keys.iter().find(|slot| slot.provider == name) else {
        return Ok(None);
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(slot.ciphertext.trim())
        .map_err(|_| UNREADABLE.to_string())?;
    let plain = unprotect(&bytes).map_err(|_| UNREADABLE.to_string())?;
    Ok(Some(plain))
}

const MCP_FILE_NAME: &str = "mcp-token.json";

/// The bearer token MCP clients send. Created on first use; `regenerate` replaces it.
pub fn mcp_token(app: &AppHandle, regenerate: bool) -> Result<String, String> {
    let path = profile_secret_file(app, &profiles::active_id(), MCP_FILE_NAME);
    if !regenerate {
        let file: SecretFile = files::read_json(&path);
        if let Some(slot) = file.keys.first()
            && let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(slot.ciphertext.trim())
            && let Ok(plain) = unprotect(&bytes)
            && plain.len() >= 32
        {
            return Ok(plain);
        }
    }
    let mut raw = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut raw);
    let token: String = raw.iter().map(|byte| format!("{byte:02x}")).collect();
    let ciphertext =
        protect(&token).map_err(|_| "Couldn't save the MCP token on this computer.".to_string())?;
    let file = SecretFile {
        keys: vec![SecretSlot {
            provider: "mcp".into(),
            ciphertext: base64::engine::general_purpose::STANDARD.encode(ciphertext),
        }],
    };
    files::write_json(&path, &file)?;
    Ok(token)
}

/// Copies this user's saved API keys from one profile to another. The ciphertext stays
/// DPAPI-protected and is never written into the syncable profile folder.
pub fn copy_keys(app: &AppHandle, from_id: &str, to_id: &str) -> Result<(), String> {
    let from = key_path(app, from_id);
    if !from.is_file() {
        return Ok(());
    }
    let to = key_path(app, to_id);
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::copy(&from, &to).map_err(|error| format!("Could not copy saved keys: {error}"))?;
    Ok(())
}

fn read_file(app: &AppHandle) -> SecretFile {
    files::read_json(&secret_path(app))
}

fn write_file(app: &AppHandle, file: &SecretFile) -> Result<(), String> {
    files::write_json(&secret_path(app), file)
}

fn read_plain(app: &AppHandle, provider: &str) -> Result<Option<String>, ()> {
    let file = read_file(app);
    let Some(slot) = file.keys.iter().find(|slot| slot.provider == provider) else {
        return Ok(None);
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(slot.ciphertext.trim())
        .map_err(|_| ())?;
    let plain = unprotect(&bytes).map_err(|_| ())?;
    if plain.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(plain))
}

fn protect(plain: &str) -> Result<Vec<u8>, ()> {
    protect_bytes(plain.as_bytes())
}

fn unprotect(ciphertext: &[u8]) -> Result<String, ()> {
    let bytes = unprotect_bytes(ciphertext)?;
    String::from_utf8(bytes).map_err(|_| ())
}

#[cfg(windows)]
fn protect_bytes(plain: &[u8]) -> Result<Vec<u8>, ()> {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData,
    };

    if plain.is_empty() {
        return Err(());
    }
    let input = CRYPT_INTEGER_BLOB {
        cbData: plain.len() as u32,
        pbData: plain.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(
            &input,
            windows::core::PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|_| ())?;
        if output.pbData.is_null() || output.cbData == 0 {
            return Err(());
        }
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData as *mut _)));
        if bytes.windows(plain.len()).any(|window| window == plain) {
            return Err(());
        }
        Ok(bytes)
    }
}

#[cfg(windows)]
fn unprotect_bytes(ciphertext: &[u8]) -> Result<Vec<u8>, ()> {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptUnprotectData,
    };

    if ciphertext.is_empty() {
        return Err(());
    }
    let input = CRYPT_INTEGER_BLOB {
        cbData: ciphertext.len() as u32,
        pbData: ciphertext.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|_| ())?;
        if output.pbData.is_null() || output.cbData == 0 {
            return Err(());
        }
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData as *mut _)));
        Ok(bytes)
    }
}

#[cfg(not(windows))]
fn protect_bytes(_plain: &[u8]) -> Result<Vec<u8>, ()> {
    Err(())
}

#[cfg(not(windows))]
fn unprotect_bytes(_ciphertext: &[u8]) -> Result<Vec<u8>, ()> {
    Err(())
}

#[cfg(test)]
mod tests {
    use super::{hint_of, protect, unprotect};

    #[test]
    fn dpapi_round_trip_hides_the_plaintext() {
        let plain = "sk-test-secret-value";
        let cipher = protect(plain).expect("encrypt");
        assert!(!cipher.is_empty());
        assert!(
            !cipher
                .windows(plain.len())
                .any(|window| window == plain.as_bytes())
        );
        let opened = unprotect(&cipher).expect("decrypt");
        assert_eq!(opened, plain);
        assert_eq!(hint_of(plain), "alue");
    }
}
