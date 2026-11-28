use std::cell::RefCell;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::browse::{Page, Reader};
use super::{CouponSearch, StoreCoupons};
use crate::shopping::{self, Coupon, SearchResult};
use crate::{settings, tools};

const PAGE_LIMIT: usize = 8;
const FETCH_BATCH: usize = 4;
/// Background pages open at once.
const READERS: usize = 2;
const READ_TIMEOUT: Duration = Duration::from_secs(20);
const ENGINE_TIMEOUT: Duration = Duration::from_secs(15);
/// Searches typed back to back look like a bot, and DuckDuckGo starts asking for a check.
const ENGINE_PAUSE_MS: std::ops::Range<u64> = 1500..3000;
const COUPON_LIMIT: usize = 40;
const AI_LIMIT: usize = 30;
const CONTEXT: usize = 120;

const COUPON_SITES: [&str; 17] = [
    "retailmenot.com",
    "couponsolver.com",
    "couponfollow.com",
    "slickdeals.net",
    "coupons.com",
    "dealspotr.com",
    "knoji.com",
    "couponbirds.com",
    "wethrift.com",
    "joinhoney.com",
    "dontpayfull.com",
    "savings.com",
    "groupon.com",
    "couponcabin.com",
    "promocodes.com",
    "simplycodes.com",
    "reddit.com",
];

/// Uppercase words that pass `code_tokens` but are page text, not codes.
const COMMON_WORDS: &[&str] = &[
    "ABOUT", "AFTERPAY", "AMAZON", "APPLY", "AUTUMN", "BLACK", "BONUS", "CANCEL", "CASHBACK",
    "CHECKOUT", "CHRISTMAS", "CLEARANCE", "CLICK", "CLOSE", "CODES", "CONTACT", "COOKIE",
    "COOKIES", "COPIED", "COUPON", "COUPONS", "CYBER", "DEALS", "DECEMBER", "DETAILS",
    "DISCOUNT", "DISCOUNTS", "EASTER", "EMAIL", "ENDS", "ENTER", "EXCLUSIVE", "EXPIRED",
    "EXPIRES", "FAQS", "FEBRUARY", "FIRST", "FREESHIPPING", "FRIDAY", "GROUPON", "HOLIDAY",
    "HONEY", "HTTPS", "INSTORE", "JANUARY", "KLARNA", "LABOR", "LIMITED", "LOGIN", "MEMORIAL",
    "MILITARY", "MONDAY", "NOVEMBER", "NURSE", "OCTOBER", "OFFER", "OFFERS", "ONLINE", "ORDER",
    "ORDERS", "PAYPAL", "POINTS", "POLICY", "PRIME", "PRINTABLE", "PRIVACY", "PROMO", "PROMOS",
    "REBATE", "REDDIT", "REDEEM", "REGISTER", "RETAILMENOT", "REVEAL", "REWARD", "REWARDS",
    "SALES", "SAVINGS", "SENIOR", "SEPTEMBER", "SHIPPING", "SHOPS", "SIGNIN", "SIGNUP",
    "SITEWIDE", "SLICKDEALS", "SPRING", "STORE", "STORES", "STUDENT", "SUBMIT", "SUMMER",
    "TEACHER", "TERMS", "TODAY", "UPDATED", "VERIFIED", "VISIT", "WALMART", "WINTER", "RIGHT",
    "WEEKS", "WEEK", "DAYS", "HOURS", "MONTHS", "YEARS", "NIGHT", "TONIGHT", "SHOPPER",
    "WORKING", "WORKS", "HTML5", "UTF8", "COVID19", "MP3", "MP4", "USB3", "H264", "H265",
];

const UNITS: &[&str] = &[
    "ST", "ND", "RD", "TH", "PM", "AM", "MM", "CM", "KG", "LB", "LBS", "OZ", "GB", "TB", "MB",
    "HZ", "MHZ", "GHZ", "W", "V", "K", "X", "PCS", "PC", "ML", "IN", "FT", "MAH", "MP", "P",
];

const ATTRIBUTES: [&str; 6] = [
    "data-code",
    "data-clipboard-text",
    "data-coupon",
    "data-coupon-code",
    "data-promo",
    "data-promo-code",
];

const KEYWORDS: [&str; 6] = ["code", "coupon", "promo", "voucher", "use ", "enter "];

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CouponSearchProgress {
    pub tab_id: u32,
    pub store: String,
    /// `searching`, `reading`, `describing`, `done` or `error`.
    pub stage: String,
    pub message: String,
    /// Steps finished so far, out of `steps`.
    pub step: usize,
    pub steps: usize,
    /// Engine that answered, or the host of the page being read.
    pub source: String,
    /// `ok`, `empty` or `blocked` for a finished step. Empty while the step runs.
    pub outcome: String,
    /// Codes found so far.
    pub codes: usize,
    /// Search results kept so far.
    pub results: usize,
}

struct Reporter<'a> {
    app: &'a AppHandle,
    progress: RefCell<CouponSearchProgress>,
}

impl Reporter<'_> {
    fn send(&self, stage: &str, message: String, source: &str, outcome: &str, update: impl FnOnce(&mut CouponSearchProgress)) {
        let mut progress = self.progress.borrow_mut();
        progress.stage = stage.to_string();
        progress.message = message;
        progress.source = source.to_string();
        progress.outcome = outcome.to_string();
        update(&mut progress);
        let _ = self.app.emit("coupon-search-progress", &*progress);
    }
}

pub fn find_coupons(app: &AppHandle, id: u32, force: bool) -> Result<CouponSearch, String> {
    let store = super::resolve_store(app, id)?;
    let current = settings::get(app);
    let ai_available = crate::ai::available(app, &current);
    if !force {
        if let Some(cached) = super::fresh(app, &store, ai_available) {
            return Ok(cached);
        }
    }
    let product = tools::read_page_price(app, id)
        .ok()
        .map(|item| shopping::clean_name(&item.title))
        .filter(|name| !name.trim().is_empty());
    let queries = build_queries(&store, product.as_deref());
    let label = store_label(&store);
    let direct = direct_pages(&store, &label);
    let report = Reporter {
        app,
        progress: RefCell::new(CouponSearchProgress {
            tab_id: id,
            store: store.clone(),
            steps: direct.len() + queries.len() + PAGE_LIMIT + usize::from(ai_available),
            ..CouponSearchProgress::default()
        }),
    };
    let file = shopping::load_shopping(app);
    let feedback = super::store_feedback(&file, &store);
    let found = RefCell::new(Found::default());
    for entry in file.entries.iter().filter(|entry| entry.deals.store == store) {
        for coupon in entry.deals.coupons.iter().filter(|coupon| worth_keeping(coupon, &label, &feedback)) {
            found.borrow_mut().add_known(coupon);
        }
    }

    report.send("reading", "Opening the browser to read coupon sites".into(), "", "", |_| {});
    let readers: Vec<Reader> = (0..READERS).map_while(|_| Reader::open(app).ok()).collect();
    let mut pages_read = 0;
    let mut read_hosts: Vec<String> = Vec::new();
    let mut step = 0;
    read_pages(&readers, &direct, false, |url, page| {
        step += 1;
        let host = host_of(url);
        let mut found = found.borrow_mut();
        let before = found.items.len();
        let (message, outcome) = match page {
            Ok(page) if !page.url.to_ascii_lowercase().contains(&label) => {
                (format!("{host} has no page for {store}"), "empty")
            }
            Ok(page) => {
                pages_read += 1;
                read_hosts.push(host.clone());
                found.add_page(&shown_url(url), &page.html, &label);
                (read_message(&host, found.items.len() - before), outcome_of(found.items.len() - before))
            }
            Err(error) if error.contains("answered 404") || error.contains("answered 410") => {
                (format!("{host} has no page for {store}"), "empty")
            }
            Err(_) => (format!("{host} asked for a check"), "blocked"),
        };
        let codes = found.items.len();
        report.send("reading", message, &host, outcome, |progress| {
            progress.step = step;
            progress.codes = codes;
        });
    });

    let region = current.shopping_region.trim().to_string();
    let offset = direct.len();
    let gathered = shopping::gather_with(
        &queries,
        |query| search_everywhere(&readers, query, &region),
        |index, query| {
            report.send("searching", format!("Searching for “{query}”"), "", "", |progress| {
                progress.step = offset + index;
            });
        },
        |done| {
            let mut found = found.borrow_mut();
            for source in done.results {
                found.add_snippet(source, &label);
            }
            let codes = found.items.len();
            let (message, source, outcome) = match done.engine {
                Some(engine) => (
                    format!("{engine}: {} results for “{}”", done.results.len(), done.query),
                    engine,
                    "ok",
                ),
                None if !done.skipped.is_empty() => (
                    format!("{} asked for a check on “{}”", done.skipped.join(", "), done.query),
                    "",
                    "blocked",
                ),
                None => (format!("Nothing relevant for “{}”", done.query), "", "empty"),
            };
            report.send("searching", message, source, outcome, |progress| {
                progress.step = offset + done.index + 1;
                progress.codes = codes;
                progress.results += done.results.len();
            });
        },
    );
    let mut found = found.into_inner();
    if gathered.sources.is_empty() && found.items.is_empty() && pages_read == 0 {
        let message = no_results(&store, &gathered.skipped);
        report.send("error", message.clone(), "", "blocked", |_| {});
        return Err(message);
    }

    let pages: Vec<String> = coupon_pages(&gathered.sources, &store)
        .into_iter()
        .map(|source| source.url)
        .filter(|url| !read_hosts.contains(&host_of(url)))
        .collect();
    let page_step = offset + queries.len();
    report.send(
        "reading",
        if pages.is_empty() {
            "No coupon sites in the results to open".to_string()
        } else {
            format!("Opening {} coupon pages", pages.len())
        },
        "",
        "",
        |progress| {
            progress.steps = page_step + pages.len() + usize::from(ai_available);
            progress.step = page_step;
        },
    );
    let mut pages_done = 0;
    read_pages(&readers, &pages, true, |url, page| {
        pages_done += 1;
        let host = host_of(url);
        let before = found.items.len();
        let (message, outcome) = match page {
            Ok(page) => {
                pages_read += 1;
                found.add_page(url, &page.html, &label);
                (read_message(&host, found.items.len() - before), outcome_of(found.items.len() - before))
            }
            Err(_) => (format!("Couldn't open {host}"), "blocked"),
        };
        let codes = found.items.len();
        report.send("reading", message, &host, outcome, |progress| {
            progress.step = page_step + pages_done;
            progress.codes = codes;
        });
    });
    drop(readers);

    let mut coupons = found.into_coupons();
    let mut summary = String::new();
    if ai_available && !coupons.is_empty() {
        report.send(
            "describing",
            format!("Asking the assistant about {} codes", coupons.len().min(AI_LIMIT)),
            "",
            "",
            |_| {},
        );
        if let Some(reply) = ai_describe(app, &current, &store, &coupons) {
            summary = shopping::field(&reply, "summary");
            let worked: Vec<String> = feedback
                .iter()
                .filter(|item| item.worked)
                .map(|item| item.code.clone())
                .collect();
            apply_ai(&reply, &mut coupons, &worked);
        }
    }
    let codes = coupons.len().min(COUPON_LIMIT);
    report.send(
        "done",
        match codes {
            0 => format!("No codes found for {store}"),
            1 => format!("Found 1 code for {store}"),
            count => format!("Found {count} codes for {store}"),
        },
        "",
        "ok",
        |progress| {
            progress.step = progress.steps;
            progress.codes = codes;
        },
    );
    super::sort_coupons(&mut coupons);
    coupons.truncate(COUPON_LIMIT);
    let entry = StoreCoupons {
        store: store.clone(),
        checked_at: shopping::now_ms(),
        coupons,
        sources: gathered.sources,
        summary,
        search_note: search_note(pages_read, &gathered.skipped),
        pages_read,
        feedback: Vec::new(),
    };
    super::remember(app, entry, ai_available)
}

fn build_queries(store: &str, product: Option<&str>) -> Vec<String> {
    let mut queries = vec![
        format!("{store} coupon code"),
        format!("{store} promo code"),
        format!("{store} discount code"),
        format!("{store} free shipping code"),
        format!("{store} slickdeals"),
    ];
    if let Some(name) = product {
        queries.push(format!("{name} coupon"));
    }
    queries
}

/// First label of the host, for example `mugshop` from `shop.mugshop.co.uk` style hosts.
fn store_label(store: &str) -> String {
    let parts: Vec<&str> = store.split('.').collect();
    let pick = match parts.len() {
        0 | 1 => store,
        2 => parts[0],
        _ if parts[parts.len() - 2].len() <= 3 => parts[parts.len() - 3],
        _ => parts[parts.len() - 2],
    };
    pick.to_ascii_lowercase()
}

fn coupon_site(url: &str) -> bool {
    shopping::store_host(url).is_some_and(|host| {
        COUPON_SITES
            .iter()
            .any(|site| host == *site || host.ends_with(&format!(".{site}")))
    })
}

/// The results most likely to list codes for this store, best first.
fn coupon_pages(sources: &[SearchResult], store: &str) -> Vec<SearchResult> {
    let label = store_label(store);
    let mut ranked: Vec<(i32, &SearchResult)> = sources
        .iter()
        .filter_map(|source| {
            let title = source.title.to_ascii_lowercase();
            let mut score = 0;
            if coupon_site(&source.url) {
                score += 3;
            }
            if ["coupon", "promo", "discount code", "voucher"]
                .iter()
                .any(|word| title.contains(word))
            {
                score += 2;
            }
            if title.contains(&label) || source.url.to_ascii_lowercase().contains(&label) {
                score += 2;
            }
            if !shopping::code_tokens(&format!("{} {}", source.title, source.snippet)).is_empty() {
                score += 1;
            }
            (score >= 3).then_some((score, source))
        })
        .collect();
    ranked.sort_by(|left, right| right.0.cmp(&left.0));
    ranked
        .into_iter()
        .take(PAGE_LIMIT)
        .map(|(_, source)| source.clone())
        .collect()
}

/// Product searches also picked up title words such as `TRUCKER`, so an earlier code needs a digit,
/// a coupon site, or a mark from the user.
fn worth_keeping(coupon: &Coupon, label: &str, feedback: &[crate::shopping::CouponFeedback]) -> bool {
    if !plausible(&coupon.code, label) {
        return false;
    }
    coupon.code.chars().any(|character| character.is_ascii_digit())
        || coupon_site(&coupon.source)
        || feedback
            .iter()
            .any(|item| item.code.eq_ignore_ascii_case(&coupon.code))
}

fn host_of(url: &str) -> String {
    shopping::store_host(url).unwrap_or_else(|| url.to_string())
}

/// Coupon sites that keep a store's codes at a fixed address, read without a search.
fn direct_pages(store: &str, label: &str) -> Vec<String> {
    vec![
        format!("https://www.retailmenot.com/view/{store}"),
        format!("https://couponfollow.com/site/{store}"),
        format!("https://dealspotr.com/promo-codes/{store}"),
        format!("https://www.couponbirds.com/codes/{store}"),
        format!("https://simplycodes.com/store/{store}"),
        format!("https://www.couponsolver.com/promo-codes/{store}"),
        format!("https://old.reddit.com/search.json?q={label}%20code&sort=new&limit=50"),
    ]
}

/// The address to show for a page that was read. Reddit is read as JSON but shown as a search.
fn shown_url(url: &str) -> String {
    url.replacen("/search.json?", "/search?", 1)
}

fn read_message(host: &str, added: usize) -> String {
    match added {
        0 => format!("Read {host}: no new codes"),
        1 => format!("Read {host}: 1 new code"),
        count => format!("Read {host}: {count} new codes"),
    }
}

fn outcome_of(added: usize) -> &'static str {
    if added > 0 { "ok" } else { "empty" }
}

/// Reads `urls`, a batch at a time, calling `done` for each. With `http_first` a plain request is
/// tried first and only the pages it can't open go to the browser.
fn read_pages(
    readers: &[Reader],
    urls: &[String],
    http_first: bool,
    mut done: impl FnMut(&str, Result<Page, String>),
) {
    let mut left: Vec<String> = Vec::new();
    if http_first || readers.is_empty() {
        for chunk in urls.chunks(FETCH_BATCH) {
            let pages = std::thread::scope(|scope| {
                let handles: Vec<_> = chunk
                    .iter()
                    .map(|url| scope.spawn(move || (url, http_page(url))))
                    .collect();
                handles
                    .into_iter()
                    .filter_map(|handle| handle.join().ok())
                    .collect::<Vec<_>>()
            });
            for (url, page) in pages {
                match page {
                    Err(_) if !readers.is_empty() => left.push(url.clone()),
                    page => done(url, page),
                }
            }
        }
    } else {
        left = urls.to_vec();
    }
    if readers.is_empty() {
        return;
    }
    for chunk in left.chunks(readers.len()) {
        let pages = std::thread::scope(|scope| {
            let handles: Vec<_> = chunk
                .iter()
                .zip(readers)
                .map(|(url, reader)| scope.spawn(move || (url, reader.read(url, READ_TIMEOUT))))
                .collect();
            handles
                .into_iter()
                .filter_map(|handle| handle.join().ok())
                .collect::<Vec<_>>()
        });
        for (url, page) in pages {
            done(url, page);
        }
    }
}

fn http_page(url: &str) -> Result<Page, String> {
    let response = ureq::get(url)
        .timeout(Duration::from_secs(10))
        .set("User-Agent", shopping::USER_AGENT)
        .set("Accept-Language", "en-US,en;q=0.9")
        .call()
        .map_err(|_| "Couldn't open that coupon page.".to_string())?;
    let url = response.get_url().to_string();
    let html = response
        .into_string()
        .map_err(|_| "Couldn't read that coupon page.".to_string())?;
    if shopping::blocked_page(&html) && html.len() < 20_000 {
        return Err("The page asked for a check.".into());
    }
    Ok(Page { url, html })
}

/// The plain engines first. When all of them are blocked or empty, the same query runs in the
/// browser, where the engines see the profile's cookies and usually answer.
fn search_everywhere(readers: &[Reader], query: &str, region: &str) -> shopping::Searched {
    let plain = shopping::search_query(query, region);
    let Some(reader) = readers.first() else {
        return plain;
    };
    if plain.engine.is_some() {
        return plain;
    }
    let in_browser = |engine: &'static str| {
        move || {
            let pause = rand::Rng::gen_range(&mut rand::thread_rng(), ENGINE_PAUSE_MS);
            std::thread::sleep(Duration::from_millis(pause));
            let page = reader.read(&shopping::engine_url(engine, query, region), ENGINE_TIMEOUT)?;
            shopping::engine_results(engine, &page.html)
        }
    };
    let duckduckgo = shopping::guarded("duckduckgo-browser", in_browser(shopping::DUCKDUCKGO));
    let brave = shopping::guarded("brave-browser", in_browser(shopping::BRAVE));
    let bing = shopping::guarded("bing-browser", in_browser(shopping::BING));
    let browsed = shopping::combine_engines(
        query,
        &[
            (shopping::DUCKDUCKGO, &duckduckgo),
            (shopping::BRAVE, &brave),
            (shopping::BING, &bing),
        ],
    );
    let mut skipped = plain.skipped;
    for name in browsed.skipped {
        if !skipped.contains(&name) {
            skipped.push(name);
        }
    }
    if let Some(engine) = browsed.engine {
        skipped.retain(|name| name != engine);
    }
    shopping::Searched {
        results: browsed.results,
        skipped,
        engine: browsed.engine,
    }
}

#[derive(Default)]
struct Found {
    items: Vec<FoundCode>,
}

struct FoundCode {
    code: String,
    description: String,
    source: String,
    expiry: String,
    urls: Vec<String>,
}

impl Found {
    fn add(&mut self, code: &str, url: &str, context: &str) {
        let description = clip(context, 140);
        match self
            .items
            .iter_mut()
            .find(|item| item.code.eq_ignore_ascii_case(code))
        {
            Some(item) => {
                if !item.urls.iter().any(|kept| host_of(kept) == host_of(url)) {
                    item.urls.push(url.to_string());
                }
                if item.description.is_empty() {
                    item.description = description;
                }
            }
            None => self.items.push(FoundCode {
                code: code.to_string(),
                expiry: shopping::expiry_hint(context, ""),
                description,
                source: url.to_string(),
                urls: vec![url.to_string()],
            }),
        }
    }

    /// A code an earlier product search for this store found, so a new search never loses it.
    fn add_known(&mut self, coupon: &Coupon) {
        if coupon.code.trim().is_empty() {
            return;
        }
        let context = if coupon.description.is_empty() {
            coupon.expiry.as_str()
        } else {
            coupon.description.as_str()
        };
        self.add(&coupon.code, &coupon.source, context);
        if let Some(item) = self
            .items
            .iter_mut()
            .find(|item| item.code.eq_ignore_ascii_case(&coupon.code))
            && item.expiry.is_empty()
        {
            item.expiry = coupon.expiry.clone();
        }
    }

    fn add_snippet(&mut self, source: &SearchResult, label: &str) {
        let text = format!("{} {}", source.title, source.snippet);
        for code in shopping::code_tokens(&text) {
            if plausible(&code, label) && contains_token(&text, &code) {
                self.add(&code, &source.url, &source.snippet);
            }
        }
    }

    fn add_page(&mut self, url: &str, html: &str, label: &str) {
        let html = own_section(html);
        for (code, context) in attribute_codes(html) {
            let code = unwrap_code(&code);
            if plausible(&code, label) && !for_other_store(&context, label) {
                self.add(&code, url, &context);
            }
        }
        let text = page_text(html);
        for (code, context) in text_codes(&text) {
            let code = unwrap_code(&code);
            if plausible(&code, label) {
                self.add(&code, url, &context);
            }
        }
    }

    fn into_coupons(self) -> Vec<Coupon> {
        self.items
            .into_iter()
            .map(|item| Coupon {
                code: item.code,
                description: item.description,
                source: item.source,
                expiry: item.expiry,
                sources: item.urls.len() as u32,
                ..Coupon::default()
            })
            .collect()
    }
}

fn plausible(code: &str, label: &str) -> bool {
    if !(3..=30).contains(&code.len()) {
        return false;
    }
    if !code
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
    {
        return false;
    }
    if code.chars().all(|character| character.is_ascii_digit()) {
        return false;
    }
    let upper = code.to_ascii_uppercase();
    if COMMON_WORDS.contains(&upper.as_str()) || (!label.is_empty() && upper == label.to_ascii_uppercase()) {
        return false;
    }
    let number = upper.trim_end_matches(|character: char| character.is_ascii_alphabetic());
    let suffix = &upper[number.len()..];
    if !number.is_empty() && number.chars().all(|character| character.is_ascii_digit()) && UNITS.contains(&suffix) {
        return false;
    }
    true
}

/// `code` appears in `text` as a whole word, not inside a longer token.
fn contains_token(text: &str, code: &str) -> bool {
    text.match_indices(code).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        let after = text[start + code.len()..].chars().next();
        !before.is_some_and(|character| character.is_ascii_alphanumeric())
            && !after.is_some_and(|character| character.is_ascii_alphanumeric())
    })
}

/// Codes held in attributes, each with the title its element gives it.
fn attribute_codes(html: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for name in ATTRIBUTES {
        for quote in ['"', '\''] {
            let needle = format!("{name}={quote}");
            let mut offset = 0;
            while let Some(found) = html[offset..].find(&needle) {
                let start = offset + found + needle.len();
                let Some(length) = html[start..].find(quote) else {
                    break;
                };
                let value = shopping::decode_entities(html[start..start + length].trim());
                if !value.is_empty() && !out.iter().any(|(kept, _)| kept.eq_ignore_ascii_case(&value)) {
                    out.push((value, tag_title(html, start)));
                }
                offset = start + length;
            }
        }
    }
    out
}

/// The title of the tag around byte `at`, such as `data-title="15% off at Katin"`.
fn tag_title(html: &str, at: usize) -> String {
    let open = html[..at].rfind('<').unwrap_or(at);
    let close = html[at..].find('>').map(|index| at + index).unwrap_or(html.len());
    let tag = &html[open..close];
    ["data-title", "aria-label", "title", "data-description"]
        .iter()
        .filter_map(|name| shopping::attribute(tag, name))
        .find(|value| !value.trim().is_empty())
        .unwrap_or_default()
}

/// Headings coupon sites put above codes for other stores.
const OTHER_STORES: [&str; 11] = [
    "similar",
    "competitor",
    "reseller",
    "other stores",
    "related",
    "popular stores",
    "also selling",
    "you may also like",
    "alternatives",
    "stores like",
    "people also",
];

/// The part of a coupon page about its own store: from the main heading to the first heading
/// that introduces codes for other stores.
fn own_section(html: &str) -> &str {
    let lower = html.to_ascii_lowercase();
    let mut offset = lower.find("<h1").unwrap_or(0);
    while let Some(found) = lower[offset..].find("<h") {
        let at = offset + found;
        offset = at + 2;
        if !matches!(lower.as_bytes().get(at + 2), Some(b'2'..=b'4')) {
            continue;
        }
        let Some(end) = lower[at..].find("</h").map(|index| at + index) else {
            break;
        };
        let heading = lower[at..end].find('>').map(|index| &lower[at + index + 1..end]).unwrap_or("");
        if OTHER_STORES.iter().any(|word| heading.contains(word)) {
            return &html[..at];
        }
    }
    html
}

/// A title like "10% off at Hobie Surf Shop" names a store other than this one.
fn for_other_store(title: &str, label: &str) -> bool {
    let Some(index) = title.to_ascii_lowercase().rfind(" at ") else {
        return false;
    };
    let after = &title[index + 4..];
    if !after.starts_with(|character: char| character.is_ascii_uppercase()) {
        return false;
    }
    let name: String = after
        .split(['(', ',', '-', '|'])
        .next()
        .unwrap_or_default()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    let generic = ["checkout", "cart", "sitewide", "storewide", "online", "instore", "select"];
    !name.is_empty()
        && !generic.iter().any(|word| name.starts_with(word))
        && !name.contains(label)
        && !label.contains(&name)
}

/// Some sites hide codes as base64, for example `U1VSRjIw` for `SURF20`.
fn unwrap_code(code: &str) -> String {
    use base64::Engine;
    let mixed = code.chars().any(|character| character.is_ascii_lowercase())
        && code.chars().any(|character| character.is_ascii_uppercase());
    if !mixed || code.len() < 8 || code.len() % 4 != 0 {
        return code.to_string();
    }
    base64::engine::general_purpose::STANDARD
        .decode(code)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .filter(|decoded| {
            decoded.len() >= 4
                && decoded
                    .chars()
                    .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
        })
        .unwrap_or_else(|| code.to_string())
}

/// Visible text of a page, without scripts, styles and markup.
fn page_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let lower = html.to_ascii_lowercase();
    let mut offset = 0;
    while offset < html.len() {
        let next = ["<script", "<style", "<noscript", "<svg"]
            .iter()
            .filter_map(|tag| lower[offset..].find(tag).map(|index| (offset + index, *tag)))
            .min_by_key(|(index, _)| *index);
        let Some((start, tag)) = next else {
            out.push_str(&html[offset..]);
            break;
        };
        out.push_str(&html[offset..start]);
        out.push(' ');
        let close = format!("</{}", &tag[1..]);
        offset = match lower[start..].find(&close) {
            Some(index) => {
                let end = start + index;
                lower[end..].find('>').map(|index| end + index + 1).unwrap_or(html.len())
            }
            None => html.len(),
        };
    }
    shopping::text(&out.replace('>', "> "))
}

/// Codes that sit near a coupon word, with the text around them.
fn text_codes(text: &str) -> Vec<(String, String)> {
    let lower = text.to_ascii_lowercase();
    let mut out: Vec<(String, String)> = Vec::new();
    for keyword in KEYWORDS {
        for (at, _) in lower.match_indices(keyword) {
            let mut start = word_start(text, at.saturating_sub(40));
            let mut end = word_end(text, (at + keyword.len() + 40).min(text.len()));
            if let Some(stop) = text[start..at].rfind(". ") {
                start += stop + 2;
            }
            if let Some(stop) = text[at..end].find(". ") {
                end = at + stop + 1;
            }
            let window = &text[start..end];
            for code in shopping::code_tokens(window) {
                if out.iter().any(|(kept, _)| kept.eq_ignore_ascii_case(&code)) {
                    continue;
                }
                let position = start + window.find(&code).unwrap_or(0);
                if !code.chars().any(|character| character.is_ascii_digit())
                    && !right_after(position, at + keyword.len())
                {
                    continue;
                }
                out.push((code, context_of(text, position)));
            }
        }
    }
    out
}

/// Headings such as "TYPICAL SAVINGS" read like codes, so a code with no digit must follow its
/// keyword closely, as in "Use code WELCOME".
fn right_after(position: usize, keyword_end: usize) -> bool {
    position >= keyword_end && position - keyword_end <= 12
}

fn boundary(text: &str, mut index: usize, forward: bool) -> usize {
    index = index.min(text.len());
    while !text.is_char_boundary(index) {
        if forward {
            index += 1;
        } else {
            index -= 1;
        }
    }
    index
}

fn word_start(text: &str, index: usize) -> usize {
    let index = boundary(text, index, false);
    text[..index]
        .rfind(char::is_whitespace)
        .map(|space| space + 1)
        .unwrap_or(0)
}

fn word_end(text: &str, index: usize) -> usize {
    let index = boundary(text, index, true);
    text[index..]
        .find(char::is_whitespace)
        .map(|space| index + space)
        .unwrap_or(text.len())
}

/// The sentence around `at`, at most `CONTEXT` bytes either side.
fn context_of(text: &str, at: usize) -> String {
    let start = boundary(text, at.saturating_sub(CONTEXT), false);
    let end = boundary(text, (at + CONTEXT).min(text.len()), true);
    let before = &text[start..at.max(start)];
    let after = &text[at.max(start)..end];
    let from = before
        .rfind(['.', '!', '?', '|'])
        .map(|index| start + index + 1)
        .unwrap_or(start);
    let to = after
        .find(['.', '!', '?', '|'])
        .map(|index| at + index + 1)
        .unwrap_or(end);
    text[from..to.max(from)].trim().to_string()
}

fn clip(text: &str, limit: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.len() <= limit {
        return text;
    }
    let end = boundary(&text, limit, false);
    format!("{}…", text[..end].trim_end())
}

fn no_results(store: &str, skipped: &[String]) -> String {
    match skipped.len() {
        0 => format!("The web search found no coupons for {store}."),
        1 => format!(
            "{} asked for a check and the other searches found no coupons for {store}. Wait a minute, then search again.",
            skipped[0]
        ),
        _ => format!(
            "{} asked for a check, so no coupons for {store} could be found. Wait a minute, then search again.",
            skipped.join(", ")
        ),
    }
}

fn search_note(pages_read: u32, skipped: &[String]) -> String {
    let mut parts = Vec::new();
    match pages_read {
        0 => parts.push("No coupon page could be opened; codes come from search results.".to_string()),
        1 => parts.push("Read 1 coupon page.".to_string()),
        count => parts.push(format!("Read {count} coupon pages.")),
    }
    if skipped.len() == 1 {
        parts.push(format!("{} was skipped.", skipped[0]));
    } else if skipped.len() > 1 {
        parts.push(format!("{} were skipped.", skipped.join(", ")));
    }
    parts.join(" ")
}

fn ai_describe(
    app: &AppHandle,
    current: &settings::BrowserSettings,
    store: &str,
    coupons: &[Coupon],
) -> Option<serde_json::Value> {
    let mut list = String::new();
    for coupon in coupons.iter().take(AI_LIMIT) {
        list.push_str(&format!(
            "- {} (on {} pages): {}\n",
            coupon.code,
            coupon.sources,
            if coupon.description.is_empty() {
                "no text"
            } else {
                coupon.description.as_str()
            }
        ));
    }
    let prompt = format!(
        "Store: {store}\n\nCodes found on coupon pages, each with the text around it:\n{list}\n\
Return a JSON object with these keys:\n\
- \"summary\": one sentence naming the code to try first and why.\n\
- \"coupons\": a list of {{\"code\", \"description\"}}. \"description\" says what the code gives, for example \"20% off orders over $50\" or \"Free shipping\", using only the text given.\n\
- \"notCodes\": codes from the list that are not coupon codes for {store}, such as a product model, a heading, a brand or a code for another store.\n\
Use only codes from the list. Do not invent codes."
    );
    let reply = tools::chat(
        app,
        current,
        &current.shopping_model,
        "You read coupon code listings. Reply with one JSON object and nothing else.",
        &prompt,
        true,
        Duration::from_secs(30),
    )
    .ok()?;
    shopping::parse_reply(&reply)
}

/// Adds descriptions from the assistant and drops entries it says are not codes, except codes in `keep`.
/// Codes it invents are ignored.
fn apply_ai(reply: &serde_json::Value, coupons: &mut Vec<Coupon>, keep: &[String]) {
    if let Some(items) = reply.get("coupons").and_then(|item| item.as_array()) {
        for item in items {
            let code = shopping::field(item, "code");
            let description = shopping::field(item, "description");
            if description.is_empty() {
                continue;
            }
            if let Some(coupon) = coupons
                .iter_mut()
                .find(|coupon| coupon.code.eq_ignore_ascii_case(&code))
            {
                coupon.description = clip(&description, 140);
            }
        }
    }
    if let Some(items) = reply.get("notCodes").and_then(|item| item.as_array()) {
        let dropped: Vec<&str> = items.iter().filter_map(|item| item.as_str()).collect();
        coupons.retain(|coupon| {
            keep.iter().any(|code| code.eq_ignore_ascii_case(&coupon.code))
                || !dropped
                    .iter()
                    .any(|code| code.trim().eq_ignore_ascii_case(&coupon.code))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Found, apply_ai, attribute_codes, build_queries, coupon_site, direct_pages, for_other_store, page_text,
        plausible, shown_url, store_label, text_codes, unwrap_code,
    };
    use crate::shopping::SearchResult;

    #[test]
    fn codes_are_read_from_coupon_attributes() {
        let html = r#"<button data-code="SAVE20" data-title="20% off">Show code</button><div data-clipboard-text='FREESHIP50'></div><a data-code="save20">x</a>"#;
        let found = attribute_codes(html);
        let codes: Vec<&str> = found.iter().map(|(code, _)| code.as_str()).collect();
        assert_eq!(codes, ["SAVE20", "FREESHIP50"]);
        assert_eq!(found[0].1, "20% off");
    }

    #[test]
    fn codes_for_other_stores_are_left_out() {
        let html = r#"<h2>Katin</h2><h3>Similar Deals</h3><h1>Katin promo codes</h1>
            <button data-code="KATIN15" data-title="15% off at Katin">x</button>
            <button data-code="VIP30" data-title="30% off at Frankies Bikinis">x</button>
            <h2>Codes for Authorized Katin Resellers</h2>
            <button data-code="hobie10" data-title="10% off at Hobie Surf Shop">x</button>"#;
        let mut found = Found::default();
        found.add_page("https://dealspotr.com/promo-codes/katinusa.com", html, "katinusa");
        let codes: Vec<String> = found.into_coupons().into_iter().map(|coupon| coupon.code).collect();
        assert_eq!(codes, ["KATIN15"]);
        assert!(!for_other_store("15% off at checkout", "katinusa"));
        assert!(!for_other_store("15% off at Checkout", "katinusa"));
        assert!(!for_other_store("15% off at Katin USA (min $50)", "katinusa"));
    }

    #[test]
    fn a_word_code_must_follow_its_keyword() {
        let text = page_text("<p>TYPICAL SAVINGS with a promo code are 15%.</p><p>Use code WELCOME today.</p>");
        let codes: Vec<String> = text_codes(&text).into_iter().map(|(code, _)| code).collect();
        assert_eq!(codes, ["WELCOME"]);
    }

    #[test]
    fn base64_codes_are_decoded() {
        assert_eq!(unwrap_code("U1VSRjIw"), "SURF20");
        assert_eq!(unwrap_code("TUFLRTIw"), "MAKE20");
        assert_eq!(unwrap_code("SAVE2026"), "SAVE2026");
        assert_eq!(unwrap_code("lap8xaw7"), "lap8xaw7");
    }

    #[test]
    fn one_site_counts_once() {
        let mut found = Found::default();
        found.add_page("https://dealspotr.com/promo-codes/a", "<p>Use code SAVE20.</p>", "mugshop");
        found.add_page("https://dealspotr.com/promo-codes/a?ref=1", "<p>Use code SAVE20.</p>", "mugshop");
        assert_eq!(found.into_coupons()[0].sources, 1);
    }

    #[test]
    fn codes_are_read_from_text_near_a_coupon_word() {
        let html = "<script>var CODE = 'NOTME99';</script><p>Use code <b>MUG15</b> at checkout for 15% off.</p><p>Model ABC123 ships free.</p>";
        let text = page_text(html);
        assert!(!text.contains("NOTME99"));
        let found = text_codes(&text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "MUG15");
        assert!(found[0].1.contains("15% off"));
    }

    #[test]
    fn everyday_words_units_and_the_store_are_rejected() {
        let label = store_label("www.mugshop.test");
        assert_eq!(label, "mugshop");
        assert!(!plausible("PROMO", &label));
        assert!(!plausible("SHIPPING", &label));
        assert!(!plausible("MUGSHOP", &label));
        assert!(!plausible("128GB", &label));
        assert!(!plausible("2026", &label));
        assert!(plausible("SAVE20", &label));
        assert!(plausible("20OFF", &label));
        assert_eq!(store_label("mugshop.co.uk"), "mugshop");
    }

    #[test]
    fn a_code_on_several_pages_counts_each_page_once() {
        let mut found = Found::default();
        let snippet = SearchResult {
            title: "Mug Shop coupons".into(),
            url: "https://coupons.test/a".into(),
            snippet: "Use code SAVE20 for 20% off.".into(),
        };
        found.add_snippet(&snippet, "mugshop");
        found.add_page("https://coupons.test/a", "<p>Use code SAVE20 today.</p>", "mugshop");
        found.add_page("https://deals.test/b", r#"<p>Promo code: SAVE20</p><i data-code="TEN10"></i>"#, "mugshop");
        let coupons = found.into_coupons();
        assert_eq!(coupons.len(), 2);
        assert_eq!(coupons[0].code, "SAVE20");
        assert_eq!(coupons[0].sources, 2);
        assert_eq!(coupons[1].code, "TEN10");
    }

    #[test]
    fn the_assistant_cannot_add_codes() {
        let mut found = Found::default();
        found.add_page("https://coupons.test/a", "<p>Use code SAVE20 or code ABC123.</p>", "mugshop");
        let mut coupons = found.into_coupons();
        let reply = serde_json::json!({
            "coupons": [
                {"code": "SAVE20", "description": "20% off"},
                {"code": "FAKE50", "description": "50% off"}
            ],
            "notCodes": ["ABC123"]
        });
        apply_ai(&reply, &mut coupons, &[]);
        assert_eq!(coupons.len(), 1);
        assert_eq!(coupons[0].code, "SAVE20");
        assert_eq!(coupons[0].description, "20% off");

        let mut found = Found::default();
        found.add_page("https://coupons.test/a", "<p>Use code ABC123.</p>", "mugshop");
        let mut coupons = found.into_coupons();
        apply_ai(&reply, &mut coupons, &["abc123".into()]);
        assert_eq!(coupons.len(), 1, "a code that worked here is never dropped");
    }

    #[test]
    fn codes_from_earlier_product_searches_are_kept() {
        let mut found = Found::default();
        found.add_known(&crate::shopping::Coupon {
            code: "RAKUTEN50".into(),
            source: "https://coupons.test/katin".into(),
            description: "50% off with Rakuten".into(),
            ..crate::shopping::Coupon::default()
        });
        found.add_page("https://deals.test/b", "<p>Promo code: RAKUTEN50</p>", "katinusa");
        let coupons = found.into_coupons();
        assert_eq!(coupons.len(), 1);
        assert_eq!(coupons[0].sources, 2);
        assert_eq!(coupons[0].description, "50% off with Rakuten");
    }

    #[test]
    fn queries_cover_coupon_sites_and_the_product() {
        let queries = build_queries("mugshop.test", Some("Blue Mug"));
        assert!(queries.iter().all(|query| !query.contains("site:")), "engines ask for a check on site: searches");
        assert!(queries.iter().any(|query| query == "mugshop.test slickdeals"));
        assert!(queries.iter().any(|query| query == "Blue Mug coupon"));
        assert_eq!(build_queries("mugshop.test", None).len(), 5);
        let direct = direct_pages("mugshop.test", "mugshop");
        assert!(direct.contains(&"https://www.retailmenot.com/view/mugshop.test".to_string()));
        assert!(direct.iter().all(|url| coupon_site(url)));
        assert_eq!(
            shown_url("https://old.reddit.com/search.json?q=mugshop%20code"),
            "https://old.reddit.com/search?q=mugshop%20code"
        );
    }
}
