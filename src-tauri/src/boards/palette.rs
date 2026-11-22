//! Main colors of an image, found by median cut on a sample of its pixels.

use image::RgbaImage;

use super::model::PaletteColor;

const MAX_COLORS: usize = 8;
const MAX_SAMPLES: usize = 12_000;
/// Colors closer than this (squared RGB distance) are merged.
const MERGE_DISTANCE: u32 = 28 * 28;
/// Colors covering less than this share of the image are left out.
const MIN_RATIO: f32 = 0.01;

pub fn palette(image: &RgbaImage) -> Vec<PaletteColor> {
    let (width, height) = image.dimensions();
    let total = (width as usize) * (height as usize);
    if total == 0 {
        return Vec::new();
    }
    let step = (total / MAX_SAMPLES).max(1);
    let mut pixels: Vec<[u8; 3]> = image
        .pixels()
        .step_by(step)
        .filter(|pixel| pixel[3] >= 128)
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect();
    if pixels.is_empty() {
        return Vec::new();
    }
    let sampled = pixels.len();

    let mut boxes: Vec<&mut [[u8; 3]]> = vec![pixels.as_mut_slice()];
    while boxes.len() < MAX_COLORS {
        let Some((index, channel)) = boxes
            .iter()
            .enumerate()
            .filter(|(_, items)| items.len() > 1)
            .map(|(index, items)| {
                let (channel, range) = widest_channel(items);
                (index, channel, range, items.len())
            })
            .filter(|(_, _, range, _)| *range > 0)
            .max_by_key(|(_, _, range, count)| u64::from(*range) * (*count as u64))
            .map(|(index, channel, _, _)| (index, channel))
        else {
            break;
        };
        let items = boxes.swap_remove(index);
        items.sort_unstable_by_key(|pixel| pixel[channel]);
        let middle = items.len() / 2;
        let (low, high) = items.split_at_mut(middle);
        boxes.push(low);
        boxes.push(high);
    }

    let mut colors: Vec<([u32; 3], usize)> = boxes
        .iter()
        .filter(|items| !items.is_empty())
        .map(|items| {
            let mut sum = [0u64; 3];
            for pixel in items.iter() {
                for channel in 0..3 {
                    sum[channel] += u64::from(pixel[channel]);
                }
            }
            let count = items.len() as u64;
            (
                [
                    (sum[0] / count) as u32,
                    (sum[1] / count) as u32,
                    (sum[2] / count) as u32,
                ],
                items.len(),
            )
        })
        .collect();
    colors.sort_by(|left, right| right.1.cmp(&left.1));

    let mut merged: Vec<([u32; 3], usize)> = Vec::new();
    for (color, count) in colors {
        if let Some(existing) = merged
            .iter_mut()
            .find(|(other, _)| distance(*other, color) < MERGE_DISTANCE)
        {
            let total = existing.1 + count;
            for channel in 0..3 {
                existing.0[channel] = ((existing.0[channel] as usize * existing.1
                    + color[channel] as usize * count)
                    / total) as u32;
            }
            existing.1 = total;
        } else {
            merged.push((color, count));
        }
    }
    merged.sort_by(|left, right| right.1.cmp(&left.1));
    merged
        .into_iter()
        .map(|(color, count)| PaletteColor {
            color: format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2]),
            ratio: ((count as f32 / sampled as f32) * 1000.0).round() / 1000.0,
        })
        .filter(|color| color.ratio >= MIN_RATIO)
        .collect()
}

fn widest_channel(items: &[[u8; 3]]) -> (usize, u8) {
    let mut low = [u8::MAX; 3];
    let mut high = [0u8; 3];
    for pixel in items {
        for channel in 0..3 {
            low[channel] = low[channel].min(pixel[channel]);
            high[channel] = high[channel].max(pixel[channel]);
        }
    }
    (0..3)
        .map(|channel| (channel, high[channel] - low[channel]))
        .max_by_key(|(_, range)| *range)
        .unwrap_or((0, 0))
}

fn distance(left: [u32; 3], right: [u32; 3]) -> u32 {
    (0..3)
        .map(|channel| {
            let delta = left[channel].abs_diff(right[channel]);
            delta * delta
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn a_two_color_image_gives_both_colors_by_share() {
        let image = RgbaImage::from_fn(100, 100, |x, _| {
            if x < 75 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 255])
            }
        });
        let colors = palette(&image);
        assert_eq!(colors.len(), 2);
        assert_eq!(colors[0].color, "#ff0000");
        assert_eq!(colors[1].color, "#0000ff");
        assert!((colors[0].ratio - 0.75).abs() < 0.02);
    }

    #[test]
    fn transparent_images_have_no_palette() {
        let image = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
        assert!(palette(&image).is_empty());
    }
}
