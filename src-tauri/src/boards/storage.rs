//! Pin files in `boards/`: the original as `<id>.<ext>`, and `<id>.thumb.jpg` for the grid.
//! Files saved by older builds are `<id>.png` (captures and images) and `<id>.bin` (files).

use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{Rgb, RgbImage, Rgba, RgbaImage};
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use super::model::{Pin, clean_ext, valid_id};
use super::palette::palette;
use crate::{capture, paths};

const IMAGE_DIR: &str = "boards";
pub const THUMB_WIDTH: u32 = 640;
/// Thumbnails of very tall captures keep only the top of the image.
pub const THUMB_MAX_HEIGHT: u32 = 1600;
/// Shell thumbnails are asked for at this size.
const SHELL_THUMB: i32 = 512;
/// Images larger than this are kept as files instead of being decoded.
pub const MAX_DECODE_BYTES: u64 = 200 * 1024 * 1024;
/// Pins filled in by one write of the backfill.
const BACKFILL_BATCH: usize = 20;

pub fn image_dir(app: &AppHandle) -> PathBuf {
    paths::data_file(app, IMAGE_DIR)
}

pub fn thumb_path(app: &AppHandle, id: &str) -> PathBuf {
    image_dir(app).join(format!("{id}.thumb.jpg"))
}

fn legacy_png(app: &AppHandle, id: &str) -> PathBuf {
    image_dir(app).join(format!("{id}.png"))
}

fn legacy_bin(app: &AppHandle, id: &str) -> PathBuf {
    image_dir(app).join(format!("{id}.bin"))
}

fn original_at(app: &AppHandle, id: &str, ext: &str) -> PathBuf {
    image_dir(app).join(format!("{id}.{ext}"))
}

/// The stored original: `<id>.<ext>`, then the older `<id>.png` and `<id>.bin`.
pub fn original_by_id(app: &AppHandle, id: &str, ext: &str) -> Option<PathBuf> {
    if !valid_id(id) {
        return None;
    }
    let ext = clean_ext(ext);
    let mut candidates = Vec::new();
    if !ext.is_empty() {
        candidates.push(original_at(app, id, &ext));
    }
    candidates.push(legacy_png(app, id));
    candidates.push(legacy_bin(app, id));
    candidates.into_iter().find(|path| path.is_file())
}

pub fn original_path(app: &AppHandle, pin: &Pin) -> Option<PathBuf> {
    original_by_id(app, &pin.id, &pin.ext)
}

/// Removes every file of these pins.
pub fn remove_files(app: &AppHandle, pins: &[Pin]) {
    for pin in pins.iter().filter(|pin| valid_id(&pin.id)) {
        if !pin.ext.is_empty() {
            let _ = fs::remove_file(original_at(app, &pin.id, &pin.ext));
        }
        let _ = fs::remove_file(legacy_png(app, &pin.id));
        let _ = fs::remove_file(legacy_bin(app, &pin.id));
        let _ = fs::remove_file(thumb_path(app, &pin.id));
    }
}

/// Removes files written for a pin that was never saved.
pub fn remove_new_files(app: &AppHandle, id: &str, ext: &str) {
    if !valid_id(id) {
        return;
    }
    if !ext.is_empty() {
        let _ = fs::remove_file(original_at(app, id, ext));
    }
    let _ = fs::remove_file(legacy_png(app, id));
    let _ = fs::remove_file(thumb_path(app, id));
}

fn ensure_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = image_dir(app);
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Couldn't create {}. {error}", dir.display()))?;
    Ok(dir)
}

/// JPEG for the grid: at most `THUMB_WIDTH` wide, only the top of very tall images, and
/// transparent pixels drawn on white.
pub fn thumbnail_jpeg(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let resized = thumbnail_image(image);
    let flat = RgbImage::from_fn(resized.width(), resized.height(), |x, y| {
        let Rgba([red, green, blue, alpha]) = *resized.get_pixel(x, y);
        let alpha = u32::from(alpha);
        let blend = |channel: u8| ((u32::from(channel) * alpha + 255 * (255 - alpha)) / 255) as u8;
        Rgb([blend(red), blend(green), blend(blue)])
    });
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 84)
        .encode_image(&flat)
        .map_err(|error| format!("Couldn't make a thumbnail. {error}"))?;
    Ok(out)
}

fn thumbnail_image(image: &RgbaImage) -> RgbaImage {
    let (width, height) = image.dimensions();
    let width = width.max(1);
    let target_width = width.min(THUMB_WIDTH);
    let scale = f64::from(target_width) / f64::from(width);
    let crop_height =
        ((f64::from(THUMB_MAX_HEIGHT) / scale).floor() as u32).clamp(1, height.max(1));
    let cropped = image::imageops::crop_imm(image, 0, 0, width, crop_height).to_image();
    let target_height = ((f64::from(crop_height) * scale).round() as u32).max(1);
    image::imageops::resize(&cropped, target_width, target_height, FilterType::Triangle)
}

/// Writes the thumbnail and returns the palette taken from it.
pub fn write_thumb(
    app: &AppHandle,
    id: &str,
    image: &RgbaImage,
) -> Result<Vec<super::model::PaletteColor>, String> {
    ensure_dir(app)?;
    let small = thumbnail_image(image);
    let jpeg = thumbnail_jpeg(&small)?;
    fs::write(thumb_path(app, id), jpeg)
        .map_err(|error| format!("Couldn't save the thumbnail. {error}"))?;
    Ok(palette(&small))
}

/// A page capture: `<id>.png` plus its thumbnail. Returns the PNG bytes.
pub fn write_capture(app: &AppHandle, id: &str, image: &RgbaImage) -> Result<Vec<u8>, String> {
    ensure_dir(app)?;
    let png = capture::encode_png(image)?;
    fs::write(original_at(app, id, "png"), &png)
        .map_err(|error| format!("Couldn't save the image. {error}"))?;
    Ok(png)
}

pub fn write_original(app: &AppHandle, id: &str, ext: &str, bytes: &[u8]) -> Result<(), String> {
    ensure_dir(app)?;
    fs::write(original_at(app, id, ext), bytes)
        .map_err(|error| format!("Couldn't save the file. {error}"))
}

/// Copies a file into the store. Returns its size and hash.
pub fn copy_original(
    app: &AppHandle,
    id: &str,
    ext: &str,
    from: &Path,
) -> Result<(u64, String), String> {
    ensure_dir(app)?;
    let mut source = fs::File::open(from)
        .map_err(|error| format!("Couldn't read {}. {error}", from.display()))?;
    let target = original_at(app, id, ext);
    let mut out =
        fs::File::create(&target).map_err(|error| format!("Couldn't save the file. {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 256 * 1024];
    let mut size = 0u64;
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|error| format!("Couldn't read {}. {error}", from.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        out.write_all(&buffer[..read])
            .map_err(|error| format!("Couldn't save the file. {error}"))?;
        size += read as u64;
    }
    Ok((size, hex_prefix(&hasher.finalize())))
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    hex_prefix(&Sha256::digest(bytes))
}

pub fn hash_file(path: &Path) -> Option<(u64, String)> {
    let mut file = fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 256 * 1024];
    let mut size = 0u64;
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size += read as u64;
    }
    Some((size, hex_prefix(&hasher.finalize())))
}

fn hex_prefix(digest: &[u8]) -> String {
    digest
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn modified_ms(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub fn decode_image(bytes: &[u8]) -> Result<RgbaImage, String> {
    image::load_from_memory(bytes)
        .map(|image| image.to_rgba8())
        .map_err(|error| format!("That image format isn't supported. {error}"))
}

/// Extension for image bytes, from their content.
pub fn image_ext(bytes: &[u8]) -> Option<&'static str> {
    use image::ImageFormat;
    match image::guess_format(bytes).ok()? {
        ImageFormat::Png => Some("png"),
        ImageFormat::Jpeg => Some("jpg"),
        ImageFormat::Gif => Some("gif"),
        ImageFormat::WebP => Some("webp"),
        ImageFormat::Bmp => Some("bmp"),
        ImageFormat::Ico => Some("ico"),
        _ => None,
    }
}

pub fn mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "m4a" => "audio/mp4",
        "flac" => "audio/flac",
        "pdf" => "application/pdf",
        "txt" | "log" => "text/plain; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        "csv" => "text/csv; charset=utf-8",
        "json" => "application/json",
        // Saved pages and scripts are shown as source, never run.
        "html" | "htm" | "css" | "js" | "xml" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Thumbnail from the Windows shell, for PDFs, videos, PSDs and other files with a
/// thumbnail handler. None when the shell only has an icon. Call on a COM thread.
pub fn shell_thumbnail(path: &Path) -> Option<RgbaImage> {
    use std::ffi::c_void;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
        DeleteObject, GetDIBits, GetObjectW, HGDIOBJ,
    };
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK,
        SIIGBF_THUMBNAILONLY,
    };
    use windows::core::HSTRING;

    unsafe {
        let path = HSTRING::from(path.as_os_str());
        let factory: IShellItemImageFactory = SHCreateItemFromParsingName(&path, None).ok()?;
        let bitmap = factory
            .GetImage(
                SIZE {
                    cx: SHELL_THUMB,
                    cy: SHELL_THUMB,
                },
                SIIGBF_BIGGERSIZEOK | SIIGBF_THUMBNAILONLY,
            )
            .ok()?;
        let mut info = BITMAP::default();
        let read = GetObjectW(
            HGDIOBJ(bitmap.0),
            size_of::<BITMAP>() as i32,
            Some((&mut info as *mut BITMAP).cast::<c_void>()),
        );
        let width = info.bmWidth;
        let height = info.bmHeight.abs();
        if read == 0 || width <= 0 || height <= 0 {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            return None;
        }
        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];
        let dc = CreateCompatibleDC(None);
        let rows = GetDIBits(
            dc,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut header,
            DIB_RGB_COLORS,
        );
        let _ = DeleteDC(dc);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        if rows == 0 {
            return None;
        }
        let opaque = pixels.chunks_exact(4).all(|pixel| pixel[3] == 0);
        let mut image = RgbaImage::new(width as u32, height as u32);
        for (index, pixel) in pixels.chunks_exact(4).enumerate() {
            let x = (index % width as usize) as u32;
            let y = (index / width as usize) as u32;
            let alpha = if opaque { 255 } else { pixel[3] };
            let unpremultiply = |channel: u8| {
                if opaque || alpha == 0 || alpha == 255 {
                    channel
                } else {
                    ((u32::from(channel) * 255) / u32::from(alpha)).min(255) as u8
                }
            };
            image.put_pixel(
                x,
                y,
                Rgba([
                    unpremultiply(pixel[2]),
                    unpremultiply(pixel[1]),
                    unpremultiply(pixel[0]),
                    alpha,
                ]),
            );
        }
        Some(image)
    }
}

/// Calls `CoInitializeEx` once on this thread so shell thumbnails work.
pub fn init_com() {
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}

/// Pins the backfill already went through this session.
static TRIED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// True the first time it is called for `id`.
fn first_try(id: &str) -> bool {
    let mut tried = TRIED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    tried
        .get_or_insert_with(HashSet::new)
        .insert(id.to_string())
}

/// What the backfill found for one pin.
#[derive(Default)]
struct Filled {
    id: String,
    ext: Option<String>,
    size: Option<u64>,
    hash: Option<String>,
    modified: Option<u64>,
    palette: Option<Vec<super::model::PaletteColor>>,
    has_thumb: bool,
}

/// Pins with a stored original. Links added by address have none.
pub fn has_file(pin: &Pin) -> bool {
    pin.has_image || pin.kind == "file"
}

fn needs_backfill(pin: &Pin) -> bool {
    has_file(pin)
        && (pin.ext.is_empty()
            || pin.hash.is_empty()
            || pin.size == 0
            || (pin.has_image && pin.palette.is_empty())
            || (pin.kind == "file" && !pin.has_thumb))
}

fn file_ext(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, ext)| clean_ext(ext))
        .unwrap_or_default()
}

fn fill(app: &AppHandle, pin: &Pin) -> Filled {
    let mut filled = Filled {
        id: pin.id.clone(),
        ..Filled::default()
    };
    let mut path = original_path(app, pin);

    if pin.ext.is_empty() {
        let ext = match path
            .as_ref()
            .and_then(|path| path.extension())
            .and_then(|ext| ext.to_str())
        {
            Some("png") => "png".to_string(),
            Some("bin") => {
                let ext = file_ext(&pin.file_name);
                let ext = if ext.is_empty() {
                    "bin".to_string()
                } else {
                    ext
                };
                if ext != "bin" {
                    let target = original_at(app, &pin.id, &ext);
                    if let Some(from) = &path {
                        if fs::rename(from, &target).is_ok() {
                            path = Some(target);
                        }
                    }
                }
                ext
            }
            Some(other) => clean_ext(other),
            None if pin.kind == "file" => {
                let ext = file_ext(&pin.file_name);
                if ext.is_empty() {
                    "bin".to_string()
                } else {
                    ext
                }
            }
            None => "png".to_string(),
        };
        filled.ext = Some(ext);
    }

    if let Some(path) = &path {
        if pin.hash.is_empty() || pin.size == 0 {
            if let Some((size, hash)) = hash_file(path) {
                filled.size = Some(size);
                filled.hash = Some(hash);
            }
        }
        if pin.file_modified_at == 0 {
            filled.modified = Some(modified_ms(path));
        }
    }

    if pin.has_image && pin.palette.is_empty() {
        let source = fs::read(thumb_path(app, &pin.id))
            .ok()
            .or_else(|| path.as_ref().and_then(|path| fs::read(path).ok()));
        let colors = source
            .and_then(|bytes| decode_image(&bytes).ok())
            .map(|image| palette(&thumbnail_image(&image)))
            .unwrap_or_default();
        filled.palette = Some(colors);
    }

    if pin.kind == "file" && !pin.has_thumb {
        if let Some(path) = &path {
            if let Some(image) = shell_thumbnail(path) {
                if let Ok(colors) = write_thumb(app, &pin.id, &image) {
                    filled.has_thumb = true;
                    if pin.palette.is_empty() {
                        filled.palette = Some(colors);
                    }
                }
            }
        }
    }
    filled
}

/// Fills in extension, size, hash, palette and shell thumbnails that older pins lack. Runs on
/// its own thread a few seconds after start-up.
pub fn start_backfill(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(4));
        init_com();
        loop {
            let snapshot = super::snapshot(&app);
            let pending: Vec<Pin> = snapshot
                .pins
                .iter()
                .filter(|pin| needs_backfill(pin) && first_try(&pin.id))
                .take(BACKFILL_BATCH)
                .cloned()
                .collect();
            if pending.is_empty() {
                return;
            }
            let filled: Vec<Filled> = pending.iter().map(|pin| fill(&app, pin)).collect();
            let result = super::change(&app, |boards| {
                for item in &filled {
                    let Some(pin) = boards.pins.iter_mut().find(|pin| pin.id == item.id) else {
                        continue;
                    };
                    if let Some(ext) = &item.ext {
                        pin.ext = ext.clone();
                    }
                    if let Some(size) = item.size {
                        pin.size = size;
                    }
                    if let Some(hash) = &item.hash {
                        pin.hash = hash.clone();
                    }
                    if let Some(modified) = item.modified {
                        pin.file_modified_at = modified;
                    }
                    if let Some(colors) = &item.palette {
                        pin.palette = colors.clone();
                    }
                    if item.has_thumb {
                        pin.has_thumb = true;
                    }
                }
                Ok(())
            });
            if result.is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnails_of_tall_images_keep_the_top() {
        let image = RgbaImage::from_pixel(1000, 10000, Rgba([0, 0, 0, 0]));
        let jpeg = thumbnail_jpeg(&image).expect("thumb");
        let thumb = image::load_from_memory(&jpeg).expect("decode");
        assert_eq!(thumb.width(), THUMB_WIDTH);
        assert!(thumb.height() <= THUMB_MAX_HEIGHT);
    }

    #[test]
    fn image_bytes_name_their_extension() {
        let image = RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 255]));
        let png = capture::encode_png(&image).expect("png");
        assert_eq!(image_ext(&png), Some("png"));
        assert_eq!(image_ext(b"not an image"), None);
        assert_eq!(hash_bytes(b"a").len(), 32);
    }
}
