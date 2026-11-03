//! Password generator choices, stored in the profile as `password.json`.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{files, paths};

const FILE: &str = "password.json";
const EXCLUDE_LIMIT: usize = 240;

fn default_length() -> u32 {
    16
}

fn default_min_symbols() -> u32 {
    1
}

fn default_true() -> bool {
    true
}

/// Options selected in the password generator. An older file that omits a key still loads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PasswordSettings {
    #[serde(default = "default_length")]
    pub length: u32,
    #[serde(default = "default_true")]
    pub upper: bool,
    #[serde(default = "default_true")]
    pub lower: bool,
    #[serde(default = "default_true")]
    pub digits: bool,
    #[serde(default = "default_true")]
    pub symbols: bool,
    #[serde(default = "default_min_symbols")]
    pub min_symbols: u32,
    #[serde(default = "default_true")]
    pub avoid_ambiguous: bool,
    /// Characters to leave out, separated by commas. `a, e, i` skips those letters.
    #[serde(default)]
    pub exclude: String,
}

impl Default for PasswordSettings {
    fn default() -> Self {
        Self {
            length: default_length(),
            upper: default_true(),
            lower: default_true(),
            digits: default_true(),
            symbols: default_true(),
            min_symbols: default_min_symbols(),
            avoid_ambiguous: default_true(),
            exclude: String::new(),
        }
    }
}

pub fn get(app: &AppHandle) -> PasswordSettings {
    read(&paths::data_file(app, FILE))
}

pub fn save(app: &AppHandle, settings: PasswordSettings) -> Result<PasswordSettings, String> {
    write(&paths::data_file(app, FILE), settings)
}

pub fn normalize(mut settings: PasswordSettings) -> PasswordSettings {
    if !(8..=64).contains(&settings.length) {
        settings.length = default_length();
    }
    if settings.min_symbols > settings.length {
        settings.min_symbols = settings.length;
    }
    settings.exclude = clean_exclude(&settings.exclude);
    settings
}

fn clean_exclude(value: &str) -> String {
    let mut out = String::new();
    for character in value.chars() {
        if character.is_control() {
            continue;
        }
        out.push(character);
        if out.chars().count() >= EXCLUDE_LIMIT {
            break;
        }
    }
    out.trim().to_string()
}

fn read(path: &Path) -> PasswordSettings {
    normalize(files::read_json(path))
}

fn write(path: &Path, settings: PasswordSettings) -> Result<PasswordSettings, String> {
    let settings = normalize(settings);
    files::write_json(path, &settings)?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_object_uses_the_generator_defaults() {
        let settings: PasswordSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, PasswordSettings::default());
    }

    #[test]
    fn an_older_file_keeps_its_choices_and_fills_exclude() {
        let settings: PasswordSettings =
            serde_json::from_str(r#"{"length":24,"upper":false}"#).unwrap();
        assert_eq!(settings.length, 24);
        assert!(!settings.upper);
        assert!(settings.lower);
        assert!(settings.digits);
        assert!(settings.symbols);
        assert_eq!(settings.min_symbols, 1);
        assert!(settings.avoid_ambiguous);
        assert_eq!(settings.exclude, "");
    }

    #[test]
    fn a_bad_length_falls_back_and_the_minimum_stays_inside_it() {
        let settings = normalize(PasswordSettings {
            length: 3,
            min_symbols: 99,
            ..PasswordSettings::default()
        });
        assert_eq!(settings.length, 16);
        assert_eq!(settings.min_symbols, 16);
    }

    #[test]
    fn the_exclude_list_drops_control_characters_and_surrounding_space() {
        let settings = normalize(PasswordSettings {
            exclude: "  a, e \n".into(),
            ..PasswordSettings::default()
        });
        assert_eq!(settings.exclude, "a, e");
    }

    #[test]
    fn a_saved_list_round_trips_and_a_broken_file_is_kept() {
        let dir = std::env::temp_dir().join(format!(
            "ce-password-settings-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("password.json");

        assert_eq!(read(&path), PasswordSettings::default());

        let saved = write(
            &path,
            PasswordSettings {
                length: 20,
                upper: false,
                exclude: "a, e, i".into(),
                ..PasswordSettings::default()
            },
        )
        .unwrap();
        assert_eq!(saved.exclude, "a, e, i");
        assert!(!saved.upper);
        assert_eq!(read(&path), saved);

        std::fs::write(&path, "{").unwrap();
        assert_eq!(read(&path), PasswordSettings::default());
        assert!(path.with_extension("json.bak").is_file());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
