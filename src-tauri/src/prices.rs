//! Tracked products in `prices.json`. A v1 file still loads; the next save writes the current shape.

use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use url::Url;

use crate::{files, paths, privacy, tabs, webview2};

const PRICES: &str = "prices.json";
const HISTORY_CAP: usize = 500;
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PricePoint {
    pub at: u64,
    pub price: f64,
    pub currency: String,
    pub availability: String,
}

impl Default for PricePoint {
    fn default() -> Self {
        Self {
            at: 0,
            price: 0.0,
            currency: String::new(),
            availability: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ProductPrice {
    pub id: String,
    pub url: String,
    pub title: String,
    pub price: f64,
    pub currency: String,
    pub availability: String,
    pub image_url: String,
    pub store: String,
    pub tracked_at: u64,
    pub last_checked_at: u64,
    pub last_error: String,
    /// `fetch` or `tab` when the last check succeeded.
    pub last_method: String,
    pub target_price: Option<f64>,
    #[serde(default = "default_true")]
    pub alert_on_drop: bool,
    #[serde(default = "default_true")]
    pub alert_below_target: bool,
    #[serde(default = "default_true")]
    pub alert_back_in_stock: bool,
    #[serde(default = "default_true")]
    pub alert_drop_armed: bool,
    #[serde(default = "default_true")]
    pub alert_target_armed: bool,
    #[serde(default = "default_true")]
    pub alert_stock_armed: bool,
    #[serde(default = "default_interval")]
    pub check_interval_hours: u32,
    pub paused: bool,
    pub note: String,
    pub tags: Vec<String>,
    pub list_price: Option<f64>,
    pub history: Vec<PricePoint>,
    pub previous_price: Option<f64>,
    pub previous_availability: String,
    pub dropped: bool,
    pub stock_changed: bool,
}

impl Default for ProductPrice {
    fn default() -> Self {
        Self {
            id: String::new(),
            url: String::new(),
            title: String::new(),
            price: 0.0,
            currency: String::new(),
            availability: String::new(),
            image_url: String::new(),
            store: String::new(),
            tracked_at: 0,
            last_checked_at: 0,
            last_error: String::new(),
            last_method: String::new(),
            target_price: None,
            alert_on_drop: true,
            alert_below_target: true,
            alert_back_in_stock: true,
            alert_drop_armed: true,
            alert_target_armed: true,
            alert_stock_armed: true,
            check_interval_hours: 12,
            paused: false,
            note: String::new(),
            tags: Vec::new(),
            list_price: None,
            history: Vec::new(),
            previous_price: None,
            previous_availability: String::new(),
            dropped: false,
            stock_changed: false,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_interval() -> u32 {
    12
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct PriceFile {
    items: Vec<ProductPrice>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceCheckProgress {
    pub checked: u32,
    pub total: u32,
    pub done: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedPricePatch {
    pub id: String,
    pub target_price: Option<f64>,
    #[serde(default)]
    pub clear_target: bool,
    pub alert_on_drop: Option<bool>,
    pub alert_below_target: Option<bool>,
    pub alert_back_in_stock: Option<bool>,
    pub check_interval_hours: Option<u32>,
    pub paused: Option<bool>,
    pub note: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlertNotice {
    pub title: String,
    pub body: String,
    pub launch: String,
}

pub fn parse_price(html: &str) -> Option<ProductPrice> {
    let mut price = price_from_json_ld(html)
        .or_else(|| price_from_meta(html))
        .or_else(|| price_from_microdata(html))
        .or_else(|| price_from_dom(html))?;
    if price.image_url.is_empty() {
        price.image_url = meta_content(html, "og:image").unwrap_or_default();
    }
    if price.title.is_empty() {
        price.title = meta_content(html, "og:title").unwrap_or_default();
    }
    Some(price)
}

/// Shipping text only when the page's structured data says so.
pub fn shipping_hint(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    if lower.contains("freeshipping")
        || lower.contains("\"shippingrate\"") && lower.contains("\"value\":0")
        || lower.contains("\"shippingrate\"") && lower.contains("\"value\": 0")
    {
        if lower.contains("shipping") {
            return "Free shipping".into();
        }
    }
    if lower.contains("schema.org/freeshipping") {
        return "Free shipping".into();
    }
    String::new()
}

pub fn store_host(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    Some(host.strip_prefix("www.").unwrap_or(&host).to_string())
}

pub fn canonical_url(raw: &str) -> Option<String> {
    let mut url = Url::parse(raw.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    url.set_host(Some(&host)).ok()?;
    url.set_fragment(None);
    let pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
    let kept: Vec<(String, String)> = pairs
        .into_iter()
        .filter(|(name, _)| !privacy::is_tracking(name))
        .collect();
    if kept.is_empty() {
        url.set_query(None);
    } else {
        url.query_pairs_mut().clear().extend_pairs(&kept);
    }
    let mut text = url.to_string();
    if url.query().is_none() && text.ends_with('/') && url.path() != "/" {
        text.pop();
    }
    Some(text)
}

pub fn product_key(url: &str, title: &str) -> String {
    if let Some(canonical) = canonical_url(url) {
        return format!("url:{canonical}");
    }
    format!(
        "name:{}|{}",
        normalize_name(title),
        store_host(url).unwrap_or_default()
    )
}

pub fn same_tracked(existing: &ProductPrice, url: &str, title: &str) -> bool {
    match (canonical_url(&existing.url), canonical_url(url)) {
        (Some(left), Some(right)) => left == right,
        _ => {
            let store = store_host(url).unwrap_or_default();
            let existing_store = if existing.store.is_empty() {
                store_host(&existing.url).unwrap_or_default()
            } else {
                existing.store.clone()
            };
            let name = normalize_name(title);
            !name.is_empty()
                && name == normalize_name(&existing.title)
                && !store.is_empty()
                && store == existing_store
        }
    }
}

pub fn normalize_name(title: &str) -> String {
    title
        .split(" | ")
        .next()
        .unwrap_or(title)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

pub fn list_prices(app: &AppHandle) -> Vec<ProductPrice> {
    let path = paths::data_file(app, PRICES);
    let mut file: PriceFile = files::read_json(&path);
    let mut changed = false;
    for item in &mut file.items {
        if migrate_item(item) {
            changed = true;
        }
    }
    if changed {
        let _ = save_prices(app, &file.items);
    }
    file.items
}

pub fn track_price(app: &AppHandle, id: u32) -> Result<ProductPrice, String> {
    let fresh = read_page_price(app, id)?;
    let items = upsert(app, fresh, true)?;
    items
        .into_iter()
        .find(|item| same_tracked(item, &tabs::tab_page_url(app, id).unwrap_or_default(), ""))
        .ok_or_else(|| "Couldn't save this product.".to_string())
}

pub fn untrack_price(app: &AppHandle, id: &str) -> Result<Vec<ProductPrice>, String> {
    let mut items = list_prices(app);
    let before = items.len();
    items.retain(|item| item.id != id);
    if items.len() == before {
        return Err("That product is not being tracked.".into());
    }
    publish(app, items)
}

pub fn update_tracked(
    app: &AppHandle,
    patch: TrackedPricePatch,
) -> Result<Vec<ProductPrice>, String> {
    let mut items = list_prices(app);
    let Some(item) = items.iter_mut().find(|item| item.id == patch.id) else {
        return Err("That product is not being tracked.".into());
    };
    if patch.clear_target {
        item.target_price = None;
        item.alert_target_armed = true;
    } else if let Some(target) = patch.target_price {
        if target <= 0.0 {
            return Err("Enter a target above zero.".into());
        }
        item.target_price = Some(target);
        item.alert_target_armed = item.price > target;
    }
    if let Some(value) = patch.alert_on_drop {
        item.alert_on_drop = value;
    }
    if let Some(value) = patch.alert_below_target {
        item.alert_below_target = value;
    }
    if let Some(value) = patch.alert_back_in_stock {
        item.alert_back_in_stock = value;
    }
    if let Some(hours) = patch.check_interval_hours {
        item.check_interval_hours = hours.clamp(1, 24 * 30);
    }
    if let Some(paused) = patch.paused {
        item.paused = paused;
    }
    if let Some(note) = patch.note {
        item.note = note.chars().take(500).collect();
    }
    if let Some(tags) = patch.tags {
        item.tags = tags
            .into_iter()
            .map(|tag| tag.trim().chars().take(32).collect())
            .filter(|tag: &String| !tag.is_empty())
            .take(12)
            .collect();
    }
    publish(app, items)
}

pub fn preview_url(url: &str) -> Result<ProductPrice, String> {
    let fetched = fetch_page(url)?;
    Ok(fetched.product)
}

pub fn add_url(app: &AppHandle, url: &str) -> Result<Vec<ProductPrice>, String> {
    let fetched = fetch_page(url)?;
    upsert(app, fetched.product, false)
}

pub fn check_price(app: &AppHandle, id: &str) -> Result<Vec<ProductPrice>, String> {
    let mut items = list_prices(app);
    let Some(index) = items.iter().position(|item| item.id == id) else {
        return Err("That product is not being tracked.".into());
    };
    check_one(app, &mut items[index], true);
    publish(app, items)
}

pub fn check_prices(app: &AppHandle, ids: Option<&[String]>) -> Result<Vec<ProductPrice>, String> {
    let mut items = list_prices(app);
    let selected: Vec<usize> = match ids {
        Some(ids) => {
            let wanted: HashSet<&str> = ids.iter().map(String::as_str).collect();
            items
                .iter()
                .enumerate()
                .filter(|(_, item)| wanted.contains(item.id.as_str()))
                .map(|(index, _)| index)
                .collect()
        }
        None => (0..items.len()).collect(),
    };
    let total = selected.len() as u32;
    emit_progress(app, 0, total, false);
    let mut done = 0u32;
    for chunk in selected.chunks(4) {
        let plan: Vec<(String, Option<u32>)> = chunk
            .iter()
            .map(|index| {
                let item = &items[*index];
                (item.url.clone(), open_tab(app, &item.url))
            })
            .collect();
        let fetched = fetch_batch(&plan);
        for (offset, outcome) in fetched.into_iter().enumerate() {
            apply_outcome(app, &mut items[chunk[offset]], outcome, true);
            done += 1;
            emit_progress(app, done, total, false);
        }
    }
    let saved = publish(app, items)?;
    emit_progress(app, total, total, true);
    Ok(saved)
}

/// Checks items whose interval has elapsed. Failures are recorded and do not alert.
pub fn check_due(app: &AppHandle) {
    let now = now_ms();
    let mut items = list_prices(app);
    let mut any = false;
    for item in &mut items {
        if item.paused || !interval_elapsed(item, now) {
            continue;
        }
        check_one(app, item, true);
        any = true;
    }
    if any {
        let _ = publish(app, items);
    }
}

pub fn start(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(30));
        loop {
            check_due(&app);
            std::thread::sleep(Duration::from_secs(15 * 60));
        }
    });
}

fn interval_elapsed(item: &ProductPrice, now: u64) -> bool {
    if item.last_checked_at == 0 {
        return true;
    }
    let hours = u64::from(item.check_interval_hours.max(1));
    now.saturating_sub(item.last_checked_at) >= hours * 60 * 60 * 1000
}

fn check_one(app: &AppHandle, item: &mut ProductPrice, alert: bool) {
    let tab = open_tab(app, &item.url);
    let outcome = if tab.is_some() {
        BatchOutcome::PreferTab(tab)
    } else {
        BatchOutcome::Fetched(fetch_page(&item.url))
    };
    apply_outcome(app, item, outcome, alert);
}

enum BatchOutcome {
    PreferTab(Option<u32>),
    Fetched(Result<FetchedPage, String>),
}

fn fetch_batch(plan: &[(String, Option<u32>)]) -> Vec<BatchOutcome> {
    let mut outcomes = Vec::with_capacity(plan.len());
    let fetch_targets: Vec<(usize, String)> = plan
        .iter()
        .enumerate()
        .filter_map(|(index, (url, tab))| tab.is_none().then(|| (index, url.clone())))
        .collect();
    let mut fetched: Vec<(usize, Result<FetchedPage, String>)> = Vec::new();
    for chunk in fetch_targets.chunks(4) {
        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for (index, url) in chunk {
                let url = url.clone();
                handles.push((*index, scope.spawn(move || fetch_page(&url))));
            }
            for (index, handle) in handles {
                fetched.push((
                    index,
                    handle
                        .join()
                        .unwrap_or_else(|_| Err("Couldn't check this price.".into())),
                ));
            }
        });
    }
    for (index, (_url, tab)) in plan.iter().enumerate() {
        if tab.is_some() {
            outcomes.push(BatchOutcome::PreferTab(*tab));
        } else if let Some((_, result)) = fetched.iter().find(|(found, _)| *found == index) {
            outcomes.push(BatchOutcome::Fetched(match result {
                Ok(page) => Ok(FetchedPage {
                    product: page.product.clone(),
                }),
                Err(error) => Err(error.clone()),
            }));
        } else {
            outcomes.push(BatchOutcome::Fetched(Err(
                "Couldn't check this price.".into()
            )));
        }
    }
    outcomes
}

fn apply_outcome(app: &AppHandle, item: &mut ProductPrice, outcome: BatchOutcome, alert: bool) {
    let at = now_ms();
    match outcome {
        BatchOutcome::PreferTab(tab) => {
            if let Some(id) = tab {
                match read_page_price(app, id) {
                    Ok(mut fresh) => {
                        fresh.last_method = "tab".into();
                        finish_success(item, &fresh, at, alert);
                        return;
                    }
                    Err(_) => {}
                }
            }
            match fetch_page(&item.url) {
                Ok(fetched) => finish_success(item, &fetched.product, at, alert),
                Err(error) => finish_failure(item, at, &error),
            }
        }
        BatchOutcome::Fetched(Ok(fetched)) => finish_success(item, &fetched.product, at, alert),
        BatchOutcome::Fetched(Err(error)) => {
            if should_try_tab(&error) {
                if let Some(id) = open_tab(app, &item.url) {
                    if let Ok(mut fresh) = read_page_price(app, id) {
                        fresh.last_method = "tab".into();
                        finish_success(item, &fresh, at, alert);
                        return;
                    }
                }
            }
            finish_failure(item, at, &error);
        }
    }
}

fn should_try_tab(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("blocked")
        || lower.contains("captcha")
        || lower.contains("no price")
        || lower.contains("empty")
        || lower.contains("couldn't download")
}

fn finish_success(item: &mut ProductPrice, fresh: &ProductPrice, at: u64, alert: bool) {
    let notices = observe(item, fresh, at);
    if alert {
        for notice in notices {
            crate::system::toast(&notice.title, &notice.body, &notice.launch);
        }
    }
}

fn finish_failure(item: &mut ProductPrice, at: u64, error: &str) {
    item.last_checked_at = at;
    item.last_error = error.chars().take(240).collect();
    item.last_method.clear();
}

fn observe(item: &mut ProductPrice, fresh: &ProductPrice, at: u64) -> Vec<AlertNotice> {
    let previous_price = item.price;
    let previous_availability = item.availability.clone();
    item.previous_price = Some(previous_price);
    item.previous_availability = previous_availability.clone();
    item.price = fresh.price;
    if !fresh.currency.is_empty() {
        item.currency = fresh.currency.clone();
    }
    item.availability = fresh.availability.clone();
    if !fresh.title.is_empty() {
        item.title = fresh.title.clone();
    }
    if !fresh.image_url.is_empty() {
        item.image_url = fresh.image_url.clone();
    }
    if fresh.list_price.is_some() {
        item.list_price = fresh.list_price;
    }
    item.last_checked_at = at;
    item.last_error.clear();
    item.last_method = if fresh.last_method.is_empty() {
        "fetch".into()
    } else {
        fresh.last_method.clone()
    };
    push_history(item, at);
    let notices = alerts_for(item, previous_price, &previous_availability);
    refresh_display(item);
    notices
}

pub fn alerts_for(
    item: &mut ProductPrice,
    previous_price: f64,
    previous_availability: &str,
) -> Vec<AlertNotice> {
    let mut notices = Vec::new();
    let dropped = item.price < previous_price;
    if item.alert_on_drop {
        if dropped && item.alert_drop_armed {
            notices.push(notice(item, "Price drop"));
            item.alert_drop_armed = false;
        } else if !dropped {
            item.alert_drop_armed = true;
        }
    }
    if let Some(target) = item.target_price {
        let hit = item.price <= target;
        if item.alert_below_target {
            if hit && item.alert_target_armed {
                notices.push(notice(item, "Target price"));
                item.alert_target_armed = false;
            } else if !hit {
                item.alert_target_armed = true;
            }
        }
    }
    let was_out = is_out_of_stock(previous_availability);
    let now_in = is_in_stock(&item.availability);
    let now_out = is_out_of_stock(&item.availability);
    if item.alert_back_in_stock {
        if was_out && now_in && item.alert_stock_armed {
            notices.push(notice(item, "Back in stock"));
            item.alert_stock_armed = false;
        } else if now_out {
            item.alert_stock_armed = true;
        }
    }
    notices
}

fn notice(item: &ProductPrice, kind: &str) -> AlertNotice {
    let price = format_money(item.price, &item.currency);
    AlertNotice {
        title: format!("{kind}: {}", item.title),
        body: price,
        launch: item.url.clone(),
    }
}

fn format_money(price: f64, currency: &str) -> String {
    if currency.is_empty() {
        format!("{price:.2}")
    } else {
        format!("{currency} {price:.2}")
    }
}

fn is_out_of_stock(availability: &str) -> bool {
    let lower = availability
        .to_ascii_lowercase()
        .replace([' ', '_', '-'], "");
    lower.contains("outofstock") || lower.contains("soldout") || lower.contains("discontinued")
}

fn is_in_stock(availability: &str) -> bool {
    if is_out_of_stock(availability) {
        return false;
    }
    let lower = availability
        .to_ascii_lowercase()
        .replace([' ', '_', '-'], "");
    lower.contains("instock")
        || lower.contains("limitedavailability")
        || lower.contains("preorder")
        || lower.contains("onlineonly")
}

fn upsert(
    app: &AppHandle,
    mut fresh: ProductPrice,
    from_tab: bool,
) -> Result<Vec<ProductPrice>, String> {
    if fresh.url.trim().is_empty() {
        return Err("This page has no address to track.".into());
    }
    if fresh.price <= 0.0 {
        return Err("No product price was found on this page.".into());
    }
    fresh.store = store_host(&fresh.url).unwrap_or_default();
    if from_tab {
        fresh.last_method = "tab".into();
    } else if fresh.last_method.is_empty() {
        fresh.last_method = "fetch".into();
    }
    let mut items = list_prices(app);
    let at = now_ms();
    if let Some(existing) = items
        .iter_mut()
        .find(|item| same_tracked(item, &fresh.url, &fresh.title))
    {
        let _ = observe(existing, &fresh, at);
        if existing.tracked_at == 0 {
            existing.tracked_at = at;
        }
    } else {
        fresh.id = new_id();
        fresh.tracked_at = at;
        fresh.last_checked_at = at;
        fresh.last_error.clear();
        push_history(&mut fresh, at);
        refresh_display(&mut fresh);
        items.insert(0, fresh);
    }
    publish(app, items)
}

fn publish(app: &AppHandle, items: Vec<ProductPrice>) -> Result<Vec<ProductPrice>, String> {
    save_prices(app, &items)?;
    let _ = app.emit("prices-changed", &items);
    Ok(items)
}

fn save_prices(app: &AppHandle, items: &[ProductPrice]) -> Result<(), String> {
    files::write_json(
        &paths::data_file(app, PRICES),
        &PriceFile {
            items: items.to_vec(),
        },
    )
}

pub fn migrate_item(item: &mut ProductPrice) -> bool {
    let mut changed = false;
    if item.id.is_empty() {
        item.id = new_id();
        changed = true;
    }
    if item.store.is_empty() {
        if let Some(store) = store_host(&item.url) {
            item.store = store;
            changed = true;
        }
    }
    if item.history.is_empty() && item.price > 0.0 {
        let at = if item.tracked_at == 0 {
            now_ms()
        } else {
            item.tracked_at
        };
        if let Some(previous) = item.previous_price {
            item.history.push(PricePoint {
                at: at.saturating_sub(1),
                price: previous,
                currency: item.currency.clone(),
                availability: item.previous_availability.clone(),
            });
        }
        item.history.push(PricePoint {
            at,
            price: item.price,
            currency: item.currency.clone(),
            availability: item.availability.clone(),
        });
        changed = true;
    }
    if item.tracked_at == 0 {
        item.tracked_at = item
            .history
            .first()
            .map(|point| point.at)
            .unwrap_or_else(now_ms);
        changed = true;
    }
    if item.check_interval_hours == 0 {
        item.check_interval_hours = 12;
        changed = true;
    }
    let before = item.history.len();
    cap_history(item);
    if item.history.len() != before {
        changed = true;
    }
    refresh_display(item);
    changed
}

fn refresh_display(item: &mut ProductPrice) {
    let first = item.history.first().map(|point| point.price);
    item.dropped = first.is_some_and(|first| item.price < first);
    let previous = item
        .history
        .iter()
        .rev()
        .nth(1)
        .map(|point| point.availability.clone())
        .unwrap_or_default();
    item.stock_changed = !item.availability.is_empty() && item.availability != previous;
}

fn push_history(item: &mut ProductPrice, at: u64) {
    let unchanged = item.history.last().is_some_and(|point| {
        (point.price - item.price).abs() < 0.000_1
            && point.availability == item.availability
            && point.currency == item.currency
    });
    if !unchanged {
        item.history.push(PricePoint {
            at,
            price: item.price,
            currency: item.currency.clone(),
            availability: item.availability.clone(),
        });
    }
    cap_history(item);
}

fn cap_history(item: &mut ProductPrice) {
    if item.history.len() > HISTORY_CAP {
        let extra = item.history.len() - HISTORY_CAP;
        item.history.drain(0..extra);
    }
}

fn new_id() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn open_tab(app: &AppHandle, url: &str) -> Option<u32> {
    let want = canonical_url(url)?;
    tabs::snapshot(app).tabs.into_iter().find_map(|tab| {
        if !tab.loaded {
            return None;
        }
        let current = tabs::tab_page_url(app, tab.id).unwrap_or_default();
        (canonical_url(&current) == Some(want.clone())).then_some(tab.id)
    })
}

struct FetchedPage {
    product: ProductPrice,
}

fn fetch_page(url: &str) -> Result<FetchedPage, String> {
    let parsed = Url::parse(url.trim()).map_err(|_| "Enter a product page address.".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Enter a product page address.".into());
    }
    match ureq::get(parsed.as_str())
        .timeout(Duration::from_secs(15))
        .set("User-Agent", USER_AGENT)
        .call()
    {
        Ok(response) => {
            let final_url = response.get_url().to_string();
            let body = response
                .into_string()
                .map_err(|_| "Couldn't download this page.".to_string())?;
            product_from_body(&final_url, &body)
        }
        Err(ureq::Error::Status(403, _)) => Err("The store blocked the check.".into()),
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().unwrap_or_default();
            if looks_like_challenge(&body) {
                Err("The store asked for a captcha.".into())
            } else if status == 404 {
                Err("That product page was not found.".into())
            } else {
                Err("Couldn't download this page.".into())
            }
        }
        Err(_) => Err("Couldn't download this page.".into()),
    }
}

fn product_from_body(url: &str, body: &str) -> Result<FetchedPage, String> {
    if body.trim().is_empty() {
        return Err("The page was empty.".into());
    }
    if looks_like_challenge(body) {
        return Err("The store asked for a captcha.".into());
    }
    let mut product =
        parse_price(body).ok_or_else(|| "No price was found on this page.".to_string())?;
    product.url = url.to_string();
    product.store = store_host(url).unwrap_or_default();
    product.last_method = "fetch".into();
    Ok(FetchedPage { product })
}

fn looks_like_challenge(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    lower.contains("cf-challenge")
        || lower.contains("just a moment")
        || lower.contains("attention required")
        || (lower.contains("captcha") && lower.contains("cloudflare"))
}

fn emit_progress(app: &AppHandle, checked: u32, total: u32, done: bool) {
    let _ = app.emit(
        "prices-check-progress",
        &PriceCheckProgress {
            checked,
            total,
            done,
        },
    );
}

const PRICE_SCRIPT: &str = r#"(function () {
  var scripts = document.querySelectorAll('script[type="application/ld+json"]');
  var blocks = [];
  scripts.forEach(function (script) { blocks.push(script.textContent || ""); });
  var amount = document.querySelector('meta[property="product:price:amount"]');
  var currency = document.querySelector('meta[property="product:price:currency"]');
  var image = document.querySelector('meta[property="og:image"]');
  var ogTitle = document.querySelector('meta[property="og:title"]');
  var micro = [];
  document.querySelectorAll('[itemprop="price"], [itemprop="priceCurrency"], [itemprop="availability"]').forEach(function (node) {
    micro.push(node.outerHTML);
  });
  var dom = "";
  var nodes = document.querySelectorAll('[data-testid*="price" i], [class*="price" i]');
  for (var i = 0; i < nodes.length && !dom; i++) {
    var text = (nodes[i].innerText || "").replace(/\s+/g, " ").trim();
    if (text && text.length < 48) dom = text;
  }
  return {
    blocks: blocks,
    amount: amount ? amount.content : "",
    currency: currency ? currency.content : "",
    image: image ? image.content : "",
    ogTitle: ogTitle ? ogTitle.content : "",
    micro: micro,
    dom: dom,
    title: document.title || ""
  };
})()"#;

pub fn read_page_price(app: &AppHandle, id: u32) -> Result<ProductPrice, String> {
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let url = tabs::tab_page_url(app, id).unwrap_or_default();
    let value = webview2::evaluate(
        &webview,
        PRICE_SCRIPT,
        Duration::from_secs(8),
        "Couldn't read a price on this page.",
    )?;
    let title = value
        .get("title")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .to_string();
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
    if let Some(image) = value.get("image").and_then(|item| item.as_str()) {
        if !image.is_empty() {
            html.push_str(&format!("<meta property=\"og:image\" content=\"{image}\">"));
        }
    }
    if let Some(og_title) = value.get("ogTitle").and_then(|item| item.as_str()) {
        if !og_title.is_empty() {
            html.push_str(&format!(
                "<meta property=\"og:title\" content=\"{og_title}\">"
            ));
        }
    }
    if let Some(micro) = value.get("micro").and_then(|item| item.as_array()) {
        for block in micro {
            if let Some(text) = block.as_str() {
                html.push_str(text);
            }
        }
    }
    if let Some(dom) = value.get("dom").and_then(|item| item.as_str()) {
        if !dom.is_empty() {
            html.push_str(&format!("<span class=\"price\">{dom}</span>"));
        }
    }
    let mut price =
        parse_price(&html).ok_or_else(|| "No product price was found on this page.".to_string())?;
    price.url = url;
    if price.title.is_empty() {
        price.title = title;
    }
    price.store = store_host(&price.url).unwrap_or_default();
    Ok(price)
}

fn price_from_meta(html: &str) -> Option<ProductPrice> {
    let amount = meta_content(html, "product:price:amount")?;
    let price = parse_amount(&amount)?;
    Some(filled(
        String::new(),
        price,
        meta_content(html, "product:price:currency").unwrap_or_default(),
        String::new(),
        None,
        String::new(),
    ))
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
        if let Some(price) = price_in_json(&search[start..end]) {
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
        return None;
    }
    let object = value.as_object()?;
    if let Some(graph) = object.get("@graph") {
        if let Some(price) = find_product(graph) {
            return Some(price);
        }
    }
    let product = is_type(value, "Product") || object.contains_key("offers");
    if product {
        if let Some(offers) = object.get("offers") {
            if let Some((price, currency, availability, list_price)) = offer_price(offers) {
                return Some(filled(
                    json_text(object.get("name")),
                    price,
                    currency,
                    availability,
                    list_price,
                    json_image(object.get("image")),
                ));
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

fn offer_price(offers: &serde_json::Value) -> Option<(f64, String, String, Option<f64>)> {
    let list: Vec<&serde_json::Value> = if let Some(items) = offers.as_array() {
        items.iter().collect()
    } else {
        vec![offers]
    };
    let mut aggregate = None;
    for offer in list {
        if is_type(offer, "AggregateOffer")
            || (offer.get("lowPrice").is_some() && offer.get("price").is_none())
        {
            if aggregate.is_none() {
                if let Some(low) = offer.get("lowPrice").and_then(json_price) {
                    aggregate = Some((
                        low,
                        json_text(offer.get("priceCurrency")),
                        availability_text(offer.get("availability")),
                        list_price_of(offer),
                    ));
                }
            }
            continue;
        }
        if let Some(price) = offer
            .get("price")
            .and_then(json_price)
            .or_else(|| nested_offer_price(offer))
        {
            return Some((
                price,
                json_text(offer.get("priceCurrency")),
                availability_text(offer.get("availability")),
                list_price_of(offer),
            ));
        }
    }
    aggregate
}

fn nested_offer_price(offer: &serde_json::Value) -> Option<f64> {
    offer.get("priceSpecification").and_then(|spec| {
        let specs: Vec<&serde_json::Value> = if let Some(items) = spec.as_array() {
            items.iter().collect()
        } else {
            vec![spec]
        };
        specs.iter().find_map(|spec| {
            let kind = json_text(spec.get("priceType")).to_ascii_lowercase();
            if kind.contains("listprice") {
                None
            } else {
                spec.get("price").and_then(json_price)
            }
        })
    })
}

fn list_price_of(offer: &serde_json::Value) -> Option<f64> {
    let spec = offer.get("priceSpecification")?;
    let specs: Vec<&serde_json::Value> = if let Some(items) = spec.as_array() {
        items.iter().collect()
    } else {
        vec![spec]
    };
    specs.into_iter().find_map(|spec| {
        let kind = json_text(spec.get("priceType")).to_ascii_lowercase();
        kind.contains("listprice")
            .then(|| spec.get("price").and_then(json_price))
            .flatten()
    })
}

fn filled(
    title: String,
    price: f64,
    currency: String,
    availability: String,
    list_price: Option<f64>,
    image_url: String,
) -> ProductPrice {
    ProductPrice {
        title,
        price,
        currency,
        availability,
        list_price,
        image_url,
        ..ProductPrice::default()
    }
}

fn is_type(value: &serde_json::Value, name: &str) -> bool {
    match value.get("@type") {
        Some(serde_json::Value::String(text)) => text.eq_ignore_ascii_case(name),
        Some(serde_json::Value::Array(items)) => items.iter().any(|item| {
            item.as_str()
                .is_some_and(|text| text.eq_ignore_ascii_case(name))
        }),
        _ => false,
    }
}

fn json_text(value: Option<&serde_json::Value>) -> String {
    value
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn availability_text(value: Option<&serde_json::Value>) -> String {
    json_text(value)
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string()
}

fn json_image(value: Option<&serde_json::Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    if let Some(text) = value.as_str() {
        return text.to_string();
    }
    if let Some(url) = value.get("url").and_then(|item| item.as_str()) {
        return url.to_string();
    }
    if let Some(items) = value.as_array() {
        for item in items {
            let image = json_image(Some(item));
            if !image.is_empty() {
                return image;
            }
        }
    }
    String::new()
}

fn json_price(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .filter(|price| *price > 0.0)
        .or_else(|| value.as_str().and_then(parse_amount))
}

fn price_from_microdata(html: &str) -> Option<ProductPrice> {
    let amount = itemprop_value(html, "price")?;
    let price = parse_amount(&amount)?;
    Some(filled(
        String::new(),
        price,
        itemprop_value(html, "priceCurrency").unwrap_or_default(),
        availability_text(
            itemprop_value(html, "availability")
                .map(|text| serde_json::Value::String(text))
                .as_ref(),
        ),
        None,
        String::new(),
    ))
}

fn itemprop_value(html: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("itemprop={quote}{name}{quote}");
        let Some(index) = html.find(&needle) else {
            continue;
        };
        let tag_start = html[..index].rfind('<')?;
        let tag_end = html[index..].find('>')? + index;
        let tag = &html[tag_start..tag_end];
        if let Some(content) = tag_attribute(tag, "content").or_else(|| tag_attribute(tag, "href"))
        {
            if !content.trim().is_empty() {
                return Some(content);
            }
        }
        let rest = &html[tag_end + 1..];
        let text_end = rest.find('<').unwrap_or(rest.len());
        let text = rest[..text_end].trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    None
}

fn price_from_dom(html: &str) -> Option<ProductPrice> {
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(rel) = lower[from..].find("price") {
        let at = from + rel;
        let Some(tag_start) = html[..at].rfind('<') else {
            break;
        };
        let Some(tag_rel) = html[at..].find('>') else {
            break;
        };
        let tag_end = at + tag_rel;
        let tag = &lower[tag_start..=tag_end];
        from = tag_end + 1;
        let interesting =
            (tag.contains("data-testid") || tag.contains("class=")) && tag.contains("price");
        if !interesting {
            continue;
        }
        let rest = &html[tag_end + 1..];
        let text_end = rest.find('<').unwrap_or(rest.len()).min(80);
        let text = rest[..text_end].trim();
        if let Some(price) = parse_amount(text) {
            return Some(filled(
                String::new(),
                price,
                currency_hint(text),
                String::new(),
                None,
                String::new(),
            ));
        }
    }
    None
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

fn parse_amount(text: &str) -> Option<f64> {
    let mut raw = String::new();
    let mut started = false;
    for character in text.chars() {
        if character.is_ascii_digit() || (started && (character == '.' || character == ',')) {
            started = true;
            raw.push(character);
        } else if started {
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
    (value > 0.0 && value < 10_000_000.0).then_some(value)
}

fn meta_content(html: &str, property: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("property={quote}{property}{quote}");
        if let Some(index) = html.find(&needle) {
            let tag_start = html[..index].rfind('<').unwrap_or(index);
            let tag_end = html[index..]
                .find('>')
                .map(|end| index + end)
                .unwrap_or(html.len());
            if let Some(content) = tag_attribute(&html[tag_start..tag_end], "content") {
                return Some(content);
            }
        }
    }
    None
}

fn tag_attribute(tag: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{name}={quote}");
        if let Some(index) = tag.find(&needle) {
            let start = index + needle.len();
            let end = tag[start..].find(quote)? + start;
            return Some(tag[start..end].to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        ProductPrice, alerts_for, canonical_url, migrate_item, parse_price, product_key,
        push_history, same_tracked, shipping_hint,
    };

    #[test]
    fn a_product_block_yields_its_price() {
        let html = r#"<script type="application/ld+json">{"@type":"Product","name":"Mug","image":"https://cdn.test/mug.jpg","offers":{"price":"12.50","priceCurrency":"USD","availability":"https://schema.org/InStock"}}</script>"#;
        let price = parse_price(html).expect("price");
        assert_eq!(price.price, 12.50);
        assert_eq!(price.currency, "USD");
        assert_eq!(price.availability, "InStock");
        assert_eq!(price.title, "Mug");
        assert_eq!(price.image_url, "https://cdn.test/mug.jpg");
    }

    #[test]
    fn graph_aggregate_microdata_and_a_blank_page() {
        let graph = r#"<script type="application/ld+json">{"@graph":[{"@type":"Organization","name":"Shop"},{"@type":["Product"],"name":"Lamp","offers":{"@type":"AggregateOffer","lowPrice":"40","highPrice":"55","priceCurrency":"USD"}}]}</script>"#;
        let price = parse_price(graph).expect("graph");
        assert_eq!(price.title, "Lamp");
        assert_eq!(price.price, 40.0);
        assert_eq!(price.currency, "USD");

        let both = r#"<script type="application/ld+json">{"@type":"Product","name":"Lamp","offers":[{"@type":"AggregateOffer","lowPrice":"10","priceCurrency":"USD"},{"@type":"Offer","price":"18","priceCurrency":"EUR","priceSpecification":{"@type":"UnitPriceSpecification","price":"24","priceType":"https://schema.org/ListPrice"}}]}</script>"#;
        let price = parse_price(both).expect("offer");
        assert_eq!(price.price, 18.0);
        assert_eq!(price.currency, "EUR");
        assert_eq!(price.list_price, Some(24.0));

        let micro = r#"<span itemprop="price" content="7.25"></span><meta itemprop="priceCurrency" content="GBP"><link itemprop="availability" href="https://schema.org/OutOfStock">"#;
        let price = parse_price(micro).expect("micro");
        assert_eq!(price.price, 7.25);
        assert_eq!(price.currency, "GBP");
        assert_eq!(price.availability, "OutOfStock");

        let image = r#"<meta property="og:image" content="https://cdn.test/a.jpg"><meta property="product:price:amount" content="3"><meta property="product:price:currency" content="USD">"#;
        let price = parse_price(image).expect("meta");
        assert_eq!(price.image_url, "https://cdn.test/a.jpg");
        assert_eq!(price.price, 3.0);

        let dom = r#"<div class="product-price">$4.50</div>"#;
        assert_eq!(parse_price(dom).expect("dom").price, 4.50);
        assert!(parse_price("<p>No product here.</p>").is_none());
        assert_eq!(
            shipping_hint(r#"{"@type":"Offer","shippingDetails":{"shippingRate":{"value":0}}}"#),
            "Free shipping"
        );
    }

    #[test]
    fn a_v1_item_gains_history_and_an_id() {
        let item: ProductPrice = serde_json::from_str(
            r#"{"url":"https://www.Shop.test/mug?utm_source=x&id=1","title":"Mug","price":10,"currency":"USD","previousPrice":12,"availability":"InStock"}"#,
        )
        .expect("v1");
        let mut item = item;
        assert!(migrate_item(&mut item));
        assert!(!item.id.is_empty());
        assert_eq!(item.store, "shop.test");
        assert_eq!(item.history.len(), 2);
        assert_eq!(item.history[0].price, 12.0);
        assert_eq!(item.history[1].price, 10.0);
        assert!(item.alert_on_drop);
        assert_eq!(item.check_interval_hours, 12);
        assert!(item.dropped);
        assert!(!migrate_item(&mut item));
    }

    #[test]
    fn canonical_urls_collapse_tracking_parameters() {
        assert_eq!(
            canonical_url("https://WWW.Shop.test/mug?utm_source=x&id=1"),
            canonical_url("https://shop.test/mug?id=1")
        );
        let left = ProductPrice {
            url: "https://shop.test/mug?utm_source=ad".into(),
            title: "Mug".into(),
            ..ProductPrice::default()
        };
        assert!(same_tracked(&left, "https://www.shop.test/mug", "Other"));
        assert!(!same_tracked(&left, "https://other.test/mug", "Mug"));
        assert_eq!(
            product_key("https://shop.test/mug?fbclid=1", "Mug"),
            product_key("https://www.shop.test/mug", "Mug")
        );
    }

    #[test]
    fn history_caps_at_five_hundred_points() {
        let mut item = ProductPrice {
            price: 1.0,
            currency: "USD".into(),
            ..ProductPrice::default()
        };
        for index in 0..520 {
            item.price = index as f64 + 1.0;
            push_history(&mut item, index);
        }
        assert_eq!(item.history.len(), 500);
        assert_eq!(item.history.first().unwrap().price, 21.0);
        assert_eq!(item.history.last().unwrap().price, 520.0);
        let len = item.history.len();
        push_history(&mut item, 900);
        assert_eq!(item.history.len(), len);
    }

    #[test]
    fn alerts_fire_once_and_rearm_when_the_condition_clears() {
        let mut item = ProductPrice {
            title: "Mug".into(),
            url: "https://shop.test/mug".into(),
            price: 8.0,
            currency: "USD".into(),
            availability: "InStock".into(),
            target_price: Some(9.0),
            ..ProductPrice::default()
        };
        let first = alerts_for(&mut item, 10.0, "OutOfStock");
        assert!(
            first
                .iter()
                .any(|notice| notice.title.starts_with("Price drop"))
        );
        assert!(
            first
                .iter()
                .any(|notice| notice.title.starts_with("Target price"))
        );
        assert!(
            first
                .iter()
                .any(|notice| notice.title.starts_with("Back in stock"))
        );
        let second = alerts_for(&mut item, 10.0, "OutOfStock");
        assert!(second.is_empty());
        item.price = 12.0;
        item.availability = "OutOfStock".into();
        let rearmed = alerts_for(&mut item, 8.0, "InStock");
        assert!(rearmed.is_empty());
        assert!(item.alert_drop_armed);
        assert!(item.alert_target_armed);
        assert!(item.alert_stock_armed);
    }
}
