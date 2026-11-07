//! Shopping summary providers. Price tracking never calls this module.

use std::io::Read;
use std::time::Duration;

use serde_json::{Value, json};
use tauri::AppHandle;

use crate::{secrets, settings};

pub const OPENAI_ENDPOINT: &str = "https://api.openai.com/v1";
pub const CLAUDE_ENDPOINT: &str = "https://api.anthropic.com/v1";
pub const LOCAL_ENDPOINT: &str = "http://127.0.0.1:11434/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const ANTHROPIC_MAX_TOKENS: u32 = 1024;

pub fn localhost_endpoint(endpoint: &str) -> bool {
    let lower = endpoint.to_ascii_lowercase();
    lower.contains("://127.0.0.1") || lower.contains("://localhost") || lower.contains("://[::1]")
}

pub fn chat_url(endpoint: &str) -> String {
    format!("{}/chat/completions", endpoint.trim().trim_end_matches('/'))
}

pub fn messages_url(endpoint: &str) -> String {
    format!("{}/messages", endpoint.trim().trim_end_matches('/'))
}

pub fn models_url(endpoint: &str) -> String {
    format!("{}/models", endpoint.trim().trim_end_matches('/'))
}

pub fn provider_id(settings: &settings::BrowserSettings) -> &'static str {
    match settings.ai_provider.as_str() {
        "openai" => "openai",
        "claude" => "claude",
        "custom" => "custom",
        _ => "local",
    }
}

pub fn active_slot(settings: &settings::BrowserSettings) -> settings::AiProviderSlot {
    match provider_id(settings) {
        "openai" => settings.ai_providers.openai.clone(),
        "claude" => settings.ai_providers.claude.clone(),
        "custom" => settings.ai_providers.custom.clone(),
        _ => settings.ai_providers.local.clone(),
    }
}

pub fn endpoint_of(settings: &settings::BrowserSettings) -> String {
    let slot = active_slot(settings);
    let endpoint = slot.endpoint.trim();
    if !endpoint.is_empty() {
        return endpoint.to_string();
    }
    match provider_id(settings) {
        "openai" => OPENAI_ENDPOINT.into(),
        "claude" => CLAUDE_ENDPOINT.into(),
        "local" => LOCAL_ENDPOINT.into(),
        _ => String::new(),
    }
}

pub fn model_of(settings: &settings::BrowserSettings, override_model: &str) -> String {
    let chosen = override_model.trim();
    if !chosen.is_empty() {
        return chosen.to_string();
    }
    let slot = active_slot(settings);
    let model = slot.model.trim();
    if !model.is_empty() {
        return model.to_string();
    }
    if provider_id(settings) == "local" {
        "llama3.2".into()
    } else {
        String::new()
    }
}

/// True when a summary can be requested. Local servers and a configured key both count.
pub fn available(app: &AppHandle, settings: &settings::BrowserSettings) -> bool {
    let provider = provider_id(settings);
    let endpoint = endpoint_of(settings);
    if endpoint.is_empty() {
        return false;
    }
    if provider == "local" || (provider == "custom" && localhost_endpoint(&endpoint)) {
        return true;
    }
    match secrets::resolve(app, provider) {
        Ok(Some(key)) => !key.trim().is_empty(),
        _ => false,
    }
}

pub fn openai_body(model: &str, system: &str, user: &str, json_only: bool) -> Value {
    let mut body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ]
    });
    if json_only {
        body["response_format"] = json!({ "type": "json_object" });
    }
    body
}

pub fn anthropic_body(model: &str, system: &str, user: &str, json_only: bool) -> Value {
    let system = if json_only {
        format!("{system}\n\nReply with one JSON object only.")
    } else {
        system.to_string()
    };
    json!({
        "model": model,
        "max_tokens": ANTHROPIC_MAX_TOKENS,
        "system": system,
        "messages": [{ "role": "user", "content": user }]
    })
}

pub fn openai_text(value: &Value) -> Option<String> {
    value
        .pointer("/choices/0/message/content")
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

pub fn anthropic_text(value: &Value) -> Option<String> {
    let blocks = value.get("content")?.as_array()?;
    let mut out = String::new();
    for block in blocks {
        if block.get("type").and_then(|item| item.as_str()) != Some("text") {
            continue;
        }
        let Some(text) = block.get("text").and_then(|item| item.as_str()) else {
            continue;
        };
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(text);
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn model_ids(body: &str) -> Vec<String> {
    let mut ids: Vec<String> = model_entries(body).into_iter().map(|(id, _)| id).collect();
    ids.sort();
    ids
}

/// Model id and a recency rank. OpenAI sends `created` as unix seconds. Claude sends `created_at`.
fn model_entries(body: &str) -> Vec<(String, i64)> {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    let Some(items) = value
        .get("data")
        .and_then(|item| item.as_array())
        .or_else(|| value.get("models").and_then(|item| item.as_array()))
    else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for item in items {
        let id = item
            .get("id")
            .and_then(|item| item.as_str())
            .or_else(|| item.get("name").and_then(|item| item.as_str()))
            .unwrap_or("")
            .trim();
        if id.is_empty() || entries.iter().any(|(kept, _): &(String, i64)| kept == id) {
            continue;
        }
        entries.push((id.to_string(), created_rank(item)));
        if entries.len() >= 2_000 {
            break;
        }
    }
    entries
}

fn created_rank(item: &Value) -> i64 {
    if let Some(created) = item.get("created").and_then(|value| value.as_i64()) {
        return created;
    }
    item.get("created_at")
        .and_then(|value| value.as_str())
        .map(date_rank)
        .unwrap_or(0)
}

/// `2026-09-01T00:00:00Z` becomes `20260901`, which sorts in the same order as the calendar.
fn date_rank(text: &str) -> i64 {
    let bytes = text.as_bytes();
    if bytes.len() >= 10 && bytes[4] == b'-' && bytes[7] == b'-' {
        let year: i64 = text[0..4].parse().unwrap_or(0);
        let month: i64 = text[5..7].parse().unwrap_or(0);
        let day: i64 = text[8..10].parse().unwrap_or(0);
        return year * 10_000 + month * 100 + day;
    }
    0
}

struct ModelPage {
    ids: Vec<String>,
    has_more: bool,
    last_id: Option<String>,
}

fn model_page(body: &str) -> ModelPage {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return ModelPage {
            ids: Vec::new(),
            has_more: false,
            last_id: None,
        };
    };
    let has_more = value
        .get("has_more")
        .and_then(|item| item.as_bool())
        .unwrap_or(false);
    let last_id = value
        .get("last_id")
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|id| page_cursor(id))
        .map(str::to_string);
    ModelPage {
        ids: model_ids(body),
        has_more,
        last_id,
    }
}

fn page_cursor(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 200
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

fn claude_models_url(endpoint: &str, after_id: Option<&str>) -> String {
    let base = models_url(endpoint);
    match after_id {
        Some(id) if page_cursor(id) => format!("{base}?limit=100&after_id={id}"),
        _ => format!("{base}?limit=100"),
    }
}

/// Chat models that return text. OpenAI's list also includes image, audio, and embedding models.
fn text_chat_model(provider: &str, id: &str) -> bool {
    let id = id.trim();
    if id.is_empty() {
        return false;
    }
    match provider {
        "openai" => openai_text_model(&id.to_ascii_lowercase()),
        "claude" => {
            let id = id.to_ascii_lowercase();
            id.starts_with("claude-") && !id.contains("embed")
        }
        _ => true,
    }
}

fn openai_text_model(id: &str) -> bool {
    let base = id
        .strip_prefix("ft:")
        .and_then(|rest| rest.split(':').next())
        .unwrap_or(id);
    if base.is_empty() || openai_non_text(id) || openai_non_text(base) || base.contains("instruct")
    {
        return false;
    }
    base.starts_with("gpt-") || base.starts_with("chatgpt-") || reasoning_model(base)
}

fn openai_non_text(id: &str) -> bool {
    const MARKERS: &[&str] = &[
        "embedding",
        "whisper",
        "tts",
        "dall-e",
        "dalle",
        "moderation",
        "realtime",
        "audio",
        "transcribe",
        "image",
        "sora",
        "davinci",
        "babbage",
        "computer-use",
    ];
    MARKERS.iter().any(|marker| id.contains(marker))
}

const RECENT_TEXT_MODELS: usize = 6;

/// The six newest general text models. Dated snapshots collapse onto their undated id.
fn recent_text_models(entries: &[(String, i64)]) -> Vec<String> {
    struct Pick {
        id: String,
        dated: bool,
        id_created: i64,
        family_created: i64,
    }
    let mut picks: Vec<Pick> = Vec::new();
    for (id, created) in entries {
        if id.starts_with("ft:") || !general_text_model(id) {
            continue;
        }
        let dated = dated_snapshot(id);
        let family = model_family(id);
        if let Some(pick) = picks
            .iter_mut()
            .find(|pick| model_family(&pick.id) == family)
        {
            pick.family_created = pick.family_created.max(*created);
            let replace = match (pick.dated, dated) {
                (true, false) => true,
                (false, true) => false,
                _ => *created >= pick.id_created,
            };
            if replace {
                pick.id = id.clone();
                pick.dated = dated;
                pick.id_created = *created;
            }
            continue;
        }
        picks.push(Pick {
            id: id.clone(),
            dated,
            id_created: *created,
            family_created: *created,
        });
    }
    picks.sort_by(|left, right| {
        right
            .family_created
            .cmp(&left.family_created)
            .then_with(|| left.id.cmp(&right.id))
    });
    picks
        .into_iter()
        .take(RECENT_TEXT_MODELS)
        .map(|pick| pick.id)
        .collect()
}

/// Text chat models, without specialty variants such as cyber or image models.
fn general_text_model(id: &str) -> bool {
    let id = id.trim();
    if id.is_empty() {
        return false;
    }
    let lower = id.to_ascii_lowercase();
    if !text_chat_model("openai", &lower) && !text_chat_model("claude", &lower) {
        return false;
    }
    const SKIP: &[&str] = &["cyber", "rosalind", "legal", "search"];
    !SKIP.iter().any(|marker| lower.contains(marker))
}

fn model_family(id: &str) -> &str {
    let id = id.trim();
    if dated_snapshot(id) {
        if iso_date_suffix(id) {
            return &id[..id.len() - 11];
        }
        if let Some((head, _)) = id.rsplit_once('-') {
            return head;
        }
    }
    id
}

fn dated_snapshot(id: &str) -> bool {
    compact_date_suffix(id) || iso_date_suffix(id)
}

fn compact_date_suffix(id: &str) -> bool {
    let Some((_, tail)) = id.rsplit_once('-') else {
        return false;
    };
    tail.len() == 8 && tail.chars().all(|character| character.is_ascii_digit())
}

fn iso_date_suffix(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.len() < 11 || bytes[bytes.len() - 11] != b'-' {
        return false;
    }
    let tail = &bytes[bytes.len() - 10..];
    tail[4] == b'-'
        && tail[7] == b'-'
        && tail[..4].iter().all(u8::is_ascii_digit)
        && tail[5..7].iter().all(u8::is_ascii_digit)
        && tail[8..].iter().all(u8::is_ascii_digit)
}

/// Models that accept a low / medium / high thinking effort.
fn model_accepts_thinking(provider: &str, id: &str) -> bool {
    let id = model_family(id).to_ascii_lowercase();
    if id.contains("chat") {
        return false;
    }
    match provider {
        "openai" => openai_thinking_model(&id),
        "claude" => claude_thinking_model(&id),
        _ => false,
    }
}

fn openai_thinking_model(id: &str) -> bool {
    if reasoning_model(id) {
        return true;
    }
    let Some(rest) = id.strip_prefix("gpt-") else {
        return false;
    };
    rest.chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit() && character != '3' && character != '4')
}

fn claude_thinking_model(id: &str) -> bool {
    if id.starts_with("claude-fable") || id.starts_with("claude-mythos") || id.contains("-3-7") {
        return true;
    }
    let Some(rest) = id.strip_prefix("claude-") else {
        return false;
    };
    let (family, version) = rest.split_once('-').unwrap_or((rest, ""));
    if !matches!(family, "opus" | "sonnet" | "haiku") {
        return false;
    }
    let mut parts = version.split('-');
    let major: u32 = parts.next().unwrap_or("").parse().unwrap_or(0);
    let minor: u32 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    if family == "haiku" {
        return major > 4 || (major == 4 && minor >= 5);
    }
    major >= 4
}

/// OpenAI reasoning models use ids such as `o1`, `o3-mini`, and `o4-mini`.
fn reasoning_model(id: &str) -> bool {
    let Some(rest) = id.strip_prefix('o') else {
        return false;
    };
    let Some(family) = rest.chars().next() else {
        return false;
    };
    if !family.is_ascii_digit() {
        return false;
    }
    let after = &rest[family.len_utf8()..];
    after.is_empty() || after.starts_with('-')
}

pub fn http_error(status: u16, endpoint: &str, body: &str) -> String {
    let summary = match status {
        401 | 403 => "The API key was rejected.".to_string(),
        429 => "The provider is rate limited. Try again in a minute.".to_string(),
        404 => "That model or endpoint was not found.".to_string(),
        _ if localhost_endpoint(endpoint) && status == 0 => {
            "The local server is not running.".to_string()
        }
        _ => format!("The provider returned an error ({status})."),
    };
    match provider_error_detail(body) {
        Some(detail) => format!("{summary}\n{detail}"),
        None => summary,
    }
}

/// Text from a provider error body, short enough to show in the agent panel.
fn provider_error_detail(body: &str) -> Option<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() || looks_like_html(trimmed) {
        return None;
    }
    let text = match serde_json::from_str::<Value>(trimmed) {
        Ok(value) => json_error_message(&value).or_else(|| {
            let compact = value.to_string();
            if compact == "null" || compact == "{}" || compact == "[]" {
                None
            } else {
                Some(compact)
            }
        })?,
        Err(_) => trimmed.to_string(),
    };
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    Some(clip_detail(text))
}

fn json_error_message(value: &Value) -> Option<String> {
    if let Some(error) = value.get("error") {
        if let Some(text) = scalar_text(error) {
            return Some(text);
        }
        if let Some(text) = error.get("message").and_then(scalar_text) {
            return Some(text);
        }
    }
    for key in ["message", "detail", "error_description"] {
        if let Some(text) = value.get(key).and_then(scalar_text) {
            return Some(text);
        }
    }
    None
}

fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn looks_like_html(text: &str) -> bool {
    let lower = text.trim_start().to_ascii_lowercase();
    lower.starts_with("<!doctype")
        || lower.starts_with("<html")
        || lower.starts_with("<head")
        || lower.starts_with("<body")
}

const ERROR_DETAIL_LIMIT: usize = 1_200;

fn clip_detail(text: &str) -> String {
    if text.chars().count() <= ERROR_DETAIL_LIMIT {
        return text.to_string();
    }
    let mut out: String = text.chars().take(ERROR_DETAIL_LIMIT).collect();
    out.push('…');
    out
}

pub fn transport_error(endpoint: &str, message: &str) -> String {
    let lower = message.to_ascii_lowercase();
    if localhost_endpoint(endpoint)
        && (lower.contains("refused")
            || lower.contains("10061")
            || lower.contains("could not connect")
            || lower.contains("couldn't connect")
            || lower.contains("connection failed"))
    {
        return "The local server is not running.".into();
    }
    "Couldn't reach the assistant.".into()
}

/// One chat completion. `model` overrides the provider model when it is not blank.
pub fn complete(
    app: &AppHandle,
    current: &settings::BrowserSettings,
    model: &str,
    system: &str,
    user: &str,
    json_only: bool,
    timeout: Duration,
) -> Result<String, String> {
    let provider = provider_id(current);
    let endpoint = endpoint_of(current);
    if endpoint.is_empty() {
        return Err("Set an assistant endpoint in Settings, under AI.".into());
    }
    let model = model_of(current, model);
    if model.is_empty() {
        return Err("Set a model in Settings, under AI.".into());
    }
    let key = request_key(app, provider, &endpoint)?;
    let url = if provider == "claude" {
        messages_url(&endpoint)
    } else {
        chat_url(&endpoint)
    };
    if !key.is_empty() && url.contains(&key) {
        return Err("Couldn't reach the assistant.".into());
    }
    let mut body = if provider == "claude" {
        anthropic_body(&model, system, user, json_only)
    } else {
        openai_body(&model, system, user, json_only)
    };
    attach_thinking(provider, &model, &slot_thinking(current), &mut body);
    let text = send(provider, &url, &endpoint, &key, &body, timeout)?;
    let value: Value =
        serde_json::from_str(&text).map_err(|_| "The assistant returned nothing.".to_string())?;
    let reply = if provider == "claude" {
        anthropic_text(&value)
    } else {
        openai_text(&value)
    };
    reply.ok_or_else(|| "The assistant returned nothing.".to_string())
}

const TOOL_MAX_TOKENS: u32 = 4096;

/// One piece of a chat message. Images are base64 without the `data:` prefix.
#[derive(Debug, Clone)]
pub enum ChatPart {
    Text(String),
    Image { media_type: String, data: String },
}

#[derive(Debug, Clone)]
pub enum ChatMessage {
    User(Vec<ChatPart>),
    Assistant {
        text: String,
        calls: Vec<ToolCall>,
    },
    ToolResult {
        call_id: String,
        parts: Vec<ChatPart>,
    },
}

/// A tool the model may call. `parameters` is a JSON Schema object.
#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Default)]
pub struct ToolReply {
    pub text: String,
    pub calls: Vec<ToolCall>,
}

fn openai_parts(parts: &[ChatPart]) -> Value {
    if let [ChatPart::Text(text)] = parts {
        return json!(text);
    }
    Value::Array(
        parts
            .iter()
            .map(|part| match part {
                ChatPart::Text(text) => json!({ "type": "text", "text": text }),
                ChatPart::Image { media_type, data } => json!({
                    "type": "image_url",
                    "image_url": { "url": format!("data:{media_type};base64,{data}") }
                }),
            })
            .collect(),
    )
}

fn part_text(parts: &[ChatPart]) -> String {
    let mut out = String::new();
    for part in parts {
        if let ChatPart::Text(text) = part {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(text);
        }
    }
    out
}

pub fn openai_tools_body(
    model: &str,
    system: &str,
    messages: &[ChatMessage],
    tools: &[ToolSpec],
) -> Value {
    let mut out = vec![json!({ "role": "system", "content": system })];
    // Tool messages carry text only, so their images follow in one user message.
    let mut pending: Vec<ChatPart> = Vec::new();
    let flush = |out: &mut Vec<Value>, pending: &mut Vec<ChatPart>| {
        if pending.is_empty() {
            return;
        }
        let mut parts = vec![ChatPart::Text("Screenshot after the last action:".into())];
        parts.append(pending);
        out.push(json!({ "role": "user", "content": openai_parts(&parts) }));
    };
    for message in messages {
        match message {
            ChatMessage::ToolResult { call_id, parts } => {
                let text = part_text(parts);
                out.push(json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": if text.is_empty() { "Done.".to_string() } else { text },
                }));
                pending.extend(
                    parts
                        .iter()
                        .filter(|part| matches!(part, ChatPart::Image { .. }))
                        .cloned(),
                );
            }
            ChatMessage::User(parts) => {
                flush(&mut out, &mut pending);
                out.push(json!({ "role": "user", "content": openai_parts(parts) }));
            }
            ChatMessage::Assistant { text, calls } => {
                flush(&mut out, &mut pending);
                let mut entry = json!({
                    "role": "assistant",
                    "content": if text.is_empty() { Value::Null } else { json!(text) },
                });
                if !calls.is_empty() {
                    entry["tool_calls"] = Value::Array(
                        calls
                            .iter()
                            .map(|call| json!({
                                "id": call.id,
                                "type": "function",
                                "function": { "name": call.name, "arguments": call.arguments.to_string() }
                            }))
                            .collect(),
                    );
                }
                out.push(entry);
            }
        }
    }
    flush(&mut out, &mut pending);
    json!({
        "model": model,
        "messages": out,
        "tools": tools.iter().map(|tool| json!({
            "type": "function",
            "function": { "name": tool.name, "description": tool.description, "parameters": tool.parameters }
        })).collect::<Vec<_>>(),
        "tool_choice": "auto",
    })
}

fn anthropic_parts(parts: &[ChatPart]) -> Vec<Value> {
    parts
        .iter()
        .map(|part| match part {
            ChatPart::Text(text) => json!({ "type": "text", "text": text }),
            ChatPart::Image { media_type, data } => json!({
                "type": "image",
                "source": { "type": "base64", "media_type": media_type, "data": data }
            }),
        })
        .collect()
}

pub fn anthropic_tools_body(
    model: &str,
    system: &str,
    messages: &[ChatMessage],
    tools: &[ToolSpec],
) -> Value {
    let mut out: Vec<Value> = Vec::new();
    let mut push = |role: &str, blocks: Vec<Value>| {
        if blocks.is_empty() {
            return;
        }
        // Claude rejects two user turns in a row, so tool results and notes share one.
        if let Some(last) = out.last_mut()
            && last["role"] == role
            && let Some(content) = last["content"].as_array_mut()
        {
            content.extend(blocks);
            return;
        }
        out.push(json!({ "role": role, "content": blocks }));
    };
    for message in messages {
        match message {
            ChatMessage::User(parts) => push("user", anthropic_parts(parts)),
            ChatMessage::ToolResult { call_id, parts } => {
                let mut content = anthropic_parts(parts);
                if content.is_empty() {
                    content.push(json!({ "type": "text", "text": "Done." }));
                }
                push(
                    "user",
                    vec![
                        json!({ "type": "tool_result", "tool_use_id": call_id, "content": content }),
                    ],
                );
            }
            ChatMessage::Assistant { text, calls } => {
                let mut blocks = Vec::new();
                if !text.trim().is_empty() {
                    blocks.push(json!({ "type": "text", "text": text }));
                }
                for call in calls {
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": call.id,
                        "name": call.name,
                        "input": if call.arguments.is_object() { call.arguments.clone() } else { json!({}) },
                    }));
                }
                push("assistant", blocks);
            }
        }
    }
    json!({
        "model": model,
        "max_tokens": TOOL_MAX_TOKENS,
        "system": system,
        "messages": out,
        "tools": tools.iter().map(|tool| json!({
            "name": tool.name,
            "description": tool.description,
            "input_schema": tool.parameters,
        })).collect::<Vec<_>>(),
    })
}

fn parse_arguments(raw: &Value) -> Value {
    match raw {
        Value::String(text) if text.trim().is_empty() => json!({}),
        Value::String(text) => serde_json::from_str(text).unwrap_or_else(|_| json!({})),
        Value::Object(_) => raw.clone(),
        _ => json!({}),
    }
}

pub fn openai_tool_reply(value: &Value) -> ToolReply {
    let message = value
        .pointer("/choices/0/message")
        .cloned()
        .unwrap_or(Value::Null);
    let text = message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let calls = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .map(|calls| {
            calls
                .iter()
                .enumerate()
                .filter_map(|(index, call)| {
                    let function = call.get("function")?;
                    let name = function.get("name")?.as_str()?.to_string();
                    let id = call
                        .get("id")
                        .and_then(Value::as_str)
                        .filter(|id| !id.is_empty())
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("call_{index}"));
                    Some(ToolCall {
                        id,
                        name,
                        arguments: parse_arguments(
                            function.get("arguments").unwrap_or(&Value::Null),
                        ),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    ToolReply { text, calls }
}

pub fn anthropic_tool_reply(value: &Value) -> ToolReply {
    let mut reply = ToolReply::default();
    let Some(blocks) = value.get("content").and_then(Value::as_array) else {
        return reply;
    };
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    if !reply.text.is_empty() {
                        reply.text.push('\n');
                    }
                    reply.text.push_str(text.trim());
                }
            }
            Some("tool_use") => {
                let id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let name = block
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if name.is_empty() {
                    continue;
                }
                reply.calls.push(ToolCall {
                    id: if id.is_empty() {
                        format!("toolu_{}", reply.calls.len())
                    } else {
                        id
                    },
                    name,
                    arguments: parse_arguments(block.get("input").unwrap_or(&Value::Null)),
                });
            }
            _ => {}
        }
    }
    reply
}

/// One model turn that may call tools. `model` overrides the provider model when it is not blank.
pub fn complete_tools(
    app: &AppHandle,
    current: &settings::BrowserSettings,
    model: &str,
    system: &str,
    messages: &[ChatMessage],
    tools: &[ToolSpec],
    timeout: Duration,
) -> Result<ToolReply, String> {
    let provider = provider_id(current);
    let endpoint = endpoint_of(current);
    if endpoint.is_empty() {
        return Err("Set an assistant endpoint in Settings, under AI.".into());
    }
    let model = model_of(current, model);
    if model.is_empty() {
        return Err("Set a model in Settings, under AI.".into());
    }
    let key = request_key(app, provider, &endpoint)?;
    let url = if provider == "claude" {
        messages_url(&endpoint)
    } else {
        chat_url(&endpoint)
    };
    if !key.is_empty() && url.contains(&key) {
        return Err("Couldn't reach the assistant.".into());
    }
    let mut body = if provider == "claude" {
        anthropic_tools_body(&model, system, messages, tools)
    } else {
        openai_tools_body(&model, system, messages, tools)
    };
    attach_tool_thinking(provider, &model, &slot_thinking(current), &mut body);
    if provider == "claude" && body["max_tokens"].as_u64().unwrap_or(0) < TOOL_MAX_TOKENS as u64 {
        body["max_tokens"] = json!(TOOL_MAX_TOKENS);
    }
    let text = send(provider, &url, &endpoint, &key, &body, timeout)?;
    let value: Value =
        serde_json::from_str(&text).map_err(|_| "The assistant returned nothing.".to_string())?;
    let reply = if provider == "claude" {
        anthropic_tool_reply(&value)
    } else {
        openai_tool_reply(&value)
    };
    if reply.text.is_empty() && reply.calls.is_empty() {
        return Err("The assistant returned nothing.".into());
    }
    Ok(reply)
}

pub fn list_models(app: &AppHandle) -> Result<Vec<String>, String> {
    let current = settings::get(app);
    let provider = provider_id(&current);
    let endpoint = endpoint_of(&current);
    if endpoint.is_empty() {
        return Err("Set an assistant endpoint in Settings, under AI.".into());
    }
    let key = request_key(app, provider, &endpoint)?;
    let mut entries = Vec::new();
    let mut after: Option<String> = None;
    for _ in 0..10 {
        let url = if provider == "claude" {
            claude_models_url(&endpoint, after.as_deref())
        } else {
            models_url(&endpoint)
        };
        if !key.is_empty() && url.contains(&key) {
            return Err("Couldn't reach the assistant.".into());
        }
        let text = get_text(provider, &url, &endpoint, &key, Duration::from_secs(20))?;
        let page = model_page(&text);
        for (id, created) in model_entries(&text) {
            if entries.iter().any(|(kept, _)| kept == &id) {
                continue;
            }
            entries.push((id, created));
            if entries.len() >= 2_000 {
                break;
            }
        }
        if provider != "claude" || !page.has_more || entries.len() >= 2_000 {
            break;
        }
        let Some(next) = page.last_id.filter(|id| after.as_ref() != Some(id)) else {
            break;
        };
        after = Some(next);
    }
    if provider == "openai" || provider == "claude" {
        return Ok(recent_text_models(&entries));
    }
    let mut ids: Vec<String> = entries
        .into_iter()
        .map(|(id, _)| id)
        .filter(|id| text_chat_model(provider, id))
        .collect();
    ids.sort();
    Ok(ids)
}

fn slot_thinking(current: &settings::BrowserSettings) -> String {
    active_slot(current).thinking
}

/// Sends a thinking effort only when the chosen model accepts one.
fn attach_thinking(provider: &str, model: &str, thinking: &str, body: &mut Value) {
    if !matches!(thinking, "low" | "medium" | "high") || !model_accepts_thinking(provider, model) {
        return;
    }
    if provider == "claude" {
        body["output_config"] = json!({ "effort": thinking });
        body["max_tokens"] = json!(8_192);
        return;
    }
    body["reasoning_effort"] = json!(thinking);
}

/// Tool calls on Chat Completions. GPT-5.4 and later reject function tools unless
/// `reasoning_effort` is `none`. The provider also fills in a higher default when the
/// field is left out, so the value has to be sent.
fn attach_tool_thinking(provider: &str, model: &str, thinking: &str, body: &mut Value) {
    if provider != "claude" && chat_tools_require_reasoning_none(model) {
        body["reasoning_effort"] = json!("none");
        return;
    }
    attach_thinking(provider, model, thinking, body);
}

fn chat_tools_require_reasoning_none(model: &str) -> bool {
    let id = model_family(model).to_ascii_lowercase();
    if chat_tools_reject_reasoning_none(&id) {
        return false;
    }
    let Some((major, minor)) = gpt_release(&id) else {
        return false;
    };
    major > 5 || (major == 5 && minor >= 4)
}

/// GPT-6 Astra and GPT-6.1 Sol reject `reasoning_effort: none` entirely.
fn chat_tools_reject_reasoning_none(id: &str) -> bool {
    id == "gpt-6-astra" || id == "gpt-6.1-sol"
}

fn gpt_release(id: &str) -> Option<(u32, u32)> {
    let rest = id.strip_prefix("gpt-")?;
    let numeric: String = rest
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '.')
        .collect();
    if numeric.is_empty() || numeric == "." {
        return None;
    }
    let mut parts = numeric.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor))
}

pub fn test_connection(app: &AppHandle) -> Result<(), String> {
    let _ = list_models(app)?;
    Ok(())
}

fn request_key(app: &AppHandle, provider: &str, endpoint: &str) -> Result<String, String> {
    let optional = provider == "local" || (provider == "custom" && localhost_endpoint(endpoint));
    match secrets::resolve(app, provider) {
        Ok(Some(key)) => Ok(key),
        Ok(None) if optional => Ok(String::new()),
        Ok(None) => Err("This endpoint needs an API key. Add it in Settings, under AI.".into()),
        Err(error) => Err(error),
    }
}

fn send(
    provider: &str,
    url: &str,
    endpoint: &str,
    key: &str,
    body: &Value,
    timeout: Duration,
) -> Result<String, String> {
    let payload =
        serde_json::to_string(body).map_err(|_| "Couldn't reach the assistant.".to_string())?;
    let mut request = ureq::post(url)
        .timeout(timeout)
        .set("Content-Type", "application/json");
    request = authorize(request, provider, key);
    match request.send_string(&payload) {
        Ok(response) => read_body(response),
        Err(ureq::Error::Status(status, response)) => {
            Err(http_error(status, endpoint, &error_body(response)))
        }
        Err(ureq::Error::Transport(error)) => Err(transport_error(endpoint, &error.to_string())),
    }
}

fn get_text(
    provider: &str,
    url: &str,
    endpoint: &str,
    key: &str,
    timeout: Duration,
) -> Result<String, String> {
    let mut request = ureq::get(url).timeout(timeout);
    request = authorize(request, provider, key);
    match request.call() {
        Ok(response) => read_body(response),
        Err(ureq::Error::Status(status, response)) => {
            Err(http_error(status, endpoint, &error_body(response)))
        }
        Err(ureq::Error::Transport(error)) => Err(transport_error(endpoint, &error.to_string())),
    }
}

fn error_body(response: ureq::Response) -> String {
    let mut bytes = Vec::new();
    let _ = response
        .into_reader()
        .take(8 * 1024)
        .read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}

fn authorize(request: ureq::Request, provider: &str, key: &str) -> ureq::Request {
    let key = key.trim();
    if key.is_empty() {
        return request;
    }
    if provider == "claude" {
        request
            .set("x-api-key", key)
            .set("anthropic-version", ANTHROPIC_VERSION)
    } else {
        request.set("Authorization", &format!("Bearer {key}"))
    }
}

fn read_body(response: ureq::Response) -> Result<String, String> {
    let text = response
        .into_string()
        .map_err(|_| "The assistant returned nothing.".to_string())?;
    if text.len() > 1_000_000 {
        return Err("The assistant returned nothing.".into());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::{
        ChatMessage, ChatPart, ToolCall, ToolSpec, anthropic_body, anthropic_text,
        anthropic_tool_reply, anthropic_tools_body, attach_thinking, attach_tool_thinking,
        chat_tools_require_reasoning_none, chat_url, claude_models_url, http_error, messages_url,
        model_accepts_thinking, model_ids, model_page, models_url, openai_body, openai_text,
        openai_tool_reply, openai_tools_body, recent_text_models, text_chat_model, transport_error,
    };

    fn tool_history() -> (Vec<ChatMessage>, Vec<ToolSpec>) {
        let messages = vec![
            ChatMessage::User(vec![ChatPart::Text("Find socks".into())]),
            ChatMessage::Assistant {
                text: "Clicking search.".into(),
                calls: vec![ToolCall {
                    id: "call_1".into(),
                    name: "click".into(),
                    arguments: serde_json::json!({ "ref": 4 }),
                }],
            },
            ChatMessage::ToolResult {
                call_id: "call_1".into(),
                parts: vec![
                    ChatPart::Text("Clicked.".into()),
                    ChatPart::Image {
                        media_type: "image/jpeg".into(),
                        data: "AAAA".into(),
                    },
                ],
            },
            ChatMessage::User(vec![ChatPart::Text("Page now:".into())]),
        ];
        let tools = vec![ToolSpec {
            name: "click",
            description: "Click",
            parameters: serde_json::json!({ "type": "object", "properties": {} }),
        }];
        (messages, tools)
    }

    #[test]
    fn openai_tool_bodies_move_images_out_of_tool_messages() {
        let (messages, tools) = tool_history();
        let body = openai_tools_body("gpt-5", "sys", &messages, &tools);
        let list = body["messages"].as_array().unwrap();
        assert_eq!(list[0]["role"], "system");
        assert_eq!(
            list[2]["tool_calls"][0]["function"]["arguments"],
            "{\"ref\":4}"
        );
        assert_eq!(list[3]["role"], "tool");
        assert_eq!(list[3]["content"], "Clicked.");
        assert_eq!(list[4]["role"], "user");
        assert_eq!(list[4]["content"][1]["type"], "image_url");
        assert_eq!(list[5]["content"], "Page now:");
        assert_eq!(body["tools"][0]["function"]["name"], "click");
    }

    #[test]
    fn anthropic_tool_bodies_merge_user_turns() {
        let (messages, tools) = tool_history();
        let body = anthropic_tools_body("claude-sonnet-5-5", "sys", &messages, &tools);
        let list = body["messages"].as_array().unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[1]["content"][1]["type"], "tool_use");
        assert_eq!(list[2]["role"], "user");
        assert_eq!(list[2]["content"][0]["type"], "tool_result");
        assert_eq!(list[2]["content"][0]["content"][1]["type"], "image");
        assert_eq!(list[2]["content"][1]["text"], "Page now:");
        assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
    }

    #[test]
    fn tool_replies_parse_for_both_providers() {
        let openai = serde_json::json!({ "choices": [{ "message": {
            "content": null,
            "tool_calls": [{ "id": "a", "function": { "name": "type", "arguments": "{\"text\":\"hi\"}" } }]
        } }] });
        let reply = openai_tool_reply(&openai);
        assert_eq!(reply.calls[0].name, "type");
        assert_eq!(reply.calls[0].arguments["text"], "hi");
        let claude = serde_json::json!({ "content": [
            { "type": "text", "text": "Typing" },
            { "type": "tool_use", "id": "t1", "name": "type", "input": { "text": "hi" } }
        ] });
        let reply = anthropic_tool_reply(&claude);
        assert_eq!(reply.text, "Typing");
        assert_eq!(reply.calls[0].id, "t1");
    }

    #[test]
    fn openai_and_anthropic_bodies_match_each_provider() {
        let openai = openai_body("gpt-4o-mini", "Be brief.", "Hello", true);
        assert_eq!(openai["model"], "gpt-4o-mini");
        assert_eq!(openai["messages"][0]["role"], "system");
        assert_eq!(openai["messages"][1]["content"], "Hello");
        assert_eq!(openai["response_format"]["type"], "json_object");

        let claude = anthropic_body("claude-3-5-haiku-latest", "Be brief.", "Hello", true);
        assert_eq!(claude["model"], "claude-3-5-haiku-latest");
        assert_eq!(claude["max_tokens"], 1024);
        assert!(claude["system"].as_str().unwrap().contains("Be brief."));
        assert!(
            claude["system"]
                .as_str()
                .unwrap()
                .contains("one JSON object only")
        );
        assert!(claude.get("messages").unwrap()[0].get("content").is_some());
        assert!(claude.get("response_format").is_none());
    }

    #[test]
    fn anthropic_content_blocks_are_joined() {
        let value = serde_json::json!({
            "content": [
                { "type": "text", "text": "Hello " },
                { "type": "tool_use", "text": "skip" },
                { "type": "text", "text": "world" }
            ]
        });
        assert_eq!(anthropic_text(&value).as_deref(), Some("Hello \nworld"));
        let openai = serde_json::json!({
            "choices": [{ "message": { "content": "  ok  " } }]
        });
        assert_eq!(openai_text(&openai).as_deref(), Some("ok"));
    }

    #[test]
    fn request_urls_never_include_a_key() {
        let key = "sk-test-secret";
        let chat = chat_url("https://api.openai.com/v1/");
        let models = models_url("https://api.anthropic.com/v1");
        let messages = messages_url("http://127.0.0.1:11434/v1");
        let page = claude_models_url("https://api.anthropic.com/v1", Some("claude-sonnet-4-5"));
        assert_eq!(chat, "https://api.openai.com/v1/chat/completions");
        assert_eq!(models, "https://api.anthropic.com/v1/models");
        assert_eq!(messages, "http://127.0.0.1:11434/v1/messages");
        assert_eq!(
            page,
            "https://api.anthropic.com/v1/models?limit=100&after_id=claude-sonnet-4-5"
        );
        for url in [chat, models, messages, page] {
            assert!(!url.contains(key));
            assert!(!url.contains("Bearer"));
        }
        let body = openai_body("gpt-4o-mini", "sys", "user", false).to_string();
        assert!(!body.contains(key));
    }

    #[test]
    fn provider_errors_stay_plain() {
        assert_eq!(
            http_error(401, "https://api.openai.com/v1", ""),
            "The API key was rejected."
        );
        assert_eq!(
            http_error(403, "https://api.openai.com/v1", ""),
            "The API key was rejected."
        );
        assert_eq!(
            http_error(429, "https://api.openai.com/v1", ""),
            "The provider is rate limited. Try again in a minute."
        );
        assert_eq!(
            http_error(404, "https://api.openai.com/v1", ""),
            "That model or endpoint was not found."
        );
        assert_eq!(
            http_error(
                400,
                "https://api.openai.com/v1",
                r#"{"error":{"message":"max_tokens is too large","type":"invalid_request_error"}}"#
            ),
            "The provider returned an error (400).\nmax_tokens is too large"
        );
        assert_eq!(
            http_error(
                400,
                "http://127.0.0.1:11434/v1",
                r#"{"error":"model 'missing' not found"}"#
            ),
            "The provider returned an error (400).\nmodel 'missing' not found"
        );
        assert_eq!(
            http_error(
                502,
                "https://api.openai.com/v1",
                "<html><body>bad gateway</body></html>"
            ),
            "The provider returned an error (502)."
        );
        let long = "x".repeat(2_000);
        let body = format!(r#"{{"message":"{long}"}}"#);
        let message = http_error(400, "https://api.openai.com/v1", &body);
        assert!(message.starts_with("The provider returned an error (400).\n"));
        assert!(message.ends_with('…'));
        assert!(message.chars().count() < 1_400);
        assert_eq!(
            transport_error(
                "http://127.0.0.1:11434/v1",
                "Connection refused (os error 10061)"
            ),
            "The local server is not running."
        );
        assert_eq!(
            transport_error("https://api.openai.com/v1", "Connection refused"),
            "Couldn't reach the assistant."
        );
    }

    #[test]
    fn model_lists_keep_ids_only() {
        let ids = model_ids(r#"{"data":[{"id":"b"},{"id":"a"},{"id":"b"}]}"#);
        assert_eq!(ids, vec!["a".to_string(), "b".to_string()]);
        let page = model_page(
            r#"{"data":[{"id":"claude-sonnet-4-5"}],"has_more":true,"last_id":"claude-sonnet-4-5"}"#,
        );
        assert_eq!(page.ids, vec!["claude-sonnet-4-5".to_string()]);
        assert!(page.has_more);
        assert_eq!(page.last_id.as_deref(), Some("claude-sonnet-4-5"));
        assert!(
            model_page(r#"{"has_more":true,"last_id":"bad id"}"#)
                .last_id
                .is_none()
        );
    }

    #[test]
    fn text_chat_models_skip_image_audio_and_embeddings() {
        for id in [
            "gpt-4o-mini",
            "gpt-4.1",
            "gpt-5",
            "gpt-4o-2024-08-06",
            "o1",
            "o3-mini",
            "o4-mini",
            "chatgpt-4o-latest",
            "ft:gpt-4o-mini:org:name:abc",
        ] {
            assert!(text_chat_model("openai", id), "{id}");
        }
        for id in [
            "text-embedding-3-small",
            "whisper-1",
            "tts-1",
            "dall-e-3",
            "gpt-image-1",
            "gpt-4o-mini-tts",
            "gpt-4o-transcribe",
            "gpt-4o-realtime-preview",
            "gpt-4o-audio-preview",
            "omni-moderation-latest",
            "davinci-002",
            "gpt-3.5-turbo-instruct",
            "computer-use-preview",
        ] {
            assert!(!text_chat_model("openai", id), "{id}");
        }
        assert!(text_chat_model("claude", "claude-sonnet-4-5"));
        assert!(text_chat_model("claude", "claude-3-5-haiku-latest"));
        assert!(!text_chat_model("claude", "embed-english"));
        assert!(text_chat_model("local", "llama3.2"));
    }

    #[test]
    fn recent_text_models_are_the_six_newest_families() {
        let entries = vec![
            ("gpt-4o-mini".into(), 1),
            ("gpt-4o".into(), 2),
            ("gpt-5-nano".into(), 3),
            ("gpt-5-mini".into(), 4),
            ("gpt-5".into(), 5),
            ("gpt-5.4".into(), 6),
            ("gpt-5.6-luna".into(), 7),
            ("gpt-5.6-sol-20260901".into(), 8),
            ("gpt-5.6-sol".into(), 8),
            ("gpt-5.6-cyber".into(), 9),
            ("text-embedding-3-small".into(), 10),
            ("ft:gpt-5:org:name:abc".into(), 11),
            ("claude-haiku-3-5".into(), 1),
            ("claude-sonnet-5-5-20260928".into(), 12),
        ];
        let ids = recent_text_models(&entries);
        assert_eq!(
            ids,
            vec![
                "claude-sonnet-5-5-20260928".to_string(),
                "gpt-5.6-sol".to_string(),
                "gpt-5.6-luna".to_string(),
                "gpt-5.4".to_string(),
                "gpt-5".to_string(),
                "gpt-5-mini".to_string(),
            ]
        );
    }

    #[test]
    fn thinking_is_sent_only_for_models_that_accept_it() {
        assert!(model_accepts_thinking("openai", "gpt-5.6-sol"));
        assert!(model_accepts_thinking("openai", "gpt-6-astra"));
        assert!(model_accepts_thinking("openai", "o3-mini"));
        assert!(!model_accepts_thinking("openai", "gpt-4o-mini"));
        assert!(!model_accepts_thinking("openai", "gpt-5-chat-latest"));
        assert!(model_accepts_thinking("claude", "claude-sonnet-5-5"));
        assert!(model_accepts_thinking("claude", "claude-haiku-4-5"));
        assert!(!model_accepts_thinking("claude", "claude-3-5-haiku-latest"));
        assert!(!model_accepts_thinking("local", "llama3.2"));

        let mut openai = openai_body("gpt-5.6-sol", "sys", "user", false);
        attach_thinking("openai", "gpt-5.6-sol", "high", &mut openai);
        assert_eq!(openai["reasoning_effort"], "high");

        let mut skipped = openai_body("gpt-4o-mini", "sys", "user", false);
        attach_thinking("openai", "gpt-4o-mini", "high", &mut skipped);
        assert!(skipped.get("reasoning_effort").is_none());

        let mut claude = anthropic_body("claude-sonnet-5-5", "sys", "user", false);
        attach_thinking("claude", "claude-sonnet-5-5", "low", &mut claude);
        assert_eq!(claude["output_config"]["effort"], "low");
        assert_eq!(claude["max_tokens"], 8192);

        let mut untouched = anthropic_body("claude-sonnet-5-5", "sys", "user", false);
        attach_thinking("claude", "claude-sonnet-5-5", "", &mut untouched);
        assert!(untouched.get("output_config").is_none());
        assert_eq!(untouched["max_tokens"], 1024);

        assert!(chat_tools_require_reasoning_none("gpt-6-sol"));
        assert!(chat_tools_require_reasoning_none("gpt-6-sol-20261106"));
        assert!(chat_tools_require_reasoning_none("gpt-6-luna"));
        assert!(chat_tools_require_reasoning_none("gpt-5.4-mini"));
        assert!(chat_tools_require_reasoning_none("gpt-5.6-sol"));
        assert!(!chat_tools_require_reasoning_none("gpt-5"));
        assert!(!chat_tools_require_reasoning_none("gpt-4o-mini"));
        assert!(!chat_tools_require_reasoning_none("gpt-6-astra"));
        assert!(!chat_tools_require_reasoning_none("gpt-6.1-sol"));
        assert!(!chat_tools_require_reasoning_none("o3-mini"));

        let mut tools = serde_json::json!({});
        attach_tool_thinking("openai", "gpt-6-sol", "high", &mut tools);
        assert_eq!(tools["reasoning_effort"], "none");
        let mut older = serde_json::json!({});
        attach_tool_thinking("openai", "gpt-5", "high", &mut older);
        assert_eq!(older["reasoning_effort"], "high");
        let mut plain = openai_body("gpt-6-sol", "sys", "user", false);
        attach_thinking("openai", "gpt-6-sol", "high", &mut plain);
        assert_eq!(plain["reasoning_effort"], "high");
    }
}
