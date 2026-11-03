//! Shopping summary providers. Price tracking never calls this module.

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
    let mut ids = Vec::new();
    for item in items {
        let id = item
            .get("id")
            .and_then(|item| item.as_str())
            .or_else(|| item.get("name").and_then(|item| item.as_str()))
            .unwrap_or("")
            .trim();
        if id.is_empty() || ids.iter().any(|kept: &String| kept == id) {
            continue;
        }
        ids.push(id.to_string());
        if ids.len() >= 200 {
            break;
        }
    }
    ids.sort();
    ids
}

pub fn http_error(status: u16, endpoint: &str) -> String {
    match status {
        401 | 403 => "The API key was rejected.".into(),
        429 => "The provider is rate limited. Try again in a minute.".into(),
        404 => "That model or endpoint was not found.".into(),
        _ if localhost_endpoint(endpoint) && status == 0 => "The local server is not running.".into(),
        _ => format!("The provider returned an error ({status})."),
    }
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
        return Err("Set an assistant endpoint in Settings, under Tools.".into());
    }
    let model = model_of(current, model);
    if model.is_empty() {
        return Err("Set a model in Settings, under Tools.".into());
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
    let body = if provider == "claude" {
        anthropic_body(&model, system, user, json_only)
    } else {
        openai_body(&model, system, user, json_only)
    };
    let text = send(provider, &url, &endpoint, &key, &body, timeout)?;
    let value: Value = serde_json::from_str(&text).map_err(|_| "The assistant returned nothing.".to_string())?;
    let reply = if provider == "claude" {
        anthropic_text(&value)
    } else {
        openai_text(&value)
    };
    reply.ok_or_else(|| "The assistant returned nothing.".to_string())
}

pub fn list_models(app: &AppHandle) -> Result<Vec<String>, String> {
    let current = settings::get(app);
    let provider = provider_id(&current);
    let endpoint = endpoint_of(&current);
    if endpoint.is_empty() {
        return Err("Set an assistant endpoint in Settings, under Tools.".into());
    }
    let key = request_key(app, provider, &endpoint)?;
    let url = models_url(&endpoint);
    if !key.is_empty() && url.contains(&key) {
        return Err("Couldn't reach the assistant.".into());
    }
    let text = get_text(provider, &url, &endpoint, &key, Duration::from_secs(20))?;
    Ok(model_ids(&text))
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
        Ok(None) => Err("This endpoint needs an API key. Add it in Settings, under Tools.".into()),
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
    let payload = serde_json::to_string(body).map_err(|_| "Couldn't reach the assistant.".to_string())?;
    let mut request = ureq::post(url)
        .timeout(timeout)
        .set("Content-Type", "application/json");
    request = authorize(request, provider, key);
    match request.send_string(&payload) {
        Ok(response) => read_body(response),
        Err(ureq::Error::Status(status, response)) => {
            let _ = response.into_string();
            Err(http_error(status, endpoint))
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
            let _ = response.into_string();
            Err(http_error(status, endpoint))
        }
        Err(ureq::Error::Transport(error)) => Err(transport_error(endpoint, &error.to_string())),
    }
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
        anthropic_body, anthropic_text, chat_url, http_error, messages_url, model_ids, models_url,
        openai_body, openai_text, transport_error,
    };

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
        assert!(claude["system"].as_str().unwrap().contains("one JSON object only"));
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
        assert_eq!(chat, "https://api.openai.com/v1/chat/completions");
        assert_eq!(models, "https://api.anthropic.com/v1/models");
        assert_eq!(messages, "http://127.0.0.1:11434/v1/messages");
        for url in [chat, models, messages] {
            assert!(!url.contains(key));
            assert!(!url.contains("Bearer"));
        }
        let body = openai_body("gpt-4o-mini", "sys", "user", false).to_string();
        assert!(!body.contains(key));
    }

    #[test]
    fn provider_errors_stay_plain() {
        assert_eq!(http_error(401, "https://api.openai.com/v1"), "The API key was rejected.");
        assert_eq!(http_error(403, "https://api.openai.com/v1"), "The API key was rejected.");
        assert_eq!(
            http_error(429, "https://api.openai.com/v1"),
            "The provider is rate limited. Try again in a minute."
        );
        assert_eq!(
            http_error(404, "https://api.openai.com/v1"),
            "That model or endpoint was not found."
        );
        assert_eq!(
            transport_error("http://127.0.0.1:11434/v1", "Connection refused (os error 10061)"),
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
    }
}
