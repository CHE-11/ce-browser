//! The Windows desktop, for a run that Settings and the user have both allowed.
//! Coordinates are pixels of the primary monitor.

use std::mem::size_of;

use image::ImageEncoder;

use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT, SendInput, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetSystemMetrics, GetWindowRect, GetWindowTextW, IsWindowVisible, SM_CXSCREEN,
    SM_CXVIRTUALSCREEN, SM_CYSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};
use windows::core::BOOL;

use crate::automation::driver::{self, Shot};

/// A JPEG of the primary monitor, no wider than 1280, and the scale back to screen pixels.
pub fn screenshot() -> Result<(Shot, f64), String> {
    let image = capture()?;
    let rgb = image::DynamicImage::ImageRgba8(image).into_rgb8();
    let width = rgb.width();
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 70)
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|error| error.to_string())?;
    let shot = driver::shrink_jpeg(&bytes, 1280, 70)?;
    let scale = if shot.width == 0 {
        1.0
    } else {
        width as f64 / shot.width as f64
    };
    Ok((shot, scale))
}

fn capture() -> Result<image::RgbaImage, String> {
    unsafe {
        let width = GetSystemMetrics(SM_CXSCREEN);
        let height = GetSystemMetrics(SM_CYSCREEN);
        if width <= 0 || height <= 0 {
            return Err("No screen to capture.".into());
        }
        let desktop = GetDC(None);
        if desktop.is_invalid() {
            return Err("Couldn't read the screen.".into());
        }
        let memory = CreateCompatibleDC(Some(desktop));
        let bitmap = CreateCompatibleBitmap(desktop, width, height);
        let previous = SelectObject(memory, bitmap.into());
        let copied = BitBlt(memory, 0, 0, width, height, Some(desktop), 0, 0, SRCCOPY);
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0 as u32,
                ..BITMAPINFOHEADER::default()
            },
            bmiColors: [Default::default()],
        };
        let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];
        let rows = GetDIBits(
            memory,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        );
        SelectObject(memory, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memory);
        ReleaseDC(None, desktop);
        if copied.is_err() || rows == 0 {
            return Err("Couldn't read the screen.".into());
        }
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        image::RgbaImage::from_raw(width as u32, height as u32, pixels)
            .ok_or_else(|| "Couldn't read the screen.".into())
    }
}

pub fn click(x: f64, y: f64) -> Result<(), String> {
    move_to(x, y)?;
    send_mouse(MOUSEEVENTF_LEFTDOWN, x, y)?;
    send_mouse(MOUSEEVENTF_LEFTUP, x, y)
}

pub fn drag(x: f64, y: f64, to_x: f64, to_y: f64) -> Result<(), String> {
    move_to(x, y)?;
    send_mouse(MOUSEEVENTF_LEFTDOWN, x, y)?;
    for step in 1..=8 {
        let t = f64::from(step) / 8.0;
        let at_x = x + (to_x - x) * t;
        let at_y = y + (to_y - y) * t;
        move_to(at_x, at_y)?;
    }
    send_mouse(MOUSEEVENTF_LEFTUP, to_x, to_y)
}

pub fn type_text(text: &str) -> Result<(), String> {
    for ch in text.chars() {
        key(ch, false)?;
        key(ch, true)?;
    }
    Ok(())
}

/// Visible top-level windows, with their screen rectangles.
pub fn windows() -> Vec<String> {
    let mut titles = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(collect),
            LPARAM(&mut titles as *mut Vec<String> as isize),
        );
    }
    titles.truncate(40);
    titles
}

unsafe extern "system" fn collect(hwnd: HWND, data: LPARAM) -> BOOL {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1);
        }
        let mut buffer = [0u16; 256];
        let count = GetWindowTextW(hwnd, &mut buffer);
        if count <= 0 {
            return BOOL(1);
        }
        let title = String::from_utf16_lossy(&buffer[..count as usize]);
        let title = title.trim();
        if title.is_empty() {
            return BOOL(1);
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return BOOL(1);
        }
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width < 40 || height < 40 {
            return BOOL(1);
        }
        let list = &mut *(data.0 as *mut Vec<String>);
        if list.len() < 40 {
            list.push(format!(
                "{title} at {},{} {width}x{height}",
                rect.left, rect.top
            ));
        }
        BOOL(1)
    }
}

pub fn move_to(x: f64, y: f64) -> Result<(), String> {
    send_mouse(MOUSEEVENTF_MOVE, x, y)
}

fn send_mouse(
    flags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS,
    x: f64,
    y: f64,
) -> Result<(), String> {
    let (dx, dy) = absolute(x, y);
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: 0,
                dwFlags: flags | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let sent = unsafe { SendInput(&[input], size_of::<INPUT>() as i32) };
    if sent == 0 {
        Err("The desktop ignored the mouse.".into())
    } else {
        Ok(())
    }
}

fn absolute(x: f64, y: f64) -> (i32, i32) {
    let origin_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let origin_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) }.max(1);
    let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) }.max(1);
    let dx = ((x.round() as i32 - origin_x) * 65535) / width;
    let dy = ((y.round() as i32 - origin_y) * 65535) / height;
    (dx.clamp(0, 65535), dy.clamp(0, 65535))
}

fn key(ch: char, up: bool) -> Result<(), String> {
    let mut flags = KEYEVENTF_UNICODE;
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: ch as u16,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let sent = unsafe { SendInput(&[input], size_of::<INPUT>() as i32) };
    if sent == 0 {
        Err("The desktop ignored the keyboard.".into())
    } else {
        Ok(())
    }
}
