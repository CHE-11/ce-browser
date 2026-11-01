//! Translate, passwords, prices, the Internet Archive, readability, and the page assistant.

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{files, paths, settings, tabs, webview2};

pub const DEFAULT_TRANSLATE_URL: &str =
    "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl=%t&dt=t&q=%s";
const TEXT_FAILURE: &str = "Couldn't read this page.";

static TRANSLATED: Mutex<Option<HashSet<u32>>> = Mutex::new(None);

fn translated() -> std::sync::MutexGuard<'static, Option<HashSet<u32>>> {
    let mut guard = TRANSLATED.lock().expect("translated");
    if guard.is_none() {
        *guard = Some(HashSet::new());
    }
    guard
}

pub fn translated_now(id: u32) -> bool {
    translated().as_ref().is_some_and(|set| set.contains(&id))
}

pub fn fill_translate_url(template: &str, text: &str, language: &str) -> String {
    let template = if template.trim().is_empty() { DEFAULT_TRANSLATE_URL } else { template };
    let text: String = url::form_urlencoded::byte_serialize(text.as_bytes()).collect();
    let language: String = url::form_urlencoded::byte_serialize(language.as_bytes()).collect();
    template.replace("%s", &text).replace("%t", &language)
}

pub fn restore_page(app: &AppHandle, id: u32) -> Result<(), String> {
    translated().as_mut().map(|set| set.remove(&id));
    tabs::reload(app, id)
}

pub fn translate_page(app: &AppHandle, id: u32, language: &str) -> Result<(), String> {
    if translated_now(id) {
        return restore_page(app, id);
    }
    let language = clean_language(language);
    let template = settings::get(app).translate_url;
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let value = webview2::evaluate(&webview, COLLECT_SCRIPT, Duration::from_secs(8), TEXT_FAILURE)?;
    let blocks: Vec<String> = serde_json::from_value(value).unwrap_or_default();
    if blocks.is_empty() {
        return Err("This page has no article text to translate.".into());
    }
    let mut pairs = Vec::new();
    for block in blocks.into_iter().take(40) {
        let translated = translate_text(&template, &block, &language)?;
        if translated != block {
            pairs.push((block, translated));
        }
    }
    if pairs.is_empty() {
        return Err("The provider returned the same text.".into());
    }
    let payload = serde_json::to_string(&pairs).unwrap_or_else(|_| "[]".into());
    let script = format!(
        r#"(function(pairs) {{
          var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
          var node;
          while ((node = walker.nextNode())) {{
            var value = node.nodeValue || "";
            for (var i = 0; i < pairs.length; i++) {{
              if (value.indexOf(pairs[i][0]) !== -1) value = value.split(pairs[i][0]).join(pairs[i][1]);
            }}
            node.nodeValue = value;
          }}
          return true;
        }})({payload})"#
    );
    webview2::evaluate(&webview, &script, Duration::from_secs(8), TEXT_FAILURE)?;
    translated().as_mut().map(|set| set.insert(id));
    Ok(())
}

fn translate_text(template: &str, text: &str, language: &str) -> Result<String, String> {
    let url = fill_translate_url(template, text, language);
    let body = ureq::get(&url)
        .timeout(Duration::from_secs(20))
        .set("User-Agent", "Mozilla/5.0")
        .call()
        .map_err(|error| format!("Couldn't reach the translate provider. {error}"))?
        .into_string()
        .map_err(|error| error.to_string())?;
    read_translation(&body)
}

fn read_translation(body: &str) -> Result<String, String> {
    let trimmed = body.trim();
    if trimmed.starts_with('[') {
        let value: serde_json::Value = serde_json::from_str(trimmed).map_err(|error| error.to_string())?;
        let mut out = String::new();
        if let Some(sentences) = value.get(0).and_then(|item| item.as_array()) {
            for sentence in sentences {
                if let Some(text) = sentence.get(0).and_then(|item| item.get(0)).and_then(|item| item.as_str()) {
                    out.push_str(text);
                }
            }
        }
        if !out.is_empty() {
            return Ok(out);
        }
    }
    if trimmed.starts_with('{') {
        let value: serde_json::Value = serde_json::from_str(trimmed).map_err(|error| error.to_string())?;
        if let Some(text) = value
            .get("translatedText")
            .or_else(|| value.get("text"))
            .and_then(|item| item.as_str())
        {
            return Ok(text.to_string());
        }
    }
    if trimmed.is_empty() {
        Err("The provider returned nothing.".into())
    } else {
        Ok(trimmed.to_string())
    }
}

fn clean_language(language: &str) -> String {
    let cleaned: String = language
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .take(12)
        .collect();
    if cleaned.is_empty() { "en" } else { &cleaned }.to_string()
}

const COLLECT_SCRIPT: &str = r#"(function () {
  var nodes = document.querySelectorAll("h1,h2,h3,p,li,blockquote");
  var out = [];
  var seen = {};
  for (var i = 0; i < nodes.length && out.length < 40; i++) {
    var text = (nodes[i].innerText || "").replace(/\s+/g, " ").trim();
    if (text.length < 2 || text.length > 500 || seen[text]) continue;
    seen[text] = true;
    out.push(text);
  }
  return out;
})()"#;

pub fn password(length: usize, upper: bool, lower: bool, digits: bool, symbols: bool, random: &[u8]) -> Result<String, String> {
    let length = length.clamp(8, 64);
    let mut alphabet = String::new();
    let mut required = Vec::new();
    if upper {
        alphabet.push_str("ABCDEFGHIJKLMNOPQRSTUVWXYZ");
        required.push("ABCDEFGHIJKLMNOPQRSTUVWXYZ");
    }
    if lower {
        alphabet.push_str("abcdefghijklmnopqrstuvwxyz");
        required.push("abcdefghijklmnopqrstuvwxyz");
    }
    if digits {
        alphabet.push_str("0123456789");
        required.push("0123456789");
    }
    if symbols {
        alphabet.push_str("!@#$%^&*-_=+");
        required.push("!@#$%^&*-_=+");
    }
    if alphabet.is_empty() {
        return Err("Pick at least one kind of character.".into());
    }
    if random.is_empty() {
        return Err("Couldn't generate a password.".into());
    }
    let chars: Vec<char> = alphabet.chars().collect();
    let mut out: Vec<char> = (0..length)
        .map(|index| chars[random[index % random.len()] as usize % chars.len()])
        .collect();
    for (index, class) in required.iter().enumerate() {
        let class: Vec<char> = class.chars().collect();
        let pick = random[(length + index) % random.len()] as usize % class.len();
        out[index] = class[pick];
    }
    Ok(out.into_iter().collect())
}

pub fn random_password(length: usize, upper: bool, lower: bool, digits: bool, symbols: bool) -> Result<String, String> {
    let mut bytes = [0u8; 64];
    fill_random(&mut bytes)?;
    password(length, upper, lower, digits, symbols, &bytes)
}

fn fill_random(bytes: &mut [u8]) -> Result<(), String> {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn SystemFunction036(buffer: *mut u8, length: u32) -> u8;
    }
    let ok = unsafe { SystemFunction036(bytes.as_mut_ptr(), bytes.len() as u32) };
    if ok == 0 {
        Err("Couldn't generate a password.".into())
    } else {
        Ok(())
    }
}

pub fn wayback_url(save: bool, url: &str) -> Result<String, String> {
    let url = url.trim();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Only a website can be looked up in the archive.".into());
    }
    let prefix = if save {
        "https://web.archive.org/save/"
    } else {
        "https://web.archive.org/web/"
    };
    Ok(format!("{prefix}{url}"))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingStats {
    pub words: u32,
    pub minutes: u32,
    pub ease: u32,
}

pub fn reading_stats(text: &str) -> ReadingStats {
    let words: Vec<&str> = text.split_whitespace().filter(|word| !word.is_empty()).collect();
    let word_count = words.len() as u32;
    let sentences = text.chars().filter(|character| matches!(character, '.' | '!' | '?')).count().max(1) as f64;
    let syllables: u32 = words.iter().map(|word| syllables(word)).sum();
    let ease = if word_count == 0 {
        0.0
    } else {
        206.835 - 1.015 * (f64::from(word_count) / sentences) - 84.6 * (f64::from(syllables) / f64::from(word_count))
    };
    ReadingStats {
        words: word_count,
        minutes: (word_count / 200).max(if word_count == 0 { 0 } else { 1 }),
        ease: ease.round().clamp(0.0, 100.0) as u32,
    }
}

fn syllables(word: &str) -> u32 {
    let word = word.to_ascii_lowercase();
    let mut groups = 0u32;
    let mut in_group = false;
    for character in word.chars() {
        let vowel = matches!(character, 'a' | 'e' | 'i' | 'o' | 'u' | 'y');
        if vowel && !in_group {
            groups += 1;
        }
        in_group = vowel;
    }
    if word.ends_with('e') && groups > 1 {
        groups -= 1;
    }
    groups.max(1)
}

pub fn page_reading(app: &AppHandle, id: u32) -> Result<ReadingStats, String> {
    let page = crate::print::extract_page(app, id)?;
    Ok(reading_stats(&plain_text(&page.html)))
}

fn plain_text(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        out.push_str(rest[..start].trim());
        out.push(' ');
        let Some(end) = rest[start..].find('>') else { break };
        let tag = &rest[start + 1..start + end];
        let name = tag.split_whitespace().next().unwrap_or("").trim_start_matches('/').to_ascii_lowercase();
        rest = &rest[start + end + 1..];
        if matches!(name.as_str(), "script" | "style") && !tag.starts_with('/') {
            if let Some(close) = rest.to_ascii_lowercase().find(&format!("</{name}>")) {
                rest = &rest[close + name.len() + 3..];
            }
        }
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ProductPrice {
    pub url: String,
    pub title: String,
    pub price: f64,
    pub currency: String,
    pub availability: String,
    pub previous_price: Option<f64>,
    pub previous_availability: String,
    pub dropped: bool,
    pub stock_changed: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct PriceFile {
    items: Vec<ProductPrice>,
}

const PRICES: &str = "prices.json";

pub fn parse_price(html: &str) -> Option<ProductPrice> {
    if let Some(price) = price_from_json_ld(html) {
        return Some(price);
    }
    let amount = meta_content(html, "product:price:amount")?;
    let price = amount.parse().ok()?;
    Some(ProductPrice {
        url: String::new(),
        title: String::new(),
        price,
        currency: meta_content(html, "product:price:currency").unwrap_or_default(),
        availability: String::new(),
        previous_price: None,
        previous_availability: String::new(),
        dropped: false,
        stock_changed: false,
    })
}

fn price_from_json_ld(html: &str) -> Option<ProductPrice> {
    let lower = html.to_ascii_lowercase();
    let mut search = html;
    let mut search_lower = lower.as_str();
    while let Some(index) = search_lower.find("application/ld+json") {
        search = &search[index..];
        search_lower = &search_lower[index..];
        let start = search.find('>')? + 1;
        let end = search_lower[start..].find("</script>")? + start;
        let json = &search[start..end];
        if let Some(price) = price_in_json(json) {
            return Some(price);
        }
        search = &search[end..];
        search_lower = &search_lower[end..];
    }
    None
}

fn price_in_json(json: &str) -> Option<ProductPrice> {
    let value: serde_json::Value = serde_json::from_str(json.trim()).ok()?;
    find_product(&value)
}

fn find_product(value: &serde_json::Value) -> Option<ProductPrice> {
    if let Some(array) = value.as_array() {
        for item in array {
            if let Some(price) = find_product(item) {
                return Some(price);
            }
        }
    }
    let object = value.as_object()?;
    let kind = object.get("@type").and_then(|item| item.as_str()).unwrap_or("");
    if kind == "Product" || object.contains_key("offers") {
        if let Some(offers) = object.get("offers") {
            let offer = offers.as_array().and_then(|items| items.first()).unwrap_or(offers);
            if let Some(price) = offer.get("price").and_then(json_price) {
                return Some(ProductPrice {
                    url: String::new(),
                    title: object.get("name").and_then(|item| item.as_str()).unwrap_or("").to_string(),
                    price,
                    currency: offer.get("priceCurrency").and_then(|item| item.as_str()).unwrap_or("").to_string(),
                    availability: offer
                        .get("availability")
                        .and_then(|item| item.as_str())
                        .unwrap_or("")
                        .rsplit('/')
                        .next()
                        .unwrap_or("")
                        .to_string(),
                    previous_price: None,
                    previous_availability: String::new(),
                    dropped: false,
                    stock_changed: false,
                });
            }
        }
    }
    for item in object.values() {
        if item.is_object() || item.is_array() {
            if let Some(price) = find_product(item) {
                return Some(price);
            }
        }
    }
    None
}

fn json_price(value: &serde_json::Value) -> Option<f64> {
    value.as_f64().or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

fn meta_content(html: &str, property: &str) -> Option<String> {
    let needle = format!("property=\"{property}\"");
    let index = html.find(&needle).or_else(|| html.find(&format!("property='{property}'")))?;
    let rest = &html[index..];
    let content = rest.find("content=")? + "content=".len();
    let quote = rest[content..].chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let start = content + 1;
    let end = rest[start..].find(quote)? + start;
    Some(rest[start..end].to_string())
}

const PRICE_SCRIPT: &str = r#"(function () {
  var scripts = document.querySelectorAll('script[type="application/ld+json"]');
  var blocks = [];
  scripts.forEach(function (script) { blocks.push(script.textContent || ""); });
  var amount = document.querySelector('meta[property="product:price:amount"]');
  var currency = document.querySelector('meta[property="product:price:currency"]');
  return {
    blocks: blocks,
    amount: amount ? amount.content : "",
    currency: currency ? currency.content : "",
    title: document.title || ""
  };
})()"#;

pub fn read_page_price(app: &AppHandle, id: u32) -> Result<ProductPrice, String> {
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let url = tabs::tab_page_url(app, id).unwrap_or_default();
    let value = webview2::evaluate(&webview, PRICE_SCRIPT, Duration::from_secs(8), "Couldn't read a price on this page.")?;
    let title = value.get("title").and_then(|item| item.as_str()).unwrap_or("").to_string();
    let mut html = String::new();
    if let Some(blocks) = value.get("blocks").and_then(|item| item.as_array()) {
        for block in blocks {
            if let Some(text) = block.as_str() {
                html.push_str("<script type=\"application/ld+json\">");
                html.push_str(text);
                html.push_str("</script>");
            }
        }
    }
    if let Some(amount) = value.get("amount").and_then(|item| item.as_str()) {
        if !amount.is_empty() {
            html.push_str(&format!(
                "<meta property=\"product:price:amount\" content=\"{amount}\"><meta property=\"product:price:currency\" content=\"{}\">",
                value.get("currency").and_then(|item| item.as_str()).unwrap_or("")
            ));
        }
    }
    let mut price = parse_price(&html).ok_or_else(|| "No product price was found on this page.".to_string())?;
    price.url = url;
    if price.title.is_empty() {
        price.title = title;
    }
    Ok(price)
}

pub fn list_prices(app: &AppHandle) -> Vec<ProductPrice> {
    let file: PriceFile = files::read_json(&paths::data_file(app, PRICES));
    file.items
}

pub fn track_price(app: &AppHandle, id: u32) -> Result<ProductPrice, String> {
    let price = read_page_price(app, id)?;
    let mut items = list_prices(app);
    if let Some(existing) = items.iter_mut().find(|item| item.url == price.url) {
        existing.previous_price = Some(existing.price);
        existing.previous_availability = existing.availability.clone();
        existing.dropped = price.price < existing.price;
        existing.stock_changed = !price.availability.is_empty() && price.availability != existing.availability;
        existing.price = price.price;
        existing.currency = price.currency.clone();
        existing.availability = price.availability.clone();
        existing.title = price.title.clone();
    } else {
        items.insert(0, price.clone());
    }
    save_prices(app, &items)?;
    Ok(items.into_iter().find(|item| item.url == price.url).unwrap_or(price))
}

pub fn check_prices(app: &AppHandle) -> Result<Vec<ProductPrice>, String> {
    let mut items = list_prices(app);
    let open = tabs::snapshot(app).tabs.into_iter().filter(|tab| tab.loaded).map(|tab| tab.id).collect::<Vec<_>>();
    for item in &mut items {
        let Some(id) = open.iter().copied().find(|id| tabs::tab_page_url(app, *id).as_deref() == Some(item.url.as_str())) else {
            continue;
        };
        let Ok(fresh) = read_page_price(app, id) else { continue };
        item.dropped = fresh.price < item.price;
        item.stock_changed = !fresh.availability.is_empty() && fresh.availability != item.availability;
        item.previous_price = Some(item.price);
        item.previous_availability = item.availability.clone();
        item.price = fresh.price;
        item.currency = fresh.currency;
        item.availability = fresh.availability;
    }
    save_prices(app, &items)?;
    Ok(items)
}

fn save_prices(app: &AppHandle, items: &[ProductPrice]) -> Result<(), String> {
    files::write_json(&paths::data_file(app, PRICES), &PriceFile { items: items.to_vec() })
}

pub fn chat_url(endpoint: &str) -> String {
    format!("{}/chat/completions", endpoint.trim().trim_end_matches('/'))
}

pub fn localhost_endpoint(endpoint: &str) -> bool {
    let lower = endpoint.to_ascii_lowercase();
    lower.contains("://127.0.0.1") || lower.contains("://localhost") || lower.contains("://[::1]")
}

pub fn ask_page(app: &AppHandle, id: u32, question: &str) -> Result<String, String> {
    let current = settings::get(app);
    let endpoint = current.assistant_endpoint.trim();
    if endpoint.is_empty() {
        return Err("Set an assistant endpoint in Settings, under Tools.".into());
    }
    if current.assistant_key.trim().is_empty() && !localhost_endpoint(endpoint) {
        return Err("This endpoint needs an API key. Add it in Settings, under Tools.".into());
    }
    let page = crate::print::extract_page(app, id)?;
    let mut article = plain_text(&page.html);
    if article.chars().count() > 12_000 {
        article = article.chars().take(12_000).collect();
    }
    let question = question.trim();
    let user = if question.is_empty() {
        format!("Summarize this page.\n\nTitle: {}\n\n{article}", page.title)
    } else {
        format!("Answer using only this page.\n\nQuestion: {question}\n\nTitle: {}\n\n{article}", page.title)
    };
    let url = chat_url(endpoint);
    let body = serde_json::json!({
        "model": if current.assistant_model.trim().is_empty() { "llama3.2" } else { current.assistant_model.trim() },
        "messages": [
            { "role": "system", "content": "You answer briefly about the web page the user is reading." },
            { "role": "user", "content": user }
        ]
    });
    let mut request = ureq::post(&url).timeout(Duration::from_secs(60)).set("Content-Type", "application/json");
    if !current.assistant_key.trim().is_empty() {
        request = request.set("Authorization", &format!("Bearer {}", current.assistant_key.trim()));
    }
    let response = request
        .send_string(&serde_json::to_string(&body).map_err(|error| error.to_string())?)
        .map_err(|error| format!("Couldn't reach the assistant. {error}"))?;
    let text = response.into_string().map_err(|error| error.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    value
        .pointer("/choices/0/message/content")
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "The assistant returned nothing.".to_string())
}

#[cfg(test)]
mod tests {
    use super::{fill_translate_url, parse_price, password, reading_stats, wayback_url, chat_url};

    #[test]
    fn the_translate_template_fills_the_text_and_language() {
        let url = fill_translate_url("https://example.test/translate?q=%s&lang=%t", "hello world", "es");
        assert!(url.contains("q=hello+world") || url.contains("q=hello%20world"));
        assert!(url.contains("lang=es"));
        assert!(!url.contains("%s"));
    }

    #[test]
    fn a_password_has_the_requested_length_and_classes() {
        let random = [3u8, 9, 1, 8, 4, 2, 7, 5, 6, 0, 11, 12, 13, 14, 15, 16];
        let password = password(12, true, true, true, true, &random).expect("password");
        assert_eq!(password.chars().count(), 12);
        assert!(password.chars().any(|character| character.is_ascii_uppercase()));
        assert!(password.chars().any(|character| character.is_ascii_lowercase()));
        assert!(password.chars().any(|character| character.is_ascii_digit()));
        assert!(password.chars().any(|character| "!@#$%^&*-_=+".contains(character)));
    }

    #[test]
    fn reading_stats_count_words_and_keep_the_score_in_range() {
        let stats = reading_stats("The cat sat on the mat.");
        assert_eq!(stats.words, 6);
        assert_eq!(stats.minutes, 1);
        assert!(stats.ease <= 100);
    }

    #[test]
    fn wayback_wraps_an_http_address() {
        assert_eq!(
            wayback_url(false, "https://example.com/a").unwrap(),
            "https://web.archive.org/web/https://example.com/a"
        );
        assert!(wayback_url(true, "ce://settings").is_err());
    }

    #[test]
    fn a_product_block_yields_its_price() {
        let html = r#"<script type="application/ld+json">{"@type":"Product","name":"Mug","offers":{"price":"12.50","priceCurrency":"USD","availability":"https://schema.org/InStock"}}</script>"#;
        let price = parse_price(html).expect("price");
        assert_eq!(price.price, 12.50);
        assert_eq!(price.currency, "USD");
        assert_eq!(price.availability, "InStock");
        assert_eq!(price.title, "Mug");
    }

    #[test]
    fn the_assistant_address_does_not_include_a_key() {
        let url = chat_url("http://127.0.0.1:11434/v1/");
        assert_eq!(url, "http://127.0.0.1:11434/v1/chat/completions");
        assert!(!url.contains("sk-"));
    }
}
