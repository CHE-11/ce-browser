//! Copy text out of a captured PNG with the Windows OCR engine.

use tauri::AppHandle;

use crate::capture;

const FAILURE: &str = "Couldn't read the text in this image.";
const COPY_FAILURE: &str = "Couldn't copy that.";

pub fn recognize_visible(app: &AppHandle, id: u32) -> Result<String, String> {
    capture::store_visible(app, id)?;
    recognize_stored()
}

pub fn recognize_region(app: &AppHandle, id: u32) -> Result<String, String> {
    capture::capture_region(app, id)?;
    recognize_stored()
}

fn recognize_stored() -> Result<String, String> {
    let png = capture::last_png().ok_or_else(|| FAILURE.to_string())?;
    let text = recognize_png(&png)?;
    copy_text(&text)?;
    Ok(text)
}

pub fn recognize_png(bytes: &[u8]) -> Result<String, String> {
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};
    let engine = OcrEngine::TryCreateFromUserProfileLanguages().map_err(|_| {
        "Windows has no OCR language pack. Add one in Settings, then Time & language, then Language.".to_string()
    })?;
    let stream = InMemoryRandomAccessStream::new().map_err(|error| format!("{FAILURE} {error}"))?;
    let writer =
        DataWriter::CreateDataWriter(&stream).map_err(|error| format!("{FAILURE} {error}"))?;
    writer
        .WriteBytes(bytes)
        .map_err(|error| format!("{FAILURE} {error}"))?;
    writer
        .StoreAsync()
        .map_err(|error| format!("{FAILURE} {error}"))?
        .get()
        .map_err(|error| format!("{FAILURE} {error}"))?;
    writer
        .DetachStream()
        .map_err(|error| format!("{FAILURE} {error}"))?;
    stream
        .Seek(0)
        .map_err(|error| format!("{FAILURE} {error}"))?;
    let decoder = BitmapDecoder::CreateAsync(&stream)
        .map_err(|error| format!("{FAILURE} {error}"))?
        .get()
        .map_err(|error| format!("{FAILURE} {error}"))?;
    let bitmap = decoder
        .GetSoftwareBitmapAsync()
        .map_err(|error| format!("{FAILURE} {error}"))?
        .get()
        .map_err(|error| format!("{FAILURE} {error}"))?;
    let result = engine
        .RecognizeAsync(&bitmap)
        .map_err(|error| format!("{FAILURE} {error}"))?
        .get()
        .map_err(|error| format!("{FAILURE} {error}"))?;
    let text = result
        .Text()
        .map_err(|error| format!("{FAILURE} {error}"))?;
    let text = text.to_string();
    if text.trim().is_empty() {
        return Err("No text was found in that image.".into());
    }
    Ok(text)
}

pub fn copy_text(text: &str) -> Result<(), String> {
    use windows::Win32::System::Ole::CF_UNICODETEXT;
    write_clipboard(&[ClipBytes {
        format: u32::from(CF_UNICODETEXT.0),
        bytes: utf16_z(text),
    }])
}

/// Copies `url` as plain text and as an HTML link labeled with `title`.
///
/// A rich-text paste shows the title. A plain paste, including the address bar, still gets the URL.
/// An empty title, or a title that is the URL itself, uses the URL as the label.
pub fn copy_link(url: &str, title: &str) -> Result<(), String> {
    use windows::Win32::System::Ole::CF_UNICODETEXT;
    let html_format = clipboard_format("HTML Format")?;
    let url_format = clipboard_format("UniformResourceLocatorW")?;
    let url_bytes = utf16_z(url);
    let mut html = cf_html(&anchor_html(url, title), url);
    html.push(0);
    write_clipboard(&[
        ClipBytes {
            format: u32::from(CF_UNICODETEXT.0),
            bytes: url_bytes.clone(),
        },
        ClipBytes {
            format: html_format,
            bytes: html,
        },
        ClipBytes {
            format: url_format,
            bytes: url_bytes,
        },
    ])
}

/// `<a href="url">title</a>` with both sides escaped. The label is the URL when `title` is blank
/// or already the URL.
pub fn anchor_html(url: &str, title: &str) -> String {
    let trimmed = title.trim();
    let label = if trimmed.is_empty() || trimmed == url {
        url
    } else {
        trimmed
    };
    format!(
        "<a href=\"{}\">{}</a>",
        escape_html(url),
        escape_html(label)
    )
}

fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Windows `HTML Format`: a description header, then the fragment between the markers.
/// Offsets are bytes from the start of this buffer and do not count a trailing NUL.
fn cf_html(fragment_html: &str, source_url: &str) -> Vec<u8> {
    const BLANK: &str = "0000000000";
    let source: String = source_url
        .chars()
        .filter(|ch| *ch != '\r' && *ch != '\n')
        .collect();
    let source_line = if source.is_empty() {
        String::new()
    } else {
        format!("SourceURL:{source}\r\n")
    };
    let head = format!(
        "Version:0.9\r\nStartHTML:{BLANK}\r\nEndHTML:{BLANK}\r\nStartFragment:{BLANK}\r\nEndFragment:{BLANK}\r\n{source_line}"
    );
    let before = "<html>\r\n<body>\r\n<!--StartFragment-->";
    let after = "<!--EndFragment-->\r\n</body>\r\n</html>";
    let start_html = head.len();
    let start_fragment = start_html + before.len();
    let end_fragment = start_fragment + fragment_html.len();
    let end_html = end_fragment + after.len();
    let mut bytes = format!(
        "Version:0.9\r\nStartHTML:{start_html:010}\r\nEndHTML:{end_html:010}\r\nStartFragment:{start_fragment:010}\r\nEndFragment:{end_fragment:010}\r\n{source_line}"
    )
    .into_bytes();
    bytes.extend(before.as_bytes());
    bytes.extend(fragment_html.as_bytes());
    bytes.extend(after.as_bytes());
    bytes
}

struct ClipBytes {
    format: u32,
    bytes: Vec<u8>,
}

fn utf16_z(text: &str) -> Vec<u8> {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    wide.iter().flat_map(|unit| unit.to_le_bytes()).collect()
}

fn clipboard_format(name: &str) -> Result<u32, String> {
    use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
    use windows::core::PCWSTR;
    let mut wide: Vec<u16> = name.encode_utf16().collect();
    wide.push(0);
    let format = unsafe { RegisterClipboardFormatW(PCWSTR(wide.as_ptr())) };
    if format == 0 {
        Err(COPY_FAILURE.to_string())
    } else {
        Ok(format)
    }
}

fn write_clipboard(parts: &[ClipBytes]) -> Result<(), String> {
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard};
    unsafe {
        OpenClipboard(None).map_err(|error| format!("{COPY_FAILURE} {error}"))?;
        let result = (|| {
            EmptyClipboard().map_err(|error| format!("{COPY_FAILURE} {error}"))?;
            for part in parts {
                set_clipboard_data(part.format, &part.bytes)?;
            }
            Ok(())
        })();
        let _ = CloseClipboard();
        result
    }
}

fn set_clipboard_data(format: u32, bytes: &[u8]) -> Result<(), String> {
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::SetClipboardData;
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, bytes.len())
            .map_err(|error| format!("{COPY_FAILURE} {error}"))?;
        let target = GlobalLock(memory);
        if target.is_null() {
            let _ = GlobalFree(Some(memory));
            return Err(COPY_FAILURE.to_string());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), target.cast::<u8>(), bytes.len());
        let _ = GlobalUnlock(memory);
        if let Err(error) = SetClipboardData(format, Some(HANDLE(memory.0))) {
            let _ = GlobalFree(Some(HGLOBAL(memory.0)));
            return Err(format!("{COPY_FAILURE} {error}"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod clipboard_tests {
    use super::*;

    fn header_offset(html: &str, name: &str) -> usize {
        let line = html
            .lines()
            .find(|line| line.starts_with(name))
            .unwrap_or_else(|| panic!("missing {name}"));
        line[name.len()..].trim().parse().expect(name)
    }

    #[test]
    fn cf_html_offsets_bound_the_anchor() {
        let url = "https://example.com/a?q=1&x=2";
        let fragment = anchor_html(url, "Tom & Jerry <3 \"ok\"");
        let bytes = cf_html(&fragment, url);
        let html = std::str::from_utf8(&bytes).expect("utf-8");
        let start_html = header_offset(html, "StartHTML:");
        let end_html = header_offset(html, "EndHTML:");
        let start_fragment = header_offset(html, "StartFragment:");
        let end_fragment = header_offset(html, "EndFragment:");
        assert_eq!(&bytes[start_html..start_html + 6], b"<html>");
        assert_eq!(&bytes[end_html - 7..end_html], b"</html>");
        let pasted = std::str::from_utf8(&bytes[start_fragment..end_fragment]).expect("fragment");
        assert_eq!(
            pasted,
            "<a href=\"https://example.com/a?q=1&amp;x=2\">Tom &amp; Jerry &lt;3 &quot;ok&quot;</a>"
        );
        assert!(!pasted.contains("<!--"));
        assert!(html.contains("SourceURL:https://example.com/a?q=1&x=2\r\n"));
    }

    #[test]
    fn cf_html_offsets_stay_on_character_boundaries() {
        let fragment = anchor_html("https://example.com/", "Café");
        let bytes = cf_html(&fragment, "https://example.com/");
        let html = std::str::from_utf8(&bytes).expect("utf-8");
        let start = header_offset(html, "StartFragment:");
        let end = header_offset(html, "EndFragment:");
        assert_eq!(
            std::str::from_utf8(&bytes[start..end]).expect("fragment"),
            "<a href=\"https://example.com/\">Café</a>"
        );
    }

    #[test]
    fn an_empty_or_matching_title_uses_the_url_as_the_label() {
        let url = "https://example.com/";
        let plain = format!("<a href=\"{url}\">{url}</a>");
        assert_eq!(anchor_html(url, "  "), plain);
        assert_eq!(anchor_html(url, url), plain);
        assert_eq!(
            anchor_html(url, "Example"),
            "<a href=\"https://example.com/\">Example</a>"
        );
    }
}
