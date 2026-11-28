//! Coupons and better prices for the product in a tab, from web search results read by the assistant.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::prices;
use crate::tools::{self, ProductPrice};
use crate::{files, paths, settings, tabs, webview2};

const SEARCH_URL: &str = "https://html.duckduckgo.com/html/";
const BING_URL: &str = "https://www.bing.com/search";
const BRAVE_URL: &str = "https://search.brave.com/search";
const RESULTS_PER_QUERY: usize = 8;
const MAX_QUERIES: usize = 12;
const CONFIRM_LIMIT: usize = 12;
const FETCH_BATCH: usize = 4;
const STORES: [&str; 6] = ["amazon", "walmart", "bestbuy", "target", "ebay", "newegg"];
pub(crate) const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";

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
    /// How many pages list this code.
    pub sources: u32,
    /// Amount taken off the order total when the code was tried on the page.
    pub saving: Option<f64>,
    pub currency: String,
    /// When the code was last applied on the page by `try_coupons`.
    pub tested_at: u64,
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
    /// The confirmed price is under the tracked price.
    pub lower: bool,
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
    /// Which identifier was searched, and which engines were skipped.
    pub search_note: String,
}

const NAME_SCRIPT: &str = r#"(function () {
  var og = document.querySelector('meta[property="og:title"]');
  var h1 = document.querySelector("h1");
  return (og && og.content) || (h1 && h1.innerText) || document.title || "";
})()"#;

const CACHE_MS: u64 = 60 * 60 * 1000;
const SHOPPING_FILE: &str = "shopping.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub(crate) struct ShoppingFile {
    pub(crate) entries: Vec<ShoppingEntry>,
    /// Store-wide coupon searches, keyed by store host.
    pub(crate) stores: Vec<crate::coupons::StoreCoupons>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct ShoppingEntry {
    pub(crate) key: String,
    pub(crate) checked_at: u64,
    pub(crate) deals: ShoppingDeals,
    pub(crate) feedback: Vec<CouponFeedback>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct CouponFeedback {
    pub(crate) code: String,
    pub(crate) tried_at: u64,
    pub(crate) worked: bool,
    pub(crate) failed: bool,
    pub(crate) saving: Option<f64>,
}

pub fn find_deals(app: &AppHandle, id: u32, force: bool) -> Result<ShoppingDeals, String> {
    let page_url = tabs::tab_page_url(app, id).unwrap_or_default();
    let product = tools::read_page_price(app, id).ok();
    let name = match product.as_ref().map(|item| item.title.trim()) {
        Some(title) if !title.is_empty() => clean_name(title),
        _ => clean_name(&page_name(app, id)?),
    };
    run_deals(
        app,
        &page_url,
        &name,
        product,
        prices::ProductFacts::default(),
        force,
    )
}

pub fn find_deals_for_product(
    app: &AppHandle,
    url: &str,
    title: &str,
    force: bool,
) -> Result<ShoppingDeals, String> {
    let loaded = prices::preview_page(url).ok();
    let facts = loaded
        .as_ref()
        .map(|page| page.facts.clone())
        .unwrap_or_default();
    let name = match loaded.as_ref().map(|page| page.product.title.trim()) {
        Some(title) if !title.is_empty() => clean_name(title),
        _ => clean_name(title),
    };
    let product = loaded.map(|page| page.product);
    run_deals(app, url, &name, product, facts, force)
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
                    ..CouponFeedback::default()
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
                    failed: true,
                    ..CouponFeedback::default()
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
    facts: prices::ProductFacts,
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
    let ai_available = crate::ai::available(app, &current);
    let room = if ai_available {
        MAX_QUERIES - 4
    } else {
        MAX_QUERIES
    };
    let mut queries = build_queries(name, &store, &facts, &current.shopping_competitors, room);
    if ai_available {
        queries = add_ai_queries(app, &current, name, &store, &facts, queries);
    }
    let gathered = gather(&queries, region);
    if gathered.sources.is_empty() {
        return Err(gathered
            .failure
            .unwrap_or_else(|| "The web search found nothing for this product.".into()));
    }
    let sources = gathered.sources;
    let skipped = gathered.skipped;

    let current_price = product.as_ref().map(|item| item.price);
    let mut coupons = extract_coupons(&sources);
    let mut summary = String::new();
    let mut picked = None;
    if ai_available {
        if let Some(parsed) = ai_pick(app, &current, name, &store, &facts, &sources) {
            summary = parsed
                .get("summary")
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let grounded = grounded_coupons(&parsed, &sources);
            if !grounded.is_empty() {
                coupons = grounded;
            }
            let urls = picked_urls(&parsed, &sources);
            if !urls.is_empty() {
                picked = Some(urls);
            }
        }
    }
    let candidates = picked.unwrap_or_else(|| rank_candidates(&sources, page_url, &facts));
    let offers = confirm_offers(&candidates, page_url, current_price, name, &facts);
    let search_note = search_note(&facts, &skipped, offers.is_empty());
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
        search_note,
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
            coupon.saving = item.saving.or(coupon.saving);
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

pub(crate) fn load_shopping(app: &AppHandle) -> ShoppingFile {
    files::read_json(&paths::data_file(app, SHOPPING_FILE))
}

pub(crate) fn save_shopping(app: &AppHandle, file: &ShoppingFile) -> Result<(), String> {
    files::write_json(&paths::data_file(app, SHOPPING_FILE), file)
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn extract_coupons(sources: &[SearchResult]) -> Vec<Coupon> {
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

pub(crate) fn code_tokens(text: &str) -> Vec<String> {
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

pub(crate) fn expiry_hint(title: &str, snippet: &str) -> String {
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

fn confirm_offers(
    candidates: &[SearchResult],
    page_url: &str,
    current_price: Option<f64>,
    name: &str,
    facts: &prices::ProductFacts,
) -> Vec<Offer> {
    let mut kept = Vec::new();
    for chunk in candidates.chunks(FETCH_BATCH) {
        let fetched = std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for source in chunk {
                let url = source.url.clone();
                handles.push(scope.spawn(move || fetch_offer(&url)));
            }
            handles
                .into_iter()
                .filter_map(|handle| handle.join().ok().and_then(Result::ok))
                .collect::<Vec<_>>()
        });
        for (fetched, page_facts) in fetched {
            if prices::canonical_url(&fetched.url).is_some()
                && prices::canonical_url(&fetched.url) == prices::canonical_url(page_url)
            {
                continue;
            }
            if fetched.price <= 0.0 || kept.iter().any(|offer: &Offer| offer.url == fetched.url) {
                continue;
            }
            let same = same_catalog(facts, &page_facts);
            kept.push(Offer {
                store: store_host(&fetched.url).unwrap_or_default(),
                price: Some(fetched.price),
                currency: fetched.currency,
                url: fetched.url,
                note: if same {
                    "Same item".into()
                } else {
                    String::new()
                },
                verified: true,
                variant_mismatch: !same && variant_mismatch(name, &fetched.title),
                shipping: fetched.shipping,
                image_url: fetched.image_url,
                lower: current_price.is_some_and(|current| fetched.price < current),
            });
        }
    }
    kept.sort_by(|left, right| match (left.price, right.price) {
        (Some(left), Some(right)) => left.total_cmp(&right),
        _ => std::cmp::Ordering::Equal,
    });
    kept.truncate(CONFIRM_LIMIT);
    kept
}

fn same_catalog(want: &prices::ProductFacts, got: &prices::ProductFacts) -> bool {
    (!want.gtin.is_empty() && want.gtin == got.gtin)
        || (!want.mpn.is_empty() && want.mpn.eq_ignore_ascii_case(&got.mpn))
}

struct FetchedOffer {
    url: String,
    title: String,
    price: f64,
    currency: String,
    shipping: String,
    image_url: String,
}

fn fetch_offer(url: &str) -> Result<(FetchedOffer, prices::ProductFacts), String> {
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
    let facts = prices::product_facts(&body);
    Ok((
        FetchedOffer {
            url: final_url,
            title: product.title,
            price: product.price,
            currency: product.currency,
            shipping: prices::shipping_hint(&body),
            image_url: product.image_url,
        },
        facts,
    ))
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

pub(crate) fn page_name(app: &AppHandle, id: u32) -> Result<String, String> {
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let value = webview2::evaluate(
        &webview,
        NAME_SCRIPT,
        Duration::from_secs(8),
        "Couldn't read this page.",
    )?;
    Ok(value.as_str().unwrap_or("").to_string())
}

pub(crate) fn store_host(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    Some(host.strip_prefix("www.").unwrap_or(&host).to_string())
}

/// Drops a trailing `| Store` segment and limits the length so the search stays focused.
pub(crate) fn clean_name(title: &str) -> String {
    let first = title.split(" | ").next().unwrap_or(title);
    first
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(120)
        .collect()
}

pub(crate) struct Gathered {
    pub(crate) sources: Vec<SearchResult>,
    pub(crate) skipped: Vec<String>,
    pub(crate) failure: Option<String>,
}

fn build_queries(
    name: &str,
    store: &str,
    facts: &prices::ProductFacts,
    competitors: &[String],
    limit: usize,
) -> Vec<String> {
    let quoted = format!("\"{name}\"");
    let mut queries = vec![
        format!("{store} coupon code"),
        format!("{name} coupon"),
        format!("{quoted} price -site:{store}"),
        format!("{quoted} buy -site:{store}"),
    ];
    if let Some((_, identifier)) = facts.identifier() {
        queries.push(identifier.to_string());
    }
    for shop in STORES {
        if queries.len() >= limit {
            break;
        }
        if store.contains(shop) {
            continue;
        }
        queries.push(format!("{quoted} {shop}"));
    }
    for host in competitors {
        if queries.len() >= limit {
            break;
        }
        let host = host.trim();
        if host.is_empty() || store.contains(host) {
            continue;
        }
        queries.push(format!("{quoted} {host}"));
    }
    queries.truncate(limit);
    queries
}

fn add_ai_queries(
    app: &AppHandle,
    current: &settings::BrowserSettings,
    name: &str,
    store: &str,
    facts: &prices::ProductFacts,
    mut queries: Vec<String>,
) -> Vec<String> {
    let identifier = facts
        .identifier()
        .map(|(kind, value)| format!("{kind} {value}"))
        .unwrap_or_else(|| "none".into());
    let prompt = format!(
        "Product: {name}\nBrand: {}\nIdentifier: {identifier}\nCurrent store: {store}\n\
Reply with one JSON object {{\"queries\": [\"...\"]}} of up to 4 short web searches for this exact product at other stores. Do not include prices or URLs.",
        if facts.brand.is_empty() {
            "unknown"
        } else {
            facts.brand.as_str()
        }
    );
    let Ok(reply) = tools::chat(
        app,
        current,
        &current.shopping_model,
        "You write web searches for one product. Reply with one JSON object and nothing else.",
        &prompt,
        true,
        Duration::from_secs(25),
    ) else {
        return queries;
    };
    let Some(parsed) = parse_reply(&reply) else {
        return queries;
    };
    let Some(items) = parsed.get("queries").and_then(|item| item.as_array()) else {
        return queries;
    };
    for item in items.iter().take(4) {
        let Some(query) = item.as_str().map(str::trim) else {
            continue;
        };
        if query.is_empty() || query.len() > 120 || query.contains("://") {
            continue;
        }
        if !queries.iter().any(|kept| kept.eq_ignore_ascii_case(query)) {
            queries.push(query.to_string());
        }
    }
    queries.truncate(MAX_QUERIES);
    queries
}

pub(crate) fn gather(queries: &[String], region: &str) -> Gathered {
    gather_with(queries, |query| search_query(query, region), |_, _| {}, |_| {})
}

/// One finished query, passed to the `after` callback of `gather_with`.
pub(crate) struct QueryReport<'a> {
    pub(crate) index: usize,
    pub(crate) query: &'a str,
    /// Engine that answered. `None` when every engine was blocked or had nothing relevant.
    pub(crate) engine: Option<&'a str>,
    pub(crate) results: &'a [SearchResult],
    pub(crate) skipped: &'a [String],
}

/// `gather` with its own `search`, calling `before` as each query starts and `after` when it finishes.
pub(crate) fn gather_with(
    queries: &[String],
    search: impl Fn(&str) -> Searched,
    mut before: impl FnMut(usize, &str),
    mut after: impl FnMut(QueryReport),
) -> Gathered {
    let mut sources = Vec::new();
    let mut skipped = Vec::new();
    let mut failure = None;
    for (index, query) in queries.iter().enumerate() {
        before(index, query);
        let searched = search(query);
        after(QueryReport {
            index,
            query,
            engine: searched.engine,
            results: &searched.results,
            skipped: &searched.skipped,
        });
        for name in searched.skipped {
            if !skipped.iter().any(|kept| kept == &name) {
                skipped.push(name);
            }
        }
        if searched.results.is_empty() {
            failure = Some(engine_failure(&skipped));
            continue;
        }
        for result in searched.results {
            if !sources
                .iter()
                .any(|kept: &SearchResult| kept.url == result.url)
            {
                sources.push(result);
            }
        }
    }
    Gathered {
        sources,
        skipped,
        failure,
    }
}

pub(crate) fn engine_failure(skipped: &[String]) -> String {
    if skipped.is_empty() {
        "The web search found nothing for this product.".into()
    } else if skipped.len() == 1 {
        format!(
            "{} asked for a check and the other searches found nothing. Wait a minute, then choose Search again.",
            skipped[0]
        )
    } else {
        format!(
            "{} asked for a check and no search found this product. Wait a minute, then choose Search again.",
            skipped.join(", ")
        )
    }
}

pub(crate) struct Searched {
    pub(crate) results: Vec<SearchResult>,
    pub(crate) skipped: Vec<String>,
    pub(crate) engine: Option<&'static str>,
}

pub(crate) type Attempt<'a> = (&'static str, &'a dyn Fn() -> Result<Vec<SearchResult>, String>);

pub(crate) const DUCKDUCKGO: &str = "DuckDuckGo";
pub(crate) const BING: &str = "Bing";
pub(crate) const BRAVE: &str = "Brave";

/// Asking an engine again while it shows a check keeps the block going, so it rests for a while.
const ENGINE_REST: Duration = Duration::from_secs(300);

fn resting() -> &'static Mutex<HashMap<&'static str, Instant>> {
    static RESTING: std::sync::OnceLock<Mutex<HashMap<&'static str, Instant>>> =
        std::sync::OnceLock::new();
    RESTING.get_or_init(Default::default)
}

/// Runs `run` unless `key` asked for a check in the last few minutes.
pub(crate) fn guarded(
    key: &'static str,
    run: impl Fn() -> Result<Vec<SearchResult>, String>,
) -> impl Fn() -> Result<Vec<SearchResult>, String> {
    move || {
        let blocked_at = resting().lock().ok().and_then(|map| map.get(key).copied());
        if blocked_at.is_some_and(|at| at.elapsed() < ENGINE_REST) {
            return Err(format!("{key} is resting after a check."));
        }
        let outcome = run();
        if let Err(error) = &outcome
            && error.contains("asked for")
            && let Ok(mut map) = resting().lock()
        {
            map.insert(key, Instant::now());
        }
        outcome
    }
}

/// Engines are asked in order and the first with relevant results wins. A blocked engine is skipped.
pub(crate) fn search_query(query: &str, region: &str) -> Searched {
    let duckduckgo = guarded("duckduckgo-http", || search_duckduckgo(query, region));
    let bing = guarded("bing-http", || search_bing(query));
    let brave = guarded("brave-http", || search_brave(query));
    combine_engines(
        query,
        &[(DUCKDUCKGO, &duckduckgo), (BING, &bing), (BRAVE, &brave)],
    )
}

/// Engines can answer a bot with unrelated pages, so results must mention the query.
pub(crate) fn combine_engines(query: &str, attempts: &[Attempt]) -> Searched {
    let terms = query_terms(query);
    let mut skipped: Vec<String> = Vec::new();
    for (name, run) in attempts {
        match run() {
            Ok(results) => {
                let kept: Vec<SearchResult> = results
                    .into_iter()
                    .filter(|result| relevant(result, &terms))
                    .collect();
                if !kept.is_empty() {
                    skipped.retain(|skip| skip != name);
                    return Searched {
                        results: kept,
                        skipped,
                        engine: Some(name),
                    };
                }
            }
            Err(_) => {
                if !skipped.iter().any(|skip| skip == name) {
                    skipped.push((*name).to_string());
                }
            }
        }
    }
    Searched {
        results: Vec::new(),
        skipped,
        engine: None,
    }
}

const QUERY_STOP_WORDS: [&str; 22] = [
    "com", "net", "org", "www", "http", "https", "coupon", "coupons", "code", "codes", "promo",
    "discount", "free", "shipping", "reddit", "price", "buy", "the", "and", "for", "with", "deals",
];

/// Words a relevant result must contain. Empty for a bare identifier search, which is not filtered.
fn query_terms(query: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    for word in query.split_whitespace() {
        if word.starts_with("-site:") || word.starts_with("site:") {
            continue;
        }
        for token in word.split(|character: char| !character.is_ascii_alphanumeric()) {
            let token = token.to_ascii_lowercase();
            if token.len() >= 3
                && !QUERY_STOP_WORDS.contains(&token.as_str())
                && !terms.contains(&token)
            {
                terms.push(token);
            }
        }
    }
    if terms
        .iter()
        .all(|term| term.chars().all(|character| character.is_ascii_digit()))
    {
        return Vec::new();
    }
    terms
}

fn relevant(result: &SearchResult, terms: &[String]) -> bool {
    if terms.is_empty() {
        return true;
    }
    let blob = format!("{} {} {}", result.title, result.snippet, result.url).to_ascii_lowercase();
    let needed = terms.len().div_ceil(3).max(1);
    terms.iter().filter(|term| blob.contains(term.as_str())).count() >= needed
}

fn search_duckduckgo(query: &str, region: &str) -> Result<Vec<SearchResult>, String> {
    engine_results(DUCKDUCKGO, &fetch_search(&engine_url(DUCKDUCKGO, query, region))?)
}

fn search_bing(query: &str) -> Result<Vec<SearchResult>, String> {
    engine_results(BING, &fetch_search(&engine_url(BING, query, ""))?)
}

fn search_brave(query: &str) -> Result<Vec<SearchResult>, String> {
    engine_results(BRAVE, &fetch_search(&engine_url(BRAVE, query, ""))?)
}

/// Address of an engine's results page for `query`.
pub(crate) fn engine_url(engine: &str, query: &str, region: &str) -> String {
    let base = match engine {
        DUCKDUCKGO => SEARCH_URL,
        BING => BING_URL,
        _ => BRAVE_URL,
    };
    let mut url = url::Url::parse(base).expect("search URLs are valid");
    url.query_pairs_mut().append_pair("q", query);
    if engine == DUCKDUCKGO && !region.is_empty() {
        url.query_pairs_mut().append_pair("kl", region);
    }
    url.to_string()
}

/// Results from an engine's page. A check page with no results is an error.
pub(crate) fn engine_results(engine: &str, body: &str) -> Result<Vec<SearchResult>, String> {
    let results = match engine {
        DUCKDUCKGO => parse_results(body, RESULTS_PER_QUERY),
        BING => parse_bing(body, RESULTS_PER_QUERY),
        _ => parse_brave(body, RESULTS_PER_QUERY),
    };
    let blocked = if engine == DUCKDUCKGO {
        body.contains("anomaly") || blocked_page(body)
    } else {
        blocked_page(body)
    };
    if results.is_empty() && blocked {
        return Err(format!("{engine} asked for a check."));
    }
    Ok(results)
}

fn fetch_search(url: &str) -> Result<String, String> {
    ureq::get(url)
        .timeout(Duration::from_secs(12))
        .set("User-Agent", USER_AGENT)
        .set(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .set("Accept-Language", "en-US,en;q=0.9")
        .call()
        .map_err(|error| match error {
            ureq::Error::Status(403 | 429, _) => "The web search asked for a check.".to_string(),
            error => format!("Couldn't reach the web search. {error}"),
        })?
        .into_string()
        .map_err(|error| error.to_string())
}

pub(crate) fn blocked_page(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    lower.contains("cf-challenge")
        || lower.contains("captcha")
        || lower.contains("unusual traffic")
        || lower.contains("just a moment")
}

/// The result address behind a Bing `/ck/a` tracking link, which carries it as `u=a1<base64url>`.
fn bing_target(href: &str) -> Option<String> {
    use base64::Engine;
    let parsed = url::Url::parse(href).ok()?;
    if !parsed.host_str().is_some_and(|host| host.ends_with("bing.com")) {
        return Some(href.to_string());
    }
    let encoded = parsed
        .query_pairs()
        .find(|(key, _)| key == "u")
        .map(|(_, value)| value.into_owned())?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded.strip_prefix("a1")?.trim_end_matches('='))
        .ok()?;
    let target = String::from_utf8(bytes).ok()?;
    (target.starts_with("http://") || target.starts_with("https://")).then_some(target)
}

fn parse_bing(html: &str, limit: usize) -> Vec<SearchResult> {
    parse_blocks(html, "class=\"b_algo\"", limit, |block| {
        let heading = block.find("<h2").map(|index| &block[index..]).unwrap_or(block);
        let href = bing_target(&first_href(heading)?)?;
        if store_host(&href).is_some_and(|host| host.contains("bing.com")) {
            return None;
        }
        let title = anchor_text(heading).unwrap_or_default();
        let snippet = paragraph_text(block);
        (!title.is_empty()).then_some(SearchResult {
            title,
            url: href,
            snippet,
        })
    })
}

fn parse_brave(html: &str, limit: usize) -> Vec<SearchResult> {
    parse_blocks(html, "data-type=\"web\"", limit, |block| {
        let href = first_href(block)?;
        if store_host(&href).is_some_and(|host| host.contains("brave.com")) {
            return None;
        }
        let title = marked_text(block, "class=\"title").or_else(|| anchor_text(block))?;
        let snippet = marked_text(block, "snippet-description").unwrap_or_default();
        (!title.is_empty()).then_some(SearchResult {
            title,
            url: href,
            snippet,
        })
    })
}

fn parse_blocks(
    html: &str,
    marker: &str,
    limit: usize,
    read: impl Fn(&str) -> Option<SearchResult>,
) -> Vec<SearchResult> {
    let mut out = Vec::new();
    let mut offset = 0;
    while out.len() < limit {
        let Some(found) = html[offset..].find(marker) else {
            break;
        };
        let at = offset + found;
        let next = html[at + marker.len()..]
            .find(marker)
            .map(|index| at + marker.len() + index)
            .unwrap_or(html.len());
        if let Some(result) = read(&html[at..next]) {
            if !out.iter().any(|kept: &SearchResult| kept.url == result.url) {
                out.push(result);
            }
        }
        offset = at + marker.len();
    }
    out
}

fn first_href(block: &str) -> Option<String> {
    let start = block.find("href=\"")? + 6;
    let end = block[start..].find('"')? + start;
    let href = decode_entities(&block[start..end]);
    if href.starts_with("http://") || href.starts_with("https://") {
        Some(href)
    } else {
        None
    }
}

fn anchor_text(block: &str) -> Option<String> {
    let start = block.find("href=\"")?;
    let open = block[start..].find('>')? + start + 1;
    let end = block[open..].find("</a>")? + open;
    let title = text(&block[open..end]);
    (!title.is_empty()).then_some(title)
}

fn paragraph_text(block: &str) -> String {
    let Some(start) = block.find("<p") else {
        return String::new();
    };
    let Some(open) = block[start..].find('>').map(|index| start + index + 1) else {
        return String::new();
    };
    let end = block[open..]
        .find("</p>")
        .map(|index| open + index)
        .unwrap_or(block.len());
    text(&block[open..end])
}

fn marked_text(block: &str, marker: &str) -> Option<String> {
    let start = block.find(marker)?;
    let open = block[start..].find('>')? + start + 1;
    let end = block[open..]
        .find('<')
        .map(|index| open + index)
        .unwrap_or(block.len());
    let value = text(&block[open..end]);
    (!value.is_empty()).then_some(value)
}

fn rank_candidates(
    sources: &[SearchResult],
    page_url: &str,
    facts: &prices::ProductFacts,
) -> Vec<SearchResult> {
    let current = store_host(page_url);
    let identifier = facts
        .identifier()
        .map(|(_, value)| value.to_ascii_lowercase())
        .unwrap_or_default();
    let mut ranked: Vec<(i32, &SearchResult)> = sources
        .iter()
        .filter(|source| {
            !same_url(&source.url, page_url)
                && store_host(&source.url) != current
                && !skip_host(&source.url)
        })
        .map(|source| {
            let blob = format!("{} {}", source.title, source.url).to_ascii_lowercase();
            let mut score = 0;
            if !identifier.is_empty() && blob.contains(&identifier) {
                score += 2;
            }
            if snippet_price(&source.snippet)
                .or_else(|| snippet_price(&source.title))
                .is_some()
            {
                score += 1;
            }
            (score, source)
        })
        .collect();
    ranked.sort_by(|left, right| right.0.cmp(&left.0));
    ranked
        .into_iter()
        .take(CONFIRM_LIMIT)
        .map(|(_, source)| source.clone())
        .collect()
}

pub(crate) fn skip_host(url: &str) -> bool {
    store_host(url).is_some_and(|host| {
        [
            "reddit.com",
            "youtube.com",
            "facebook.com",
            "pinterest.com",
            "wikipedia.org",
        ]
        .iter()
        .any(|blocked| host == *blocked || host.ends_with(&format!(".{blocked}")))
    })
}

fn picked_urls(reply: &serde_json::Value, sources: &[SearchResult]) -> Vec<SearchResult> {
    let Some(items) = reply.get("urls").and_then(|item| item.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items {
        let Some(url) = item.as_str() else {
            continue;
        };
        let Some(source) = sources.iter().find(|source| same_url(&source.url, url)) else {
            continue;
        };
        if !out
            .iter()
            .any(|kept: &SearchResult| same_url(&kept.url, &source.url))
        {
            out.push(source.clone());
        }
        if out.len() >= CONFIRM_LIMIT {
            break;
        }
    }
    out
}

fn ai_pick(
    app: &AppHandle,
    current: &settings::BrowserSettings,
    name: &str,
    store: &str,
    facts: &prices::ProductFacts,
    sources: &[SearchResult],
) -> Option<serde_json::Value> {
    let identifier = facts
        .identifier()
        .map(|(kind, value)| format!("{kind} {value}"))
        .unwrap_or_else(|| "none".into());
    let mut results = String::new();
    for (index, result) in sources.iter().take(24).enumerate() {
        results.push_str(&format!(
            "[{}] {}\nURL: {}\n{}\n\n",
            index + 1,
            result.title,
            result.url,
            result.snippet
        ));
    }
    let prompt = format!(
        "Product: {name}\nBrand: {}\nIdentifier: {identifier}\nCurrent store: {store}\n\nSearch results:\n{results}\
Return a JSON object with these keys:\n\
- \"summary\": one or two sentences on whether another store sells this product.\n\
- \"coupons\": a list of {{\"code\", \"description\", \"source\"}} for coupon codes that appear word for word in the results. \"source\" is that result's URL.\n\
- \"urls\": the result URLs that are this same product sold by another store, not a review, an accessory, or {store}.\n\
Use only the results above. Do not invent codes or URLs.",
        if facts.brand.is_empty() {
            "unknown"
        } else {
            facts.brand.as_str()
        }
    );
    let reply = tools::chat(
        app,
        current,
        &current.shopping_model,
        "You match a product to web search results. Reply with one JSON object and nothing else.",
        &prompt,
        true,
        Duration::from_secs(45),
    )
    .ok()?;
    parse_reply(&reply)
}

fn search_note(facts: &prices::ProductFacts, skipped: &[String], none_confirmed: bool) -> String {
    let mut parts = Vec::new();
    if let Some((kind, value)) = facts.identifier() {
        parts.push(format!("Searched {kind} {value}."));
    }
    if skipped.len() == 1 {
        parts.push(format!("{} was skipped.", skipped[0]));
    } else if skipped.len() > 1 {
        parts.push(format!("{} were skipped.", skipped.join(", ")));
    }
    if none_confirmed {
        parts.push("No price could be confirmed.".into());
    }
    parts.join(" ")
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

pub(crate) fn attribute(tag: &str, name: &str) -> Option<String> {
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

pub(crate) fn text(html: &str) -> String {
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

pub(crate) fn decode_entities(text: &str) -> String {
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

pub(crate) fn parse_reply(reply: &str) -> Option<serde_json::Value> {
    let start = reply.find('{')?;
    let end = reply.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<serde_json::Value>(&reply[start..=end])
        .ok()
        .filter(serde_json::Value::is_object)
}

pub(crate) fn field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

pub(crate) fn same_url(left: &str, right: &str) -> bool {
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
        SearchResult, accept_confirmed_price, build_queries, clean_name, combine_engines,
        extract_coupons, extract_offers, grounded_coupons, grounded_offers, parse_bing,
        parse_brave, parse_reply, parse_results, search_note, target_url, variant_mismatch,
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
    fn queries_include_the_identifier_and_other_stores() {
        let facts = crate::prices::ProductFacts {
            gtin: "0012345678905".into(),
            ..crate::prices::ProductFacts::default()
        };
        let queries = build_queries(
            "Blue Mug",
            "mugshop.test",
            &facts,
            &["bhphotovideo.com".into()],
            12,
        );
        assert!(queries.iter().any(|query| query == "0012345678905"));
        assert!(queries.iter().any(|query| query.contains("amazon")));
        assert!(
            queries
                .iter()
                .any(|query| query.contains("-site:mugshop.test"))
        );
        assert!(
            queries
                .iter()
                .any(|query| query.contains("bhphotovideo.com"))
        );
    }

    #[test]
    fn a_blocked_engine_is_skipped_when_another_returns_results() {
        let hit = SearchResult {
            title: "Blue Mug".into(),
            url: "https://other.test/mug".into(),
            snippet: "$9.99".into(),
        };
        let blocked = || Err("captcha".to_string());
        let found = move || Ok(vec![hit.clone()]);
        let searched = combine_engines(
            "\"Blue Mug\" price",
            &[("DuckDuckGo", &blocked), ("Bing", &found), ("Brave", &blocked)],
        );
        assert_eq!(searched.results.len(), 1);
        assert_eq!(searched.results[0].url, "https://other.test/mug");
        assert_eq!(searched.skipped, vec!["DuckDuckGo".to_string()]);
        assert_eq!(searched.engine, Some("Bing"));
    }

    #[test]
    fn every_blocked_engine_is_named() {
        let blocked = || Err("captcha".to_string());
        let searched = combine_engines(
            "Blue Mug",
            &[("DuckDuckGo", &blocked), ("Bing", &blocked), ("Brave", &blocked)],
        );
        assert!(searched.results.is_empty());
        assert_eq!(searched.skipped, vec!["DuckDuckGo", "Bing", "Brave"]);
        assert_eq!(searched.engine, None);
    }

    #[test]
    fn unrelated_results_fall_through_to_the_next_engine() {
        let junk = || {
            Ok(vec![SearchResult {
                title: "Changing your pickup location".into(),
                url: "https://help.uber.com/riders".into(),
                snippet: "Update where your driver meets you.".into(),
            }])
        };
        let real = || {
            Ok(vec![SearchResult {
                title: "Katin USA Promo Codes".into(),
                url: "https://coupons.test/katinusa".into(),
                snippet: "Use code SAVE20.".into(),
            }])
        };
        let searched = combine_engines(
            "site:retailmenot.com katinusa.com",
            &[("Bing", &junk), ("Brave", &real)],
        );
        assert_eq!(searched.engine, Some("Brave"));
        assert_eq!(searched.results[0].url, "https://coupons.test/katinusa");
        assert!(super::query_terms("0012345678905").is_empty());
    }

    #[test]
    fn bing_and_brave_results_keep_the_product_link() {
        let bing = r#"<li class="b_algo"><h2><a href="https://other.test/mug">Blue Mug</a></h2><div class="b_caption"><p>Blue Mug for $9.99</p></div></li>"#;
        let parsed = parse_bing(bing, 8);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].url, "https://other.test/mug");
        assert_eq!(parsed[0].title, "Blue Mug");
        assert!(parsed[0].snippet.contains("9.99"));

        let tracked = r#"<li class="b_algo"><h2><a href="https://www.bing.com/ck/a?!&amp;&amp;p=abc&amp;u=a1aHR0cHM6Ly9vdGhlci50ZXN0L211Zw&amp;ntb=1">Blue Mug</a></h2><p>Blue Mug for $9.99</p></li>"#;
        let parsed = parse_bing(tracked, 8);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].url, "https://other.test/mug");

        let brave = r#"<div class="snippet" data-type="web"><a href="https://other.test/mug"><div class="title">Blue Mug</div></a><div class="snippet-description">Blue Mug for $9.99</div></div>"#;
        let parsed = parse_brave(brave, 8);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].url, "https://other.test/mug");
        assert_eq!(parsed[0].title, "Blue Mug");
        assert!(parsed[0].snippet.contains("9.99"));
    }

    #[test]
    fn the_search_note_names_the_identifier_and_a_skipped_engine() {
        let facts = crate::prices::ProductFacts {
            gtin: "0012345678905".into(),
            ..crate::prices::ProductFacts::default()
        };
        let note = search_note(&facts, &["DuckDuckGo".into()], true);
        assert!(note.contains("GTIN 0012345678905"));
        assert!(note.contains("DuckDuckGo was skipped"));
        assert!(note.contains("No price could be confirmed"));
    }

    #[test]
    fn a_confirmed_price_that_is_not_lower_is_dropped() {
        assert!(!accept_confirmed_price(14.0, Some(14.0)));
        assert!(!accept_confirmed_price(16.0, Some(14.0)));
        assert!(accept_confirmed_price(9.5, Some(14.0)));
        assert!(!accept_confirmed_price(0.0, Some(14.0)));
    }
}
