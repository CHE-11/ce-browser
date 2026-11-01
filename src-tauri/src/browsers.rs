//! Launch the current page in Chrome, Firefox, or Edge when that browser is installed.

use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalBrowser {
    pub id: String,
    pub name: String,
}

pub fn launchable(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

pub fn installed() -> Vec<ExternalBrowser> {
    [
        ("chrome", "Chrome"),
        ("firefox", "Firefox"),
        ("edge", "Edge"),
    ]
    .into_iter()
    .filter(|(id, _)| executable(id).is_some())
    .map(|(id, name)| ExternalBrowser {
        id: id.into(),
        name: name.into(),
    })
    .collect()
}

pub fn open(kind: &str, url: &str) -> Result<(), String> {
    if !launchable(url) {
        return Err("Only a website can be opened in another browser.".into());
    }
    let path = executable(kind).ok_or_else(|| format!("{kind} is not installed."))?;
    Command::new(path)
        .arg(url)
        .spawn()
        .map_err(|error| format!("Couldn't open {kind}. {error}"))?;
    Ok(())
}

fn executable(kind: &str) -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let program_files = std::env::var_os("ProgramFiles").map(PathBuf::from);
    let program_files_x86 = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from);
    let candidates: Vec<PathBuf> = match kind {
        "chrome" => {
            let mut paths = Vec::new();
            if let Some(local) = local {
                paths.push(local.join("Google\\Chrome\\Application\\chrome.exe"));
            }
            for root in [program_files.clone(), program_files_x86.clone()]
                .into_iter()
                .flatten()
            {
                paths.push(root.join("Google\\Chrome\\Application\\chrome.exe"));
            }
            paths
        }
        "edge" => [program_files.clone(), program_files_x86.clone()]
            .into_iter()
            .flatten()
            .map(|root| root.join("Microsoft\\Edge\\Application\\msedge.exe"))
            .collect(),
        "firefox" => [program_files, program_files_x86]
            .into_iter()
            .flatten()
            .map(|root| root.join("Mozilla Firefox\\firefox.exe"))
            .collect(),
        _ => Vec::new(),
    };
    candidates.into_iter().find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::launchable;

    #[test]
    fn an_internal_page_stays_in_this_browser() {
        assert!(!launchable("ce://settings"));
        assert!(!launchable("about:blank"));
        assert!(launchable("https://example.com/a"));
        assert!(launchable("http://localhost:8080"));
    }
}
