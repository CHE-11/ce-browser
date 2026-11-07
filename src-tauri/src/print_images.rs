//! Copies remote images into the print HTML as data URLs.
//!
//! The print host loads the document from a temporary file. A later viewer, and that host
//! itself, would otherwise have to fetch every picture again. Hotlink checks reject those
//! requests, so the pictures are saved into the file while the original tab is still open.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use base64::Engine;
use serde_json::Value;
use tauri::{AppHandle, Manager, Webview};
use url::Url;

use crate::{tabs, webview2};

const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 32 * 1024 * 1024;
const MAX_IMAGES: usize = 120;
const MAX_STYLESHEETS: usize = 12;
const MAX_STYLESHEET_BYTES: u64 = 1024 * 1024;
const WORKERS: usize = 4;
const DEFAULT_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

struct Attr {
    name: String,
    /// Decoded value. `None` is a boolean attribute.
    value: Option<String>,
    /// Raw value bytes in the source, excluding quotes.
    span: Option<(usize, usize)>,
}

struct Tag {
    name: String,
    start: usize,
    end: usize,
    attrs: Vec<Attr>,
}

enum Event {
    Open(Tag),
    Close(String),
    Style {
        content_start: usize,
        content_end: usize,
    },
}

enum CssToken {
    Url {
        start: usize,
        end: usize,
        resolved: String,
    },
    Import {
        start: usize,
        end: usize,
        url: String,
    },
}

struct CssRefs {
    images: Vec<String>,
    imports: Vec<String>,
}

struct CachedResource {
    frame_id: String,
    url: String,
    mime: String,
    kind: String,
}

struct Choice {
    url: String,
    width: f64,
    placeholder: bool,
}

/// Rewrites `html` so its images travel with the document.
pub fn embed(app: &AppHandle, html: &str, page_url: &str, tab_id: Option<u32>) -> String {
    let Some(base) = document_base(html, page_url) else {
        return html.to_string();
    };
    let (image_urls, sheet_urls) = collect(html, &base);
    if image_urls.is_empty() && sheet_urls.is_empty() {
        return html.to_string();
    }

    let source = tab_id.and_then(|id| tabs::page_view(app, id));
    let cache = source.as_ref().map(index_resources).unwrap_or_default();
    let referer = referer_for(page_url, &base);
    let sheets = gather_sheets(app, source.as_ref(), &cache, &sheet_urls, &referer);

    let mut urls = image_urls;
    for (url, css) in &sheets {
        let Ok(sheet_base) = Url::parse(url) else {
            continue;
        };
        urls.extend(css_refs(css, &sheet_base).images);
    }
    let urls = dedup_cap(urls, MAX_IMAGES);
    let images = fetch_images(app, source.as_ref(), &cache, &urls, &referer);
    if images.is_empty() && sheets.is_empty() {
        return html.to_string();
    }

    let rewritten = apply(html, &base, &images, &sheets);
    if !images.is_empty() || !sheets.is_empty() {
        eprintln!(
            "print: embedded {} {}, inlined {} {}",
            images.len(),
            if images.len() == 1 { "image" } else { "images" },
            sheets.len(),
            if sheets.len() == 1 {
                "stylesheet"
            } else {
                "stylesheets"
            },
        );
    }
    rewritten
}

fn document_base(html: &str, page_url: &str) -> Option<Url> {
    if let Some(href) = base_href(html) {
        if let Ok(url) = Url::parse(&href) {
            if matches!(url.scheme(), "http" | "https" | "file") {
                return Some(url);
            }
        }
    }
    Url::parse(page_url)
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https" | "file"))
}

fn base_href(html: &str) -> Option<String> {
    for event in scan_events(html) {
        let Event::Open(tag) = event else {
            continue;
        };
        if tag.name == "base" {
            return attr(&tag, "href").map(str::to_string);
        }
    }
    None
}

fn referer_for(page_url: &str, base: &Url) -> String {
    if let Ok(url) = Url::parse(page_url) {
        if matches!(url.scheme(), "http" | "https") {
            return url.to_string();
        }
    }
    if matches!(base.scheme(), "http" | "https") {
        base.to_string()
    } else {
        String::new()
    }
}

fn collect(html: &str, base: &Url) -> (Vec<String>, Vec<String>) {
    let mut images = Vec::new();
    let mut sheets = Vec::new();
    let mut in_picture = 0i32;
    for event in scan_events(html) {
        match event {
            Event::Open(tag) if tag.name == "picture" => {
                let close_at = matching_close(html, tag.end, "picture");
                if let Some(url) = picture_choice(&html[tag.end..close_at], base) {
                    images.push(url);
                }
                in_picture += 1;
            }
            Event::Close(name) if name == "picture" => {
                in_picture = in_picture.saturating_sub(1);
            }
            Event::Open(tag) => {
                collect_tag(&tag, base, in_picture > 0, &mut images, &mut sheets);
            }
            Event::Style {
                content_start,
                content_end,
            } => {
                let refs = css_refs(&html[content_start..content_end], base);
                images.extend(refs.images);
                sheets.extend(refs.imports);
            }
            Event::Close(_) => {}
        }
    }
    (images, sheets)
}

fn collect_tag(
    tag: &Tag,
    base: &Url,
    in_picture: bool,
    images: &mut Vec<String>,
    sheets: &mut Vec<String>,
) {
    if !in_picture && (tag.name == "img" || (tag.name == "input" && input_is_image(tag))) {
        if let Some(url) = chosen_image_url(attr(tag, "src"), attr(tag, "srcset"), base) {
            images.push(url);
        }
    }
    if tag.name == "image" {
        for name in ["href", "xlink:href", "src"] {
            if let Some(url) = attr(tag, name).and_then(|value| resolve(value, base)) {
                if !skip_image_url(&url) {
                    images.push(url);
                    break;
                }
            }
        }
    }
    if tag.name == "video" {
        if let Some(url) = attr(tag, "poster").and_then(|value| resolve(value, base)) {
            if !skip_image_url(&url) {
                images.push(url);
            }
        }
    }
    if tag.name == "link" && active_stylesheet(tag) {
        if let Some(url) = attr(tag, "href").and_then(|value| resolve(value, base)) {
            sheets.push(url);
        }
    }
    if let Some(style) = attr(tag, "style") {
        let refs = css_refs(style, base);
        images.extend(refs.images);
        sheets.extend(refs.imports);
    }
}

fn apply(
    html: &str,
    base: &Url,
    images: &HashMap<String, String>,
    sheets: &HashMap<String, String>,
) -> String {
    let mut edits = Vec::new();
    let mut locked: Vec<Option<String>> = Vec::new();
    for event in scan_events(html) {
        match event {
            Event::Open(tag) if tag.name == "picture" => {
                let close_at = matching_close(html, tag.end, "picture");
                let data = picture_choice(&html[tag.end..close_at], base)
                    .and_then(|url| images.get(&url).cloned());
                locked.push(data);
            }
            Event::Close(name) if name == "picture" => {
                locked.pop();
            }
            Event::Open(tag) => {
                let forced = locked.last().and_then(|value| value.clone());
                if let Some(data) = forced.as_deref() {
                    if tag.name == "img" {
                        edits.push((
                            tag.start,
                            tag.end,
                            force_img(&tag, data, base, images, sheets),
                        ));
                        continue;
                    }
                    if tag.name == "source" {
                        edits.push((tag.start, tag.end, String::new()));
                        continue;
                    }
                }
                if let Some(replacement) = replace_tag(&tag, base, images, sheets) {
                    edits.push((tag.start, tag.end, replacement));
                } else {
                    edits.extend(style_edits(&tag, base, images, sheets));
                }
            }
            Event::Style {
                content_start,
                content_end,
            } => {
                let css = &html[content_start..content_end];
                let prepared = prepare_css(css, base, sheets, images, &mut HashSet::new(), false);
                if prepared != css {
                    edits.push((content_start, content_end, escape_style(&prepared)));
                }
            }
            Event::Close(_) => {}
        }
    }
    stitch(html, edits)
}

fn replace_tag(
    tag: &Tag,
    base: &Url,
    images: &HashMap<String, String>,
    sheets: &HashMap<String, String>,
) -> Option<String> {
    match tag.name.as_str() {
        "img" => rewrite_img(tag, base, images, sheets),
        "input" if input_is_image(tag) => rewrite_img(tag, base, images, sheets),
        "image" => rewrite_svg_image(tag, base, images, sheets),
        "video" => rewrite_poster(tag, base, images, sheets),
        "link" => rewrite_link(tag, base, images, sheets),
        _ => None,
    }
}

fn rewrite_img(
    tag: &Tag,
    base: &Url,
    images: &HashMap<String, String>,
    sheets: &HashMap<String, String>,
) -> Option<String> {
    let src = attr(tag, "src");
    let embedded =
        chosen_image_url(src, attr(tag, "srcset"), base).and_then(|url| images.get(&url).cloned());
    let keep_data = embedded.is_none() && src.is_some_and(src_is_embedded);
    if embedded.is_none() && !(keep_data && attr(tag, "srcset").is_some()) {
        return None;
    }
    Some(render_img(tag, embedded.as_deref(), base, images, sheets))
}

fn force_img(
    tag: &Tag,
    data: &str,
    base: &Url,
    images: &HashMap<String, String>,
    sheets: &HashMap<String, String>,
) -> String {
    render_img(tag, Some(data), base, images, sheets)
}

fn render_img(
    tag: &Tag,
    embedded: Option<&str>,
    base: &Url,
    images: &HashMap<String, String>,
    sheets: &HashMap<String, String>,
) -> String {
    let mut attrs = Vec::new();
    let mut wrote_src = false;
    for attribute in &tag.attrs {
        match attribute.name.as_str() {
            "srcset" | "sizes" => continue,
            "src" => {
                wrote_src = true;
                let value = embedded
                    .map(str::to_string)
                    .or_else(|| attribute.value.clone())
                    .unwrap_or_default();
                attrs.push(("src".to_string(), Some(value)));
            }
            "style" => attrs.push((
                "style".to_string(),
                Some(prepared_style(attribute, base, images, sheets)),
            )),
            _ => attrs.push((attribute.name.clone(), attribute.value.clone())),
        }
    }
    if !wrote_src {
        if let Some(value) = embedded {
            attrs.insert(0, ("src".to_string(), Some(value.to_string())));
        }
    }
    render_tag(&tag.name, &attrs)
}

fn rewrite_svg_image(
    tag: &Tag,
    base: &Url,
    images: &HashMap<String, String>,
    sheets: &HashMap<String, String>,
) -> Option<String> {
    let mut replaced = false;
    let mut attrs = Vec::new();
    for attribute in &tag.attrs {
        if matches!(attribute.name.as_str(), "href" | "xlink:href" | "src") {
            if let Some(url) = attribute
                .value
                .as_deref()
                .and_then(|value| resolve(value, base))
            {
                if let Some(data) = images.get(&url) {
                    attrs.push((attribute.name.clone(), Some(data.clone())));
                    replaced = true;
                    continue;
                }
            }
        }
        if attribute.name == "style" {
            attrs.push((
                "style".to_string(),
                Some(prepared_style(attribute, base, images, sheets)),
            ));
            continue;
        }
        attrs.push((attribute.name.clone(), attribute.value.clone()));
    }
    replaced.then(|| render_tag("image", &attrs))
}

fn rewrite_poster(
    tag: &Tag,
    base: &Url,
    images: &HashMap<String, String>,
    sheets: &HashMap<String, String>,
) -> Option<String> {
    let data = attr(tag, "poster")
        .and_then(|value| resolve(value, base))
        .and_then(|url| images.get(&url).cloned());
    if data.is_none() {
        return None;
    }
    let mut attrs = Vec::new();
    for attribute in &tag.attrs {
        if attribute.name == "poster" {
            attrs.push(("poster".to_string(), data.clone()));
        } else if attribute.name == "style" {
            attrs.push((
                "style".to_string(),
                Some(prepared_style(attribute, base, images, sheets)),
            ));
        } else {
            attrs.push((attribute.name.clone(), attribute.value.clone()));
        }
    }
    Some(render_tag("video", &attrs))
}

fn rewrite_link(
    tag: &Tag,
    base: &Url,
    images: &HashMap<String, String>,
    sheets: &HashMap<String, String>,
) -> Option<String> {
    if !active_stylesheet(tag) {
        return None;
    }
    let href = attr(tag, "href").and_then(|value| resolve(value, base))?;
    let css = sheets.get(&href)?;
    let sheet_base = Url::parse(&href).ok()?;
    let prepared = prepare_css(css, &sheet_base, sheets, images, &mut HashSet::new(), true);
    Some(render_style(&prepared, attr(tag, "media"), &href))
}

fn prepared_style(
    attribute: &Attr,
    base: &Url,
    images: &HashMap<String, String>,
    sheets: &HashMap<String, String>,
) -> String {
    prepare_css(
        attribute.value.as_deref().unwrap_or(""),
        base,
        sheets,
        images,
        &mut HashSet::new(),
        false,
    )
}

fn style_edits(
    tag: &Tag,
    base: &Url,
    images: &HashMap<String, String>,
    sheets: &HashMap<String, String>,
) -> Vec<(usize, usize, String)> {
    let mut edits = Vec::new();
    for attribute in &tag.attrs {
        if attribute.name != "style" {
            continue;
        }
        let (Some(value), Some((start, end))) = (attribute.value.as_deref(), attribute.span) else {
            continue;
        };
        let prepared = prepare_css(value, base, sheets, images, &mut HashSet::new(), false);
        if prepared != value {
            edits.push((start, end, escape_attr(&prepared)));
        }
    }
    edits
}

fn active_stylesheet(tag: &Tag) -> bool {
    if tag.name != "link" || tag.attrs.iter().any(|item| item.name == "disabled") {
        return false;
    }
    let Some(rel) = attr(tag, "rel") else {
        return false;
    };
    let words: Vec<String> = rel
        .split_ascii_whitespace()
        .map(|word| word.to_ascii_lowercase())
        .collect();
    words.iter().any(|word| word == "stylesheet") && words.iter().all(|word| word != "alternate")
}

fn input_is_image(tag: &Tag) -> bool {
    attr(tag, "type").is_some_and(|value| value.eq_ignore_ascii_case("image"))
}

fn src_is_embedded(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("data:image/") && value.len() > 64
}

fn chosen_image_url(src: Option<&str>, srcset: Option<&str>, base: &Url) -> Option<String> {
    if let Some(src) = src.map(str::trim).filter(|value| !value.is_empty()) {
        if src_is_embedded(src) {
            return None;
        }
        if let Some(url) = resolve(src, base) {
            if !is_placeholder(&url) && !skip_image_url(&url) {
                return Some(url);
            }
        }
    }
    best_srcset(srcset.unwrap_or(""), base).or_else(|| {
        src.and_then(|value| resolve(value, base))
            .filter(|url| !skip_image_url(url))
    })
}

fn picture_choice(inner: &str, base: &Url) -> Option<String> {
    let mut nested = 0i32;
    let mut fallback: Option<Choice> = None;
    let mut index = 0;
    while index < inner.len() {
        let Some(relative) = inner[index..].find('<') else {
            break;
        };
        index += relative;
        if inner[index..].starts_with("<!--") {
            if let Some(end) = inner[index + 4..].find("-->") {
                index = index + 4 + end + 3;
                continue;
            }
            break;
        }
        if let Some(tag) = parse_tag(inner, index) {
            if tag.name == "picture" {
                nested += 1;
                index = tag.end;
                continue;
            }
            if nested == 0 && tag.name == "img" {
                if let Some(url) = chosen_image_url(attr(&tag, "src"), attr(&tag, "srcset"), base) {
                    if !is_placeholder(&url) {
                        return Some(url);
                    }
                    consider(&mut fallback, url, 0.0);
                }
            }
            if nested == 0 && tag.name == "source" {
                if let Some((url, width)) =
                    best_srcset_scored(attr(&tag, "srcset").unwrap_or(""), base)
                {
                    consider(&mut fallback, url, width);
                }
            }
            index = tag.end;
            continue;
        }
        if inner.as_bytes().get(index + 1) == Some(&b'/') {
            if let Some(end) = find_tag_end(inner, index) {
                if close_name(&inner[index + 1..end]) == "picture" && nested > 0 {
                    nested -= 1;
                }
                index = end + 1;
                continue;
            }
        }
        index += 1;
    }
    fallback.map(|choice| choice.url)
}

fn consider(current: &mut Option<Choice>, url: String, width: f64) {
    let placeholder = is_placeholder(&url);
    let replace = match current {
        None => true,
        Some(choice) if choice.placeholder && !placeholder => true,
        Some(choice) if !choice.placeholder && placeholder => false,
        Some(choice) => width > choice.width,
    };
    if replace {
        *current = Some(Choice {
            url,
            width,
            placeholder,
        });
    }
}

fn best_srcset(value: &str, base: &Url) -> Option<String> {
    best_srcset_scored(value, base).map(|(url, _)| url)
}

fn best_srcset_scored(value: &str, base: &Url) -> Option<(String, f64)> {
    let mut best: Option<(String, f64, f64)> = None;
    for part in value.split(',') {
        let mut bits = part.split_whitespace();
        let Some(raw) = bits.next() else {
            continue;
        };
        let Some(url) = resolve(raw, base) else {
            continue;
        };
        if skip_image_url(&url) {
            continue;
        }
        let (width, density) = bits.next().map(descriptor_score).unwrap_or((0.0, 0.0));
        let replace = match &best {
            None => true,
            Some((_, best_width, best_density)) => {
                width > *best_width || (width == *best_width && density > *best_density)
            }
        };
        if replace {
            best = Some((url, width, density));
        }
    }
    best.map(|(url, width, _)| (url, width))
}

fn descriptor_score(desc: &str) -> (f64, f64) {
    let desc = desc.to_ascii_lowercase();
    if let Some(number) = desc.strip_suffix('w') {
        return (number.parse().unwrap_or(0.0), 0.0);
    }
    if let Some(number) = desc.strip_suffix('x') {
        return (0.0, number.parse().unwrap_or(0.0));
    }
    (0.0, 0.0)
}

fn is_placeholder(url: &str) -> bool {
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .to_ascii_lowercase();
    [
        "placeholder",
        "spacer",
        "1x1",
        "pixel.gif",
        "blank.gif",
        "blank.png",
        "lazyload",
    ]
    .iter()
    .any(|hint| path.contains(hint))
}

fn skip_image_url(url: &str) -> bool {
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .to_ascii_lowercase();
    [
        ".js", ".mjs", ".css", ".html", ".htm", ".json", ".map", ".mp4", ".webm", ".mp3", ".wav",
        ".ogg", ".pdf", ".zip", ".woff", ".woff2", ".ttf", ".otf", ".eot", ".m4a", ".aac",
    ]
    .iter()
    .any(|ext| path.ends_with(ext))
}

fn resolve(value: &str, base: &Url) -> Option<String> {
    let value = decode_entities(value).trim().to_string();
    if value.is_empty() || value.starts_with('#') {
        return None;
    }
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("data:") || lower.starts_with("blob:") || lower.starts_with("javascript:")
    {
        return None;
    }
    let mut url = base.join(&value).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    url.set_fragment(None);
    Some(url.to_string())
}

fn normalize_url(value: &str) -> Option<String> {
    let mut url = Url::parse(value).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    url.set_fragment(None);
    Some(url.to_string())
}

fn css_refs(css: &str, base: &Url) -> CssRefs {
    let mut refs = CssRefs {
        images: Vec::new(),
        imports: Vec::new(),
    };
    for token in scan_css(css, base) {
        match token {
            CssToken::Url { resolved, .. } if !skip_image_url(&resolved) => {
                refs.images.push(resolved)
            }
            CssToken::Import { url, .. } => refs.imports.push(url),
            CssToken::Url { .. } => {}
        }
    }
    refs
}

fn prepare_css(
    css: &str,
    base: &Url,
    sheets: &HashMap<String, String>,
    images: &HashMap<String, String>,
    stack: &mut HashSet<String>,
    absolutize: bool,
) -> String {
    let mut edits = Vec::new();
    for token in scan_css(css, base) {
        match token {
            CssToken::Url {
                start,
                end,
                resolved,
            } => {
                if let Some(data) = images.get(&resolved) {
                    edits.push((start, end, format!("url(\"{data}\")")));
                } else if absolutize {
                    edits.push((start, end, format!("url(\"{resolved}\")")));
                }
            }
            CssToken::Import { start, end, url } => {
                if let Some(child) = sheets.get(&url) {
                    if stack.insert(url.clone()) {
                        let child_base = Url::parse(&url).unwrap_or_else(|_| base.clone());
                        let body = prepare_css(child, &child_base, sheets, images, stack, true);
                        stack.remove(&url);
                        edits.push((start, end, body));
                    } else {
                        edits.push((start, end, String::new()));
                    }
                } else if absolutize {
                    edits.push((start, end, format!("@import url(\"{url}\");")));
                }
            }
        }
    }
    if edits.is_empty() {
        css.to_string()
    } else {
        stitch(css, edits)
    }
}

fn scan_css(css: &str, base: &Url) -> Vec<CssToken> {
    let bytes = css.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
            if let Some(end) = css[index + 2..].find("*/") {
                index = index + 2 + end + 2;
                continue;
            }
        }
        if bytes[index] == b'"' || bytes[index] == b'\'' {
            index = skip_css_string(bytes, index);
            continue;
        }
        if import_at(bytes, index) {
            if let Some((end, url)) = take_import(css, index, base) {
                tokens.push(CssToken::Import {
                    start: index,
                    end,
                    url,
                });
                index = end;
                continue;
            }
        }
        if url_fn_at(bytes, index) {
            if let Some((end, resolved)) = take_url(css, index, base) {
                tokens.push(CssToken::Url {
                    start: index,
                    end,
                    resolved,
                });
                index = end;
                continue;
            }
        }
        index += css[index..].chars().next().map(char::len_utf8).unwrap_or(1);
    }
    tokens
}

fn import_at(bytes: &[u8], index: usize) -> bool {
    let end = index + "@import".len();
    end <= bytes.len()
        && bytes[index..end].eq_ignore_ascii_case(b"@import")
        && (index == 0 || !is_ident_byte(bytes[index - 1]))
        && bytes.get(end).is_some_and(|next| {
            next.is_ascii_whitespace() || matches!(next, b'"' | b'\'' | b'u' | b'U')
        })
}

fn url_fn_at(bytes: &[u8], index: usize) -> bool {
    let end = index + "url(".len();
    end <= bytes.len()
        && bytes[index..end].eq_ignore_ascii_case(b"url(")
        && (index == 0 || !is_ident_byte(bytes[index - 1]))
}

fn take_import(css: &str, start: usize, base: &Url) -> Option<(usize, String)> {
    let bytes = css.as_bytes();
    let index = skip_ws(bytes, start + "@import".len());
    let (index, raw) = take_css_url_value(css, index)?;
    let url = resolve(&raw, base)?;
    let index = skip_ws(bytes, index);
    let mut end = index;
    while end < bytes.len() && bytes[end] != b';' {
        if bytes[end] == b'"' || bytes[end] == b'\'' {
            end = skip_css_string(bytes, end);
            continue;
        }
        end += 1;
    }
    if end >= bytes.len() {
        return None;
    }
    Some((end + 1, url))
}

fn take_url(css: &str, start: usize, base: &Url) -> Option<(usize, String)> {
    let (index, raw) = take_css_url_value(css, start)?;
    let resolved = resolve(&raw, base)?;
    Some((index, resolved))
}

/// Reads a `url(...)` function or a quoted CSS string. Returns the index after it and the text.
fn take_css_url_value(css: &str, start: usize) -> Option<(usize, String)> {
    let bytes = css.as_bytes();
    if url_fn_at(bytes, start) {
        let mut index = skip_ws(bytes, start + "url(".len());
        let raw = if bytes
            .get(index)
            .is_some_and(|byte| *byte == b'"' || *byte == b'\'')
        {
            let value_start = index + 1;
            index = skip_quoted(bytes, index)?;
            if css[value_start..index.saturating_sub(1)].contains('\\') {
                return None;
            }
            css[value_start..index - 1].trim().to_string()
        } else {
            let value_start = index;
            while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b')'
            {
                index += 1;
            }
            if css[value_start..index].contains('\\') {
                return None;
            }
            css[value_start..index].trim().to_string()
        };
        index = skip_ws(bytes, index);
        if bytes.get(index) != Some(&b')') {
            return None;
        }
        return Some((index + 1, raw));
    }
    if bytes
        .get(start)
        .is_some_and(|byte| *byte == b'"' || *byte == b'\'')
    {
        let value_start = start + 1;
        let end = skip_quoted(bytes, start)?;
        if css[value_start..end.saturating_sub(1)].contains('\\') {
            return None;
        }
        return Some((end, css[value_start..end - 1].trim().to_string()));
    }
    None
}

fn skip_css_string(bytes: &[u8], start: usize) -> usize {
    skip_quoted(bytes, start).unwrap_or(bytes.len())
}

fn skip_quoted(bytes: &[u8], start: usize) -> Option<usize> {
    let quote = *bytes.get(start)?;
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 1 < bytes.len() {
            index += 2;
            continue;
        }
        if bytes[index] == quote {
            return Some(index + 1);
        }
        index += 1;
    }
    None
}

fn skip_ws(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

fn scan_events(html: &str) -> Vec<Event> {
    let mut events = Vec::new();
    let mut index = 0;
    while index < html.len() {
        let Some(relative) = html[index..].find('<') else {
            break;
        };
        index += relative;
        if html[index..].starts_with("<!--") {
            if let Some(end) = html[index + 4..].find("-->") {
                index = index + 4 + end + 3;
                continue;
            }
            break;
        }
        if html[index..].starts_with("<!") || html[index..].starts_with("<?") {
            if let Some(end) = find_tag_end(html, index) {
                index = end + 1;
                continue;
            }
            break;
        }
        if html.as_bytes().get(index + 1) == Some(&b'/') {
            if let Some(end) = find_tag_end(html, index) {
                events.push(Event::Close(close_name(&html[index + 1..end])));
                index = end + 1;
                continue;
            }
            break;
        }
        let Some(tag) = parse_tag(html, index) else {
            index += 1;
            continue;
        };
        if matches!(tag.name.as_str(), "script" | "textarea" | "title") {
            index = find_close_tag(html, tag.end, &tag.name)
                .and_then(|at| find_tag_end(html, at))
                .map(|end| end + 1)
                .unwrap_or(html.len());
            continue;
        }
        if tag.name == "style" {
            let content_start = tag.end;
            if let Some(close_at) = find_close_tag(html, content_start, "style") {
                let after = find_tag_end(html, close_at)
                    .map(|end| end + 1)
                    .unwrap_or(html.len());
                events.push(Event::Style {
                    content_start,
                    content_end: close_at,
                });
                index = after;
            } else {
                index = html.len();
            }
            continue;
        }
        index = tag.end;
        events.push(Event::Open(tag));
    }
    events
}

fn matching_close(html: &str, from: usize, name: &str) -> usize {
    let mut depth = 1i32;
    let mut index = from;
    while index < html.len() {
        let Some(relative) = html[index..].find('<') else {
            return html.len();
        };
        index += relative;
        if html[index..].starts_with("<!--") {
            if let Some(end) = html[index + 4..].find("-->") {
                index = index + 4 + end + 3;
                continue;
            }
            return html.len();
        }
        if let Some(tag) = parse_tag(html, index) {
            if tag.name == name {
                depth += 1;
            }
            index = tag.end;
            continue;
        }
        if html.as_bytes().get(index + 1) == Some(&b'/') {
            if let Some(end) = find_tag_end(html, index) {
                if close_name(&html[index + 1..end]) == name {
                    depth -= 1;
                    if depth == 0 {
                        return index;
                    }
                }
                index = end + 1;
                continue;
            }
        }
        index += 1;
    }
    html.len()
}

fn find_close_tag(html: &str, from: usize, name: &str) -> Option<usize> {
    let bytes = html.as_bytes();
    let name = name.as_bytes();
    let mut index = from;
    while index + 2 + name.len() < bytes.len() {
        if bytes[index] == b'<' && bytes[index + 1] == b'/' {
            let start = index + 2;
            let end = start + name.len();
            if end <= bytes.len() && bytes[start..end].eq_ignore_ascii_case(name) {
                let mut after = end;
                while after < bytes.len() && bytes[after].is_ascii_whitespace() {
                    after += 1;
                }
                if bytes.get(after) == Some(&b'>') {
                    return Some(index);
                }
            }
        }
        index += 1;
    }
    None
}

fn find_tag_end(html: &str, start: usize) -> Option<usize> {
    let bytes = html.as_bytes();
    let mut index = start + 1;
    let mut quote = 0u8;
    while index < bytes.len() {
        let byte = bytes[index];
        if quote != 0 {
            if byte == quote {
                quote = 0;
            }
        } else if byte == b'"' || byte == b'\'' {
            quote = byte;
        } else if byte == b'>' {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn parse_tag(html: &str, start: usize) -> Option<Tag> {
    let end_bracket = find_tag_end(html, start)?;
    let body_start = start + 1;
    let body = &html[body_start..end_bracket];
    if body.is_empty() || body.starts_with(['!', '?', '/']) {
        return None;
    }
    let bytes = body.as_bytes();
    let mut index = 0;
    while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'/' {
        index += 1;
    }
    if index == 0 {
        return None;
    }
    let name = body[..index].to_ascii_lowercase();
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b':')
    {
        return None;
    }
    let mut attrs = Vec::new();
    while index < bytes.len() {
        while index < bytes.len() && (bytes[index].is_ascii_whitespace() || bytes[index] == b'/') {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        let name_start = index;
        while index < bytes.len()
            && !bytes[index].is_ascii_whitespace()
            && bytes[index] != b'='
            && bytes[index] != b'/'
        {
            index += 1;
        }
        if index == name_start {
            index += 1;
            continue;
        }
        let attr_name = body[name_start..index].to_ascii_lowercase();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index < bytes.len() && bytes[index] == b'=' {
            index += 1;
            while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            if index >= bytes.len() {
                attrs.push(Attr {
                    name: attr_name,
                    value: Some(String::new()),
                    span: None,
                });
                break;
            }
            if bytes[index] == b'"' || bytes[index] == b'\'' {
                let quote = bytes[index];
                index += 1;
                let value_start = index;
                while index < bytes.len() && bytes[index] != quote {
                    index += 1;
                }
                let raw = &body[value_start..index];
                let span = (body_start + value_start, body_start + index);
                if index < bytes.len() {
                    index += 1;
                }
                attrs.push(Attr {
                    name: attr_name,
                    value: Some(decode_entities(raw)),
                    span: Some(span),
                });
            } else {
                let value_start = index;
                while index < bytes.len()
                    && !bytes[index].is_ascii_whitespace()
                    && bytes[index] != b'/'
                {
                    index += 1;
                }
                let raw = &body[value_start..index];
                attrs.push(Attr {
                    name: attr_name,
                    value: Some(decode_entities(raw)),
                    span: Some((body_start + value_start, body_start + index)),
                });
            }
        } else {
            attrs.push(Attr {
                name: attr_name,
                value: None,
                span: None,
            });
        }
    }
    Some(Tag {
        name,
        start,
        end: end_bracket + 1,
        attrs,
    })
}

fn close_name(body: &str) -> String {
    let body = body.trim_start_matches('/').trim();
    let end = body
        .find(|ch: char| ch.is_ascii_whitespace() || ch == '>')
        .unwrap_or(body.len());
    body[..end].to_ascii_lowercase()
}

fn attr<'a>(tag: &'a Tag, name: &str) -> Option<&'a str> {
    tag.attrs
        .iter()
        .find(|item| item.name == name)
        .and_then(|item| item.value.as_deref())
}

fn render_tag(name: &str, attrs: &[(String, Option<String>)]) -> String {
    let mut out = String::from("<");
    out.push_str(name);
    for (name, value) in attrs {
        out.push(' ');
        out.push_str(name);
        if let Some(value) = value {
            out.push_str("=\"");
            out.push_str(&escape_attr(value));
            out.push('"');
        }
    }
    out.push('>');
    out
}

fn render_style(css: &str, media: Option<&str>, href: &str) -> String {
    let mut out = String::from("<style");
    if let Some(media) = media.map(str::trim).filter(|value| !value.is_empty()) {
        out.push_str(" media=\"");
        out.push_str(&escape_attr(media));
        out.push('"');
    }
    out.push_str(" data-ce-inlined=\"");
    out.push_str(&escape_attr(href));
    out.push_str("\">");
    out.push_str(&escape_style(css));
    out.push_str("</style>");
    out
}

fn escape_attr(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_style(css: &str) -> String {
    let lower = css.to_ascii_lowercase();
    let mut out = String::with_capacity(css.len());
    let mut index = 0;
    while index < css.len() {
        if lower[index..].starts_with("</style") {
            out.push_str("<\\/style");
            index += "</style".len();
            continue;
        }
        let ch = css[index..].chars().next().unwrap_or('\u{FFFD}');
        out.push(ch);
        index += ch.len_utf8();
    }
    out
}

fn decode_entities(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'&' {
            if let Some((ch, next)) = decode_entity(&input[index..]) {
                out.push(ch);
                index += next;
                continue;
            }
        }
        let ch = input[index..].chars().next().unwrap_or('\u{FFFD}');
        out.push(ch);
        index += ch.len_utf8();
    }
    out
}

fn decode_entity(input: &str) -> Option<(char, usize)> {
    let rest = input.strip_prefix('&')?;
    if let Some(name) = rest.strip_prefix("amp;") {
        return Some(('&', input.len() - name.len()));
    }
    if let Some(name) = rest.strip_prefix("lt;") {
        return Some(('<', input.len() - name.len()));
    }
    if let Some(name) = rest.strip_prefix("gt;") {
        return Some(('>', input.len() - name.len()));
    }
    if let Some(name) = rest.strip_prefix("quot;") {
        return Some(('"', input.len() - name.len()));
    }
    if let Some(name) = rest.strip_prefix("apos;") {
        return Some(('\'', input.len() - name.len()));
    }
    if let Some(name) = rest.strip_prefix("nbsp;") {
        return Some(('\u{00a0}', input.len() - name.len()));
    }
    let numeric = rest.strip_prefix('#')?;
    let (digits, radix) = if let Some(hex) = numeric
        .strip_prefix('x')
        .or_else(|| numeric.strip_prefix('X'))
    {
        (hex, 16)
    } else {
        (numeric, 10)
    };
    let end =
        digits.find(|ch: char| !ch.is_ascii_hexdigit() || (radix == 10 && !ch.is_ascii_digit()))?;
    if end == 0 || !digits[end..].starts_with(';') {
        return None;
    }
    let code = u32::from_str_radix(&digits[..end], radix).ok()?;
    let ch = char::from_u32(code)?;
    let consumed = input.len() - digits.len() + end + 1;
    Some((ch, consumed))
}

fn stitch(source: &str, mut edits: Vec<(usize, usize, String)>) -> String {
    edits.sort_by_key(|edit| edit.0);
    let mut out = String::with_capacity(source.len());
    let mut cursor = 0;
    for (start, end, text) in edits {
        if start < cursor || end > source.len() || start > end {
            continue;
        }
        out.push_str(&source[cursor..start]);
        out.push_str(&text);
        cursor = end;
    }
    out.push_str(&source[cursor..]);
    out
}

fn dedup_cap(urls: Vec<String>, cap: usize) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for url in urls {
        if seen.insert(url.clone()) {
            out.push(url);
            if out.len() == cap {
                break;
            }
        }
    }
    out
}

fn gather_sheets(
    app: &AppHandle,
    source: Option<&Webview>,
    cache: &HashMap<String, CachedResource>,
    seeds: &[String],
    referer: &str,
) -> HashMap<String, String> {
    let mut have: HashMap<String, String> = HashMap::new();
    let mut queue = dedup_cap(seeds.to_vec(), MAX_STYLESHEETS);
    for _ in 0..3 {
        if have.len() >= MAX_STYLESHEETS || queue.is_empty() {
            break;
        }
        let room = MAX_STYLESHEETS - have.len();
        let mut batch = Vec::new();
        let mut later = Vec::new();
        for url in queue.drain(..) {
            if have.contains_key(&url) {
                continue;
            }
            if batch.len() < room {
                batch.push(url);
            } else {
                later.push(url);
            }
        }
        if batch.is_empty() {
            break;
        }
        let fetched = fetch_sheets(app, source, cache, &batch, referer);
        let mut next = later;
        for (url, css) in fetched {
            if let Ok(base) = Url::parse(&url) {
                next.extend(css_refs(&css, &base).imports);
            }
            have.insert(url, css);
        }
        queue = next;
    }
    have
}

fn fetch_sheets(
    app: &AppHandle,
    source: Option<&Webview>,
    cache: &HashMap<String, CachedResource>,
    urls: &[String],
    referer: &str,
) -> Vec<(String, String)> {
    let mut pending = Vec::new();
    let mut found = Vec::new();
    for url in urls {
        if let Some(text) = source.and_then(|webview| cached_text(webview, cache.get(url))) {
            found.push((url.clone(), text));
        } else {
            pending.push(url.clone());
        }
    }
    if pending.is_empty() {
        return found;
    }
    let view = cookie_view(app, source);
    let (agent_name, cookies) = client_hints(view.as_ref(), &pending);
    let http = http_agent();
    for url in pending {
        if let Some(text) = http_text(
            &http,
            &url,
            referer,
            &agent_name,
            &cookie_for(&cookies, &url),
            MAX_STYLESHEET_BYTES,
        ) {
            if !looks_like_html(&text) {
                found.push((url, text));
            }
        }
    }
    found
}

fn fetch_images(
    app: &AppHandle,
    source: Option<&Webview>,
    cache: &HashMap<String, CachedResource>,
    urls: &[String],
    referer: &str,
) -> HashMap<String, String> {
    let total = AtomicU64::new(0);
    let mut images = HashMap::new();
    let mut pending = Vec::new();
    if let Some(webview) = source {
        for url in urls {
            match cached_image(webview, cache.get(url), &total) {
                Some(data) => {
                    images.insert(url.clone(), data);
                }
                None => pending.push(url.clone()),
            }
        }
    } else {
        pending.extend(urls.iter().cloned());
    }
    if pending.is_empty() {
        return images;
    }
    let view = cookie_view(app, source);
    let (agent_name, cookies) = client_hints(view.as_ref(), &pending);
    let http = http_agent();
    let fetched = fetch_parallel(&pending, |url| {
        let bytes = http_bytes(
            &http,
            url,
            referer,
            &agent_name,
            &cookie_for(&cookies, url),
            MAX_IMAGE_BYTES,
            true,
        )?;
        let mime = accepted_mime("", &bytes)?;
        admit(&total, bytes.len() as u64).then(|| to_data_url(&mime, &bytes))
    });
    images.extend(fetched);
    images
}

fn fetch_parallel(
    urls: &[String],
    fetch: impl Fn(&str) -> Option<String> + Sync,
) -> HashMap<String, String> {
    let queue = std::sync::Mutex::new(urls.to_vec());
    let out = std::sync::Mutex::new(HashMap::new());
    let workers = WORKERS.min(urls.len()).max(1);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let next = queue.lock().ok().and_then(|mut queue| queue.pop());
                    let Some(url) = next else {
                        break;
                    };
                    if let Some(data) = fetch(&url) {
                        if let Ok(mut out) = out.lock() {
                            out.insert(url, data);
                        }
                    }
                }
            });
        }
    });
    out.into_inner().unwrap_or_default()
}

fn cached_image(
    webview: &Webview,
    resource: Option<&CachedResource>,
    total: &AtomicU64,
) -> Option<String> {
    let resource = resource?;
    if matches!(
        resource.kind.as_str(),
        "Script" | "Stylesheet" | "Document" | "Font" | "Media"
    ) {
        return None;
    }
    let (mime, bytes) = cached_bytes(webview, resource)?;
    let mime = accepted_mime(&mime, &bytes)?;
    admit(total, bytes.len() as u64).then(|| to_data_url(&mime, &bytes))
}

fn cached_text(webview: &Webview, resource: Option<&CachedResource>) -> Option<String> {
    let resource = resource?;
    let value = resource_content(webview, resource)?;
    let text = if value.1 {
        String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(value.0)
                .ok()?,
        )
        .ok()?
    } else {
        value.0
    };
    (!looks_like_html(&text) && text.len() as u64 <= MAX_STYLESHEET_BYTES).then_some(text)
}

fn cached_bytes(webview: &Webview, resource: &CachedResource) -> Option<(String, Vec<u8>)> {
    let (content, encoded) = resource_content(webview, resource)?;
    let bytes = if encoded {
        base64::engine::general_purpose::STANDARD
            .decode(content)
            .ok()?
    } else {
        content.into_bytes()
    };
    if bytes.is_empty() || bytes.len() as u64 > MAX_IMAGE_BYTES {
        return None;
    }
    Some((resource.mime.clone(), bytes))
}

fn resource_content(webview: &Webview, resource: &CachedResource) -> Option<(String, bool)> {
    let params =
        serde_json::json!({ "frameId": resource.frame_id, "url": resource.url }).to_string();
    let value = webview2::call_cdp_json(
        webview,
        "Page.getResourceContent",
        &params,
        Duration::from_secs(4),
        "print",
    )
    .ok()?;
    let content = value.get("content")?.as_str()?.to_string();
    if content.is_empty() {
        return None;
    }
    let encoded = value
        .get("base64Encoded")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Some((content, encoded))
}

fn index_resources(webview: &Webview) -> HashMap<String, CachedResource> {
    let Ok(tree) = webview2::call_cdp_json(
        webview,
        "Page.getResourceTree",
        "{}",
        Duration::from_secs(5),
        "print",
    ) else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    if let Some(frame) = tree.get("frameTree") {
        walk_resource_tree(frame, &mut out);
    }
    out
}

fn walk_resource_tree(node: &Value, out: &mut HashMap<String, CachedResource>) {
    let frame_id = node
        .pointer("/frame/id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if let Some(resources) = node.get("resources").and_then(Value::as_array) {
        for resource in resources {
            let Some(url) = resource.get("url").and_then(Value::as_str) else {
                continue;
            };
            let Some(key) = normalize_url(url) else {
                continue;
            };
            out.insert(
                key,
                CachedResource {
                    frame_id: frame_id.clone(),
                    url: url.to_string(),
                    mime: resource
                        .get("mimeType")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    kind: resource
                        .get("type")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                },
            );
        }
    }
    if let Some(children) = node.get("childFrames").and_then(Value::as_array) {
        for child in children {
            walk_resource_tree(child, out);
        }
    }
}

fn cookie_view(app: &AppHandle, source: Option<&Webview>) -> Option<Webview> {
    source.cloned().or_else(|| app.get_webview(tabs::UI))
}

fn client_hints(webview: Option<&Webview>, urls: &[String]) -> (String, HashMap<String, String>) {
    let Some(webview) = webview else {
        return (DEFAULT_UA.to_string(), HashMap::new());
    };
    let mut cookies = HashMap::new();
    let mut agent = String::new();
    let mut seen = HashSet::new();
    for url in urls {
        let Some(origin) = origin_of(url) else {
            continue;
        };
        if !seen.insert(origin.clone()) {
            continue;
        }
        let (cookie, name) = webview2::session_for(webview, url);
        if agent.is_empty() && !name.is_empty() {
            agent = name;
        }
        if !cookie.is_empty() && header_safe(&cookie) {
            cookies.insert(origin, cookie);
        }
    }
    if agent.is_empty() {
        agent = DEFAULT_UA.to_string();
    }
    (agent, cookies)
}

fn origin_of(url: &str) -> Option<String> {
    let url = Url::parse(url).ok()?;
    let host = url.host_str()?;
    Some(match url.port() {
        Some(port) => format!("{}://{host}:{port}", url.scheme()),
        None => format!("{}://{host}", url.scheme()),
    })
}

fn cookie_for(cookies: &HashMap<String, String>, url: &str) -> String {
    origin_of(url)
        .and_then(|origin| cookies.get(&origin).cloned())
        .unwrap_or_default()
}

fn header_safe(value: &str) -> bool {
    value.bytes().all(|byte| (32..127).contains(&byte))
}

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .redirects(5)
        .build()
}

fn http_bytes(
    agent: &ureq::Agent,
    url: &str,
    referer: &str,
    user_agent: &str,
    cookie: &str,
    limit: u64,
    image: bool,
) -> Option<Vec<u8>> {
    let mut request = agent
        .get(url)
        .timeout(Duration::from_secs(12))
        .set("User-Agent", user_agent)
        .set(
            "Accept",
            if image {
                "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8"
            } else {
                "text/css,*/*;q=0.1"
            },
        );
    if header_safe(referer) && !referer.is_empty() {
        request = request.set("Referer", referer);
    }
    if !cookie.is_empty() {
        request = request.set("Cookie", cookie);
    }
    let response = request.call().ok()?;
    if image {
        let declared = response.header("Content-Type").unwrap_or("");
        let kind = declared
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if kind.starts_with("text/html")
            || kind == "application/json"
            || kind.contains("javascript")
            || kind == "text/css"
        {
            return None;
        }
    }
    read_capped(response, limit)
}

fn http_text(
    agent: &ureq::Agent,
    url: &str,
    referer: &str,
    user_agent: &str,
    cookie: &str,
    limit: u64,
) -> Option<String> {
    let bytes = http_bytes(agent, url, referer, user_agent, cookie, limit, false)?;
    let text = String::from_utf8_lossy(&bytes)
        .trim_start_matches('\u{feff}')
        .to_string();
    let declared_ok = text.contains('{') || text.to_ascii_lowercase().contains("@import");
    declared_ok.then_some(text)
}

fn read_capped(response: ureq::Response, limit: u64) -> Option<Vec<u8>> {
    if response
        .header("Content-Length")
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|len| len > limit)
    {
        return None;
    }
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.is_empty() || bytes.len() as u64 > limit {
        return None;
    }
    Some(bytes)
}

fn admit(total: &AtomicU64, bytes: u64) -> bool {
    if bytes == 0 || bytes > MAX_IMAGE_BYTES {
        return false;
    }
    let mut current = total.load(Ordering::Relaxed);
    loop {
        let Some(next) = current.checked_add(bytes) else {
            return false;
        };
        if next > MAX_TOTAL_BYTES {
            return false;
        }
        match total.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return true,
            Err(actual) => current = actual,
        }
    }
}

fn accepted_mime(declared: &str, bytes: &[u8]) -> Option<String> {
    let declared = declared
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if declared.starts_with("text/html") || looks_like_html_bytes(bytes) {
        return None;
    }
    if let Some(sniffed) = sniff(bytes) {
        return Some(sniffed.to_string());
    }
    if declared == "image/svg+xml" && looks_like_svg(bytes) {
        return Some(declared);
    }
    if declared.starts_with("image/")
        && declared
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '+' | '-' | '.'))
    {
        return Some(declared);
    }
    None
}

fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.starts_with(b"RIFF") && bytes.len() >= 12 && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    if bytes.starts_with(b"BM") {
        return Some("image/bmp");
    }
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        let brand = &bytes[8..12];
        if brand == b"avif" || brand == b"avis" {
            return Some("image/avif");
        }
    }
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return Some("image/x-icon");
    }
    looks_like_svg(bytes).then_some("image/svg+xml")
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]);
    let text = head.trim_start().to_ascii_lowercase();
    text.contains("<svg") && !text.contains("<html")
}

fn looks_like_html(text: &str) -> bool {
    let text = text.trim_start().to_ascii_lowercase();
    text.starts_with("<!doctype html") || text.starts_with("<html")
}

fn looks_like_html_bytes(bytes: &[u8]) -> bool {
    looks_like_html(&String::from_utf8_lossy(&bytes[..bytes.len().min(64)]))
}

fn to_data_url(mime: &str, bytes: &[u8]) -> String {
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use url::Url;

    use super::{apply, css_refs, document_base, prepare_css, src_is_embedded};

    fn base() -> Url {
        Url::parse("https://news.example/story").unwrap()
    }

    fn images(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(url, data)| ((*url).to_string(), (*data).to_string()))
            .collect()
    }

    #[test]
    fn an_image_and_its_srcset_become_one_embedded_picture() {
        let html = r#"<img alt="Shot" src="https://cdn.example/a.png?x=1&amp;y=2" srcset="https://cdn.example/a.png?x=1&amp;y=2 800w">"#;
        let map = images(&[(
            "https://cdn.example/a.png?x=1&y=2",
            "data:image/png;base64,QQ==",
        )]);
        let out = apply(html, &base(), &map, &HashMap::new());
        assert!(out.contains("src=\"data:image/png;base64,QQ==\""));
        assert!(out.contains("alt=\"Shot\""));
        assert!(!out.contains("srcset"));
        assert!(!out.contains("cdn.example"));
    }

    #[test]
    fn a_placeholder_uses_the_srcset_candidate() {
        let html = r#"<img src="https://cdn.example/placeholder.gif" srcset="https://cdn.example/small.jpg 400w, https://cdn.example/large.jpg 1200w">"#;
        let map = images(&[(
            "https://cdn.example/large.jpg",
            "data:image/jpeg;base64,QQ==",
        )]);
        let out = apply(html, &base(), &map, &HashMap::new());
        assert!(out.contains("src=\"data:image/jpeg;base64,QQ==\""));
        assert!(!out.contains("placeholder"));
        assert!(!out.contains("srcset"));
    }

    #[test]
    fn a_picture_keeps_the_shown_image_and_drops_the_other_sources() {
        let html = r#"<picture><source srcset="https://cdn.example/wide.jpg 1200w"><img src="https://cdn.example/shot.jpg"></picture>"#;
        let map = images(&[(
            "https://cdn.example/shot.jpg",
            "data:image/jpeg;base64,QQ==",
        )]);
        let out = apply(html, &base(), &map, &HashMap::new());
        assert!(out.contains("src=\"data:image/jpeg;base64,QQ==\""));
        assert!(!out.contains("<source"));
        assert!(!out.contains("wide.jpg"));
    }

    #[test]
    fn links_scripts_and_misses_stay_as_they_were() {
        let html = r#"<a href="https://cdn.example/a.png">link</a><script>const x = "<img src=\"https://cdn.example/secret.png\">";</script><img src="https://cdn.example/missing.png">"#;
        let map = images(&[
            ("https://cdn.example/a.png", "data:image/png;base64,QQ=="),
            (
                "https://cdn.example/secret.png",
                "data:image/png;base64,QQ==",
            ),
        ]);
        let out = apply(html, &base(), &map, &HashMap::new());
        assert!(out.contains("href=\"https://cdn.example/a.png\""));
        assert!(out.contains("secret.png"));
        assert!(out.contains("src=\"https://cdn.example/missing.png\""));
        assert!(!out.contains("data:image"));
    }

    #[test]
    fn an_embedded_src_drops_a_remote_srcset() {
        let src = format!("data:image/png;base64,{}", "A".repeat(80));
        let html = format!(r#"<img src="{src}" srcset="https://cdn.example/other.png 2x">"#);
        let out = apply(&html, &base(), &HashMap::new(), &HashMap::new());
        assert!(out.contains(&src));
        assert!(!out.contains("srcset"));
        assert!(src_is_embedded(&src));
    }

    #[test]
    fn a_stylesheet_image_is_inlined_and_a_font_keeps_an_absolute_address() {
        let html = r#"<link rel="stylesheet" href="https://cdn.example/app.css">"#;
        let mut sheets = HashMap::new();
        sheets.insert(
            "https://cdn.example/app.css".to_string(),
            ".a{background:url(\"hero.png\")}@font-face{src:url(\"f.woff2\")}".to_string(),
        );
        let map = images(&[("https://cdn.example/hero.png", "data:image/png;base64,QQ==")]);
        let out = apply(html, &base(), &map, &sheets);
        assert!(out.contains("data:image/png;base64,QQ=="));
        assert!(out.contains("url(\"https://cdn.example/f.woff2\")"));
        assert!(out.contains("data-ce-inlined=\"https://cdn.example/app.css\""));
        assert!(!out.contains("<link"));
    }

    #[test]
    fn an_inline_background_is_embedded_without_touching_the_font() {
        let html = r#"<style>.a{background:url("hero.png")}@font-face{src:url("f.woff2")}</style>"#;
        let css_base = Url::parse("https://news.example/story").unwrap();
        let map = images(&[(
            "https://news.example/hero.png",
            "data:image/png;base64,QQ==",
        )]);
        let out = apply(html, &css_base, &map, &HashMap::new());
        assert!(out.contains("url(\"data:image/png;base64,QQ==\")"));
        assert!(out.contains("url(\"f.woff2\")"));
    }

    #[test]
    fn imported_css_is_pulled_into_the_style() {
        let mut sheets = HashMap::new();
        sheets.insert(
            "https://cdn.example/extra.css".to_string(),
            ".x{background:url(\"a.png\")}".to_string(),
        );
        let map = images(&[("https://cdn.example/a.png", "data:image/png;base64,QQ==")]);
        let css = prepare_css(
            "@import \"extra.css\";body{color:red}",
            &Url::parse("https://cdn.example/app.css").unwrap(),
            &sheets,
            &map,
            &mut std::collections::HashSet::new(),
            true,
        );
        assert!(css.contains("data:image/png;base64,QQ=="));
        assert!(css.contains("body{color:red}"));
        assert!(!css.contains("@import"));
    }

    #[test]
    fn quoted_css_text_is_not_treated_as_an_image() {
        let refs = css_refs(
            "a{content:\"url(nope.png)\"}b{background:url(real.png)}",
            &base(),
        );
        assert_eq!(
            refs.images,
            vec!["https://news.example/real.png".to_string()]
        );
    }

    #[test]
    fn the_base_tag_is_the_document_address() {
        let html = r#"<head><base href="https://cdn.example/articles/"></head>"#;
        let base = document_base(html, "https://news.example/story").unwrap();
        assert_eq!(base.as_str(), "https://cdn.example/articles/");
    }

    #[test]
    fn nothing_to_embed_leaves_the_markup_unchanged() {
        let html = r#"<p>Hello</p><img alt="x" src="https://cdn.example/a.png">"#;
        let out = apply(html, &base(), &HashMap::new(), &HashMap::new());
        assert_eq!(out, html);
    }
}
