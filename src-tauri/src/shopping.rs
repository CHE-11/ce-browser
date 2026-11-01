//! Coupons and better prices for the product in a tab, from web search results read by the assistant.

use std::time::Duration;

use serde::Serialize;
use tauri::AppHandle;

use crate::tools::{self, ProductPrice};
use crate::{settings, tabs, webview2};

const SEARCH_URL: &str = "https://html.duckduckgo.com/html/";
const RESULTS_PER_QUERY: usize = 8;
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Coupon {
    pub code: String,
    pub description: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    pub store: String,
    pub price: Option<f64>,
    pub currency: String,
    pub url: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
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
}

const NAME_SCRIPT: &str = r#"(function () {
  var og = document.querySelector('meta[property="og:title"]');
  var h1 = document.querySelector("h1");
  return (og && og.content) || (h1 && h1.innerText) || document.title || "";
})()"#;

pub fn find_deals(app: &AppHandle, id: u32) -> Result<ShoppingDeals, String> {
    let current = settings::get(app);
    let page_url = tabs::tab_page_url(app, id).unwrap_or_default();
    let store = store_host(&page_url).ok_or("Open a store's product page to find deals.")?;
    let product = tools::read_page_price(app, id).ok();
    let name = match product.as_ref().map(|item| item.title.trim()) {
        Some(title) if !title.is_empty() => clean_name(title),
        _ => clean_name(&page_name(app, id)?),
    };
    if name.is_empty() {
        return Err("Couldn't tell which product this page is for.".into());
    }

    let region = current.shopping_region.trim();
    let mut sources = Vec::new();
    let mut failure = None;
    for query in [format!("{store} coupon code"), format!("{name} price")] {
        match search(&query, region) {
            Ok(results) => {
                for result in results {
                    if !sources.iter().any(|kept: &SearchResult| kept.url == result.url) {
                        sources.push(result);
                    }
                }
            }
            Err(error) => failure = Some(error),
        }
    }
    if sources.is_empty() {
        return Err(failure.unwrap_or_else(|| "The web search found nothing for this product.".into()));
    }

    let reply = tools::chat(
        &current,
        &current.shopping_model,
        "You find shopping deals in web search results. Reply with one JSON object and nothing else.",
        &deals_prompt(&store, &name, product.as_ref(), &sources),
        Duration::from_secs(90),
    )?;
    let parsed = parse_reply(&reply).ok_or("The assistant's reply wasn't valid JSON. Try again or pick another model.")?;
    let current_price = product.as_ref().map(|item| item.price);
    Ok(ShoppingDeals {
        coupons: grounded_coupons(&parsed, &sources),
        offers: grounded_offers(&parsed, &sources, &page_url, current_price),
        summary: parsed
            .get("summary")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .trim()
            .to_string(),
        product,
        query: name,
        store,
        sources,
    })
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
        return Err("The web search asked for a captcha. Try again in a minute.".into());
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
        let Some(url) = attribute(&html[tag_start..tag_end], "href").and_then(|href| target_url(&href)) else {
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
            out.push(SearchResult { title, url, snippet });
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
                .or_else(|| entity.strip_prefix('#').and_then(|number| number.parse().ok()))
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

fn deals_prompt(store: &str, name: &str, product: Option<&ProductPrice>, sources: &[SearchResult]) -> String {
    let price = product
        .map(|item| format!("{} {}", item.currency, item.price).trim().to_string())
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
            || !code.chars().any(|character| character.is_ascii_digit() || character.is_ascii_uppercase())
            || out.iter().any(|kept| kept.code.eq_ignore_ascii_case(&code))
        {
            continue;
        }
        let shows = |result: &&SearchResult| result.title.contains(&code) || result.snippet.contains(&code);
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
        let Some(source) = sources.iter().find(|result| same_url(&result.url, &claimed)) else {
            continue;
        };
        if same_url(&source.url, page_url) || out.iter().any(|kept| kept.url == source.url) {
            continue;
        }
        let price = item.get("price").and_then(offer_price).filter(|value| *value > 0.0);
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
    use super::{SearchResult, clean_name, grounded_coupons, grounded_offers, parse_reply, parse_results, target_url};

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
        assert_eq!(results[0].snippet, "Use code SAVE20 for 20% off at mugshop.test.");
        assert_eq!(results[1].url, "https://other.test/mug");
    }

    #[test]
    fn a_wrapped_link_resolves_to_the_target() {
        assert_eq!(
            target_url("//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fa%3Fb%3D1&rut=x").as_deref(),
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

        let offers = grounded_offers(&reply, &sources(), "https://mugshop.test/blue-mug", Some(14.0));
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].price, Some(9.99));
        assert!(grounded_offers(&reply, &sources(), "https://mugshop.test/blue-mug", Some(8.0)).is_empty());
    }

    #[test]
    fn a_store_suffix_is_removed_from_the_name() {
        assert_eq!(clean_name("Blue  Mug 12oz | Mug Shop"), "Blue Mug 12oz");
    }
}
