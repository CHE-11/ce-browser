//! Decides which automated actions run, which need the user's approval, and which never run.

use crate::paths;
use crate::settings::{AutomationSettings, sleep_site_key};

use super::driver::ElementInfo;

/// What an action that needs approval could do. Autonomy levels skip asking for some of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    /// Opening a site that is not on the allow list.
    Open,
    /// Sending, posting or publishing something.
    Send,
    /// Deleting, closing or cancelling something.
    Destructive,
    /// Spending money or entering payment details.
    Payment,
    /// Putting local files on a page.
    Upload,
    /// Running a script in the page.
    Script,
    /// Using the mouse and keyboard outside the browser. Always asks once per run.
    Desktop,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Allow,
    Ask { prompt: String, risk: Risk },
    Block(String),
}

impl Verdict {
    pub fn ask(risk: Risk, prompt: impl Into<String>) -> Self {
        Verdict::Ask {
            prompt: prompt.into(),
            risk,
        }
    }
}

pub const AUTONOMY_LEVELS: [&str; 3] = ["ask", "auto", "full"];

/// `ask`, `auto` or `full`. Anything else is `ask`.
pub fn autonomy(value: &str) -> &'static str {
    AUTONOMY_LEVELS
        .into_iter()
        .find(|level| *level == value.trim())
        .unwrap_or("ask")
}

fn autonomy_rank(value: &str) -> usize {
    AUTONOMY_LEVELS
        .iter()
        .position(|level| *level == autonomy(value))
        .unwrap_or(0)
}

/// The lower of two autonomy levels, so a helper never gets more freedom than its parent.
pub fn lower_autonomy(a: &str, b: &str) -> &'static str {
    AUTONOMY_LEVELS[autonomy_rank(a).min(autonomy_rank(b))]
}

/// Whether a run at `level` goes ahead without asking about `risk`. Payments and uploads always ask.
pub fn autonomy_allows(level: &str, risk: Risk) -> bool {
    match autonomy(level) {
        "full" => !matches!(risk, Risk::Payment | Risk::Upload | Risk::Desktop),
        "auto" => matches!(risk, Risk::Open | Risk::Send),
        _ => false,
    }
}

/// Words that mark a click or submit as spending money.
const PAYMENT: &[&str] = &[
    "pay",
    "pay now",
    "buy",
    "buy now",
    "purchase",
    "checkout",
    "check out",
    "place order",
    "place your order",
    "order now",
    "confirm order",
    "submit order",
    "complete order",
    "complete purchase",
    "confirm payment",
    "send money",
    "transfer",
    "subscribe",
    "donate",
    "book now",
    "reserve",
    "withdraw",
    "accept and pay",
];

/// Words that mark a click or submit as deleting or closing something.
const DESTRUCTIVE: &[&str] = &[
    "delete",
    "remove",
    "unsubscribe",
    "close account",
    "deactivate",
    "cancel subscription",
];

/// Words that mark a click or submit as sending or publishing.
const SENDING: &[&str] = &["send", "publish", "post", "tweet", "sign and submit"];

fn word_risk(word: &str) -> Risk {
    if PAYMENT.contains(&word) {
        Risk::Payment
    } else if DESTRUCTIVE.contains(&word) {
        Risk::Destructive
    } else {
        Risk::Send
    }
}

fn host_listed(list: &[String], host: &str) -> bool {
    !host.is_empty()
        && list
            .iter()
            .any(|site| host == site || host.ends_with(&format!(".{site}")))
}

/// May automation act on a page at `url`?
pub fn page(settings: &AutomationSettings, url: &str) -> Verdict {
    if url.starts_with(paths::INTERNAL_SCHEME) && url != paths::NEW_TAB {
        return Verdict::Block("Automation can't act on the browser's own pages.".into());
    }
    let host = sleep_site_key(url);
    if host_listed(&settings.blocked_sites, &host) {
        return Verdict::Block(format!("Automation is turned off for {host}."));
    }
    Verdict::Allow
}

/// May automation open `url`?
pub fn open(settings: &AutomationSettings, url: &str) -> Verdict {
    let lower = url.trim().to_ascii_lowercase();
    if lower.starts_with("javascript:") || lower.starts_with("file:") || lower.starts_with("data:")
    {
        return Verdict::Block("Automation can't open that kind of address.".into());
    }
    if lower.starts_with(paths::INTERNAL_SCHEME) && lower != paths::NEW_TAB {
        return Verdict::Block("Automation can't open the browser's own pages.".into());
    }
    let host = sleep_site_key(url);
    if host_listed(&settings.blocked_sites, &host) {
        return Verdict::Block(format!("Automation is turned off for {host}."));
    }
    if !settings.allowed_sites.is_empty()
        && !host.is_empty()
        && !host_listed(&settings.allowed_sites, &host)
    {
        return Verdict::ask(
            Risk::Open,
            format!("Open {host}? It is not on the automation allow list."),
        );
    }
    Verdict::Allow
}

fn contains_word(text: &str, phrase: &str) -> bool {
    let text = text.to_ascii_lowercase();
    let mut start = 0;
    while let Some(at) = text[start..].find(phrase) {
        let begin = start + at;
        let end = begin + phrase.len();
        let before = text[..begin].chars().next_back();
        let after = text[end..].chars().next();
        let edge = |ch: Option<char>| ch.is_none_or(|ch| !ch.is_alphanumeric());
        if edge(before) && edge(after) {
            return true;
        }
        start = end;
    }
    false
}

/// The sensitive word in `label`, if any.
pub fn sensitive_word(label: &str) -> Option<&'static str> {
    let label = label.trim();
    if label.is_empty() || label.len() > 160 {
        return None;
    }
    PAYMENT
        .iter()
        .chain(DESTRUCTIVE)
        .chain(SENDING)
        .filter(|word| contains_word(label, word))
        .max_by_key(|word| word.len())
        .copied()
}

/// What the sensitive word in `label` could do, if it has one.
pub fn sensitive_risk(label: &str) -> Option<(&'static str, Risk)> {
    sensitive_word(label).map(|word| (word, word_risk(word)))
}

fn label_of(info: &ElementInfo) -> String {
    if info.name.trim().is_empty() {
        info.role.clone()
    } else {
        format!("\"{}\"", info.name.trim())
    }
}

/// Clicking this element may spend money, send something or delete something.
pub fn click(settings: &AutomationSettings, info: &ElementInfo) -> Verdict {
    if info.file {
        return Verdict::ask(Risk::Upload, "Choose a file to upload on this page?");
    }
    if !settings.confirm_sensitive {
        return Verdict::Allow;
    }
    let submit = info.kind == "submit"
        || (info.tag == "button" && info.kind.is_empty() && !info.submits.is_empty());
    if let Some((word, risk)) = sensitive_risk(&info.name) {
        return Verdict::ask(
            risk,
            format!(
                "Click {}? It looks like a \"{word}\" action.",
                label_of(info)
            ),
        );
    }
    if submit && info.payment {
        return Verdict::ask(
            Risk::Payment,
            format!("Click {}? It submits a payment form.", label_of(info)),
        );
    }
    Verdict::Allow
}

/// Typing into this field may enter payment details.
pub fn typing(settings: &AutomationSettings, info: &ElementInfo, secret: bool) -> Verdict {
    if !settings.confirm_sensitive || secret {
        return Verdict::Allow;
    }
    if info.autocomplete.starts_with("cc-") {
        return Verdict::ask(Risk::Payment, "Type into a payment card field?");
    }
    let name = info.name.to_ascii_lowercase();
    if name.contains("card number")
        || name.contains("cvv")
        || name.contains("cvc")
        || name.contains("security code")
    {
        return Verdict::ask(Risk::Payment, "Type into a payment card field?");
    }
    Verdict::Allow
}

/// Pressing Enter in this field submits its form. Ask when that form looks sensitive.
pub fn submit(settings: &AutomationSettings, focused: Option<&ElementInfo>) -> Verdict {
    if !settings.confirm_sensitive {
        return Verdict::Allow;
    }
    let Some(info) = focused else {
        return Verdict::Allow;
    };
    if info.payment {
        return Verdict::ask(Risk::Payment, "Press Enter? It submits a payment form.");
    }
    if let Some((word, risk)) = info.submits.iter().find_map(|label| sensitive_risk(label)) {
        return Verdict::ask(
            risk,
            format!("Press Enter? The form's button is a \"{word}\" action."),
        );
    }
    Verdict::Allow
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(name: &str) -> ElementInfo {
        ElementInfo {
            tag: "button".into(),
            role: "button".into(),
            name: name.into(),
            ..Default::default()
        }
    }

    #[test]
    fn sensitive_words_match_whole_words() {
        assert_eq!(sensitive_word("Place order"), Some("place order"));
        assert_eq!(sensitive_word("Buy now"), Some("buy now"));
        assert_eq!(sensitive_word("Send"), Some("send"));
        assert_eq!(sensitive_word("Payment options"), None);
        assert_eq!(sensitive_word("Posts"), None);
        assert_eq!(sensitive_word("Search"), None);
    }

    #[test]
    fn clicks_ask_only_when_sensitive() {
        let settings = AutomationSettings::default();
        assert_eq!(click(&settings, &element("Next")), Verdict::Allow);
        assert!(matches!(
            click(&settings, &element("Delete repository")),
            Verdict::Ask {
                risk: Risk::Destructive,
                ..
            }
        ));
        assert!(matches!(
            click(&settings, &element("Place your order")),
            Verdict::Ask {
                risk: Risk::Payment,
                ..
            }
        ));
        assert!(matches!(
            click(&settings, &element("Send money")),
            Verdict::Ask {
                risk: Risk::Payment,
                ..
            }
        ));
        assert!(matches!(
            click(&settings, &element("Post")),
            Verdict::Ask {
                risk: Risk::Send,
                ..
            }
        ));
        let relaxed = AutomationSettings {
            confirm_sensitive: false,
            ..AutomationSettings::default()
        };
        assert_eq!(
            click(&relaxed, &element("Delete repository")),
            Verdict::Allow
        );
    }

    #[test]
    fn sites_are_blocked_or_asked() {
        let settings = AutomationSettings {
            allowed_sites: vec!["example.com".into()],
            blocked_sites: vec!["bank.com".into()],
            ..AutomationSettings::default()
        };
        assert_eq!(
            open(&settings, "https://shop.example.com/a"),
            Verdict::Allow
        );
        assert!(matches!(
            open(&settings, "https://other.org"),
            Verdict::Ask {
                risk: Risk::Open,
                ..
            }
        ));
        assert!(matches!(
            open(&settings, "https://www.bank.com"),
            Verdict::Block(_)
        ));
        assert!(matches!(
            open(&settings, "javascript:alert(1)"),
            Verdict::Block(_)
        ));
        assert!(matches!(
            page(&settings, "ce://settings"),
            Verdict::Block(_)
        ));
        assert_eq!(page(&settings, "ce://newtab"), Verdict::Allow);
    }

    #[test]
    fn autonomy_levels_skip_some_questions() {
        assert!(!autonomy_allows("ask", Risk::Open));
        assert!(autonomy_allows("auto", Risk::Send));
        assert!(!autonomy_allows("auto", Risk::Destructive));
        assert!(autonomy_allows("full", Risk::Destructive));
        assert!(autonomy_allows("full", Risk::Script));
        assert!(!autonomy_allows("full", Risk::Payment));
        assert!(!autonomy_allows("full", Risk::Upload));
        assert!(!autonomy_allows("full", Risk::Desktop));
        assert!(!autonomy_allows("reckless", Risk::Open));
        assert_eq!(autonomy("bogus"), "ask");
        assert_eq!(lower_autonomy("full", "auto"), "auto");
        assert_eq!(lower_autonomy("ask", "full"), "ask");
    }
}
