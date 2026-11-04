use std::path::Path;
use std::sync::OnceLock;

use base64::Engine;
use image::imageops::FilterType;
use image::{ExtendedColorType, ImageEncoder, Rgba, RgbaImage};

#[derive(Clone, Copy)]
enum IconKind {
    Logo(Option<f32>),
    Symbol(Symbol, f32),
}

#[derive(Clone, Copy)]
enum Symbol {
    Heart,
    Play,
    Note,
    Briefcase,
    Book,
    Star,
    Home,
    Bag,
}

struct Preset {
    id: &'static str,
    label: &'static str,
    kind: IconKind,
}

const PRESETS: &[Preset] = &[
    Preset {
        id: "default",
        label: "Default",
        kind: IconKind::Logo(None),
    },
    Preset {
        id: "amber",
        label: "Amber",
        kind: IconKind::Logo(Some(36.0)),
    },
    Preset {
        id: "emerald",
        label: "Emerald",
        kind: IconKind::Logo(Some(152.0)),
    },
    Preset {
        id: "rose",
        label: "Rose",
        kind: IconKind::Logo(Some(346.0)),
    },
    Preset {
        id: "violet",
        label: "Violet",
        kind: IconKind::Logo(Some(272.0)),
    },
    Preset {
        id: "orange",
        label: "Orange",
        kind: IconKind::Logo(Some(18.0)),
    },
    Preset {
        id: "teal",
        label: "Teal",
        kind: IconKind::Logo(Some(174.0)),
    },
    Preset {
        id: "heart",
        label: "Heart",
        kind: IconKind::Symbol(Symbol::Heart, 346.0),
    },
    Preset {
        id: "media",
        label: "Media",
        kind: IconKind::Symbol(Symbol::Play, 211.0),
    },
    Preset {
        id: "music",
        label: "Music",
        kind: IconKind::Symbol(Symbol::Note, 282.0),
    },
    Preset {
        id: "work",
        label: "Work",
        kind: IconKind::Symbol(Symbol::Briefcase, 32.0),
    },
    Preset {
        id: "book",
        label: "Book",
        kind: IconKind::Symbol(Symbol::Book, 142.0),
    },
    Preset {
        id: "star",
        label: "Star",
        kind: IconKind::Symbol(Symbol::Star, 46.0),
    },
    Preset {
        id: "home",
        label: "Home",
        kind: IconKind::Symbol(Symbol::Home, 204.0),
    },
    Preset {
        id: "shopping",
        label: "Shopping",
        kind: IconKind::Symbol(Symbol::Bag, 16.0),
    },
];

#[derive(Clone)]
pub struct IconChoice {
    pub id: &'static str,
    pub label: &'static str,
    pub preview: String,
}

pub fn default_png() -> &'static [u8] {
    include_bytes!("../icons/icon.png")
}

pub fn choices() -> Vec<IconChoice> {
    cached_choices().clone()
}

pub fn preview(id: &str) -> Option<String> {
    cached_choices()
        .iter()
        .find(|choice| choice.id == id)
        .map(|choice| choice.preview.clone())
}

pub fn render(id: &str) -> Result<RgbaImage, String> {
    let preset = PRESETS
        .iter()
        .find(|preset| preset.id == id)
        .ok_or("That icon is not available.")?;
    let colored = match preset.kind {
        IconKind::Logo(None) => source().clone(),
        IconKind::Logo(Some(hue)) => recolor(source(), hue),
        IconKind::Symbol(symbol, hue) => draw_symbol(symbol, hue),
    };
    Ok(image::imageops::resize(
        &colored,
        256,
        256,
        FilterType::Lanczos3,
    ))
}

pub fn from_file(path: &Path) -> Result<RgbaImage, String> {
    let meta = std::fs::metadata(path).map_err(|_| "That image could not be read.".to_string())?;
    if !meta.is_file() {
        return Err("Choose an image file.".into());
    }
    if meta.len() > 12 * 1024 * 1024 {
        return Err("That image is larger than 12 MB.".into());
    }
    let image = image::ImageReader::open(path)
        .map_err(|_| "That image could not be read.".to_string())?
        .with_guessed_format()
        .map_err(|_| "That image could not be read.".to_string())?
        .decode()
        .map_err(|_| "Use a PNG, JPEG, GIF, BMP, or ICO image.".to_string())?;
    if image.width() > 4096 || image.height() > 4096 {
        return Err("That image is larger than 4096 pixels.".into());
    }
    let rgba = image.to_rgba8();
    let side = rgba.width().min(rgba.height());
    if side == 0 {
        return Err("That image is empty.".into());
    }
    let x = (rgba.width() - side) / 2;
    let y = (rgba.height() - side) / 2;
    let cropped = image::imageops::crop_imm(&rgba, x, y, side, side).to_image();
    Ok(image::imageops::resize(
        &cropped,
        256,
        256,
        FilterType::Lanczos3,
    ))
}

pub fn preview_file(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let small = image::imageops::resize(&image, 64, 64, FilterType::Triangle);
    encode_png(&small).ok().map(|png| data_url(&png))
}

pub fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|_| "The icon could not be saved.".to_string())?;
    Ok(bytes)
}

pub fn encode_ico(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut frames = Vec::new();
    for size in [16u32, 32, 48, 256] {
        let resized = image::imageops::resize(image, size, size, FilterType::Lanczos3);
        let frame = image::codecs::ico::IcoFrame::as_png(
            resized.as_raw(),
            size,
            size,
            ExtendedColorType::Rgba8,
        )
        .map_err(|_| "The icon could not be saved.".to_string())?;
        frames.push(frame);
    }
    let mut bytes = Vec::new();
    image::codecs::ico::IcoEncoder::new(&mut bytes)
        .encode_images(&frames)
        .map_err(|_| "The icon could not be saved.".to_string())?;
    Ok(bytes)
}

fn cached_choices() -> &'static Vec<IconChoice> {
    static CHOICES: OnceLock<Vec<IconChoice>> = OnceLock::new();
    CHOICES.get_or_init(|| {
        PRESETS
            .iter()
            .filter_map(|preset| {
                let image = render(preset.id).ok()?;
                let small = image::imageops::resize(&image, 64, 64, FilterType::Triangle);
                let png = encode_png(&small).ok()?;
                Some(IconChoice {
                    id: preset.id,
                    label: preset.label,
                    preview: data_url(&png),
                })
            })
            .collect()
    })
}

fn source() -> &'static RgbaImage {
    static SOURCE: OnceLock<RgbaImage> = OnceLock::new();
    SOURCE.get_or_init(|| {
        let image = image::load_from_memory(default_png())
            .expect("app icon")
            .to_rgba8();
        if image.width() > 512 || image.height() > 512 {
            image::imageops::resize(&image, 512, 512, FilterType::Lanczos3)
        } else {
            image
        }
    })
}

fn recolor(image: &RgbaImage, hue: f32) -> RgbaImage {
    let mut out = image.clone();
    for pixel in out.pixels_mut() {
        let [red, green, blue, alpha] = pixel.0;
        if alpha == 0 {
            continue;
        }
        let (_, saturation, lightness) = rgb_to_hsl(red, green, blue);
        if saturation < 0.12 {
            continue;
        }
        let [red, green, blue] = hsl_to_rgb(hue, saturation, lightness);
        *pixel = Rgba([red, green, blue, alpha]);
    }
    out
}

fn data_url(png: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    )
}

fn rgb_to_hsl(red: u8, green: u8, blue: u8) -> (f32, f32, f32) {
    let red = red as f32 / 255.0;
    let green = green as f32 / 255.0;
    let blue = blue as f32 / 255.0;
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let lightness = (max + min) / 2.0;
    let delta = max - min;
    if delta < f32::EPSILON {
        return (0.0, 0.0, lightness);
    }
    let saturation = if lightness > 0.5 {
        delta / (2.0 - max - min)
    } else {
        delta / (max + min)
    };
    let hue = if max == red {
        (green - blue) / delta + if green < blue { 6.0 } else { 0.0 }
    } else if max == green {
        (blue - red) / delta + 2.0
    } else {
        (red - green) / delta + 4.0
    };
    (hue * 60.0, saturation, lightness)
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> [u8; 3] {
    let hue = hue.rem_euclid(360.0) / 360.0;
    if saturation <= f32::EPSILON {
        let value = (lightness * 255.0).round() as u8;
        return [value, value, value];
    }
    let q = if lightness < 0.5 {
        lightness * (1.0 + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let p = 2.0 * lightness - q;
    let red = hue_channel(p, q, hue + 1.0 / 3.0);
    let green = hue_channel(p, q, hue);
    let blue = hue_channel(p, q, hue - 1.0 / 3.0);
    [
        (red * 255.0).round() as u8,
        (green * 255.0).round() as u8,
        (blue * 255.0).round() as u8,
    ]
}

fn hue_channel(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 0.5 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

fn draw_symbol(symbol: Symbol, hue: f32) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(256, 256, Rgba([0, 0, 0, 0]));
    let [red, green, blue] = hsl_to_rgb(hue, 0.62, 0.46);
    let background = Rgba([red, green, blue, 255]);
    let white = Rgba([255, 255, 255, 255]);
    fill_round_rect(&mut image, 8.0, 8.0, 240.0, 240.0, 54.0, background);
    match symbol {
        Symbol::Heart => draw_heart(&mut image, white),
        Symbol::Play => fill_polygon(
            &mut image,
            &[(96.0, 72.0), (96.0, 184.0), (188.0, 128.0)],
            white,
        ),
        Symbol::Note => draw_note(&mut image, white),
        Symbol::Briefcase => draw_briefcase(&mut image, white, background),
        Symbol::Book => draw_book(&mut image, white, background),
        Symbol::Star => fill_polygon(&mut image, &star_points(128.0, 132.0, 78.0, 32.0), white),
        Symbol::Home => draw_home(&mut image, white, background),
        Symbol::Bag => draw_bag(&mut image, white, background),
    }
    image
}

fn draw_heart(image: &mut RgbaImage, color: Rgba<u8>) {
    fill_circle(image, 100.0, 108.0, 36.0, color);
    fill_circle(image, 156.0, 108.0, 36.0, color);
    fill_polygon(
        image,
        &[(68.0, 122.0), (188.0, 122.0), (128.0, 198.0)],
        color,
    );
}

fn draw_note(image: &mut RgbaImage, color: Rgba<u8>) {
    fill_ellipse(image, 108.0, 172.0, 36.0, 24.0, -0.55, color);
    fill_round_rect(image, 128.0, 64.0, 16.0, 116.0, 6.0, color);
    fill_polygon(
        image,
        &[(144.0, 64.0), (196.0, 92.0), (184.0, 116.0), (144.0, 98.0)],
        color,
    );
}

fn draw_briefcase(image: &mut RgbaImage, color: Rgba<u8>, background: Rgba<u8>) {
    fill_round_rect(image, 52.0, 104.0, 152.0, 92.0, 16.0, color);
    fill_round_rect(image, 100.0, 64.0, 56.0, 52.0, 16.0, color);
    fill_round_rect(image, 114.0, 78.0, 28.0, 26.0, 10.0, background);
}

fn draw_book(image: &mut RgbaImage, color: Rgba<u8>, background: Rgba<u8>) {
    let spine = darker(background);
    fill_round_rect(image, 64.0, 48.0, 136.0, 160.0, 14.0, color);
    fill_round_rect(image, 64.0, 48.0, 36.0, 160.0, 14.0, spine);
    fill_rect(image, 82.0, 48.0, 18.0, 160.0, spine);
    fill_polygon(
        image,
        &[(148.0, 48.0), (172.0, 48.0), (160.0, 100.0)],
        spine,
    );
}

fn darker(color: Rgba<u8>) -> Rgba<u8> {
    Rgba([
        (color.0[0] as f32 * 0.55).round() as u8,
        (color.0[1] as f32 * 0.55).round() as u8,
        (color.0[2] as f32 * 0.55).round() as u8,
        color.0[3],
    ])
}

fn draw_home(image: &mut RgbaImage, color: Rgba<u8>, background: Rgba<u8>) {
    fill_polygon(
        image,
        &[(128.0, 48.0), (48.0, 124.0), (208.0, 124.0)],
        color,
    );
    fill_round_rect(image, 72.0, 112.0, 112.0, 96.0, 8.0, color);
    fill_rect(image, 112.0, 152.0, 32.0, 56.0, background);
}

fn draw_bag(image: &mut RgbaImage, color: Rgba<u8>, background: Rgba<u8>) {
    fill_polygon(
        image,
        &[(72.0, 104.0), (60.0, 204.0), (196.0, 204.0), (184.0, 104.0)],
        color,
    );
    fill_round_rect(image, 88.0, 56.0, 80.0, 64.0, 28.0, color);
    fill_round_rect(image, 108.0, 72.0, 40.0, 32.0, 14.0, background);
}

fn star_points(cx: f32, cy: f32, outer: f32, inner: f32) -> [(f32, f32); 10] {
    let mut points = [(0.0, 0.0); 10];
    for index in 0..10 {
        let angle = -std::f32::consts::FRAC_PI_2 + index as f32 * std::f32::consts::PI / 5.0;
        let radius = if index % 2 == 0 { outer } else { inner };
        points[index] = (cx + radius * angle.cos(), cy + radius * angle.sin());
    }
    points
}

fn fill_polygon(image: &mut RgbaImage, points: &[(f32, f32)], color: Rgba<u8>) {
    if points.len() < 3 {
        return;
    }
    let mut min_x = points[0].0;
    let mut max_x = points[0].0;
    let mut min_y = points[0].1;
    let mut max_y = points[0].1;
    for point in points {
        min_x = min_x.min(point.0);
        max_x = max_x.max(point.0);
        min_y = min_y.min(point.1);
        max_y = max_y.max(point.1);
    }
    let x0 = min_x.floor().max(0.0) as u32;
    let y0 = min_y.floor().max(0.0) as u32;
    let x1 = max_x.ceil().min(image.width() as f32 - 1.0) as u32;
    let y1 = max_y.ceil().min(image.height() as f32 - 1.0) as u32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            if point_in_polygon(x as f32 + 0.5, y as f32 + 0.5, points) {
                image.put_pixel(x, y, color);
            }
        }
    }
}

fn point_in_polygon(x: f32, y: f32, points: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let mut previous = points.len() - 1;
    for index in 0..points.len() {
        let (xi, yi) = points[index];
        let (xj, yj) = points[previous];
        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        previous = index;
    }
    inside
}

fn fill_circle(image: &mut RgbaImage, cx: f32, cy: f32, radius: f32, color: Rgba<u8>) {
    fill_ellipse(image, cx, cy, radius, radius, 0.0, color);
}

fn fill_ellipse(
    image: &mut RgbaImage,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    rotation: f32,
    color: Rgba<u8>,
) {
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let reach = rx.max(ry);
    let x0 = (cx - reach).floor().max(0.0) as u32;
    let y0 = (cy - reach).floor().max(0.0) as u32;
    let x1 = (cx + reach).ceil().min(image.width() as f32 - 1.0) as u32;
    let y1 = (cy + reach).ceil().min(image.height() as f32 - 1.0) as u32;
    let (sin, cos) = rotation.sin_cos();
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let local_x = dx * cos + dy * sin;
            let local_y = -dx * sin + dy * cos;
            if (local_x / rx).powi(2) + (local_y / ry).powi(2) <= 1.0 {
                image.put_pixel(x, y, color);
            }
        }
    }
}

fn fill_round_rect(
    image: &mut RgbaImage,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    color: Rgba<u8>,
) {
    let radius = radius.max(0.0).min(width / 2.0).min(height / 2.0);
    let right = x + width;
    let bottom = y + height;
    let x0 = x.floor().max(0.0) as u32;
    let y0 = y.floor().max(0.0) as u32;
    let x1 = right.ceil().min(image.width() as f32 - 1.0) as u32;
    let y1 = bottom.ceil().min(image.height() as f32 - 1.0) as u32;
    for py in y0..=y1 {
        for px in x0..=x1 {
            let point_x = px as f32 + 0.5;
            let point_y = py as f32 + 0.5;
            if point_x < x || point_x >= right || point_y < y || point_y >= bottom {
                continue;
            }
            let inside_body = (point_x >= x + radius && point_x < right - radius)
                || (point_y >= y + radius && point_y < bottom - radius);
            if inside_body {
                image.put_pixel(px, py, color);
                continue;
            }
            let corner_x = if point_x < x + radius {
                x + radius
            } else {
                right - radius
            };
            let corner_y = if point_y < y + radius {
                y + radius
            } else {
                bottom - radius
            };
            let dx = point_x - corner_x;
            let dy = point_y - corner_y;
            if dx * dx + dy * dy <= radius * radius {
                image.put_pixel(px, py, color);
            }
        }
    }
}

fn fill_rect(image: &mut RgbaImage, x: f32, y: f32, width: f32, height: f32, color: Rgba<u8>) {
    fill_round_rect(image, x, y, width, height, 0.0, color);
}

/// Draws the first 10 characters of `name` at the top center. An empty name leaves the icon unchanged.
pub fn stamp_name(image: &RgbaImage, name: &str) -> Result<RgbaImage, String> {
    let label: String = name.trim().chars().take(10).collect();
    if label.is_empty() {
        return Ok(image.clone());
    }
    let fill = text_coverage(image.width(), image.height(), &label)?;
    let halo = dilate(&fill, image.width() as usize, image.height() as usize, 2);
    let mut out = image.clone();
    for (index, pixel) in out.pixels_mut().enumerate() {
        let fill_alpha = fill[index];
        let halo_alpha = halo[index];
        if fill_alpha == 0 && halo_alpha == 0 {
            continue;
        }
        let (red, green, blue, alpha) = if fill_alpha > 0 {
            (255, 255, 255, fill_alpha)
        } else {
            (16, 16, 20, halo_alpha)
        };
        let cover = alpha as f32 / 255.0;
        let keep = 1.0 - cover;
        pixel[0] = (red as f32 * cover + pixel[0] as f32 * keep).round() as u8;
        pixel[1] = (green as f32 * cover + pixel[1] as f32 * keep).round() as u8;
        pixel[2] = (blue as f32 * cover + pixel[2] as f32 * keep).round() as u8;
        pixel[3] = pixel[3].max(alpha);
    }
    Ok(out)
}

pub fn read_png(path: &Path) -> Result<RgbaImage, String> {
    let bytes = std::fs::read(path).map_err(|_| "That icon could not be read.".to_string())?;
    let image = image::load_from_memory(&bytes)
        .map_err(|_| "That icon could not be read.".to_string())?
        .to_rgba8();
    if image.width() == 256 && image.height() == 256 {
        return Ok(image);
    }
    Ok(image::imageops::resize(
        &image,
        256,
        256,
        FilterType::Lanczos3,
    ))
}

fn text_coverage(width: u32, height: u32, label: &str) -> Result<Vec<u8>, String> {
    use std::ffi::c_void;

    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        ANTIALIASED_QUALITY, BITMAPINFO, BITMAPINFOHEADER, CLIP_DEFAULT_PRECIS, CreateCompatibleDC,
        CreateDIBSection, CreateFontW, DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, DT_CENTER,
        DT_NOPREFIX, DT_SINGLELINE, DT_TOP, DeleteDC, DeleteObject, DrawTextW, FW_SEMIBOLD,
        GetTextExtentPoint32W, OUT_DEFAULT_PRECIS, SelectObject, SetBkMode, SetTextColor,
        TRANSPARENT,
    };
    use windows::core::w;

    let mut wide: Vec<u16> = label.encode_utf16().collect();
    let margin = 36i32;
    let top = 18i32;
    let limit = width as i32 - margin * 2;
    unsafe {
        let memory = CreateCompatibleDC(None);
        if memory.is_invalid() {
            return Err("The icon text could not be drawn.".into());
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let bitmap = match CreateDIBSection(Some(memory), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        {
            Ok(bitmap) => bitmap,
            Err(_) => {
                let _ = DeleteDC(memory);
                return Err("The icon text could not be drawn.".into());
            }
        };
        if bits.is_null() {
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(memory);
            return Err("The icon text could not be drawn.".into());
        }
        let previous_bitmap = SelectObject(memory, bitmap.into());
        let mut height_px = 40i32;
        let mut chosen = None;
        let mut previous_font = None;
        while height_px >= 16 {
            let font = CreateFontW(
                -height_px,
                0,
                0,
                0,
                FW_SEMIBOLD.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                ANTIALIASED_QUALITY,
                DEFAULT_PITCH.0 as u32,
                w!("Segoe UI"),
            );
            if font.is_invalid() {
                height_px -= 2;
                continue;
            }
            let replaced = SelectObject(memory, font.into());
            if previous_font.is_none() {
                previous_font = Some(replaced);
            }
            let mut size = windows::Win32::Foundation::SIZE::default();
            let measured = GetTextExtentPoint32W(memory, &wide, &mut size);
            if measured.as_bool() && (size.cx <= limit || height_px <= 16) {
                chosen = Some(font);
                break;
            }
            SelectObject(memory, replaced);
            let _ = DeleteObject(font.into());
            height_px -= 2;
        }
        let Some(font) = chosen else {
            SelectObject(memory, previous_bitmap);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(memory);
            return Err("The icon text could not be drawn.".into());
        };
        let _ = SetBkMode(memory, TRANSPARENT);
        let _ = SetTextColor(memory, COLORREF(0x00FF_FFFF));
        let count = (width * height) as usize;
        std::ptr::write_bytes(bits, 0, count * 4);
        let mut rect = RECT {
            left: margin,
            top,
            right: width as i32 - margin,
            bottom: height as i32,
        };
        DrawTextW(
            memory,
            &mut wide,
            &mut rect,
            DT_CENTER | DT_TOP | DT_SINGLELINE | DT_NOPREFIX,
        );
        let bytes = std::slice::from_raw_parts(bits.cast::<u8>(), count * 4);
        let mut coverage = vec![0u8; count];
        for (pixel, chunk) in coverage.iter_mut().zip(bytes.chunks_exact(4)) {
            *pixel = chunk[0].max(chunk[1]).max(chunk[2]);
        }
        if let Some(previous_font) = previous_font {
            SelectObject(memory, previous_font);
        }
        SelectObject(memory, previous_bitmap);
        let _ = DeleteObject(font.into());
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memory);
        Ok(coverage)
    }
}

fn dilate(source: &[u8], width: usize, height: usize, radius: i32) -> Vec<u8> {
    let mut out = vec![0u8; source.len()];
    let radius_sq = radius * radius;
    for y in 0..height {
        for x in 0..width {
            let mut best = 0u8;
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx * dx + dy * dy > radius_sq {
                        continue;
                    }
                    let next_x = x as i32 + dx;
                    let next_y = y as i32 + dy;
                    if next_x < 0 || next_y < 0 || next_x >= width as i32 || next_y >= height as i32
                    {
                        continue;
                    }
                    best = best.max(source[next_y as usize * width + next_x as usize]);
                }
            }
            out[y * width + x] = best;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_encode_to_icons() {
        let amber = render("amber").unwrap();
        let sky = render("default").unwrap();
        let heart = render("heart").unwrap();
        let ico = encode_ico(&heart).unwrap();
        assert_eq!(&ico[0..4], &[0, 0, 1, 0]);
        assert_eq!(heart.dimensions(), (256, 256));
        assert!(
            amber
                .pixels()
                .zip(sky.pixels())
                .any(|(left, right)| left.0 != right.0)
        );
        assert!(
            heart
                .pixels()
                .zip(sky.pixels())
                .any(|(left, right)| left.0 != right.0)
        );
        for preset in PRESETS {
            assert_eq!(render(preset.id).unwrap().dimensions(), (256, 256));
        }
    }

    #[test]
    fn unknown_preset_is_rejected() {
        assert!(render("nope").is_err());
    }

    #[test]
    fn stamped_name_changes_the_top_and_keeps_ten_letters() {
        let image = image::RgbaImage::from_pixel(256, 256, image::Rgba([20, 80, 160, 255]));
        let stamped = stamp_name(&image, "Workbench profile").unwrap();
        let changed = (16..90)
            .any(|y| (36..220).any(|x| stamped.get_pixel(x, y).0 != image.get_pixel(x, y).0));
        assert!(changed);
        let lower_same = (150..240)
            .all(|y| (0..256).all(|x| stamped.get_pixel(x, y).0 == image.get_pixel(x, y).0));
        assert!(lower_same);
        let ten = stamp_name(&image, "AAAAAAAAAA").unwrap();
        let eleven = stamp_name(&image, "AAAAAAAAAAB").unwrap();
        assert_eq!(ten.as_raw(), eleven.as_raw());
        assert_eq!(stamp_name(&image, "   ").unwrap().as_raw(), image.as_raw());
    }
}
