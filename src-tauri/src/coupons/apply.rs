use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use super::{CouponSearch, Outcome};
use crate::{tabs, webview2};

const SCRIPT: &str = include_str!("checkout.js");
const MAX_CODES: usize = 25;
const POLL: Duration = Duration::from_millis(500);
const SETTLE: Duration = Duration::from_secs(8);
const EVAL_TIMEOUT: Duration = Duration::from_secs(10);
/// An error that was already on the page counts only after this long, in case the page clears it late.
const STALE_ERROR: Duration = Duration::from_secs(2);
const NO_FIELD: &str = "Open the cart or checkout page to try codes.";

static RUNNING: AtomicBool = AtomicBool::new(false);
static CANCEL: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CouponTryProgress {
    pub tab_id: u32,
    /// Zero-based position of `code` in the run.
    pub index: usize,
    pub total: usize,
    pub code: String,
    /// `trying`, `worked`, `failed`, `done`, `cancelled` or `error`.
    pub status: String,
    pub saving: Option<f64>,
    pub best_code: String,
    pub best_saving: Option<f64>,
    pub currency: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CouponField {
    /// A coupon field, or a link that opens one, is on the page.
    pub found: bool,
    pub total: Option<f64>,
    pub currency: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct PageState {
    found: bool,
    toggle: bool,
    total: Option<f64>,
    currency: String,
    error: String,
    success: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct Applied {
    ok: bool,
    reason: String,
}

struct Running;

impl Drop for Running {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::Release);
        CANCEL.store(false, Ordering::Release);
    }
}

fn run(app: &AppHandle, id: u32, call: &str) -> Result<serde_json::Value, String> {
    let webview = tabs::page_view(app, id).ok_or("This page is not loaded yet.")?;
    let expression = format!("(async function () {{\n{SCRIPT}\nreturn await {call};\n}})()");
    webview2::evaluate(&webview, &expression, EVAL_TIMEOUT, "Couldn't reach the checkout page.")
}

fn read(app: &AppHandle, id: u32, open: bool) -> Result<PageState, String> {
    let value = run(app, id, &format!("detect({open})"))?;
    serde_json::from_value(value).map_err(|error| error.to_string())
}

pub fn detect_field(app: &AppHandle, id: u32) -> Result<CouponField, String> {
    let state = read(app, id, false)?;
    Ok(CouponField {
        found: state.found || state.toggle,
        total: state.total,
        currency: state.currency,
    })
}

pub fn cancel() {
    if RUNNING.load(Ordering::Acquire) {
        CANCEL.store(true, Ordering::Release);
    }
}

fn cancelled() -> bool {
    CANCEL.load(Ordering::Acquire)
}

/// Removes an applied code and waits for the total to settle. Returns the total afterwards.
fn clear(app: &AppHandle, id: u32) -> Option<f64> {
    let removed = run(app, id, "removeApplied()")
        .ok()
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    if removed == 0 {
        return read(app, id, false).ok().and_then(|state| state.total);
    }
    let started = Instant::now();
    let mut last = None;
    while started.elapsed() < SETTLE {
        std::thread::sleep(POLL);
        let Ok(state) = read(app, id, false) else {
            continue;
        };
        if state.total.is_some() && state.total == last {
            return last;
        }
        last = state.total;
    }
    last
}

enum Attempt {
    Saved(f64),
    Rejected(String),
}

/// Enters `code`, then watches the page until the total drops, an error shows, or `SETTLE` passes.
fn attempt(app: &AppHandle, id: u32, code: &str, baseline: f64) -> Attempt {
    let before = read(app, id, true).unwrap_or_default();
    let call = format!(
        "applyCode({})",
        serde_json::to_string(code).unwrap_or_else(|_| "\"\"".into())
    );
    // The page may start navigating before the script returns, so a failed call still gets watched.
    if let Ok(value) = run(app, id, &call) {
        let applied: Applied = serde_json::from_value(value).unwrap_or_default();
        if !applied.ok {
            return Attempt::Rejected(applied.reason);
        }
    }
    let started = Instant::now();
    let mut last: Option<f64> = None;
    let mut accepted = String::new();
    while started.elapsed() < SETTLE {
        std::thread::sleep(POLL);
        if cancelled() {
            break;
        }
        let Ok(state) = read(app, id, false) else {
            continue;
        };
        // A rejection message beats a lower total, which can be a misread line such as the subtotal.
        let dropped = state.total.is_some_and(|total| total < baseline - 0.005);
        if !state.error.is_empty() {
            let new_error = state.error != before.error;
            if new_error || (!dropped && started.elapsed() > STALE_ERROR) {
                return Attempt::Rejected(state.error);
            }
        }
        if dropped && last == state.total {
            return Attempt::Saved(baseline - state.total.unwrap_or(baseline));
        }
        last = state.total;
        if !state.success.is_empty() && state.success != before.success {
            accepted = state.success;
        }
    }
    if let Some(total) = last.filter(|total| *total < baseline - 0.005) {
        return Attempt::Saved(baseline - total);
    }
    if accepted.is_empty() {
        Attempt::Rejected("The total did not change.".into())
    } else {
        Attempt::Rejected("Accepted, but the total did not go down.".into())
    }
}

fn emit(app: &AppHandle, progress: &CouponTryProgress) {
    let _ = app.emit("coupon-try-progress", progress);
}

fn fail(app: &AppHandle, progress: &mut CouponTryProgress, message: &str) -> String {
    progress.status = "error".into();
    progress.message = message.to_string();
    emit(app, progress);
    message.to_string()
}

pub(crate) fn money(amount: f64, currency: &str) -> String {
    match currency {
        "USD" | "CAD" | "AUD" | "NZD" | "" => format!("${amount:.2}"),
        "GBP" => format!("£{amount:.2}"),
        "EUR" => format!("€{amount:.2}"),
        other => format!("{amount:.2} {other}"),
    }
}

fn unique_codes(codes: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for code in codes {
        let code = code.trim().to_string();
        if code.is_empty() || out.iter().any(|kept| kept.eq_ignore_ascii_case(&code)) {
            continue;
        }
        out.push(code);
        if out.len() >= MAX_CODES {
            break;
        }
    }
    out
}

/// Tries each code on the page in tab `id`, keeps the one that saves the most applied, and records every result for the store.
pub fn try_codes(app: &AppHandle, id: u32, codes: Vec<String>) -> Result<CouponSearch, String> {
    if RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("Codes are already being tried on a page.".into());
    }
    let _running = Running;
    CANCEL.store(false, Ordering::Release);

    let codes = unique_codes(codes);
    let mut progress = CouponTryProgress {
        tab_id: id,
        total: codes.len(),
        ..CouponTryProgress::default()
    };
    let store = super::resolve_store(app, id).map_err(|_| fail(app, &mut progress, NO_FIELD))?;
    if codes.is_empty() {
        return Err(fail(app, &mut progress, "Find coupons before trying them."));
    }
    let state = read(app, id, true).map_err(|error| fail(app, &mut progress, &error))?;
    if !state.found {
        return Err(fail(app, &mut progress, NO_FIELD));
    }
    let Some(baseline) = clear(app, id).or(state.total) else {
        return Err(fail(
            app,
            &mut progress,
            "Couldn't read the order total on this page.",
        ));
    };
    let currency = state.currency.clone();
    progress.currency = currency.clone();

    let mut outcomes: Vec<Outcome> = Vec::new();
    let mut applied: Option<String> = None;
    let mut stopped = false;
    for (index, code) in codes.iter().enumerate() {
        if cancelled() {
            stopped = true;
            break;
        }
        progress.index = index;
        progress.code = code.clone();
        progress.status = "trying".into();
        progress.saving = None;
        progress.message = String::new();
        emit(app, &progress);
        if applied.take().is_some() {
            clear(app, id);
        }
        let result = attempt(app, id, code, baseline);
        if cancelled() {
            stopped = true;
        }
        match result {
            Attempt::Saved(saving) => {
                outcomes.push(Outcome {
                    code: code.clone(),
                    worked: true,
                    saving: Some(saving),
                });
                applied = Some(code.clone());
                progress.status = "worked".into();
                progress.saving = Some(saving);
                if progress.best_saving.is_none_or(|best| saving > best) {
                    progress.best_saving = Some(saving);
                    progress.best_code = code.clone();
                }
            }
            Attempt::Rejected(reason) => {
                if stopped {
                    break;
                }
                outcomes.push(Outcome {
                    code: code.clone(),
                    worked: false,
                    saving: None,
                });
                progress.status = "failed".into();
                progress.message = reason;
            }
        }
        emit(app, &progress);
        if stopped {
            break;
        }
    }

    if stopped {
        progress.status = "cancelled".into();
        progress.message = "Stopped.".into();
    } else if let Some(best_saving) = progress.best_saving {
        let best = progress.best_code.clone();
        let mut kept = applied.as_deref().is_some_and(|code| code.eq_ignore_ascii_case(&best));
        if !kept {
            if applied.is_some() {
                clear(app, id);
            }
            kept = matches!(attempt(app, id, &best, baseline), Attempt::Saved(_));
        }
        progress.status = "done".into();
        progress.message = if kept {
            format!("Best: {best} saved {}, applied.", money(best_saving, &currency))
        } else {
            format!(
                "Best: {best} saved {}, but it could not be applied again. Enter it by hand.",
                money(best_saving, &currency)
            )
        };
    } else {
        progress.status = "done".into();
        progress.message = "No code lowered the total.".into();
    }
    progress.code = String::new();
    progress.saving = None;
    emit(app, &progress);
    super::record(app, &store, &outcomes, &currency)
}

#[cfg(test)]
mod tests {
    use super::{money, unique_codes};

    #[test]
    fn codes_are_trimmed_deduplicated_and_capped() {
        let codes = unique_codes(vec![" SAVE20 ".into(), "save20".into(), "".into(), "TEN10".into()]);
        assert_eq!(codes, ["SAVE20", "TEN10"]);
        let many = unique_codes((0..40).map(|index| format!("CODE{index}")).collect());
        assert_eq!(many.len(), super::MAX_CODES);
    }

    #[test]
    fn savings_read_as_money() {
        assert_eq!(money(12.4, "USD"), "$12.40");
        assert_eq!(money(3.0, "GBP"), "£3.00");
        assert_eq!(money(5.5, "SEK"), "5.50 SEK");
    }
}
