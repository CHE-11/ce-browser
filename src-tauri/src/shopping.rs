//! Coupons and better prices for the product in a tab, from web search results read by the assistant.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::prices;
use crate::tools::{self, ProductPrice};
use crate::{files, paths, settings, tabs, webview2};

const SEARCH_URL: &str = "https://html.duckduckgo.com/html/";
const RESULTS_PER_QUERY: usize = 8;
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Coupon {
    pub code: String,
    pub description: String,
    pub source: String,
    pub expiry: String,
    pub tried_at: u64,
    pub worked: bool,
    pub failed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Offer {
    pub store: String,
    pub price: Option<f64>,
    pub currency: String,
    pub url: String,
    pub note: String,
    pub verified: bool,
    pub variant_mismatch: bool,
    pub shipping: String,
    /// Product image from the other page. Empty until that page is read.
    pub image_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ShoppingDeals {
    /// Price read from the page, when it has structured product data.
    pub product: Option<ProductPrice>,
    /// Product name that was searched for.
    pub query: String,
    /// Host of the current store, without `www.`.
    pub store: String,
    pub summary: String,
    pub coupons: Vec<Coupon>,
    pub offers: Vec<Offer>,
    pub sources: Vec<SearchResult>,
    pub ai_available: bool,
    pub checked_at: u64,
    pub cached: bool,
    /// Canonical product key shared with price tracking.
    pub key: String,
}

const NAME_SCRIPT: &str = r#"(function () {
  var og = document.querySelector('meta[property="og:title"]');
  var h1 = document.querySelector("h1");
  return (og && og.content) || (h1 && h1.innerText) || document.title || "";
})()"#;

const CACHE_MS: u64 = 60 * 60 * 1000;
const SHOPPING_FILE: &str = "shopping.json";
const VERIFY_LIMIT: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct ShoppingFile {
    entries: Vec<ShoppingEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ShoppingEntry {
    key: String,
    checked_at: u64,
    deals: ShoppingDeals,
    feedback: Vec<CouponFeedback>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CouponFeedback {
    code: String,
    tried_at: u64,
    worked: bool,
    failed: bool,
}

pub fn find_deals(app: &AppHandle, id: u32, force: bool) -> Result<ShoppingDeals, String> {
    let page_url = tabs::tab_page_url(app, id).unwrap_or_default();
    let product = tools::read_page_price(app, id).ok();
    let name = match product.as_ref().map(|item| item.title.trim()) {
        Some(title) if !title.is_empty() => clean_name(title),
        _ => clean_name(&page_name(app, id)?),
    };
    run_deals(app, &page_url, &name, product, force)
}

pub fn find_deals_for_product(
    app: &AppHandle,
    url: &str,
    title: &str,
    force: bool,
) -> Result<ShoppingDeals, String> {
    let product = prices::preview_url(url).ok();
    let name = match product.as_ref().map(|item| item.title.trim()) {
        Some(title) if !title.is_empty() => clean_name(title),
        _ => clean_name(title),
    };
    run_deals(app, url, &name, product, force)
}

pub fn set_coupon_feedback(
    app: &AppHandle,
    key: &str,
    code: &str,
    action: &str,
) -> Result<ShoppingDeals, String> {
    let code = code.trim();
    if code.is_empty() {
        return Err("That coupon has no code.".into());
    }
    let mut file = load_shopping(app);
    let entry = file
        .entries
        .iter_mut()
        .find(|entry| entry.key == key)
        .ok_or("Find deals for this product before marking a code.")?;
    let now = now_ms();
    let feedback = entry
        .feedback
        .iter_mut()
        .find(|item| item.code.eq_ignore_ascii_case(code));
    match action {
        "tried" => {
            if let Some(item) = feedback {
                item.tried_at = now;
            } else {
                entry.feedback.push(CouponFeedback {
                    code: code.to_string(),
                    tried_at: now,
                    ..CouponFeedback::default()
                });
            }
        }
        "worked" => {
            if let Some(item) = feedback {
                item.tried_at = now;
                item.worked = true;
                item.failed = false;
            } else {
                entry.feedback.push(CouponFeedback {
                    code: code.to_string(),
                    tried_at: now,
                    worked: true,
                    failed: false,
                });
            }
        }
        "failed" => {
            if let Some(item) = feedback {
                item.tried_at = now;
                item.worked = false;
                item.failed = true;
            } else {
                entry.feedback.push(CouponFeedback {
                    code: code.to_string(),
                    tried_at: now,
                    worked: false,
                    failed: true,
                });
            }
        }
        _ => return Err("Choose tried, worked, or failed.".into()),
    }
    let deals = apply_feedback(&entry.deals, &entry.feedback, true);
    save_shopping(app, &file)?;
    Ok(deals)
}

fn run_deals(
    app: &AppHandle,
    page_url: &str,
    name: &str,
    product: Option<ProductPrice>,
    force: bool,
) -> Result<ShoppingDeals, String> {
    let store = store_host(page_url).ok_or("Open a store's product page to find deals.")?;
    if name.trim().is_empty() {
        return Err("Couldn't tell which product this page is for.".into());
    }
    let key = prices::product_key(page_url, name);
    if !force {
        if let Some(cached) = fresh_cache(app, &key) {
            return Ok(cached);
        }
    }
    let current = settings::get(app);
    let region = current.shopping_region.trim();
    let mut queries = vec![
        format!("{store} coupon code"),
        format!("{name} price"),
        format!("{name} coupon"),
        format!("{name} buy"),
    ];
    if !current.shopping_competitors.is_empty() {
        queries.push(format!(
            "{} {}",
            name,
            current.shopping_competitors.join(" ")
        ));
    }
    let mut sources = Vec::new();
    let mut failure = None;
    for query in queries {
        match search(&query, region) {
            Ok(results) => {
                for result in results {
                    if !sources
                        .iter()
                        .any(|kept: &SearchResult| kept.url == result.url)
                    {
                        sources.push(result);
                    }
                }
            }
            Err(error) => failure = Some(error),
        }
    }
    if sources.is_empty() {
        return Err(
            failure.unwrap_or_else(|| "The web search found nothing for this product.".into())
        );
    }

    let current_price = product.as_ref().map(|item| item.price);
    let mut coupons = extract_coupons(&sources);
    let mut offers = extract_offers(&sources, page_url, current_price);
    let ai_available = crate::ai::available(app, &current);
    let mut summary = String::new();
    if ai_available {
        match tools::chat(
            app,
            &current,
            &current.shopping_model,
            "You find shopping deals in web search results. Reply with one JSON object and nothing else.",
            &deals_prompt(&store, name, product.as_ref(), &sources),
            true,
            Duration::from_secs(90),
        ) {
            Ok(reply) => {
                if let Some(parsed) = parse_reply(&reply) {
                    summary = parsed
                        .get("summary")
                        .and_then(|item| item.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    coupons = grounded_coupons(&parsed, &sources);
                    offers = grounded_offers(&parsed, &sources, page_url, current_price);
                }
            }
            Err(_) => {}
        }
    }
    offers = verify_offers(offers, page_url, current_price, name);
    let checked_at = now_ms();
    let mut deals = ShoppingDeals {
        product,
        query: name.to_string(),
        store,
        summary,
        coupons,
        offers,
        sources,
        ai_available,
        checked_at,
        cached: false,
        key: key.clone(),
    };
    let feedback = remember(app, &key, &deals)?;
    deals = apply_feedback(&deals, &feedback, false);
    Ok(deals)
}

fn fresh_cache(app: &AppHandle, key: &str) -> Option<ShoppingDeals> {
    let file = load_shopping(app);
    let entry = file.entries.iter().find(|entry| entry.key == key)?;
    if entry.checked_at == 0 || now_ms().saturating_sub(entry.checked_at) > CACHE_MS {
        return None;
    }
    if entry.deals.coupons.is_empty()
        && entry.deals.offers.is_empty()
        && entry.deals.sources.is_empty()
    {
        return None;
    }
    Some(apply_feedback(&entry.deals, &entry.feedback, true))
}

fn remember(
    app: &AppHandle,
    key: &str,
    deals: &ShoppingDeals,
) -> Result<Vec<CouponFeedback>, String> {
    let mut file = load_shopping(app);
    let feedback = file
        .entries
        .iter()
        .find(|entry| entry.key == key)
        .map(|entry| entry.feedback.clone())
        .unwrap_or_default();
    file.entries.retain(|entry| entry.key != key);
    file.entries.insert(
        0,
        ShoppingEntry {
            key: key.to_string(),
            checked_at: deals.checked_at,
            deals: deals.clone(),
            feedback: feedback.clone(),
        },
    );
    file.entries.truncate(200);
    save_shopping(app, &file)?;
    Ok(feedback)
}

fn apply_feedback(
    deals: &ShoppingDeals,
    feedback: &[CouponFeedback],
    cached: bool,
) -> ShoppingDeals {
    let mut deals = deals.clone();
    deals.cached = cached;
    for coupon in &mut deals.coupons {
        if let Some(item) = feedback
            .iter()
            .find(|item| item.code.eq_ignore_ascii_case(&coupon.code))
        {
            coupon.tried_at = item.tried_at;
            coupon.worked = item.worked;
            coupon.failed = item.failed;
        }
    }
    deals.coupons.sort_by(|left, right| {
        right
            .worked
            .cmp(&left.worked)
            .then(right.tried_at.cmp(&left.tried_at))
    });
    deals
}

fn load_shopping(app: &AppHandle) -> ShoppingFile {
    files::read_json(&paths::data_file(app, SHOPPING_FILE))
}

fn save_shopping(app: &AppHandle, file: &ShoppingFile) -> Result<(), String> {
    files::write_json(&paths::data_file(app, SHOPPING_FILE), file)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn extract_coupons(sources: &[SearchResult]) -> Vec<Coupon> {
    let mut out: Vec<Coupon> = Vec::new();
    for source in sources {
        let text = format!("{} {}", source.title, source.snippet);
        for code in code_tokens(&text) {
            if out
                .iter()
                .any(|kept: &Coupon| kept.code.eq_ignore_ascii_case(&code))
            {
                continue;
            }
            if !source.title.contains(&code) && !source.snippet.contains(&code) {
                continue;
            }
            out.push(Coupon {
                code,
                description: String::new(),
                source: source.url.clone(),
                expiry: expiry_hint(&source.title, &source.snippet),
                ..Coupon::default()
            });
        }
    }
    out
}

fn code_tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        if character.is_ascii_alphanumeric() {
            current.push(character);
        } else {
            consider_code(&mut out, &current);
            current.clear();
        }
    }
    consider_code(&mut out, &current);
    out
}

fn consider_code(out: &mut Vec<String>, token: &str) {
    if token.len() < 3 || token.len() > 30 || token.chars().any(char::is_whitespace) {
        return;
    }
    if token.chars().all(|character| character.is_ascii_digit()) {
        return;
    }
    let letters_upper = token
        .chars()
        .all(|character| !character.is_ascii_lowercase());
    let has_digit = token.chars().any(|character| character.is_ascii_digit());
    if !(letters_upper && (has_digit || token.len() >= 5)) {
        return;
    }
    if out.iter().any(|kept| kept.eq_ignore_ascii_case(token)) {
        return;
    }
    out.push(token.to_string());
}

fn expiry_hint(title: &str, snippet: &str) -> String {
    let text = format!("{title} {snippet}").to_ascii_lowercase();
    if text.contains("expired") {
        return "May be expired".into();
    }
    if text.contains("until ") || text.contains("expires") || text.contains("valid through") {
        return "Mentions a date".into();
    }
    String::new()
}

fn extract_offers(
    sources: &[SearchResult],
    page_url: &str,
    current_price: Option<f64>,
) -> Vec<Offer> {
    let mut out: Vec<Offer> = Vec::new();
    for source in sources {
        if same_url(&source.url, page_url) || out.iter().any(|kept| kept.url == source.url) {
            continue;
        }
        let Some(price) = snippet_price(&source.snippet).or_else(|| snippet_price(&source.title))
        else {
            continue;
        };
        if let Some(current) = current_price {
            if price >= current {
                continue;
            }
        }
        out.push(Offer {
            store: store_host(&source.url).unwrap_or_default(),
            price: Some(price),
            currency: currency_hint(&format!("{} {}", source.title, source.snippet)),
            url: source.url.clone(),
            note: String::new(),
            verified: false,
            ..Offer::default()
        });
    }
    out.sort_by(|left, right| match (left.price, right.price) {
        (Some(left), Some(right)) => left.total_cmp(&right),
        _ => std::cmp::Ordering::Equal,
    });
    out
}

fn snippet_price(text: &str) -> Option<f64> {
    let chars: Vec<char> = text.chars().collect();
    for (index, character) in chars.iter().enumerate() {
        if matches!(character, '$' | '£' | '€') {
            let rest: String = chars[index + 1..].iter().collect();
            if let Some(price) = leading_amount(&rest) {
                return Some(price);
            }
        }
    }
    None
}

fn leading_amount(text: &str) -> Option<f64> {
    let trimmed = text.trim_start();
    let mut raw = String::new();
    for character in trimmed.chars() {
        if character.is_ascii_digit() || character == '.' || character == ',' {
            raw.push(character);
        } else {
            break;
        }
    }
    if raw.is_empty() {
        return None;
    }
    let normalized = if raw.contains(',') && raw.contains('.') {
        raw.replace(',', "")
    } else if raw.matches(',').count() == 1
        && raw
            .split(',')
            .next_back()
            .is_some_and(|part| part.len() == 2)
    {
        raw.replace(',', ".")
    } else {
        raw.replace(',', "")
    };
    let value: f64 = normalized.parse().ok()?;
    (value > 0.0).then_some(value)
}

fn currency_hint(text: &str) -> String {
    if text.contains('$') {
        "USD".into()
    } else if text.contains('£') {
        "GBP".into()
    } else if text.contains('€') {
        "EUR".into()
    } else {
        String::new()
    }
}

fn verify_offers(
    offers: Vec<Offer>,
    page_url: &str,
    current_price: Option<f64>,
    name: &str,
) -> Vec<Offer> {
    let mut ranked = offers;
    ranked.sort_by(|left, right| match (left.price, right.price) {
        (Some(left), Some(right)) => left.total_cmp(&right),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    ranked.truncate(VERIFY_LIMIT);
    let mut kept = Vec::new();
    for offer in ranked {
        let Ok(fetched) = fetch_offer(&offer.url) else {
            continue;
        };
        if prices::canonical_url(&fetched.url).is_some()
            && prices::canonical_url(&fetched.url) == prices::canonical_url(page_url)
        {
            continue;
        }
        let Some(price) = (fetched.price > 0.0).then_some(fetched.price) else {
            continue;
        };
        if !accept_confirmed_price(price, current_price) {
            continue;
        }
        kept.push(Offer {
            store: if offer.store.is_empty() {
                store_host(&fetched.url).unwrap_or_default()
            } else {
                offer.store
            },
            price: Some(price),
            currency: if fetched.currency.is_empty() {
                offer.currency
            } else {
                fetched.currency
            },
            url: fetched.url,
            note: offer.note,
            verified: true,
            variant_mismatch: variant_mismatch(name, &fetched.title),
            shipping: fetched.shipping,
            image_url: fetched.image_url,
        });
    }
    kept.sort_by(|left, right| match (left.price, right.price) {
        (Some(left), Some(right)) => left.total_cmp(&right),
        _ => std::cmp::Ordering::Equal,
    });
    kept
}

struct FetchedOffer {
    url: String,
    title: String,
    price: f64,
    currency: String,
    shipping: String,
    image_url: String,
}

fn fetch_offer(url: &str) -> Result<FetchedOffer, String> {
    let response = ureq::get(url)
        .timeout(Duration::from_secs(12))
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|_| "Couldn't open that offer.".to_string())?;
    let final_url = response.get_url().to_string();
    let body = response
        .into_string()
        .map_err(|_| "Couldn't open that offer.".to_string())?;
    let product = prices::parse_price(&body)
        .ok_or_else(|| "No price was found on that offer.".to_string())?;
    Ok(FetchedOffer {
        url: final_url,
        title: product.title,
        price: product.price,
        currency: product.currency,
        shipping: prices::shipping_hint(&body),
        image_url: product.image_url,
    })
}

pub fn variant_mismatch(product: &str, title: &str) -> bool {
    let left = name_tokens(product);
    let right = name_tokens(title);
    if left.is_empty() || right.is_empty() {
        return false;
    }
    let overlap = left
        .iter()
        .filter(|token| right.iter().any(|other| other == *token))
        .count();
    overlap * 2 < left.len()
}

fn accept_confirmed_price(confirmed: f64, current: Option<f64>) -> bool {
    confirmed > 0.0 && current.is_none_or(|current| confirmed < current)
}

fn name_tokens(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| token.len() > 2)
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn page_name(app: &AppHandle, id: u32) -> Result<String, String> {
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let value = webview2::evaluate(
        &webview,
        NAME_SCRIPT,
        Duration::from_secs(8),
        "Couldn't read this page.",
    )?;
    Ok(value.as_str().unwrap_or("").to_string())
}

fn store_host(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    Some(host.strip_prefix("www.").unwrap_or(&host).to_string())
}

/// Drops a trailing `| Store` segment and limits the length so the search stays focused.
fn clean_name(title: &str) -> String {
    let first = title.split(" | ").next().unwrap_or(title);
    first
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(120)
        .collect()
}

fn search(query: &str, region: &str) -> Result<Vec<SearchResult>, String> {
    let mut request = ureq::get(SEARCH_URL)
        .timeout(Duration::from_secs(15))
        .set("User-Agent", USER_AGENT)
        .query("q", query);
    if !region.is_empty() {
        request = request.query("kl", region);
    }
    let body = request
        .call()
        .map_err(|error| format!("Couldn't reach the web search. {error}"))?
        .into_string()
        .map_err(|error| error.to_string())?;
    let results = parse_results(&body, RESULTS_PER_QUERY);
    if results.is_empty() && body.contains("anomaly") {
        return Err(
            "The web search asked for a captcha. Wait a minute, then choose Search again.".into(),
        );
    }
    Ok(results)
}

/// Reads organic results from a DuckDuckGo HTML results page. Ads are skipped.
fn parse_results(html: &str, limit: usize) -> Vec<SearchResult> {
    const TITLE: &str = "class=\"result__a\"";
    const SNIPPET: &str = "class=\"result__snippet\"";
    let mut out = Vec::new();
    let mut offset = 0;
    while out.len() < limit {
        let Some(found) = html[offset..].find(TITLE) else {
            break;
        };
        let at = offset + found;
        let next = html[at + TITLE.len()..]
            .find(TITLE)
            .map(|index| at + TITLE.len() + index)
            .unwrap_or(html.len());
        offset = at + TITLE.len();

        let Some(tag_start) = html[..at].rfind('<') else {
            continue;
        };
        let Some(tag_end) = html[at..].find('>').map(|index| at + index) else {
            break;
        };
        let Some(url) =
            attribute(&html[tag_start..tag_end], "href").and_then(|href| target_url(&href))
        else {
            continue;
        };
        let title_end = html[tag_end..]
            .find("</a>")
            .map(|index| tag_end + index)
            .unwrap_or(tag_end + 1);
        let title = text(&html[tag_end + 1..title_end]);

        let block = &html[title_end..next];
        let snippet = block
            .find(SNIPPET)
            .and_then(|index| {
                let start = block[index..].find('>')? + index + 1;
                let end = ["</a>", "</div>", "</td>"]
                    .iter()
                    .filter_map(|close| block[start..].find(close))
                    .min()
                    .map(|length| start + length)
                    .unwrap_or(block.len());
                Some(text(&block[start..end]))
            })
            .unwrap_or_default();

        if !title.is_empty() && !out.iter().any(|kept: &SearchResult| kept.url == url) {
            out.push(SearchResult {
                title,
                url,
                snippet,
            });
        }
    }
    out
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{name}={quote}");
        if let Some(index) = tag.find(&needle) {
            let start = index + needle.len();
            let end = tag[start..].find(quote)? + start;
            return Some(decode_entities(&tag[start..end]));
        }
    }
    None
}

/// The real address behind a result link. DuckDuckGo wraps it in `/l/?uddg=`.
fn target_url(href: &str) -> Option<String> {
    let absolute = if href.starts_with("//") {
        format!("https:{href}")
    } else {
        href.to_string()
    };
    let parsed = url::Url::parse(&absolute).ok()?;
    let host = parsed.host_str().unwrap_or("");
    if host.ends_with("duckduckgo.com") {
        if parsed.path() != "/l/" {
            return None;
        }
        let target = parsed
            .query_pairs()
            .find(|(key, _)| key == "uddg")
            .map(|(_, value)| value.into_owned())?;
        return target_url(&target);
    }
    matches!(parsed.scheme(), "http" | "https").then(|| parsed.to_string())
}

fn text(html: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for character in html.chars() {
        match character {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => out.push(character),
            _ => {}
        }
    }
    decode_entities(&out)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find(';').filter(|end| *end <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| {
                    entity
                        .strip_prefix('#')
                        .and_then(|number| number.parse().ok())
                })
                .and_then(char::from_u32),
        };
        match decoded {
            Some(character) => {
                out.push(character);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn deals_prompt(
    store: &str,
    name: &str,
    product: Option<&ProductPrice>,
    sources: &[SearchResult],
) -> String {
    let price = product
        .map(|item| {
            format!("{} {}", item.currency, item.price)
                .trim()
                .to_string()
        })
        .unwrap_or_else(|| "unknown".into());
    let mut results = String::new();
    for (index, result) in sources.iter().enumerate() {
        results.push_str(&format!(
            "[{}] {}\nURL: {}\n{}\n\n",
            index + 1,
            result.title,
            result.url,
            result.snippet
        ));
    }
    format!(
        "Store: {store}\nProduct: {name}\nCurrent price: {price}\n\nSearch results:\n{results}\
Return a JSON object with these keys:\n\
- \"summary\": one or two sentences on whether a coupon or a lower price was found.\n\
- \"coupons\": a list of {{\"code\", \"description\", \"source\"}} for coupon or promo codes for {store} that appear word for word in the results. \"source\" is the URL of that result.\n\
- \"offers\": a list of {{\"store\", \"price\", \"currency\", \"url\", \"note\"}} for this same product sold somewhere else. \"price\" is a number or null. \"url\" is the URL of that result.\n\
Use only the results above. Leave a list empty when nothing qualifies. Do not invent codes, prices or URLs."
    )
}

fn parse_reply(reply: &str) -> Option<serde_json::Value> {
    let start = reply.find('{')?;
    let end = reply.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<serde_json::Value>(&reply[start..=end])
        .ok()
        .filter(serde_json::Value::is_object)
}

fn field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn same_url(left: &str, right: &str) -> bool {
    let left = left.trim().trim_end_matches('/');
    let right = right.trim().trim_end_matches('/');
    !left.is_empty() && left.eq_ignore_ascii_case(right)
}

/// Keeps codes that appear verbatim in a search result, credited to a result that shows them.
fn grounded_coupons(reply: &serde_json::Value, sources: &[SearchResult]) -> Vec<Coupon> {
    let mut out: Vec<Coupon> = Vec::new();
    let Some(items) = reply.get("coupons").and_then(|item| item.as_array()) else {
        return out;
    };
    for item in items {
        let code = field(item, "code");
        if code.len() < 3
            || code.len() > 30
            || code.chars().any(char::is_whitespace)
            || !code
                .chars()
                .any(|character| character.is_ascii_digit() || character.is_ascii_uppercase())
            || out.iter().any(|kept| kept.code.eq_ignore_ascii_case(&code))
        {
            continue;
        }
        let shows =
            |result: &&SearchResult| result.title.contains(&code) || result.snippet.contains(&code);
        let claimed = field(item, "source");
        let source = sources
            .iter()
            .filter(shows)
            .find(|result| same_url(&result.url, &claimed))
            .or_else(|| sources.iter().find(shows));
        if let Some(source) = source {
            out.push(Coupon {
                code,
                description: field(item, "description"),
                source: source.url.clone(),
                expiry: expiry_hint(&source.title, &source.snippet),
                ..Coupon::default()
            });
        }
    }
    out
}

/// Keeps offers that link to a search result other than this page, cheapest first.
/// When the page price is known, offers that cost the same or more are dropped.
fn grounded_offers(
    reply: &serde_json::Value,
    sources: &[SearchResult],
    page_url: &str,
    current_price: Option<f64>,
) -> Vec<Offer> {
    let mut out: Vec<Offer> = Vec::new();
    let Some(items) = reply.get("offers").and_then(|item| item.as_array()) else {
        return out;
    };
    for item in items {
        let claimed = field(item, "url");
        let Some(source) = sources
            .iter()
            .find(|result| same_url(&result.url, &claimed))
        else {
            continue;
        };
        if same_url(&source.url, page_url) || out.iter().any(|kept| kept.url == source.url) {
            continue;
        }
        let price = item
            .get("price")
            .and_then(offer_price)
            .filter(|value| *value > 0.0);
        if let (Some(price), Some(current)) = (price, current_price) {
            if price >= current {
                continue;
            }
        }
        let store = field(item, "store");
        out.push(Offer {
            store: if store.is_empty() {
                store_host(&source.url).unwrap_or_default()
            } else {
                store
            },
            price,
            currency: field(item, "currency"),
            url: source.url.clone(),
            note: field(item, "note"),
            ..Offer::default()
        });
    }
    out.sort_by(|left, right| match (left.price, right.price) {
        (Some(left), Some(right)) => left.total_cmp(&right),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    out
}

fn offer_price(value: &serde_json::Value) -> Option<f64> {
    if let Some(number) = value.as_f64() {
        return Some(number);
    }
    let digits: String = value
        .as_str()?
        .chars()
        .filter(|character| character.is_ascii_digit() || *character == '.')
        .collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{
        SearchResult, accept_confirmed_price, clean_name, extract_coupons, extract_offers,
        grounded_coupons, grounded_offers, parse_reply, parse_results, target_url,
        variant_mismatch,
    };

    const PAGE: &str = r#"
<div class="result results_links results_links_deep result--ad">
  <a rel="nofollow" class="result__a" href="https://duckduckgo.com/y.js?ad_domain=ads.test&amp;u3=x">Sponsored mug</a>
  <a class="result__snippet" href="https://duckduckgo.com/y.js?x">Buy now</a>
</div>
<div class="result results_links results_links_deep web-result">
  <h2 class="result__title">
    <a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fcoupons.test%2Fmugshop&amp;rut=abc">Mug Shop <b>Coupon</b> Codes &amp; Deals</a>
  </h2>
  <a class="result__snippet" href="//duckduckgo.com/l/?uddg=x">Use code <b>SAVE20</b> for 20% off at mugshop.test.</a>
</div>
<div class="result results_links results_links_deep web-result">
  <h2 class="result__title">
    <a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fother.test%2Fmug">Blue Mug - Other Store</a>
  </h2>
  <a class="result__snippet" href="//duckduckgo.com/l/?uddg=x">Blue Mug for $9.99 with free shipping.</a>
</div>
"#;

    fn sources() -> Vec<SearchResult> {
        parse_results(PAGE, 8)
    }

    #[test]
    fn results_skip_ads_and_decode_titles_and_snippets() {
        let results = sources();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].url, "https://coupons.test/mugshop");
        assert_eq!(results[0].title, "Mug Shop Coupon Codes & Deals");
        assert_eq!(
            results[0].snippet,
            "Use code SAVE20 for 20% off at mugshop.test."
        );
        assert_eq!(results[1].url, "https://other.test/mug");
    }

    #[test]
    fn a_wrapped_link_resolves_to_the_target() {
        assert_eq!(
            target_url("//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fa%3Fb%3D1&rut=x")
                .as_deref(),
            Some("https://example.com/a?b=1")
        );
        assert_eq!(target_url("https://duckduckgo.com/y.js?ad_domain=x"), None);
        assert_eq!(target_url("javascript:alert(1)"), None);
    }

    #[test]
    fn invented_codes_and_unknown_links_are_dropped() {
        let reply = parse_reply(
            r#"Here you go: {"summary":"Found one.","coupons":[
              {"code":"SAVE20","description":"20% off","source":"https://made-up.test"},
              {"code":"FAKE50","description":"50% off","source":"https://coupons.test/mugshop"}
            ],"offers":[
              {"store":"Other Store","price":"$9.99","currency":"USD","url":"https://other.test/mug","note":""},
              {"store":"Nowhere","price":5,"currency":"USD","url":"https://nowhere.test/mug","note":""}
            ]}"#,
        )
        .expect("reply");
        let coupons = grounded_coupons(&reply, &sources());
        assert_eq!(coupons.len(), 1);
        assert_eq!(coupons[0].code, "SAVE20");
        assert_eq!(coupons[0].source, "https://coupons.test/mugshop");

        let offers = grounded_offers(
            &reply,
            &sources(),
            "https://mugshop.test/blue-mug",
            Some(14.0),
        );
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].price, Some(9.99));
        assert!(
            grounded_offers(
                &reply,
                &sources(),
                "https://mugshop.test/blue-mug",
                Some(8.0)
            )
            .is_empty()
        );
    }

    #[test]
    fn a_store_suffix_is_removed_from_the_name() {
        assert_eq!(clean_name("Blue  Mug 12oz | Mug Shop"), "Blue Mug 12oz");
    }

    #[test]
    fn codes_and_prices_are_read_without_a_provider() {
        let sources = sources();
        let coupons = extract_coupons(&sources);
        assert_eq!(coupons.len(), 1);
        assert_eq!(coupons[0].code, "SAVE20");
        assert_eq!(coupons[0].source, "https://coupons.test/mugshop");
        let offers = extract_offers(&sources, "https://mugshop.test/blue-mug", Some(14.0));
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].price, Some(9.99));
        assert!(!offers[0].verified);
        assert!(extract_offers(&sources, "https://mugshop.test/blue-mug", Some(8.0)).is_empty());
    }

    #[test]
    fn a_different_variant_is_flagged() {
        assert!(variant_mismatch(
            "Blue ceramic mug 12oz",
            "Stainless travel tumbler"
        ));
        assert!(!variant_mismatch(
            "Blue ceramic mug 12oz",
            "Blue ceramic mug 12oz at Other Store"
        ));
    }

    #[test]
    fn a_confirmed_price_that_is_not_lower_is_dropped() {
        assert!(!accept_confirmed_price(14.0, Some(14.0)));
        assert!(!accept_confirmed_price(16.0, Some(14.0)));
        assert!(accept_confirmed_price(9.5, Some(14.0)));
        assert!(!accept_confirmed_price(0.0, Some(14.0)));
    }
}
