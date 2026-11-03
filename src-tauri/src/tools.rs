//! Translate, passwords, prices, the Internet Archive, and readability.

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::AppHandle;

use crate::{settings, tabs, webview2};

pub use crate::prices::{
    ProductPrice, check_prices, list_prices, parse_price, read_page_price, track_price,
};

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
    let template = if template.trim().is_empty() {
        DEFAULT_TRANSLATE_URL
    } else {
        template
    };
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
    let value = webview2::evaluate(
        &webview,
        COLLECT_SCRIPT,
        Duration::from_secs(8),
        TEXT_FAILURE,
    )?;
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

/// Translates selected text into the language from settings. Up to 4000 characters.
pub fn translate_selection(app: &AppHandle, text: &str) -> Result<String, String> {
    let text: String = text.trim().chars().take(4000).collect();
    if text.is_empty() {
        return Err("There is no text to translate.".into());
    }
    let settings = settings::get(app);
    translate_text(
        &settings.translate_url,
        &text,
        &clean_language(&settings.translate_language),
    )
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
    if trimmed.is_empty() {
        return Err("The provider returned nothing.".into());
    }
    // Google's gtx client returns [[["translated", "original", ...], ...], ...].
    // The translation is the first string of each sentence, not a nested array.
    if trimmed.starts_with('[') {
        let value: serde_json::Value =
            serde_json::from_str(trimmed).map_err(|error| error.to_string())?;
        let out = google_sentences(&value);
        if out.trim().is_empty() {
            return Err("The provider returned nothing.".into());
        }
        return Ok(out);
    }
    if trimmed.starts_with('{') {
        let value: serde_json::Value =
            serde_json::from_str(trimmed).map_err(|error| error.to_string())?;
        if let Some(text) = value
            .get("translatedText")
            .or_else(|| value.get("text"))
            .and_then(|item| item.as_str())
            .filter(|text| !text.trim().is_empty())
        {
            return Ok(text.to_string());
        }
        return Err("The provider returned nothing.".into());
    }
    Ok(trimmed.to_string())
}

/// Joins the translated segments from a `translate_a/single` response.
fn google_sentences(value: &serde_json::Value) -> String {
    let mut out = String::new();
    let Some(sentences) = value.get(0).and_then(|item| item.as_array()) else {
        return out;
    };
    for sentence in sentences {
        if let Some(text) = sentence.get(0).and_then(|item| item.as_str()) {
            out.push_str(text);
        }
    }
    out
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

const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const DIGITS: &str = "0123456789";
const SYMBOLS: &str = "!@#$%^&*-_=+";
/// Letters and digits that are easy to mix up. Matches the set Bitwarden skips.
const AMBIGUOUS: &str = "lI1O0";

/// Characters named in a comma-separated list, such as `a, e, i`.
pub fn excluded_chars(value: &str) -> String {
    let mut out = String::new();
    for part in value.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        for character in part.chars() {
            if !out.contains(character) {
                out.push(character);
            }
        }
    }
    out
}

fn pool(source: &str, avoid_ambiguous: bool, exclude: &str) -> Vec<char> {
    source
        .chars()
        .filter(|character| !avoid_ambiguous || !AMBIGUOUS.contains(*character))
        .filter(|character| !exclude.contains(*character))
        .collect()
}

pub fn password(
    length: usize,
    upper: bool,
    lower: bool,
    digits: bool,
    symbols: bool,
    min_symbols: usize,
    avoid_ambiguous: bool,
    exclude: &str,
    random: &[u8],
) -> Result<String, String> {
    let length = length.clamp(8, 64);
    if random.is_empty() {
        return Err("Couldn't generate a password.".into());
    }
    let exclude = excluded_chars(exclude);
    let mut alphabet = Vec::new();
    let mut required: Vec<Vec<char>> = Vec::new();
    let mut add = |source: &str, count: usize| -> Result<(), String> {
        let class = pool(source, avoid_ambiguous, &exclude);
        if class.is_empty() {
            return Err("That mix of characters is too small.".into());
        }
        alphabet.extend(class.iter().copied());
        for _ in 0..count {
            required.push(class.clone());
        }
        Ok(())
    };
    if upper {
        add(UPPER, 1)?;
    }
    if lower {
        add(LOWER, 1)?;
    }
    if digits {
        add(DIGITS, 1)?;
    }
    if symbols {
        add(SYMBOLS, min_symbols)?;
    }
    if alphabet.is_empty() {
        return Err("Pick at least one kind of character.".into());
    }
    if required.len() > length {
        return Err("Length is too short for those minimums.".into());
    }
    let mut cursor = 0usize;
    let mut next = || {
        let byte = random[cursor % random.len()] as usize;
        cursor += 1;
        byte
    };
    let mut out = Vec::with_capacity(length);
    for class in &required {
        out.push(class[next() % class.len()]);
    }
    while out.len() < length {
        out.push(alphabet[next() % alphabet.len()]);
    }
    for index in (1..out.len()).rev() {
        out.swap(index, next() % (index + 1));
    }
    Ok(out.into_iter().collect())
}

pub fn random_password(
    length: usize,
    upper: bool,
    lower: bool,
    digits: bool,
    symbols: bool,
    min_symbols: usize,
    avoid_ambiguous: bool,
    exclude: &str,
) -> Result<String, String> {
    let mut bytes = [0u8; 256];
    fill_random(&mut bytes)?;
    password(
        length,
        upper,
        lower,
        digits,
        symbols,
        min_symbols,
        avoid_ambiguous,
        exclude,
        &bytes,
    )
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingStats {
    pub words: u32,
    pub minutes: u32,
    pub ease: u32,
}

pub fn reading_stats(text: &str) -> ReadingStats {
    let words: Vec<&str> = text
        .split_whitespace()
        .filter(|word| !word.is_empty())
        .collect();
    let word_count = words.len() as u32;
    let sentences = text
        .chars()
        .filter(|character| matches!(character, '.' | '!' | '?'))
        .count()
        .max(1) as f64;
    let syllables: u32 = words.iter().map(|word| syllables(word)).sum();
    let ease = if word_count == 0 {
        0.0
    } else {
        206.835
            - 1.015 * (f64::from(word_count) / sentences)
            - 84.6 * (f64::from(syllables) / f64::from(word_count))
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
        let Some(end) = rest[start..].find('>') else {
            break;
        };
        let tag = &rest[start + 1..start + end];
        let name = tag
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches('/')
            .to_ascii_lowercase();
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

pub fn chat_url(endpoint: &str) -> String {
    format!("{}/chat/completions", endpoint.trim().trim_end_matches('/'))
}

/// Sends one Shopping summary request through the active provider.
pub(crate) fn chat(
    app: &AppHandle,
    current: &settings::BrowserSettings,
    model: &str,
    system: &str,
    user: &str,
    json_only: bool,
    timeout: Duration,
) -> Result<String, String> {
    crate::ai::complete(app, current, model, system, user, json_only, timeout)
}

#[cfg(test)]
mod tests {
    use super::{
        chat_url, fill_translate_url, parse_price, password, read_translation, reading_stats,
    };

    #[test]
    fn a_google_translate_response_keeps_only_the_translated_sentences() {
        let body = r#"[[["Hola Mundo. ","Hello world. ",null,null,10],["Esta es una prueba.","This is a test.",null,null,3,null,null,[[]],[[["e050167b38f2a566522b4157651fc616","en_es_2023q1.md"]]]]],null,"en"]"#;
        let text = read_translation(body).expect("translation");
        assert_eq!(text, "Hola Mundo. Esta es una prueba.");
        assert!(!text.contains("en_es_2023q1.md"));
        assert!(!text.contains("e050167b38f2a566522b4157651fc616"));
    }

    #[test]
    fn a_named_translate_response_uses_the_translated_text_field() {
        let text = read_translation(r#"{"translatedText":"Hola"}"#).expect("translation");
        assert_eq!(text, "Hola");
    }

    #[test]
    fn the_translate_template_fills_the_text_and_language() {
        let url = fill_translate_url(
            "https://example.test/translate?q=%s&lang=%t",
            "hello world",
            "es",
        );
        assert!(url.contains("q=hello+world") || url.contains("q=hello%20world"));
        assert!(url.contains("lang=es"));
        assert!(!url.contains("%s"));
    }

    #[test]
    fn a_password_has_the_requested_length_and_classes() {
        let random = [3u8, 9, 1, 8, 4, 2, 7, 5, 6, 0, 11, 12, 13, 14, 15, 16];
        let password =
            password(12, true, true, true, true, 1, false, "", &random).expect("password");
        assert_eq!(password.chars().count(), 12);
        assert!(
            password
                .chars()
                .any(|character| character.is_ascii_uppercase())
        );
        assert!(
            password
                .chars()
                .any(|character| character.is_ascii_lowercase())
        );
        assert!(password.chars().any(|character| character.is_ascii_digit()));
        assert!(
            password
                .chars()
                .any(|character| "!@#$%^&*-_=+".contains(character))
        );
    }

    #[test]
    fn a_password_keeps_the_minimum_number_of_symbols() {
        let random: Vec<u8> = (0..=127).collect();
        let password =
            password(16, true, true, true, true, 4, false, "", &random).expect("password");
        let symbols = password
            .chars()
            .filter(|character| "!@#$%^&*-_=+".contains(*character))
            .count();
        assert!(symbols >= 4, "{password}");
    }

    #[test]
    fn ambiguous_characters_are_left_out() {
        let random: Vec<u8> = (0..=127).collect();
        let password =
            password(32, true, true, true, true, 1, true, "", &random).expect("password");
        assert!(
            password
                .chars()
                .all(|character| !"lI1O0".contains(character)),
            "{password}"
        );
    }

    #[test]
    fn too_many_required_characters_is_an_error() {
        let error = password(8, true, true, true, true, 8, false, "", &[1, 2, 3]).unwrap_err();
        assert_eq!(error, "Length is too short for those minimums.");
    }

    #[test]
    fn symbols_stay_out_when_that_class_is_off() {
        let random: Vec<u8> = (0..=63).collect();
        let password =
            password(12, true, true, true, false, 5, false, "", &random).expect("password");
        assert!(
            password
                .chars()
                .all(|character| !"!@#$%^&*-_=+".contains(character))
        );
    }

    #[test]
    fn listed_characters_are_left_out() {
        let random: Vec<u8> = (0..=127).collect();
        let password = password(
            32,
            true,
            true,
            true,
            true,
            1,
            false,
            "a, e, i, 1, @",
            &random,
        )
        .expect("password");
        assert!(
            password
                .chars()
                .all(|character| !"aei1@".contains(character)),
            "{password}"
        );
    }

    #[test]
    fn excluding_a_whole_class_is_an_error() {
        let error = password(
            12,
            false,
            false,
            true,
            false,
            0,
            false,
            "0, 1, 2, 3, 4, 5, 6, 7, 8, 9",
            &[1, 2, 3],
        )
        .unwrap_err();
        assert_eq!(error, "That mix of characters is too small.");
    }

    #[test]
    fn reading_stats_count_words_and_keep_the_score_in_range() {
        let stats = reading_stats("The cat sat on the mat.");
        assert_eq!(stats.words, 6);
        assert_eq!(stats.minutes, 1);
        assert!(stats.ease <= 100);
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
