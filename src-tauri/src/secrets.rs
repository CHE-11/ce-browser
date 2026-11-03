//! API keys for Shopping summaries, encrypted with Windows DPAPI for the current user.
//! Ciphertext lives under local app data, never in the syncable profile folder.

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
    let id = profiles::active_id();
    let name = if id == profiles::DEFAULT_ID {
        FILE_NAME.to_string()
    } else {
        format!("{id}-{FILE_NAME}")
    };
    paths::root_local_dir(app).join("secrets").join(name)
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
