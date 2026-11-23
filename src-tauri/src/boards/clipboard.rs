//! Pasting files, images and links from the Windows clipboard onto a board.

use std::io::Cursor;
use std::path::PathBuf;

use base64::Engine;
use image::{ImageFormat, Rgba, RgbaImage};
use tauri::AppHandle;

use super::import::{BoardImportItem, BoardImportResult, MAX_DATA_BYTES, import_items};

/// Links taken from copied text.
const MAX_LINKS: usize = 200;
/// Files taken from a copied file list.
const MAX_FILES: usize = 2000;

/// Saves whatever is on the clipboard. An image wins over a link copied with it. An empty
/// `folder` uses the last folder.
pub fn paste_clipboard(app: &AppHandle, folder: &str) -> Result<BoardImportResult, String> {
    import_items(app, folder, &clipboard_import_items()?)
}

fn clipboard_import_items() -> Result<Vec<BoardImportItem>, String> {
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
        RegisterClipboardFormatW,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use windows::Win32::System::Ole::{CF_DIB, CF_HDROP, CF_UNICODETEXT};
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
    use windows::core::w;

    unsafe {
        OpenClipboard(None).map_err(|_| "The clipboard is busy. Try again.".to_string())?;
        let result = (|| {
            if format_ready(u32::from(CF_HDROP.0)) {
                let files = hdrop_files();
                if !files.is_empty() {
                    return Ok(files);
                }
            }
            let png_format = RegisterClipboardFormatW(w!("PNG"));
            if png_format != 0
                && format_ready(png_format)
                && let Some(bytes) = clipboard_bytes(png_format)
                && let Ok(item) = image_item(bytes)
            {
                return Ok(vec![item]);
            }
            for format in [u32::from(CF_DIB.0), 17u32] {
                if !format_ready(format) {
                    continue;
                }
                if let Some(bytes) = clipboard_bytes(format)
                    && let Ok(png) = png_from_dib(&bytes)
                    && let Ok(item) = image_item(png)
                {
                    return Ok(vec![item]);
                }
            }
            if format_ready(u32::from(CF_UNICODETEXT.0))
                && let Some(bytes) = clipboard_bytes(u32::from(CF_UNICODETEXT.0))
            {
                let links = links_in_text(&clipboard_text(&bytes));
                if !links.is_empty() {
                    return Ok(links.into_iter().take(MAX_LINKS).map(link_item).collect());
                }
            }
            Err("Copy an image, file, or link first.".into())
        })();
        let _ = CloseClipboard();
        return result;
    }

    fn format_ready(format: u32) -> bool {
        unsafe { IsClipboardFormatAvailable(format).is_ok() }
    }

    fn clipboard_bytes(format: u32) -> Option<Vec<u8>> {
        unsafe {
            let handle = GetClipboardData(format).ok()?;
            let memory = windows::Win32::Foundation::HGLOBAL(handle.0);
            let pointer = GlobalLock(memory);
            if pointer.is_null() {
                return None;
            }
            let size = GlobalSize(memory);
            let bytes = std::slice::from_raw_parts(pointer.cast::<u8>(), size).to_vec();
            let _ = GlobalUnlock(memory);
            Some(bytes)
        }
    }

    fn hdrop_files() -> Vec<BoardImportItem> {
        let Some(handle) = (unsafe { GetClipboardData(u32::from(CF_HDROP.0)).ok() }) else {
            return Vec::new();
        };
        let drop = HDROP(handle.0);
        let count = unsafe { DragQueryFileW(drop, 0xFFFF_FFFF, None) };
        let mut items = Vec::new();
        for index in 0..count {
            let length = unsafe { DragQueryFileW(drop, index, None) } as usize;
            if length == 0 {
                continue;
            }
            let mut buffer = vec![0u16; length + 1];
            unsafe { DragQueryFileW(drop, index, Some(&mut buffer)) };
            let path = PathBuf::from(String::from_utf16_lossy(&buffer[..length]));
            if path.is_file() {
                items.push(BoardImportItem {
                    path: path.to_string_lossy().into_owned(),
                    ..BoardImportItem::default()
                });
            }
            if items.len() == MAX_FILES {
                break;
            }
        }
        items
    }
}

fn image_item(png: Vec<u8>) -> Result<BoardImportItem, String> {
    if png.is_empty() {
        return Err("That image is empty.".into());
    }
    if png.len() as u64 > MAX_DATA_BYTES {
        return Err("That image is too large to paste.".into());
    }
    // A PNG copied from another app is already encoded. A bitmap is converted before this.
    if image::load_from_memory(&png).is_err() {
        return Err("That image format isn't supported.".into());
    }
    Ok(BoardImportItem {
        name: "Pasted image.png".into(),
        mime: "image/png".into(),
        data: base64::engine::general_purpose::STANDARD.encode(png),
        ..BoardImportItem::default()
    })
}

fn link_item(url: String) -> BoardImportItem {
    BoardImportItem {
        url,
        ..BoardImportItem::default()
    }
}

fn clipboard_text(bytes: &[u8]) -> String {
    let wide: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    String::from_utf16_lossy(&wide)
}

fn links_in_text(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for token in text.split_whitespace() {
        let token = token.trim_matches(|character: char| {
            matches!(
                character,
                '<' | '>' | '"' | '\'' | '(' | ')' | ',' | ';' | '.'
            )
        });
        let lower = token.to_ascii_lowercase();
        if (lower.starts_with("http://") || lower.starts_with("https://"))
            && !urls.iter().any(|url| url == token)
        {
            urls.push(token.to_string());
        }
    }
    urls
}

/// Turns a device-independent bitmap from the clipboard into a PNG.
fn png_from_dib(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Graphics::Gdi::BITMAPINFOHEADER;

    let header_size = size_of::<BITMAPINFOHEADER>();
    if bytes.len() < header_size {
        return Err("That image isn't a format the clipboard can paste.".into());
    }
    let header = unsafe { bytes.as_ptr().cast::<BITMAPINFOHEADER>().read_unaligned() };
    let declared = header.biSize as usize;
    if declared < header_size || declared > bytes.len() {
        return Err("That image isn't a format the clipboard can paste.".into());
    }
    let width = header.biWidth.unsigned_abs();
    let top_down = header.biHeight < 0;
    let height = header.biHeight.unsigned_abs();
    if width == 0 || height == 0 || width > 16_000 || height > 16_000 {
        return Err("That image isn't a format the clipboard can paste.".into());
    }
    let bpp = header.biBitCount;
    if bpp != 24 && bpp != 32 {
        return Err("That image isn't a format the clipboard can paste.".into());
    }
    // BI_RGB is 0. BI_BITFIELDS is 3 and, for a 40-byte header, the masks follow it.
    let pixels_at = if header.biCompression == 3 && declared == header_size {
        declared + 12
    } else if header.biCompression == 0 || header.biCompression == 3 {
        declared
    } else {
        return Err("That image isn't a format the clipboard can paste.".into());
    };
    let channels = bpp as usize / 8;
    let stride = (width as usize * channels).div_ceil(4) * 4;
    let needed = pixels_at + stride * height as usize;
    if bytes.len() < needed {
        return Err("That image isn't a format the clipboard can paste.".into());
    }
    let mut image = RgbaImage::new(width, height);
    for y in 0..height {
        let source_y = if top_down { y } else { height - 1 - y };
        let row = pixels_at + source_y as usize * stride;
        for x in 0..width {
            let index = row + x as usize * channels;
            let blue = bytes[index];
            let green = bytes[index + 1];
            let red = bytes[index + 2];
            let alpha = if channels == 4 { bytes[index + 3] } else { 255 };
            image.put_pixel(x, y, Rgba([red, green, blue, alpha]));
        }
    }
    if channels == 4 && image.pixels().all(|pixel| pixel[3] == 0) {
        for pixel in image.pixels_mut() {
            pixel[3] = 255;
        }
    }
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|error| format!("Couldn't read the copied image. {error}"))?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_text_keeps_links_and_a_bitmap_becomes_a_png() {
        assert_eq!(
            links_in_text("see https://example.com/a, and https://example.com/a again"),
            vec!["https://example.com/a".to_string()]
        );
        assert!(links_in_text("just a note").is_empty());
        let mut bytes = vec![0u8; 40 + 8];
        bytes[0] = 40;
        bytes[4] = 2;
        bytes[8] = 1;
        bytes[12] = 1;
        bytes[14] = 32;
        bytes[40] = 255;
        bytes[41] = 0;
        bytes[42] = 0;
        bytes[43] = 255;
        bytes[44] = 0;
        bytes[45] = 255;
        bytes[46] = 0;
        bytes[47] = 255;
        let png = png_from_dib(&bytes).expect("png");
        let image = image::load_from_memory(&png).expect("decode").to_rgba8();
        assert_eq!(image.dimensions(), (2, 1));
        assert_eq!(image.get_pixel(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(image.get_pixel(1, 0).0, [0, 255, 0, 255]);
    }
}
