//! Four-connection downloads for large files, plus the download file name.

use std::io::Read;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::files;

pub const MIN_BYTES: u64 = 8 * 1024 * 1024;
pub const CONNECTIONS: u32 = 4;
pub const CHUNK_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
    pub received: u64,
}

impl ByteRange {
    pub fn length(&self) -> u64 {
        self.end.saturating_sub(self.start).saturating_add(1)
    }

    pub fn remaining(&self) -> u64 {
        self.length().saturating_sub(self.received)
    }

    pub fn done(&self) -> bool {
        self.remaining() == 0
    }
}

/// Splits `total` bytes into `parts` inclusive ranges that meet with no gap and no overlap.
pub fn split_ranges(total: u64, parts: u32) -> Vec<ByteRange> {
    if total == 0 || parts == 0 {
        return Vec::new();
    }
    let parts = parts.min(total as u32).max(1);
    let size = total / u64::from(parts);
    let mut ranges = Vec::with_capacity(parts as usize);
    let mut start = 0u64;
    for index in 0..parts {
        let end = if index + 1 == parts {
            total - 1
        } else {
            start + size - 1
        };
        ranges.push(ByteRange {
            start,
            end,
            received: 0,
        });
        start = end + 1;
    }
    ranges
}

pub fn total_from_content_range(header: &str) -> Option<u64> {
    let total = header.split('/').nth(1)?.trim();
    if total == "*" || total.is_empty() {
        return None;
    }
    total.parse().ok()
}

/// Empty `template` keeps the name the site suggested. `{ext}` is the extension without the dot.
/// A template that omits `{ext}` still keeps the extension, so the file opens in the right app.
pub fn download_name(template: &str, url: &str, suggested: &str, date: &str) -> String {
    let suggested = files::sanitize_file_name(suggested);
    let template = template.trim();
    if template.is_empty() {
        return suggested;
    }
    let path = Path::new(&suggested);
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| suggested.clone());
    let ext = path
        .extension()
        .map(|ext| ext.to_string_lossy().into_owned())
        .unwrap_or_default();
    let site = site_of(url);
    let mut name = template
        .replace("{site}", &site)
        .replace("{date}", date)
        .replace("{name}", &stem)
        .replace("{ext}", &ext);
    if !ext.is_empty() && !template.contains("{ext}") {
        let suffix = format!(".{}", ext.to_ascii_lowercase());
        if !name.to_ascii_lowercase().ends_with(&suffix) {
            name.push('.');
            name.push_str(&ext);
        }
    }
    files::sanitize_file_name(&name)
}

pub fn preview_kind(name: &str) -> &'static str {
    let ext = Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "json" => "json",
        "csv" => "csv",
        "md" | "markdown" => "markdown",
        _ if ext == "log" || ext == "txt" => "log",
        _ => "other",
    }
}

pub fn local_date() -> String {
    #[repr(C)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLocalTime(time: *mut SystemTime);
    }
    let mut time = SystemTime {
        year: 0,
        month: 0,
        day_of_week: 0,
        day: 0,
        hour: 0,
        minute: 0,
        second: 0,
        milliseconds: 0,
    };
    unsafe { GetLocalTime(&mut time) };
    format!("{:04}-{:02}-{:02}", time.year, time.month, time.day)
}

fn site_of(url: &str) -> String {
    let host = url::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_string()))
        .unwrap_or_else(|| "download".into());
    let host = host.strip_prefix("www.").unwrap_or(&host);
    if host.is_empty() { "download" } else { host }.to_string()
}

pub fn probe(url: &str, cookie: &str, user_agent: &str) -> Result<u64, String> {
    let response = request(url, cookie, user_agent, "bytes=0-0", Duration::from_secs(8))?;
    if response.status() != 206 {
        return Err("This server does not accept range downloads.".into());
    }
    let header = response.header("Content-Range").unwrap_or("");
    let total = total_from_content_range(header)
        .ok_or_else(|| "The server did not say how big the file is.".to_string())?;
    if total < MIN_BYTES {
        return Err("The file is under 8 MB.".into());
    }
    let _ = response.into_reader().take(8).read_to_end(&mut Vec::new());
    Ok(total)
}

pub fn read_range(
    url: &str,
    cookie: &str,
    user_agent: &str,
    start: u64,
    end: u64,
) -> Result<Vec<u8>, String> {
    let response = request(
        url,
        cookie,
        user_agent,
        &format!("bytes={start}-{end}"),
        Duration::from_secs(30),
    )?;
    if response.status() != 206 {
        return Err("The server stopped accepting ranges.".into());
    }
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(end.saturating_sub(start).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.is_empty() {
        return Err("The server sent an empty range.".into());
    }
    Ok(bytes)
}

fn request(
    url: &str,
    cookie: &str,
    user_agent: &str,
    range: &str,
    timeout: Duration,
) -> Result<ureq::Response, String> {
    let mut request = ureq::get(url).timeout(timeout).set("Range", range);
    if !cookie.is_empty() {
        request = request.set("Cookie", cookie);
    }
    if !user_agent.is_empty() {
        request = request.set("User-Agent", user_agent);
    }
    request.call().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{download_name, preview_kind, split_ranges, total_from_content_range};

    #[test]
    fn template_uses_site_date_and_keeps_the_extension() {
        let name = download_name(
            "{site}-{date}-{name}",
            "https://www.example.com/files/report.pdf",
            "report.pdf",
            "2026-10-31",
        );
        assert_eq!(name, "example.com-2026-10-31-report.pdf");
    }

    #[test]
    fn an_empty_template_keeps_the_suggested_name() {
        assert_eq!(
            download_name("", "https://example.com/a", "My File.zip", "2026-10-31"),
            "My File.zip"
        );
    }

    #[test]
    fn ranges_cover_the_file_without_gaps() {
        let ranges = split_ranges(10_000_000, 4);
        assert_eq!(ranges.len(), 4);
        let mut cursor = 0u64;
        for range in &ranges {
            assert_eq!(range.start, cursor);
            assert!(range.end >= range.start);
            cursor = range.end + 1;
        }
        assert_eq!(cursor, 10_000_000);
        assert_eq!(
            total_from_content_range("bytes 0-0/10485760"),
            Some(10_485_760)
        );
    }

    #[test]
    fn preview_kind_detects_viewers() {
        assert_eq!(preview_kind("notes.JSON"), "json");
        assert_eq!(preview_kind("table.csv"), "csv");
        assert_eq!(preview_kind("readme.md"), "markdown");
        assert_eq!(preview_kind("app.log"), "log");
        assert_eq!(preview_kind("photo.png"), "other");
    }
}
