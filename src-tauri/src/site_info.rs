use std::time::Duration;

use serde::Serialize;
use tauri::AppHandle;
use url::Url;

use crate::{adblock, page_errors, paths, privacy, storage, tabs, webview2};

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CertificateInfo {
    pub subject: String,
    pub issuer: String,
    /// Seconds since 1970.
    pub valid_from: f64,
    pub valid_to: f64,
    /// For example `TLS 1.3`.
    pub protocol: String,
    pub cipher: String,
    pub key_exchange: String,
}

/// Everything the site panel shows for the page in one tab.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SiteInfo {
    pub url: String,
    pub host: String,
    /// `https://example.com`. Empty for pages that are not http or https.
    pub origin: String,
    /// `secure`, `neutral`, `insecure`, `insecure-broken`, or `unknown`.
    pub security: String,
    pub certificate: Option<CertificateInfo>,
    /// The user passed a certificate error for this host this session.
    pub certificate_exception: bool,
    pub permissions: Vec<privacy::PermissionGrant>,
    /// Null when the page is not loaded.
    pub cookies: Option<usize>,
    /// Bytes in local storage, IndexedDB, cache storage and the like. Null when unknown.
    pub storage_bytes: Option<u64>,
    pub ads_blocked: u32,
}

fn text(value: &serde_json::Value, path: &str) -> String {
    value
        .pointer(path)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string()
}

fn number(value: &serde_json::Value, path: &str) -> f64 {
    value
        .pointer(path)
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0)
}

/// Reads the DevTools security state into the fields the panel shows.
fn certificate_from(state: &serde_json::Value) -> (String, Option<CertificateInfo>) {
    let security = text(state, "/visibleSecurityState/securityState");
    let base = "/visibleSecurityState/certificateSecurityState";
    if state.pointer(base).is_none() {
        return (security, None);
    }
    let info = CertificateInfo {
        subject: text(state, &format!("{base}/subjectName")),
        issuer: text(state, &format!("{base}/issuer")),
        valid_from: number(state, &format!("{base}/validFrom")),
        valid_to: number(state, &format!("{base}/validTo")),
        protocol: text(state, &format!("{base}/protocol")),
        cipher: text(state, &format!("{base}/cipher")),
        key_exchange: text(state, &format!("{base}/keyExchange")),
    };
    (security, Some(info))
}

/// Connection, permissions, cookies, storage and blocked requests for tab `id`. Waits on the
/// main thread, so call it from a worker thread.
pub fn site_info(app: &AppHandle, id: u32) -> Result<SiteInfo, String> {
    let url = tabs::tab_page_url(app, id).ok_or("That tab is no longer open.")?;
    let host = Url::parse(&url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_string))
        .unwrap_or_default();
    let Some(origin) = paths::http_origin(&url) else {
        return Ok(SiteInfo {
            security: if url.starts_with(paths::INTERNAL_SCHEME) {
                "secure".into()
            } else {
                "unknown".into()
            },
            url,
            host,
            ..SiteInfo::default()
        });
    };
    let webview = tabs::page_view(app, id);
    let (mut security, certificate) = match webview.as_ref().filter(|_| url.starts_with("https://"))
    {
        Some(webview) => webview2::security_state(webview)
            .map(|state| certificate_from(&state))
            .unwrap_or_else(|| ("unknown".into(), None)),
        None => ("insecure".into(), None),
    };
    if security.is_empty() {
        security = "unknown".into();
    }
    let storage_bytes = webview.as_ref().and_then(|webview| {
        let params = serde_json::json!({ "origin": origin }).to_string();
        webview2::call_cdp_json(
            webview,
            "Storage.getUsageAndQuota",
            &params,
            Duration::from_secs(2),
            "storage",
        )
        .ok()
        .and_then(|value| value.get("usage").and_then(|usage| usage.as_f64()))
        .map(|usage| usage.max(0.0) as u64)
    });
    let cookies = webview
        .as_ref()
        .and_then(|_| storage::cookies(app, id).ok())
        .map(|list| list.len());
    let permissions = privacy::grants(app)
        .into_iter()
        .filter(|grant| grant.origin == origin)
        .collect();
    Ok(SiteInfo {
        certificate_exception: page_errors::certificate_allowed(&url),
        url,
        host,
        origin,
        security,
        certificate,
        permissions,
        cookies,
        storage_bytes,
        ads_blocked: adblock::stats(id).blocked,
    })
}

#[cfg(test)]
mod tests {
    use super::certificate_from;

    #[test]
    fn the_security_event_fills_the_certificate() {
        let state = serde_json::json!({
            "visibleSecurityState": {
                "securityState": "secure",
                "certificateSecurityState": {
                    "protocol": "TLS 1.3",
                    "cipher": "AES_128_GCM",
                    "keyExchange": "",
                    "subjectName": "example.com",
                    "issuer": "Example CA",
                    "validFrom": 1_700_000_000.0,
                    "validTo": 1_730_000_000.0
                }
            }
        });
        let (security, certificate) = certificate_from(&state);
        let certificate = certificate.expect("certificate");
        assert_eq!(security, "secure");
        assert_eq!(certificate.issuer, "Example CA");
        assert_eq!(certificate.protocol, "TLS 1.3");
        assert!(certificate.valid_to > certificate.valid_from);
    }
}
