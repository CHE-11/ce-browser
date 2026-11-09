//! Strokes burned into each video frame. Coordinates are fractions of the frame, so a stroke
//! stays put when a capture larger than 2560 on its long side is scaled down.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stroke {
    pub tool: String,
    pub color: String,
    pub width: f32,
    pub points: Vec<StrokePoint>,
    #[serde(default)]
    pub draft: bool,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrokePoint {
    pub x: f32,
    pub y: f32,
}

pub fn sanitize(mut stroke: Stroke) -> Option<Stroke> {
    stroke.tool = match stroke.tool.as_str() {
        "arrow" => "arrow".into(),
        "rectangle" => "rectangle".into(),
        "highlighter" => "highlighter".into(),
        "eraser" => "eraser".into(),
        _ => "pen".into(),
    };
    stroke.width = stroke.width.clamp(0.001, 0.08);
    if stroke.color.len() != 7 || !stroke.color.starts_with('#') {
        stroke.color = "#ef4444".into();
    }
    stroke.points.truncate(4_000);
    stroke.points.retain(|point| point.x.is_finite() && point.y.is_finite());
    if stroke.points.is_empty() {
        None
    } else {
        Some(stroke)
    }
}

/// A ring drawn in place of the system pointer.
pub fn cursor_mark(frame: &mut [u8], width: u32, height: u32, cx: i32, cy: i32) {
    if width == 0 || height == 0 || frame.len() < (width as usize) * (height as usize) * 4 {
        return;
    }
    let outer = 18i32;
    let inner = 13i32;
    let y0 = (cy - outer).max(0);
    let y1 = (cy + outer).min(height as i32 - 1);
    let x0 = (cx - outer).max(0);
    let x1 = (cx + outer).min(width as i32 - 1);
    let ring = [255, 255, 255, 230];
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x - cx;
            let dy = y - cy;
            let distance = dx * dx + dy * dy;
            if distance > outer * outer || distance < inner * inner {
                continue;
            }
            let index = ((y as u32 * width + x as u32) * 4) as usize;
            blend(&mut frame[index..index + 4], ring);
        }
    }
    stamp(frame, width, height, cx, cy, 3, [20, 20, 20, 230]);
}

pub fn touches(stroke: &Stroke, point: StrokePoint, radius: f32) -> bool {
    let radius = radius.max(stroke.width * 0.5).max(0.008);
    let edge = |a: StrokePoint, b: StrokePoint| near_segment(point, a, b, radius);
    match stroke.tool.as_str() {
        "rectangle" => {
            let Some(first) = stroke.points.first().copied() else {
                return false;
            };
            let Some(last) = stroke.points.last().copied() else {
                return false;
            };
            edge(first, StrokePoint { x: last.x, y: first.y })
                || edge(StrokePoint { x: last.x, y: first.y }, last)
                || edge(last, StrokePoint { x: first.x, y: last.y })
                || edge(StrokePoint { x: first.x, y: last.y }, first)
        }
        "arrow" => {
            let (Some(first), Some(last)) = (stroke.points.first().copied(), stroke.points.last().copied()) else {
                return false;
            };
            edge(first, last)
        }
        _ => stroke.points.windows(2).any(|pair| edge(pair[0], pair[1])) || stroke.points.iter().any(|sample| near_segment(point, *sample, *sample, radius)),
    }
}

fn near_segment(point: StrokePoint, start: StrokePoint, end: StrokePoint, radius: f32) -> bool {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len2 = dx * dx + dy * dy;
    let t = if len2 <= f32::EPSILON {
        0.0
    } else {
        ((point.x - start.x) * dx + (point.y - start.y) * dy) / len2
    };
    let t = t.clamp(0.0, 1.0);
    let px = start.x + dx * t - point.x;
    let py = start.y + dy * t - point.y;
    px * px + py * py <= radius * radius
}

/// Paints `strokes` onto a tightly packed top-down BGRA frame.
pub fn paint(frame: &mut [u8], width: u32, height: u32, strokes: &[Stroke]) {
    if width == 0 || height == 0 || frame.len() < (width as usize) * (height as usize) * 4 {
        return;
    }
    for stroke in strokes {
        let alpha = if stroke.tool == "highlighter" { 110 } else { 255 };
        let color = color_of(&stroke.color, alpha);
        let radius = ((stroke.width * width as f32).clamp(2.0, 72.0) / 2.0).round() as i32;
        let radius = if stroke.tool == "highlighter" { radius.max(6) } else { radius.max(1) };
        match stroke.tool.as_str() {
            "eraser" => {}
            "rectangle" => {
                if let (Some(first), Some(last)) = (stroke.points.first(), stroke.points.last()) {
                    let x0 = px(first.x, width);
                    let y0 = px(first.y, height);
                    let x1 = px(last.x, width);
                    let y1 = px(last.y, height);
                    line(frame, width, height, x0, y0, x1, y0, radius, color);
                    line(frame, width, height, x1, y0, x1, y1, radius, color);
                    line(frame, width, height, x1, y1, x0, y1, radius, color);
                    line(frame, width, height, x0, y1, x0, y0, radius, color);
                }
            }
            "arrow" => {
                if let (Some(first), Some(last)) = (stroke.points.first(), stroke.points.last()) {
                    let x0 = px(first.x, width);
                    let y0 = px(first.y, height);
                    let x1 = px(last.x, width);
                    let y1 = px(last.y, height);
                    line(frame, width, height, x0, y0, x1, y1, radius, color);
                    arrow_head(frame, width, height, x0, y0, x1, y1, radius, color);
                }
            }
            _ => {
                if stroke.points.len() == 1 {
                    let point = stroke.points[0];
                    stamp(frame, width, height, px(point.x, width), px(point.y, height), radius, color);
                }
                for pair in stroke.points.windows(2) {
                    line(
                        frame,
                        width,
                        height,
                        px(pair[0].x, width),
                        px(pair[0].y, height),
                        px(pair[1].x, width),
                        px(pair[1].y, height),
                        radius,
                        color,
                    );
                }
            }
        }
    }
}

fn px(value: f32, span: u32) -> i32 {
    (value.clamp(0.0, 1.0) * span.saturating_sub(1) as f32).round() as i32
}

fn color_of(text: &str, alpha: u8) -> [u8; 4] {
    let hex = text.trim().trim_start_matches('#');
    let channel = |index: usize| u8::from_str_radix(hex.get(index..index + 2).unwrap_or("00"), 16).unwrap_or(0);
    if hex.len() == 6 {
        [channel(4), channel(2), channel(0), alpha]
    } else {
        [0x85, 0x71, 0xfb, alpha]
    }
}

fn blend(pixel: &mut [u8], color: [u8; 4]) {
    let alpha = u32::from(color[3]);
    if alpha >= 255 {
        pixel[..3].copy_from_slice(&color[..3]);
        pixel[3] = 255;
        return;
    }
    if alpha == 0 {
        return;
    }
    for channel in 0..3 {
        let mixed = color[channel] as u32 * alpha + pixel[channel] as u32 * (255 - alpha);
        pixel[channel] = (mixed / 255) as u8;
    }
    pixel[3] = 255;
}

fn stamp(frame: &mut [u8], width: u32, height: u32, cx: i32, cy: i32, radius: i32, color: [u8; 4]) {
    let radius = radius.max(1);
    let reach = radius * radius;
    let y0 = (cy - radius).max(0);
    let y1 = (cy + radius).min(height as i32 - 1);
    let x0 = (cx - radius).max(0);
    let x1 = (cx + radius).min(width as i32 - 1);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy > reach {
                continue;
            }
            let index = ((y as u32 * width + x as u32) * 4) as usize;
            blend(&mut frame[index..index + 4], color);
        }
    }
}

fn line(frame: &mut [u8], width: u32, height: u32, x0: i32, y0: i32, x1: i32, y1: i32, radius: i32, color: [u8; 4]) {
    let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = (x0 as f32 + (x1 - x0) as f32 * t).round() as i32;
        let y = (y0 as f32 + (y1 - y0) as f32 * t).round() as i32;
        stamp(frame, width, height, x, y, radius, color);
    }
}

fn arrow_head(frame: &mut [u8], width: u32, height: u32, x0: i32, y0: i32, x1: i32, y1: i32, radius: i32, color: [u8; 4]) {
    let angle = ((y1 - y0) as f32).atan2((x1 - x0) as f32);
    let length = (radius * 6).max(12) as f32;
    for turn in [-0.45_f32, 0.45] {
        let head = angle + std::f32::consts::PI + turn;
        let x2 = x1 + (head.cos() * length).round() as i32;
        let y2 = y1 + (head.sin() * length).round() as i32;
        line(frame, width, height, x1, y1, x2, y2, radius, color);
    }
}

#[cfg(test)]
mod tests {
    use super::{Stroke, StrokePoint, paint};

    #[test]
    fn a_pen_stroke_marks_the_frame() {
        let mut frame = vec![0u8; 20 * 20 * 4];
        paint(
            &mut frame,
            20,
            20,
            &[Stroke {
                tool: "pen".into(),
                color: "#ff0000".into(),
                width: 0.25,
                points: vec![
                    StrokePoint { x: 0.1, y: 0.5 },
                    StrokePoint { x: 0.9, y: 0.5 },
                ],
                draft: false,
            }],
        );
        let mid = (10 * 20 + 10) * 4;
        assert_eq!(&frame[mid..mid + 3], &[0, 0, 255]);
    }
}
