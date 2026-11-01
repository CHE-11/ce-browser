//! Copy text out of a captured PNG with the Windows OCR engine.

use tauri::AppHandle;

use crate::capture;

const FAILURE: &str = "Couldn't read the text in this image.";

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

pub(crate) fn copy_text(text: &str) -> Result<(), String> {
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
    use windows::Win32::System::Ole::CF_UNICODETEXT;
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    let bytes = unsafe { std::slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * 2) };
    unsafe {
        OpenClipboard(None).map_err(|error| format!("{FAILURE} {error}"))?;
        let result = (|| {
            EmptyClipboard().map_err(|error| format!("{FAILURE} {error}"))?;
            let memory = GlobalAlloc(GMEM_MOVEABLE, bytes.len())
                .map_err(|error| format!("{FAILURE} {error}"))?;
            let target = GlobalLock(memory);
            if target.is_null() {
                let _ = GlobalFree(Some(memory));
                return Err(FAILURE.to_string());
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), target.cast::<u8>(), bytes.len());
            let _ = GlobalUnlock(memory);
            if let Err(error) =
                SetClipboardData(u32::from(CF_UNICODETEXT.0), Some(HANDLE(memory.0)))
            {
                let _ = GlobalFree(Some(HGLOBAL(memory.0)));
                return Err(format!("{FAILURE} {error}"));
            }
            Ok(())
        })();
        let _ = CloseClipboard();
        result
    }
}
