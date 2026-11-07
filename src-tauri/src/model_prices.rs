//! Model prices for the agent panel's model picker, from OpenRouter's public model list.
//! Kept for a day in `model-prices.json`, so an offline start still shows the last prices.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::AppHandle;

use crate::{ai, files, paths, settings};

const SOURCE: &str = "https://openrouter.ai/api/v1/models";
const FILE: &str = "model-prices.json";
const MAX_AGE_MS: u64 = 24 * 60 * 60 * 1000;
/// Wait before asking OpenRouter again after a failed request.
const RETRY_MS: u64 = 10 * 60 * 1000;

/// US dollars per million tokens.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ModelPrice {
    pub input: f64,
    pub output: f64,
    /// Runs on this computer, so it costs nothing per token.
    pub local: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Cache {
    fetched: u64,
    /// Keyed by `vendor/model`, normalized.
    prices: BTreeMap<String, ModelPrice>,
}

struct State {
    cache: Option<Cache>,
    failed: u64,
}

static STATE: Mutex<State> = Mutex::new(State {
    cache: None,
    failed: 0,
});

fn now_ms() -> u64 {
    crate::automation::now_ms()
}

/// Lowercase, dots as dashes, and without `-latest` or a date snapshot, so
/// `claude-sonnet-5-5-20260928` and `anthropic/claude-sonnet-5.5` meet.
fn normalize(id: &str) -> String {
    let mut id = id.trim().to_ascii_lowercase().replace('.', "-");
    if let Some(stripped) = id.strip_suffix("-latest") {
        id = stripped.to_string();
    }
    if let Some((head, tail)) = id.rsplit_once('-')
        && tail.len() == 8
        && tail.chars().all(|ch| ch.is_ascii_digit())
    {
        id = head.to_string();
    }
    let bytes = id.as_bytes();
    if id.is_ascii()
        && bytes.len() > 11
        && bytes[bytes.len() - 11] == b'-'
        && id[id.len() - 10..]
            .split('-')
            .map(|part| part.len())
            .eq([4, 2, 2])
        && id[id.len() - 10..].chars().all(|ch| ch.is_ascii_digit() || ch == '-')
    {
        id.truncate(id.len() - 11);
    }
    id
}

fn per_million(value: Option<&Value>) -> Option<f64> {
    let value = value?;
    let per_token = value
        .as_str()
        .and_then(|text| text.trim().parse::<f64>().ok())
        .or_else(|| value.as_f64())?;
    (per_token >= 0.0).then_some(per_token * 1_000_000.0)
}

fn parse(body: &str) -> BTreeMap<String, ModelPrice> {
    let mut prices = BTreeMap::new();
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return prices;
    };
    for item in value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(id) = item.get("id").and_then(Value::as_str) else {
            continue;
        };
        // `:batch`, `:free` and similar are other ways to buy the same model.
        if id.contains(':') || !id.contains('/') {
            continue;
        }
        let pricing = item.get("pricing");
        let (Some(input), Some(output)) = (
            per_million(pricing.and_then(|p| p.get("prompt"))),
            per_million(pricing.and_then(|p| p.get("completion"))),
        ) else {
            continue;
        };
        prices.insert(
            normalize(id),
            ModelPrice {
                input,
                output,
                local: false,
            },
        );
    }
    prices
}

fn fetch() -> Result<Cache, String> {
    let body = ureq::get(SOURCE)
        .timeout(Duration::from_secs(15))
        .call()
        .map_err(|error| error.to_string())?
        .into_string()
        .map_err(|error| error.to_string())?;
    let prices = parse(&body);
    if prices.is_empty() {
        return Err("No prices in the reply.".into());
    }
    Ok(Cache {
        fetched: now_ms(),
        prices,
    })
}

fn current(app: &AppHandle) -> Cache {
    let path = paths::data_file(app, FILE);
    let now = now_ms();
    {
        let mut state = STATE.lock().expect("model prices lock");
        if state.cache.is_none() {
            let saved: Cache = files::read_json(&path);
            if !saved.prices.is_empty() {
                state.cache = Some(saved);
            }
        }
        let fresh = state
            .cache
            .as_ref()
            .is_some_and(|cache| now.saturating_sub(cache.fetched) < MAX_AGE_MS);
        if fresh || now.saturating_sub(state.failed) < RETRY_MS {
            return state.cache.clone().unwrap_or_default();
        }
    }
    match fetch() {
        Ok(cache) => {
            if let Err(error) = files::write_json(&path, &cache) {
                eprintln!("model prices: {error}");
            }
            let mut state = STATE.lock().expect("model prices lock");
            state.cache = Some(cache.clone());
            cache
        }
        Err(error) => {
            eprintln!("model prices: {error}");
            let mut state = STATE.lock().expect("model prices lock");
            state.failed = now;
            state.cache.clone().unwrap_or_default()
        }
    }
}

fn vendor(provider: &str) -> Option<&'static str> {
    match provider {
        "openai" => Some("openai"),
        "claude" => Some("anthropic"),
        _ => None,
    }
}

/// Prices for these model ids with the active AI provider. Models without a known price are
/// left out.
pub fn prices(app: &AppHandle, ids: &[String]) -> BTreeMap<String, ModelPrice> {
    let provider = ai::provider_id(&settings::get(app));
    if provider == "local" {
        return ids
            .iter()
            .map(|id| {
                (
                    id.clone(),
                    ModelPrice {
                        local: true,
                        ..ModelPrice::default()
                    },
                )
            })
            .collect();
    }
    let cache = current(app);
    let mut out = BTreeMap::new();
    for id in ids {
        let key = normalize(id);
        let price = match vendor(provider) {
            Some(vendor) => cache.prices.get(&format!("{vendor}/{key}")),
            None => cache.prices.get(&key).or_else(|| {
                cache
                    .prices
                    .iter()
                    .find(|(name, _)| name.split_once('/').is_some_and(|(_, model)| model == key))
                    .map(|(_, price)| price)
            }),
        };
        if let Some(price) = price {
            out.insert(id.clone(), *price);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_meet_across_naming_styles() {
        assert_eq!(normalize("claude-sonnet-5-5-20260928"), "claude-sonnet-5-5");
        assert_eq!(normalize("anthropic/claude-sonnet-5.5"), "anthropic/claude-sonnet-5-5");
        assert_eq!(normalize("gpt-4o-2024-08-06"), "gpt-4o");
        assert_eq!(normalize("claude-3-5-haiku-latest"), "claude-3-5-haiku");
        assert_eq!(normalize("gpt-6.1-sol"), "gpt-6-1-sol");
    }

    #[test]
    fn prices_are_per_million_tokens() {
        let body = r#"{"data":[
            {"id":"openai/gpt-6-sol","pricing":{"prompt":"0.000002","completion":"0.00001"}},
            {"id":"openai/gpt-6-sol:batch","pricing":{"prompt":"0.000001","completion":"0.000005"}}
        ]}"#;
        let prices = parse(body);
        assert_eq!(prices.len(), 1);
        let price = prices["openai/gpt-6-sol"];
        assert!((price.input - 2.0).abs() < 1e-9);
        assert!((price.output - 10.0).abs() < 1e-9);
    }
}
