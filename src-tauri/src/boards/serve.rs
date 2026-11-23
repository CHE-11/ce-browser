//! The `ceboard` protocol: `http://ceboard.localhost/thumb/<id>` and
//! `http://ceboard.localhost/file/<id>.<ext>`. Only app pages (the browser UI, popups and
//! `ce://` pages) are answered; remote sites in tabs get a 403.

use std::fs;
use std::io::{Read, Seek, SeekFrom};

use tauri::AppHandle;
use tauri::http::{Request, Response, StatusCode, header};

use super::model::valid_id;
use super::storage;
use crate::{paths, popup, tabs};

/// Bytes returned for an open-ended range, so a long video streams in parts.
const RANGE_CHUNK: u64 = 4 * 1024 * 1024;

fn allowed(app: &AppHandle, label: &str, request: &Request<Vec<u8>>) -> bool {
    let origin = paths::app_origin(app);
    let same = |value: &str| {
        url::Url::parse(value).is_ok_and(|url| {
            url.scheme() == origin.scheme()
                && url.host_str() == origin.host_str()
                && url.port_or_known_default() == origin.port_or_known_default()
        })
    };
    let header_matches = [header::ORIGIN, header::REFERER].iter().any(|name| {
        request
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .is_some_and(same)
    });
    header_matches || label == tabs::UI || popup::is_popup_label(label)
}

fn empty(status: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Vec::new())
        .unwrap_or_default()
}

pub fn respond(app: &AppHandle, label: &str, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    if !allowed(app, label, request) {
        return empty(StatusCode::FORBIDDEN);
    }
    let path = request.uri().path().trim_start_matches('/');
    let Some((kind, name)) = path.split_once('/') else {
        return empty(StatusCode::NOT_FOUND);
    };
    let (id, ext) = name.split_once('.').unwrap_or((name, ""));
    if !valid_id(id) {
        return empty(StatusCode::NOT_FOUND);
    }
    match kind {
        "thumb" => thumb(app, id),
        "file" => file(app, id, ext, request),
        _ => empty(StatusCode::NOT_FOUND),
    }
}

fn thumb(app: &AppHandle, id: &str) -> Response<Vec<u8>> {
    let path = storage::thumb_path(app, id);
    let bytes = fs::read(&path).ok().or_else(|| {
        // Older pins made before thumbnails, or a thumbnail removed by hand.
        let source = storage::original_by_id(app, id, "")?;
        let image = storage::decode_image(&fs::read(source).ok()?).ok()?;
        storage::write_thumb(app, id, &image).ok()?;
        fs::read(&path).ok()
    });
    match bytes {
        Some(bytes) => Response::builder()
            .header(header::CONTENT_TYPE, "image/jpeg")
            .header(header::CACHE_CONTROL, "max-age=31536000, immutable")
            .body(bytes)
            .unwrap_or_default(),
        None => empty(StatusCode::NOT_FOUND),
    }
}

/// `bytes=start-end` against a file of `size` bytes.
fn parse_range(value: &str, size: u64) -> Option<(u64, u64)> {
    let spec = value.trim().strip_prefix("bytes=")?;
    let spec = spec.split(',').next()?.trim();
    let (start, end) = spec.split_once('-')?;
    if size == 0 {
        return None;
    }
    if start.is_empty() {
        let suffix: u64 = end.parse().ok()?;
        let start = size.saturating_sub(suffix);
        return Some((start, size - 1));
    }
    let start: u64 = start.parse().ok()?;
    if start >= size {
        return None;
    }
    let end = if end.is_empty() {
        (start + RANGE_CHUNK - 1).min(size - 1)
    } else {
        end.parse::<u64>().ok()?.min(size - 1)
    };
    (end >= start).then_some((start, end))
}

fn file(app: &AppHandle, id: &str, ext: &str, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let Some(path) = storage::original_by_id(app, id, ext) else {
        return empty(StatusCode::NOT_FOUND);
    };
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let mime = storage::mime_for_ext(&ext);
    let Ok(mut handle) = fs::File::open(&path) else {
        return empty(StatusCode::NOT_FOUND);
    };
    let size = handle.metadata().map(|meta| meta.len()).unwrap_or(0);
    let range = request
        .headers()
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());
    let mut builder = Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "max-age=31536000, immutable")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    // The PDF viewer needs scripts; any other file opened as a document must not run any.
    if ext != "pdf" {
        builder = builder.header(header::CONTENT_SECURITY_POLICY, "sandbox");
    }
    if let Some(range) = range {
        let Some((start, end)) = parse_range(range, size) else {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{size}"))
                .body(Vec::new())
                .unwrap_or_default();
        };
        let length = end - start + 1;
        let mut bytes = vec![0u8; length as usize];
        if handle.seek(SeekFrom::Start(start)).is_err() || handle.read_exact(&mut bytes).is_err() {
            return empty(StatusCode::INTERNAL_SERVER_ERROR);
        }
        return builder
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"))
            .body(bytes)
            .unwrap_or_default();
    }
    let mut bytes = Vec::with_capacity(size as usize);
    if handle.read_to_end(&mut bytes).is_err() {
        return empty(StatusCode::INTERNAL_SERVER_ERROR);
    }
    builder.body(bytes).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_cover_open_ends_and_suffixes() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=900-", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(
            parse_range("bytes=0-", 10 * RANGE_CHUNK),
            Some((0, RANGE_CHUNK - 1))
        );
        assert_eq!(parse_range("bytes=1000-", 1000), None);
        assert_eq!(parse_range("items=0-1", 1000), None);
    }
}
