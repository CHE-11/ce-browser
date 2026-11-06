use std::path::Path;
use std::sync::OnceLock;

use base64::Engine;
use image::imageops::FilterType;
use image::{ExtendedColorType, ImageEncoder, Rgba, RgbaImage};

#[derive(Clone, Copy)]
enum IconKind {
    Logo(Option<f32>),
    Symbol(Symbol, f32),
    /// A browser mark drawn in its own colors. Background tints do not apply.
    Mark(Mark),
}

#[derive(Clone, Copy)]
enum Mark {
    Chrome,
    Chromium,
    Edge,
    Firefox,
    Opera,
    Safari,
    Brave,
    Vivaldi,
    Tor,
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

const COLORS: &[(&str, &str, f32)] = &[
    ("amber", "Amber", 36.0),
    ("emerald", "Emerald", 152.0),
    ("rose", "Rose", 346.0),
    ("violet", "Violet", 272.0),
    ("orange", "Orange", 18.0),
    ("teal", "Teal", 174.0),
];

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
    Preset {
        id: "chrome",
        label: "Chrome",
        kind: IconKind::Mark(Mark::Chrome),
    },
    Preset {
        id: "edge",
        label: "Edge",
        kind: IconKind::Mark(Mark::Edge),
    },
    Preset {
        id: "firefox",
        label: "Firefox",
        kind: IconKind::Mark(Mark::Firefox),
    },
    Preset {
        id: "opera",
        label: "Opera",
        kind: IconKind::Mark(Mark::Opera),
    },
    Preset {
        id: "safari",
        label: "Safari",
        kind: IconKind::Mark(Mark::Safari),
    },
    Preset {
        id: "brave",
        label: "Brave",
        kind: IconKind::Mark(Mark::Brave),
    },
    Preset {
        id: "vivaldi",
        label: "Vivaldi",
        kind: IconKind::Mark(Mark::Vivaldi),
    },
    Preset {
        id: "chromium",
        label: "Chromium",
        kind: IconKind::Mark(Mark::Chromium),
    },
    Preset {
        id: "tor",
        label: "Tor",
        kind: IconKind::Mark(Mark::Tor),
    },
];

#[derive(Clone)]
pub struct IconChoice {
    pub id: &'static str,
    pub label: &'static str,
    pub preview: String,
    /// Background colors apply. Browser marks keep their own colors.
    pub colorable: bool,
}

pub fn default_png() -> &'static [u8] {
    include_bytes!("../icons/icon.png")
}

pub fn choices() -> Vec<IconChoice> {
    cached_choices().clone()
}

pub fn color_choices() -> Vec<IconChoice> {
    cached_colors().clone()
}

pub fn preview(id: &str) -> Option<String> {
    cached_choices()
        .iter()
        .find(|choice| choice.id == id)
        .map(|choice| choice.preview.clone())
}

/// A glyph the icon grid can select. Color names are backgrounds, not icons.
pub fn is_glyph(id: &str) -> bool {
    PRESETS.iter().any(|preset| {
        preset.id == id
            && matches!(
                preset.kind,
                IconKind::Logo(None) | IconKind::Symbol(_, _) | IconKind::Mark(_)
            )
    })
}

/// False for browser marks, which are drawn in fixed colors.
pub fn is_tinted(id: &str) -> bool {
    if id.is_empty() || id == "default" {
        return true;
    }
    PRESETS.iter().any(|preset| {
        preset.id == id && matches!(preset.kind, IconKind::Logo(None) | IconKind::Symbol(_, _))
    })
}

pub fn is_color(id: &str) -> bool {
    color_hue(id).is_some()
}

pub fn render(id: &str) -> Result<RgbaImage, String> {
    if let Some(hue) = color_hue(id) {
        return sized(recolor(source(), hue));
    }
    render_icon(id, "")
}

/// Draws `icon_id` on `color_id`. An empty color keeps the icon's own color.
pub fn render_icon(icon_id: &str, color_id: &str) -> Result<RgbaImage, String> {
    let icon_id = if icon_id.is_empty() {
        "default"
    } else {
        icon_id
    };
    let preset = PRESETS
        .iter()
        .find(|preset| preset.id == icon_id && is_glyph(preset.id))
        .ok_or("That icon is not available.")?;
    let tint = color_hue(color_id);
    let colored = match preset.kind {
        IconKind::Logo(_) => match tint {
            Some(hue) => recolor(source(), hue),
            None => source().clone(),
        },
        IconKind::Symbol(symbol, hue) => draw_symbol(symbol, tint.unwrap_or(hue)),
        IconKind::Mark(mark) => draw_mark(mark),
    };
    sized(colored)
}

fn sized(image: RgbaImage) -> Result<RgbaImage, String> {
    Ok(image::imageops::resize(
        &image,
        256,
        256,
        FilterType::Lanczos3,
    ))
}

fn color_hue(id: &str) -> Option<f32> {
    COLORS
        .iter()
        .find(|(color, _, _)| *color == id)
        .map(|(_, _, hue)| *hue)
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
            .filter(|preset| is_glyph(preset.id))
            .filter_map(|preset| {
                let image = render(preset.id).ok()?;
                let small = image::imageops::resize(&image, 64, 64, FilterType::Triangle);
                let png = encode_png(&small).ok()?;
                Some(IconChoice {
                    id: preset.id,
                    label: preset.label,
                    preview: data_url(&png),
                    colorable: !matches!(preset.kind, IconKind::Mark(_)),
                })
            })
            .collect()
    })
}

fn cached_colors() -> &'static Vec<IconChoice> {
    static COLORS_CACHE: OnceLock<Vec<IconChoice>> = OnceLock::new();
    COLORS_CACHE.get_or_init(|| {
        let mut choices = vec![IconChoice {
            id: "default",
            label: "Default",
            preview: swatch_preview(None),
            colorable: false,
        }];
        choices.extend(COLORS.iter().map(|(id, label, hue)| IconChoice {
            id,
            label,
            preview: swatch_preview(Some(*hue)),
            colorable: false,
        }));
        choices
    })
}

fn swatch_preview(hue: Option<f32>) -> String {
    let mut image = RgbaImage::from_pixel(64, 64, Rgba([0, 0, 0, 0]));
    let color = match hue {
        Some(hue) => {
            let [red, green, blue] = hsl_to_rgb(hue, 0.62, 0.46);
            Rgba([red, green, blue, 255])
        }
        None => Rgba([161, 161, 170, 255]),
    };
    fill_round_rect(&mut image, 4.0, 4.0, 56.0, 56.0, 16.0, color);
    encode_png(&image)
        .map(|png| data_url(&png))
        .unwrap_or_default()
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

fn rgba(red: u8, green: u8, blue: u8) -> Rgba<u8> {
    Rgba([red, green, blue, 255])
}

fn draw_mark(mark: Mark) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(256, 256, Rgba([0, 0, 0, 0]));
    match mark {
        Mark::Chrome => draw_wheel(
            &mut image,
            rgba(0xEA, 0x43, 0x35),
            rgba(0x34, 0xA8, 0x53),
            rgba(0xFB, 0xBC, 0x05),
            rgba(0x42, 0x85, 0xF4),
        ),
        Mark::Chromium => draw_wheel(
            &mut image,
            rgba(0x7B, 0xAA, 0xF7),
            rgba(0x1A, 0x73, 0xE8),
            rgba(0x17, 0x4E, 0xA6),
            rgba(0x1A, 0x73, 0xE8),
        ),
        Mark::Edge => draw_edge(&mut image),
        Mark::Firefox => draw_firefox(&mut image),
        Mark::Opera => draw_opera(&mut image),
        Mark::Safari => draw_safari(&mut image),
        Mark::Brave => draw_brave(&mut image),
        Mark::Vivaldi => draw_vivaldi(&mut image),
        Mark::Tor => draw_tor(&mut image),
    }
    image
}

/// Chrome-style wheel. `top`, `right`, and `left` are the outer thirds; `center` is the hub.
fn draw_wheel(
    image: &mut RgbaImage,
    top: Rgba<u8>,
    right: Rgba<u8>,
    left: Rgba<u8>,
    center: Rgba<u8>,
) {
    let (cx, cy, radius) = (128.0, 128.0, 116.0);
    fill_sector(image, cx, cy, radius, 300.0, 60.0, top);
    fill_sector(image, cx, cy, radius, 60.0, 180.0, right);
    fill_sector(image, cx, cy, radius, 180.0, 300.0, left);
    fill_circle(image, cx, cy, 68.0, rgba(255, 255, 255));
    fill_circle(image, cx, cy, 46.0, center);
}

fn draw_opera(image: &mut RgbaImage) {
    fill_circle(image, 128.0, 128.0, 116.0, rgba(0xFF, 0x1B, 0x2D));
    fill_circle(image, 128.0, 128.0, 64.0, rgba(255, 255, 255));
}

fn draw_vivaldi(image: &mut RgbaImage) {
    fill_circle(image, 128.0, 128.0, 116.0, rgba(0xEF, 0x39, 0x39));
    fill_polygon(
        image,
        &[
            (68.0, 76.0),
            (106.0, 76.0),
            (128.0, 150.0),
            (150.0, 76.0),
            (188.0, 76.0),
            (142.0, 196.0),
            (114.0, 196.0),
        ],
        rgba(255, 255, 255),
    );
}

fn draw_tor(image: &mut RgbaImage) {
    let purple = rgba(0x7D, 0x46, 0x98);
    let pale = rgba(0xF6, 0xEE, 0xF8);
    let mid = rgba(0xC4, 0x8E, 0xD4);
    fill_circle(image, 128.0, 128.0, 116.0, purple);
    fill_round_rect(image, 120.0, 42.0, 16.0, 48.0, 7.0, pale);
    fill_circle(image, 128.0, 154.0, 72.0, pale);
    fill_circle(image, 128.0, 162.0, 50.0, mid);
    fill_circle(image, 128.0, 168.0, 30.0, pale);
    fill_circle(image, 128.0, 174.0, 14.0, purple);
}

fn draw_safari(image: &mut RgbaImage) {
    fill_circle(image, 128.0, 128.0, 116.0, rgba(0xD5, 0xDC, 0xE6));
    fill_circle(image, 128.0, 128.0, 106.0, rgba(0x1F, 0x6F, 0xE5));
    fill_circle(image, 128.0, 128.0, 90.0, rgba(255, 255, 255));
    for index in 0..12 {
        let (x, y) = polar(128.0, 128.0, 76.0, index as f32 * 30.0);
        let radius = if index % 3 == 0 { 3.4 } else { 1.8 };
        fill_circle(image, x, y, radius, rgba(0x9A, 0xA8, 0xBA));
    }
    draw_needle(
        image,
        128.0,
        128.0,
        36.0 + 180.0,
        62.0,
        13.0,
        rgba(0xB7, 0xC2, 0xD0),
    );
    draw_needle(
        image,
        128.0,
        128.0,
        36.0,
        68.0,
        13.0,
        rgba(0xC4, 0x45, 0x3C),
    );
    draw_needle(
        image,
        128.0,
        128.0,
        36.0 + 180.0,
        58.0,
        10.0,
        rgba(255, 255, 255),
    );
    draw_needle(
        image,
        128.0,
        128.0,
        36.0,
        64.0,
        10.0,
        rgba(0xE1, 0x06, 0x00),
    );
    fill_circle(image, 128.0, 128.0, 9.0, rgba(0xE6, 0xEC, 0xF2));
    fill_circle(image, 128.0, 128.0, 4.5, rgba(0x1F, 0x6F, 0xE5));
}

fn draw_needle(
    image: &mut RgbaImage,
    cx: f32,
    cy: f32,
    deg: f32,
    len: f32,
    half: f32,
    color: Rgba<u8>,
) {
    let rad = deg.to_radians();
    let (sin, cos) = rad.sin_cos();
    let forward = (sin, -cos);
    let right = (cos, sin);
    let tip = (cx + forward.0 * len, cy + forward.1 * len);
    let left = (cx + right.0 * half, cy + right.1 * half);
    let right_point = (cx - right.0 * half, cy - right.1 * half);
    fill_polygon(image, &[tip, left, right_point], color);
}

/// A tapering ribbon: blue at the outer tail, green at the inner tip.
fn draw_edge(image: &mut RgbaImage) {
    let steps = 280;
    let mut tip = (128.0, 128.0);
    let mut aim = 0.0;
    for step in 0..steps {
        let t = step as f32 / (steps - 1) as f32;
        let theta = 230.0 + t * 470.0;
        let thickness = 46.0 - t * 30.0;
        let orbit = if t < 0.72 {
            114.0 - thickness * 0.5
        } else {
            let u = (t - 0.72) / 0.28;
            let rim = 114.0 - thickness * 0.5;
            rim + (34.0 - rim) * u
        };
        let (x, y) = polar(128.0, 128.0, orbit.max(6.0), theta);
        fill_circle(image, x, y, (thickness * 0.5).max(3.5), edge_color(t));
        tip = (x, y);
        aim = theta;
    }
    let (fx, fy) = polar(0.0, 0.0, 1.0, aim + 28.0);
    let (px, py) = (-fy, fx);
    fill_polygon(
        image,
        &[
            (tip.0 + fx * 22.0, tip.1 + fy * 22.0),
            (tip.0 + px * 8.0, tip.1 + py * 8.0),
            (tip.0 - px * 8.0, tip.1 - py * 8.0),
        ],
        rgba(0x3C, 0xC8, 0x5A),
    );
    clip_circle(image, 128.0, 128.0, 116.0);
}

fn edge_color(t: f32) -> Rgba<u8> {
    let (start, end, blend) = if t < 0.42 {
        ((0x0C_u8, 0x59, 0xB0), (0x1A, 0xA8, 0xE2), t / 0.42)
    } else {
        ((0x1A, 0xA8, 0xE2), (0x3C, 0xC8, 0x5A), (t - 0.42) / 0.58)
    };
    rgba(
        lerp_u8(start.0, end.0, blend),
        lerp_u8(start.1, end.1, blend),
        lerp_u8(start.2, end.2, blend),
    )
}

fn lerp_u8(start: u8, end: u8, t: f32) -> u8 {
    (start as f32 + (end as f32 - start as f32) * t.clamp(0.0, 1.0)).round() as u8
}

fn draw_firefox(image: &mut RgbaImage) {
    let orange = rgba(0xFF, 0x71, 0x39);
    let yellow = rgba(0xFF, 0xD2, 0x3A);
    let deep = rgba(0xE0, 0x3E, 0x00);
    let purple = rgba(0x8E, 0x24, 0xAA);
    let blue = rgba(0x00, 0x62, 0xD6);
    let cyan = rgba(0x00, 0xC2, 0xF0);
    let ink = rgba(0x3A, 0x15, 0x10);
    fill_circle(image, 128.0, 128.0, 116.0, orange);
    fill_ellipse(image, 74.0, 150.0, 32.0, 58.0, -0.8, purple);
    fill_ellipse(image, 80.0, 138.0, 14.0, 32.0, -0.8, rgba(0xE0, 0x40, 0xA0));
    fill_circle(image, 140.0, 172.0, 58.0, blue);
    fill_ellipse(image, 140.0, 158.0, 56.0, 16.0, -0.25, cyan);
    fill_polygon(
        image,
        &[(92.0, 108.0), (114.0, 58.0), (134.0, 104.0)],
        yellow,
    );
    fill_polygon(
        image,
        &[(100.0, 106.0), (114.0, 72.0), (126.0, 104.0)],
        deep,
    );
    fill_polygon(
        image,
        &[(126.0, 100.0), (164.0, 56.0), (170.0, 112.0)],
        yellow,
    );
    fill_polygon(
        image,
        &[(136.0, 102.0), (160.0, 70.0), (162.0, 110.0)],
        deep,
    );
    fill_ellipse(image, 124.0, 132.0, 52.0, 44.0, 0.0, yellow);
    fill_ellipse(image, 146.0, 154.0, 24.0, 16.0, 0.2, rgba(0xFF, 0xE4, 0x90));
    fill_circle(image, 158.0, 156.0, 6.0, ink);
    fill_circle(image, 108.0, 126.0, 5.5, ink);
    fill_circle(image, 140.0, 122.0, 5.5, ink);
    fill_circle(image, 110.0, 124.0, 2.0, rgba(255, 255, 255));
    fill_circle(image, 142.0, 120.0, 2.0, rgba(255, 255, 255));
    clip_circle(image, 128.0, 128.0, 116.0);
}

fn draw_brave(image: &mut RgbaImage) {
    let orange = rgba(0xFB, 0x54, 0x2B);
    let white = rgba(255, 255, 255);
    fill_circle(image, 128.0, 128.0, 116.0, orange);
    for deg in [210.0, 245.0, 280.0, 320.0, 0.0, 40.0, 80.0, 115.0, 150.0] {
        let (x, y) = polar(128.0, 134.0, 46.0, deg);
        fill_circle(image, x, y, 24.0, white);
    }
    fill_polygon(image, &[(102.0, 96.0), (116.0, 50.0), (132.0, 98.0)], white);
    fill_polygon(image, &[(154.0, 96.0), (140.0, 50.0), (124.0, 98.0)], white);
    fill_circle(image, 128.0, 142.0, 40.0, white);
    fill_ellipse(image, 128.0, 166.0, 18.0, 12.0, 0.0, white);
    fill_circle(image, 128.0, 186.0, 12.0, white);
    fill_circle(image, 114.0, 136.0, 5.5, orange);
    fill_circle(image, 142.0, 136.0, 5.5, orange);
    fill_circle(image, 116.0, 134.0, 2.0, white);
    fill_circle(image, 144.0, 134.0, 2.0, white);
    fill_polygon(
        image,
        &[(128.0, 148.0), (120.0, 160.0), (136.0, 160.0)],
        orange,
    );
    clip_circle(image, 128.0, 128.0, 116.0);
}

/// `deg` is clockwise from 12 o'clock.
fn polar(cx: f32, cy: f32, radius: f32, deg: f32) -> (f32, f32) {
    let rad = deg.to_radians();
    (cx + radius * rad.sin(), cy - radius * rad.cos())
}

fn fill_sector(
    image: &mut RgbaImage,
    cx: f32,
    cy: f32,
    radius: f32,
    start: f32,
    end: f32,
    color: Rgba<u8>,
) {
    if radius <= 0.0 {
        return;
    }
    let x0 = (cx - radius).floor().max(0.0) as u32;
    let y0 = (cy - radius).floor().max(0.0) as u32;
    let x1 = (cx + radius).ceil().min(image.width() as f32 - 1.0) as u32;
    let y1 = (cy + radius).ceil().min(image.height() as f32 - 1.0) as u32;
    let radius_sq = radius * radius;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            if dx * dx + dy * dy > radius_sq {
                continue;
            }
            let deg = dx.atan2(-dy).to_degrees().rem_euclid(360.0);
            if angle_in(deg, start, end) {
                image.put_pixel(x, y, color);
            }
        }
    }
}

fn angle_in(deg: f32, start: f32, end: f32) -> bool {
    let start = start.rem_euclid(360.0);
    let end = end.rem_euclid(360.0);
    if start <= end {
        deg >= start && deg < end
    } else {
        deg >= start || deg < end
    }
}

fn clip_circle(image: &mut RgbaImage, cx: f32, cy: f32, radius: f32) {
    let radius_sq = radius * radius;
    for y in 0..image.height() {
        for x in 0..image.width() {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            if dx * dx + dy * dy > radius_sq {
                image.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
    }
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
    fn a_background_color_changes_the_symbol() {
        let own = render_icon("heart", "").unwrap();
        let teal = render_icon("heart", "teal").unwrap();
        assert!(
            own.pixels()
                .zip(teal.pixels())
                .any(|(left, right)| left.0 != right.0)
        );
        assert!(render_icon("nope", "teal").is_err());
    }

    #[test]
    fn browser_marks_keep_their_colors() {
        let chrome = render_icon("chrome", "").unwrap();
        let tinted = render_icon("chrome", "rose").unwrap();
        assert_eq!(chrome.as_raw(), tinted.as_raw());
        assert!(is_glyph("chrome"));
        assert!(!is_tinted("chrome"));
        assert!(is_tinted("default"));
        assert!(is_tinted(""));
        assert!(is_tinted("heart"));
        let listed = choices();
        let chrome_choice = listed.iter().find(|choice| choice.id == "chrome").unwrap();
        assert_eq!(chrome_choice.label, "Chrome");
        assert!(!chrome_choice.colorable);
        assert!(
            listed
                .iter()
                .any(|choice| choice.id == "heart" && choice.colorable)
        );
        for id in [
            "chrome", "edge", "firefox", "opera", "safari", "brave", "vivaldi", "chromium", "tor",
        ] {
            assert!(is_glyph(id), "{id}");
            assert!(!is_tinted(id), "{id}");
            let image = render(id).unwrap();
            assert_eq!(image.dimensions(), (256, 256));
            assert!(image.pixels().any(|pixel| pixel.0[3] > 0));
        }
        assert!(has_near(&chrome, [0xEA, 0x43, 0x35]));
        assert!(has_near(&chrome, [0x34, 0xA8, 0x53]));
        assert!(has_near(&chrome, [0xFB, 0xBC, 0x05]));
        assert!(has_near(&chrome, [0x42, 0x85, 0xF4]));
    }

    fn has_near(image: &RgbaImage, target: [u8; 3]) -> bool {
        image.pixels().any(|pixel| {
            let [red, green, blue, alpha] = pixel.0;
            alpha > 200
                && (red as i16 - target[0] as i16).abs() < 8
                && (green as i16 - target[1] as i16).abs() < 8
                && (blue as i16 - target[2] as i16).abs() < 8
        })
    }
}
