//! Links that open a desktop app, such as `claude://` or `ms-teams:`. WebView2 cancels the
//! navigation once it hands the link off, which reads as a dropped connection unless the tab
//! ignores it. The site asks through the permission prompt before the app opens.

use url::Url;
use windows::Win32::System::Registry::HKEY_CLASSES_ROOT;
use windows::Win32::UI::Shell::{
    ASSOCF_IS_PROTOCOL, ASSOCF_NOTRUNCATE, ASSOCSTR_FRIENDLYAPPNAME, AssocQueryStringW,
};
use windows::core::{HSTRING, PCWSTR, PWSTR};

/// Schemes the page view loads itself. Anything else is handed to Windows.
const PAGE_SCHEMES: &[&str] = &[
    "http",
    "https",
    "file",
    "about",
    "data",
    "blob",
    "javascript",
    "ws",
    "wss",
    "view-source",
    "edge",
    "chrome",
    "chrome-extension",
    "extension",
    "devtools",
    "filesystem",
    "ce",
    "ceboard",
    "tauri",
    "ipc",
    "asset",
];

/// The scheme of `uri` when it opens an app outside the browser.
pub fn external_scheme(uri: &str) -> Option<String> {
    let scheme = Url::parse(uri.trim()).ok()?.scheme().to_ascii_lowercase();
    if PAGE_SCHEMES.contains(&scheme.as_str()) {
        None
    } else {
        Some(scheme)
    }
}

/// The name Windows shows for the app registered to open `scheme`, or `None` when no app is.
/// Store apps register their schemes outside `HKCR\<scheme>`, so the shell answers first.
pub fn app_name(scheme: &str) -> Option<String> {
    if let Some(name) = shell_app_name(scheme) {
        return Some(name);
    }
    crate::extension_store::registry_string(
        HKEY_CLASSES_ROOT,
        &format!("{scheme}\\shell\\open\\command"),
        "",
    )?;
    let described = crate::extension_store::registry_string(HKEY_CLASSES_ROOT, scheme, "")
        .map(|name| friendly_name(&name))
        .unwrap_or_default();
    Some(if described.is_empty() {
        capitalized(scheme)
    } else {
        described
    })
}

fn shell_app_name(scheme: &str) -> Option<String> {
    let assoc = HSTRING::from(scheme);
    let flags = ASSOCF_IS_PROTOCOL | ASSOCF_NOTRUNCATE;
    let mut size = 0u32;
    let _ = unsafe {
        AssocQueryStringW(flags, ASSOCSTR_FRIENDLYAPPNAME, &assoc, PCWSTR::null(), None, &mut size)
    };
    if size < 2 || size > 1024 {
        return None;
    }
    let mut buffer = vec![0u16; size as usize];
    let result = unsafe {
        AssocQueryStringW(
            flags,
            ASSOCSTR_FRIENDLYAPPNAME,
            &assoc,
            PCWSTR::null(),
            Some(PWSTR(buffer.as_mut_ptr())),
            &mut size,
        )
    };
    if result.is_err() {
        return None;
    }
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    let name = String::from_utf16_lossy(&buffer[..len]).trim().to_string();
    if name.is_empty() { None } else { Some(name) }
}

/// `URL:Claude Protocol` reads as `Claude`. A bare `URL:claude` keeps the scheme.
fn friendly_name(raw: &str) -> String {
    let mut name = raw.trim();
    if let Some(rest) = name.strip_prefix("URL:").or_else(|| name.strip_prefix("url:")) {
        name = rest.trim();
    }
    for suffix in [" Protocol", " protocol", " URL", " Url", " Link", " link"] {
        if let Some(rest) = name.strip_suffix(suffix) {
            name = rest.trim();
        }
    }
    if name.is_empty() || name.eq_ignore_ascii_case("url") || name.len() > 60 {
        return String::new();
    }
    if name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return capitalized(name);
    }
    name.to_string()
}

fn capitalized(scheme: &str) -> String {
    let mut chars = scheme.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_pages_are_not_external() {
        assert_eq!(external_scheme("https://claude.ai/desktop"), None);
        assert_eq!(external_scheme("about:blank"), None);
        assert_eq!(external_scheme("ce://settings"), None);
    }

    #[test]
    fn an_app_link_is_external() {
        assert_eq!(
            external_scheme("claude://artifact/HPtmhTpoSdHmRKNCJEUsoS").as_deref(),
            Some("claude")
        );
        assert_eq!(external_scheme("ms-teams:l/chat").as_deref(), Some("ms-teams"));
        assert_eq!(external_scheme("mailto:a@b.c").as_deref(), Some("mailto"));
    }

    #[test]
    fn a_protocol_description_becomes_the_app_name() {
        assert_eq!(friendly_name("URL:Claude Protocol"), "Claude");
        assert_eq!(friendly_name("URL:claude"), "Claude");
        assert_eq!(friendly_name("Zoom Meeting Link"), "Zoom Meeting");
        assert_eq!(friendly_name("URL Protocol"), "");
    }
}
