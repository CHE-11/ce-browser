//! A site color for the tab and address bar: the page's theme color, or its favicon.

pub fn parse_color(input: &str) -> Option<String> {
    let text = input.trim();
    if let Some(hex) = text.strip_prefix('#') {
        return match hex.len() {
            3 | 4 => {
                let chars: Vec<char> = hex.chars().collect();
                if !chars.iter().all(|character| character.is_ascii_hexdigit()) {
                    return None;
                }
                let expand = |character: char| {
                    let digit = character.to_digit(16)? as u8;
                    Some(digit * 16 + digit)
                };
                Some(format!(
                    "#{:02x}{:02x}{:02x}",
                    expand(chars[0])?,
                    expand(chars[1])?,
                    expand(chars[2])?
                ))
            }
            6 | 8 => {
                if !hex.chars().all(|character| character.is_ascii_hexdigit()) {
                    return None;
                }
                Some(format!("#{}", &hex[..6].to_ascii_lowercase()))
            }
            _ => None,
        };
    }
    let lower = text.to_ascii_lowercase();
    let body = lower
        .strip_prefix("rgba(")
        .or_else(|| lower.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let parts: Vec<&str> = body.split(',').map(str::trim).collect();
    if parts.len() < 3 {
        return None;
    }
    let channel = |part: &str| {
        part.parse::<u16>()
            .ok()
            .filter(|value| *value <= 255)
            .map(|value| value as u8)
    };
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        channel(parts[0])?,
        channel(parts[1])?,
        channel(parts[2])?
    ))
}

pub fn average_png(bytes: &[u8]) -> Option<String> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let mut red = 0u64;
    let mut green = 0u64;
    let mut blue = 0u64;
    let mut count = 0u64;
    for pixel in image.pixels() {
        if pixel[3] < 128 {
            continue;
        }
        red += u64::from(pixel[0]);
        green += u64::from(pixel[1]);
        blue += u64::from(pixel[2]);
        count += 1;
    }
    if count == 0 {
        return None;
    }
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        red / count,
        green / count,
        blue / count
    ))
}

#[cfg(test)]
mod tests {
    use super::parse_color;

    #[test]
    fn a_theme_color_that_is_not_a_color_is_ignored() {
        assert_eq!(parse_color("javascript:alert(1)"), None);
        assert_eq!(parse_color("red"), None);
        assert_eq!(parse_color("#ff8800"), Some("#ff8800".into()));
        assert_eq!(parse_color("rgb(10, 20, 30)"), Some("#0a141e".into()));
    }
}
