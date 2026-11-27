//! User attachments for agent chat: images and text files.

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::ai::ChatPart;
use crate::automation::{clip, driver, jpeg_data_url};

const MAX_ATTACHMENTS: usize = 6;
const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_TEXT_CHARS: usize = 80_000;
const MODEL_IMAGE_WIDTH: u32 = 1280;
const THUMB_WIDTH: u32 = 480;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentAttachment {
    pub id: String,
    /// `image` or `text`.
    pub kind: String,
    pub name: String,
    pub media_type: String,
    /// Image: base64 without a `data:` prefix. Text: UTF-8 body.
    pub data: String,
}

#[derive(Debug)]
pub struct PreparedAttachments {
    pub parts: Vec<ChatPart>,
    pub detail: String,
    pub thumb: Option<String>,
    pub image_count: usize,
    pub text_count: usize,
}

fn image_mime(media_type: &str) -> bool {
    matches!(
        media_type.to_ascii_lowercase().as_str(),
        "image/png" | "image/jpeg" | "image/jpg" | "image/webp" | "image/gif"
    )
}

fn text_mime(media_type: &str, name: &str) -> bool {
    let mime = media_type.to_ascii_lowercase();
    if mime.starts_with("text/")
        || matches!(
            mime.as_str(),
            "application/json" | "application/javascript" | "application/typescript"
        )
    {
        return true;
    }
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".txt")
        || lower.ends_with(".md")
        || lower.ends_with(".json")
        || lower.ends_with(".csv")
        || lower.ends_with(".log")
        || lower.ends_with(".html")
        || lower.ends_with(".htm")
        || lower.ends_with(".css")
        || lower.ends_with(".js")
        || lower.ends_with(".ts")
        || lower.ends_with(".tsx")
        || lower.ends_with(".jsx")
}

fn strip_data_url(data: &str) -> &str {
    data.find(',')
        .map(|index| &data[index + 1..])
        .unwrap_or(data)
        .trim()
}

fn decode_image(data: &str) -> Result<Vec<u8>, String> {
    let raw = strip_data_url(data);
    base64::engine::general_purpose::STANDARD
        .decode(raw)
        .map_err(|_| "That image could not be read.".into())
}

fn compress_image(bytes: &[u8]) -> Result<(String, String), String> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("That image is too large (max 8 MB).".into());
    }
    let model = driver::shrink_jpeg(bytes, MODEL_IMAGE_WIDTH, 70)?;
    let thumb = driver::shrink_jpeg(&model.bytes, THUMB_WIDTH, 60)?;
    Ok((
        base64::engine::general_purpose::STANDARD.encode(&model.bytes),
        jpeg_data_url(&thumb.bytes),
    ))
}

/// Validates and compresses attachments for the model and the chat UI.
pub fn prepare(attachments: &[AgentAttachment]) -> Result<PreparedAttachments, String> {
    if attachments.is_empty() {
        return Ok(PreparedAttachments {
            parts: Vec::new(),
            detail: String::new(),
            thumb: None,
            image_count: 0,
            text_count: 0,
        });
    }
    if attachments.len() > MAX_ATTACHMENTS {
        return Err(format!("Attach at most {MAX_ATTACHMENTS} files."));
    }
    let mut parts = Vec::new();
    let mut names = Vec::new();
    let mut thumb = None;
    let mut image_count = 0usize;
    let mut text_count = 0usize;

    for item in attachments {
        let name = clip(item.name.trim(), 120);
        let label = if name.is_empty() {
            "attachment".into()
        } else {
            name.clone()
        };
        let kind = item.kind.trim().to_ascii_lowercase();
        let media = item.media_type.trim().to_ascii_lowercase();

        if kind == "image" || image_mime(&media) {
            let bytes = decode_image(&item.data)?;
            let (data, preview) = compress_image(&bytes)?;
            if thumb.is_none() {
                thumb = Some(preview);
            }
            parts.push(ChatPart::Image {
                media_type: "image/jpeg".into(),
                data,
            });
            names.push(format!("image:{label}"));
            image_count += 1;
            continue;
        }

        if kind == "text" || text_mime(&media, &label) {
            let body = item.data.trim();
            if body.is_empty() {
                continue;
            }
            let clipped = clip(body, MAX_TEXT_CHARS);
            parts.push(ChatPart::Text(format!("Attached file `{label}`:\n\n{clipped}")));
            names.push(format!("file:{label}"));
            text_count += 1;
            continue;
        }

        return Err(format!(
            "Unsupported attachment `{label}`. Use an image or a text file."
        ));
    }

    Ok(PreparedAttachments {
        parts,
        detail: names.join("\n"),
        thumb,
        image_count,
        text_count,
    })
}

/// Label appended to a user chat step when files were attached.
pub fn step_suffix(prepared: &PreparedAttachments) -> String {
    let mut bits = Vec::new();
    if prepared.image_count > 0 {
        bits.push(format!(
            "{} {}",
            prepared.image_count,
            if prepared.image_count == 1 {
                "image"
            } else {
                "images"
            }
        ));
    }
    if prepared.text_count > 0 {
        bits.push(format!(
            "{} {}",
            prepared.text_count,
            if prepared.text_count == 1 {
                "file"
            } else {
                "files"
            }
        ));
    }
    if bits.is_empty() {
        String::new()
    } else {
        format!("({})", bits.join(", "))
    }
}

/// Reads a local file into an attachment the panel can send.
pub fn read_file(path: &str) -> Result<AgentAttachment, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("No file path.".into());
    }
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file")
        .to_string();
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("That file is too large (max 8 MB).".into());
    }
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".webp")
        || lower.ends_with(".gif")
    {
        let media_type = if lower.ends_with(".png") {
            "image/png"
        } else if lower.ends_with(".webp") {
            "image/webp"
        } else if lower.ends_with(".gif") {
            "image/gif"
        } else {
            "image/jpeg"
        };
        return Ok(AgentAttachment {
            id: format!("file-{}", now_stamp()),
            kind: "image".into(),
            name,
            media_type: media_type.into(),
            data: base64::engine::general_purpose::STANDARD.encode(&bytes),
        });
    }
    if text_mime("", &name) {
        let text = String::from_utf8(bytes)
            .map_err(|_| "That text file is not valid UTF-8.".to_string())?;
        return Ok(AgentAttachment {
            id: format!("file-{}", now_stamp()),
            kind: "text".into(),
            name,
            media_type: "text/plain".into(),
            data: text,
        });
    }
    Err("Use an image or a text file.".into())
}

fn now_stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_empty() {
        let prepared = prepare(&[]).unwrap();
        assert!(prepared.parts.is_empty());
        assert!(prepared.detail.is_empty());
        assert!(prepared.thumb.is_none());
    }

    #[test]
    fn prepare_text_file() {
        let prepared = prepare(&[AgentAttachment {
            id: "1".into(),
            kind: "text".into(),
            name: "notes.md".into(),
            media_type: "text/markdown".into(),
            data: "hello world".into(),
        }])
        .unwrap();
        assert_eq!(prepared.text_count, 1);
        assert_eq!(prepared.image_count, 0);
        assert!(prepared.thumb.is_none());
        assert!(matches!(&prepared.parts[0], ChatPart::Text(text) if text.contains("notes.md") && text.contains("hello world")));
        assert!(step_suffix(&prepared).contains("1 file"));
    }

    #[test]
    fn prepare_rejects_unknown() {
        let err = prepare(&[AgentAttachment {
            id: "1".into(),
            kind: "pdf".into(),
            name: "doc.pdf".into(),
            media_type: "application/pdf".into(),
            data: "x".into(),
        }])
        .unwrap_err();
        assert!(err.contains("Unsupported"));
    }

    #[test]
    fn prepare_rejects_empty_batch_over_cap() {
        let many: Vec<_> = (0..7)
            .map(|i| AgentAttachment {
                id: format!("{i}"),
                kind: "text".into(),
                name: format!("f{i}.txt"),
                media_type: "text/plain".into(),
                data: "a".into(),
            })
            .collect();
        assert!(prepare(&many).unwrap_err().contains("at most"));
    }
}
