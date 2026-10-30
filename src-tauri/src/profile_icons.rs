use std::path::Path;
use std::sync::OnceLock;

use base64::Engine;
use image::imageops::FilterType;
use image::{ExtendedColorType, ImageEncoder, Rgba, RgbaImage};

const PRESETS: &[(&str, &str, Option<f32>)] = &[
    ("default", "Default", None),
    ("amber", "Amber", Some(36.0)),
    ("emerald", "Emerald", Some(152.0)),
    ("rose", "Rose", Some(346.0)),
    ("violet", "Violet", Some(272.0)),
    ("orange", "Orange", Some(18.0)),
    ("teal", "Teal", Some(174.0)),
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
    let hue = hue_for(id).ok_or("That icon is not available.")?;
    let colored = match hue {
        Some(hue) => recolor(source(), hue),
        None => source().clone(),
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
            .filter_map(|(id, label, _)| {
                let image = render(id).ok()?;
                let small = image::imageops::resize(&image, 64, 64, FilterType::Triangle);
                let png = encode_png(&small).ok()?;
                Some(IconChoice {
                    id,
                    label,
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

fn hue_for(id: &str) -> Option<Option<f32>> {
    PRESETS
        .iter()
        .find(|(preset, _, _)| *preset == id)
        .map(|(_, _, hue)| *hue)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_encode_to_icons() {
        let amber = render("amber").unwrap();
        let sky = render("default").unwrap();
        let ico = encode_ico(&amber).unwrap();
        assert_eq!(&ico[0..4], &[0, 0, 1, 0]);
        assert!(
            amber
                .pixels()
                .zip(sky.pixels())
                .any(|(left, right)| left.0 != right.0)
        );
    }

    #[test]
    fn unknown_preset_is_rejected() {
        assert!(render("nope").is_err());
    }
}
