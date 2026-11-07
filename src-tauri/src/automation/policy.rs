//! Decides which automated actions run, which need the user's approval, and which never run.

use crate::paths;
use crate::settings::{AutomationSettings, sleep_site_key};

use super::driver::ElementInfo;

#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Allow,
    Ask(String),
    Block(String),
}

/// Words that mark a click or submit as spending money, sending, publishing or deleting.
const SENSITIVE: &[&str] = &[
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
    "send",
    "send money",
    "delete",
    "remove",
    "transfer",
    "subscribe",
    "unsubscribe",
    "donate",
    "book now",
    "reserve",
    "publish",
    "post",
    "tweet",
    "withdraw",
    "close account",
    "deactivate",
    "cancel subscription",
    "sign and submit",
    "accept and pay",
];

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
        return Verdict::Ask(format!(
            "Open {host}? It is not on the automation allow list."
        ));
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
    SENSITIVE
        .iter()
        .rev()
        .find(|word| contains_word(label, word))
        .copied()
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
        return Verdict::Ask("Choose a file to upload on this page?".into());
    }
    if !settings.confirm_sensitive {
        return Verdict::Allow;
    }
    let submit = info.kind == "submit"
        || (info.tag == "button" && info.kind.is_empty() && !info.submits.is_empty());
    if let Some(word) = sensitive_word(&info.name) {
        return Verdict::Ask(format!(
            "Click {}? It looks like a \"{word}\" action.",
            label_of(info)
        ));
    }
    if submit && info.payment {
        return Verdict::Ask(format!(
            "Click {}? It submits a payment form.",
            label_of(info)
        ));
    }
    Verdict::Allow
}

/// Typing into this field may enter payment details.
pub fn typing(settings: &AutomationSettings, info: &ElementInfo, secret: bool) -> Verdict {
    if !settings.confirm_sensitive || secret {
        return Verdict::Allow;
    }
    if info.autocomplete.starts_with("cc-") {
        return Verdict::Ask("Type into a payment card field?".into());
    }
    let name = info.name.to_ascii_lowercase();
    if name.contains("card number")
        || name.contains("cvv")
        || name.contains("cvc")
        || name.contains("security code")
    {
        return Verdict::Ask("Type into a payment card field?".into());
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
        return Verdict::Ask("Press Enter? It submits a payment form.".into());
    }
    if let Some(word) = info.submits.iter().find_map(|label| sensitive_word(label)) {
        return Verdict::Ask(format!(
            "Press Enter? The form's button is a \"{word}\" action."
        ));
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
            Verdict::Ask(_)
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
            Verdict::Ask(_)
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
}
