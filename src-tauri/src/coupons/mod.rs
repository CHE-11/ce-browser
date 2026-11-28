//! Store-wide coupon codes from web search and coupon sites, and trying them on the store's cart or checkout page.

mod apply;
mod browse;
mod search;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::shopping::{self, Coupon, CouponFeedback, SearchResult, ShoppingFile};

pub use apply::{CouponField, cancel, detect_field, try_codes};
pub use search::find_coupons;

const CACHE_MS: u64 = 60 * 60 * 1000;
const MAX_STORES: usize = 100;

/// Checkout pages hosted for many stores. The store is the site the page links back to.
const HOSTED_CHECKOUTS: [&str; 5] = [
    "shop.app",
    "checkout.shopify.com",
    "pay.shopify.com",
    "checkout.stripe.com",
    "paypal.com",
];

const MERCHANT_SCRIPT: &str = r#"(function () {
  var counts = {};
  var links = document.querySelectorAll("a[href]");
  for (var i = 0; i < links.length; i++) {
    try {
      var host = new URL(links[i].href).hostname.replace(/^www\./, "");
      if (host && host !== location.hostname) {
        counts[host] = (counts[host] || 0) + 1;
      }
    } catch (error) {}
  }
  return Object.keys(counts).sort(function (left, right) { return counts[right] - counts[left]; });
})()"#;

fn hosted_checkout(host: &str) -> bool {
    HOSTED_CHECKOUTS
        .iter()
        .any(|hosted| host == *hosted || host.ends_with(&format!(".{hosted}")))
}

/// Store host for the page in tab `id`. A hosted checkout resolves to the store it links back to.
pub(crate) fn resolve_store(app: &AppHandle, id: u32) -> Result<String, String> {
    let page_url = crate::tabs::tab_page_url(app, id).unwrap_or_default();
    let host = shopping::store_host(&page_url).ok_or("Open a store's page to find coupons.")?;
    if !hosted_checkout(&host) {
        return Ok(host);
    }
    let webview = crate::tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let hosts = crate::webview2::evaluate(
        &webview,
        MERCHANT_SCRIPT,
        std::time::Duration::from_secs(8),
        "Couldn't read this page.",
    )?;
    Ok(merchant_host(&hosts).unwrap_or(host))
}

fn merchant_host(hosts: &serde_json::Value) -> Option<String> {
    hosts
        .as_array()?
        .iter()
        .filter_map(|item| item.as_str())
        .find(|host| {
            !hosted_checkout(host)
                && !["shopify.com", "stripe.com", "google.com", "apple.com", "facebook.com", "instagram.com"]
                    .iter()
                    .any(|skip| host == skip || host.ends_with(&format!(".{skip}")))
        })
        .map(str::to_string)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct CouponSearch {
    /// Host of the store, without `www.`.
    pub store: String,
    pub coupons: Vec<Coupon>,
    pub sources: Vec<SearchResult>,
    pub summary: String,
    pub ai_available: bool,
    pub checked_at: u64,
    pub cached: bool,
    /// Pages read, and which engines were skipped.
    pub search_note: String,
    /// Coupon pages opened and read in full.
    pub pages_read: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct StoreCoupons {
    pub(crate) store: String,
    pub(crate) checked_at: u64,
    pub(crate) coupons: Vec<Coupon>,
    pub(crate) sources: Vec<SearchResult>,
    pub(crate) summary: String,
    pub(crate) search_note: String,
    pub(crate) pages_read: u32,
    pub(crate) feedback: Vec<CouponFeedback>,
}

/// Feedback for `store`. Codes marked on a product before coupons were kept per store come from that product's entry.
fn store_feedback(file: &ShoppingFile, store: &str) -> Vec<CouponFeedback> {
    let mut out = file
        .stores
        .iter()
        .find(|entry| entry.store == store)
        .map(|entry| entry.feedback.clone())
        .unwrap_or_default();
    for entry in file.entries.iter().filter(|entry| entry.deals.store == store) {
        for item in &entry.feedback {
            if !out
                .iter()
                .any(|kept| kept.code.eq_ignore_ascii_case(&item.code))
            {
                out.push(item.clone());
            }
        }
    }
    out
}

fn view(entry: &StoreCoupons, feedback: &[CouponFeedback], ai_available: bool, cached: bool) -> CouponSearch {
    let mut coupons = entry.coupons.clone();
    for coupon in &mut coupons {
        if let Some(item) = feedback
            .iter()
            .find(|item| item.code.eq_ignore_ascii_case(&coupon.code))
        {
            coupon.tried_at = item.tried_at;
            coupon.worked = item.worked;
            coupon.failed = item.failed;
            coupon.saving = item.saving.or(coupon.saving);
            if item.saving.is_some() {
                coupon.tested_at = item.tried_at;
            }
        }
    }
    sort_coupons(&mut coupons);
    CouponSearch {
        store: entry.store.clone(),
        coupons,
        sources: entry.sources.clone(),
        summary: entry.summary.clone(),
        ai_available,
        checked_at: entry.checked_at,
        cached,
        search_note: entry.search_note.clone(),
        pages_read: entry.pages_read,
    }
}

/// Worked codes first, largest saving first among them. Failed codes last. More sources ranks higher.
pub(crate) fn sort_coupons(coupons: &mut [Coupon]) {
    let rank = |coupon: &Coupon| {
        if coupon.worked {
            0
        } else if coupon.failed {
            2
        } else {
            1
        }
    };
    coupons.sort_by(|left, right| {
        rank(left)
            .cmp(&rank(right))
            .then_with(|| {
                right
                    .saving
                    .unwrap_or(0.0)
                    .total_cmp(&left.saving.unwrap_or(0.0))
            })
            .then(right.sources.cmp(&left.sources))
    });
}

fn fresh(app: &AppHandle, store: &str, ai_available: bool) -> Option<CouponSearch> {
    let file = shopping::load_shopping(app);
    let entry = file.stores.iter().find(|entry| entry.store == store)?;
    if entry.checked_at == 0
        || shopping::now_ms().saturating_sub(entry.checked_at) > CACHE_MS
        || (entry.coupons.is_empty() && entry.sources.is_empty())
    {
        return None;
    }
    Some(view(entry, &store_feedback(&file, store), ai_available, true))
}

/// Saves a fresh search for its store and keeps the store's feedback.
fn remember(app: &AppHandle, mut entry: StoreCoupons, ai_available: bool) -> Result<CouponSearch, String> {
    let mut file = shopping::load_shopping(app);
    entry.feedback = store_feedback(&file, &entry.store);
    file.stores.retain(|kept| kept.store != entry.store);
    let search = view(&entry, &entry.feedback, ai_available, false);
    file.stores.insert(0, entry);
    file.stores.truncate(MAX_STORES);
    shopping::save_shopping(app, &file)?;
    Ok(search)
}

/// One result of trying a code on the page.
pub(crate) struct Outcome {
    pub(crate) code: String,
    pub(crate) worked: bool,
    pub(crate) saving: Option<f64>,
}

/// Records tried codes for `store`, creating its entry when no search has run yet.
fn record(app: &AppHandle, store: &str, outcomes: &[Outcome], currency: &str) -> Result<CouponSearch, String> {
    let mut file = shopping::load_shopping(app);
    let mut feedback = store_feedback(&file, store);
    let now = shopping::now_ms();
    for outcome in outcomes {
        let next = CouponFeedback {
            code: outcome.code.clone(),
            tried_at: now,
            worked: outcome.worked,
            failed: !outcome.worked,
            saving: outcome.saving,
        };
        match feedback
            .iter_mut()
            .find(|item| item.code.eq_ignore_ascii_case(&outcome.code))
        {
            Some(item) => *item = next,
            None => feedback.push(next),
        }
    }
    let index = match file.stores.iter().position(|entry| entry.store == store) {
        Some(index) => index,
        None => {
            file.stores.insert(
                0,
                StoreCoupons {
                    store: store.to_string(),
                    ..StoreCoupons::default()
                },
            );
            0
        }
    };
    let entry = &mut file.stores[index];
    entry.feedback = feedback;
    for outcome in outcomes {
        if let Some(coupon) = entry
            .coupons
            .iter_mut()
            .find(|coupon| coupon.code.eq_ignore_ascii_case(&outcome.code))
        {
            coupon.saving = outcome.saving;
            coupon.currency = currency.to_string();
            coupon.tested_at = now;
        } else {
            entry.coupons.push(Coupon {
                code: outcome.code.clone(),
                saving: outcome.saving,
                currency: currency.to_string(),
                tested_at: now,
                ..Coupon::default()
            });
        }
    }
    let ai_available = crate::ai::available(app, &crate::settings::get(app));
    let search = view(entry, &entry.feedback, ai_available, true);
    shopping::save_shopping(app, &file)?;
    Ok(search)
}

pub fn set_feedback(app: &AppHandle, store: &str, code: &str, action: &str) -> Result<CouponSearch, String> {
    let code = code.trim();
    if code.is_empty() {
        return Err("That coupon has no code.".into());
    }
    if store.trim().is_empty() {
        return Err("Find coupons for this store before marking a code.".into());
    }
    let mut file = shopping::load_shopping(app);
    let mut feedback = store_feedback(&file, store);
    let now = shopping::now_ms();
    let item = match feedback
        .iter_mut()
        .position(|item| item.code.eq_ignore_ascii_case(code))
    {
        Some(index) => &mut feedback[index],
        None => {
            feedback.push(CouponFeedback {
                code: code.to_string(),
                ..CouponFeedback::default()
            });
            feedback.last_mut().expect("pushed")
        }
    };
    item.tried_at = now;
    match action {
        "tried" => {}
        "worked" => {
            item.worked = true;
            item.failed = false;
        }
        "failed" => {
            item.worked = false;
            item.failed = true;
            item.saving = None;
        }
        _ => return Err("Choose tried, worked, or failed.".into()),
    }
    let failed = action == "failed";
    let entry = file
        .stores
        .iter_mut()
        .find(|entry| entry.store == store)
        .ok_or("Find coupons for this store before marking a code.")?;
    entry.feedback = feedback;
    if failed {
        for coupon in entry
            .coupons
            .iter_mut()
            .filter(|coupon| coupon.code.eq_ignore_ascii_case(code))
        {
            coupon.saving = None;
        }
    }
    let ai_available = crate::ai::available(app, &crate::settings::get(app));
    let search = view(entry, &entry.feedback, ai_available, true);
    shopping::save_shopping(app, &file)?;
    Ok(search)
}

#[cfg(test)]
mod tests {
    use super::{StoreCoupons, merchant_host, sort_coupons, store_feedback};
    use crate::shopping::{Coupon, CouponFeedback, ShoppingDeals, ShoppingEntry, ShoppingFile};

    #[test]
    fn a_hosted_checkout_resolves_to_the_store_it_links_to() {
        let hosts = serde_json::json!(["shopify.com", "pay.shopify.com", "katinusa.com", "instagram.com"]);
        assert_eq!(merchant_host(&hosts).as_deref(), Some("katinusa.com"));
        assert_eq!(merchant_host(&serde_json::json!(["shopify.com"])), None);
    }

    #[test]
    fn product_feedback_carries_over_to_the_store() {
        let file = ShoppingFile {
            entries: vec![ShoppingEntry {
                key: "mug".into(),
                deals: ShoppingDeals {
                    store: "mugshop.test".into(),
                    ..ShoppingDeals::default()
                },
                feedback: vec![
                    CouponFeedback {
                        code: "SAVE20".into(),
                        worked: true,
                        ..CouponFeedback::default()
                    },
                    CouponFeedback {
                        code: "OLD10".into(),
                        failed: true,
                        ..CouponFeedback::default()
                    },
                ],
                ..ShoppingEntry::default()
            }],
            stores: vec![StoreCoupons {
                store: "mugshop.test".into(),
                feedback: vec![CouponFeedback {
                    code: "save20".into(),
                    failed: true,
                    ..CouponFeedback::default()
                }],
                ..StoreCoupons::default()
            }],
        };
        let feedback = store_feedback(&file, "mugshop.test");
        assert_eq!(feedback.len(), 2);
        assert!(feedback[0].failed, "the store's own mark wins");
        assert_eq!(feedback[1].code, "OLD10");
        assert!(store_feedback(&file, "other.test").is_empty());
    }

    #[test]
    fn an_old_file_without_stores_still_loads() {
        let file: ShoppingFile =
            serde_json::from_str(r#"{"entries":[{"key":"mug","feedback":[{"code":"SAVE20","worked":true}]}]}"#)
                .expect("old file");
        assert!(file.stores.is_empty());
        assert_eq!(file.entries[0].feedback[0].saving, None);
    }

    #[test]
    fn worked_codes_lead_and_failed_codes_trail() {
        let mut coupons = vec![
            Coupon {
                code: "FAILED".into(),
                failed: true,
                sources: 9,
                ..Coupon::default()
            },
            Coupon {
                code: "MANY".into(),
                sources: 5,
                ..Coupon::default()
            },
            Coupon {
                code: "FEW".into(),
                sources: 1,
                ..Coupon::default()
            },
            Coupon {
                code: "SMALL".into(),
                worked: true,
                saving: Some(2.0),
                ..Coupon::default()
            },
            Coupon {
                code: "BIG".into(),
                worked: true,
                saving: Some(9.0),
                ..Coupon::default()
            },
        ];
        sort_coupons(&mut coupons);
        let order: Vec<&str> = coupons.iter().map(|coupon| coupon.code.as_str()).collect();
        assert_eq!(order, ["BIG", "SMALL", "MANY", "FEW", "FAILED"]);
    }
}
