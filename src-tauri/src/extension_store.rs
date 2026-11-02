//! Chrome Web Store and Edge Add-ons download, CRX3 verification, and update checks.
//!
//! The feeds are the ones Chrome and Edge call. They are not a public API.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use base64::Engine;
use sha2::{Digest, Sha256};
use tauri::AppHandle;
use url::Url;
use windows::Win32::Foundation::{ERROR_SUCCESS, NTSTATUS};
use windows::Win32::Security::Cryptography::{
    BCRYPT_ALG_HANDLE, BCRYPT_ECCKEY_BLOB, BCRYPT_ECCPUBLIC_BLOB, BCRYPT_ECDSA_P256_ALGORITHM,
    BCRYPT_ECDSA_PUBLIC_P256_MAGIC, BCRYPT_KEY_HANDLE, BCRYPT_PAD_PKCS1, BCRYPT_PKCS1_PADDING_INFO,
    BCRYPT_RSA_ALGORITHM, BCRYPT_RSAKEY_BLOB, BCRYPT_RSAPUBLIC_BLOB, BCRYPT_RSAPUBLIC_MAGIC,
    BCRYPT_SHA256_ALGORITHM, BCryptCloseAlgorithmProvider, BCryptDestroyKey, BCryptExportKey,
    BCryptFinalizeKeyPair, BCryptGenerateKeyPair, BCryptImportKeyPair, BCryptOpenAlgorithmProvider,
    BCryptSignHash, BCryptVerifySignature,
};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW,
};
use windows::core::{HSTRING, PCWSTR};

use crate::extensions::MAX_EXTENSION_ZIP;

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(90);
const CHECK_TIMEOUT: Duration = Duration::from_secs(20);
const SIGNED_CONTEXT: &[u8] = b"CRX3 SignedData\0";
const WEBVIEW_CLIENT: &str =
    r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
const WEBVIEW_CLIENT_NATIVE: &str =
    r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
const FALLBACK_VERSION: &str = "131.0.6778.204";
const FIRST_CHECK: Duration = Duration::from_secs(20);
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

static ANNOUNCED: Mutex<String> = Mutex::new(String::new());

/// Checks for extension updates soon after startup, then once a day.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(FIRST_CHECK);
        loop {
            if let Err(error) = crate::extensions::check_store_updates(&app, true) {
                eprintln!("extension updates: {error}");
            }
            thread::sleep(CHECK_EVERY);
        }
    });
}

/// Remembers a pending set so the same toast is not shown again until the next launch.
pub fn claim_announcement(fingerprint: &str) -> bool {
    let mut announced = ANNOUNCED
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if *announced == fingerprint {
        return false;
    }
    *announced = fingerprint.to_string();
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Store {
    Chrome,
    Edge,
}

impl Store {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chrome => "chrome",
            Self::Edge => "edge",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Chrome => "Chrome Web Store",
            Self::Edge => "Edge Add-ons",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub store: Store,
    pub id: String,
}

#[derive(Debug)]
pub struct DecodedArchive {
    pub zip: Vec<u8>,
    pub public_key: Option<Vec<u8>>,
}

#[derive(Debug)]
struct Proof {
    kind: ProofKind,
    public_key: Vec<u8>,
    signature: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProofKind {
    Rsa,
    Ecdsa,
}

/// A Chrome Web Store or Edge Add-ons listing, or a bare 32-character id (treated as Chrome).
pub fn parse_listing(input: &str) -> Result<Listing, String> {
    let input = input.trim();
    if is_extension_id(input) {
        return Ok(Listing {
            store: Store::Chrome,
            id: input.to_string(),
        });
    }
    let url = Url::parse(input).map_err(|_| store_link_error())?;
    let host = url
        .host_str()
        .unwrap_or_default()
        .trim_start_matches("www.")
        .to_ascii_lowercase();
    let store = if host == "chromewebstore.google.com" || host == "chrome.google.com" {
        Store::Chrome
    } else if host == "microsoftedge.microsoft.com" {
        Store::Edge
    } else {
        return Err(store_link_error());
    };
    let path = url.path().to_ascii_lowercase();
    let listing = match store {
        Store::Chrome => path.contains("/detail/"),
        Store::Edge => path.contains("/addons/detail/"),
    };
    if !listing {
        return Err(store_link_error());
    }
    let id = url
        .path_segments()
        .into_iter()
        .flatten()
        .find(|segment| is_extension_id(segment))
        .ok_or_else(|| "That store link does not include an extension id.".to_string())?;
    Ok(Listing {
        store,
        id: id.to_string(),
    })
}

pub fn store_of_update_url(url: &str) -> Option<Store> {
    let url = url.to_ascii_lowercase();
    if url.contains("clients2.google.com") {
        Some(Store::Chrome)
    } else if url.contains("edge.microsoft.com") {
        Some(Store::Edge)
    } else {
        None
    }
}

pub fn decode_extension_key(value: &str) -> Option<Vec<u8>> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(value))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(value))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(value))
        .ok()
}

pub fn encode_extension_key(public_key: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(public_key)
}

/// Chrome compares dotted numeric versions. `1.2.10` is newer than `1.2.9`. Missing parts are zero.
pub fn version_is_newer(candidate: &str, current: &str) -> bool {
    if candidate.is_empty() {
        return false;
    }
    let candidate = version_parts(candidate);
    let current = version_parts(current);
    let width = candidate.len().max(current.len());
    for index in 0..width {
        let left = candidate.get(index).copied().unwrap_or(0);
        let right = current.get(index).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    false
}

/// Downloads the current CRX3 for `id`. The body is verified by [`open_crx`].
pub fn download_crx(store: Store, id: &str) -> Result<Vec<u8>, String> {
    let bytes = http_bytes(&feed_url(store, id, None, true), DOWNLOAD_TIMEOUT)?;
    if bytes.starts_with(b"Cr24") {
        return Ok(bytes);
    }
    Err(refused(store))
}

/// The newest published version, when it is newer than `current`.
pub fn latest_version(store: Store, id: &str, current: &str) -> Result<Option<String>, String> {
    let bytes = http_bytes(&feed_url(store, id, Some(current), false), CHECK_TIMEOUT)?;
    let text = String::from_utf8_lossy(&bytes);
    if text.trim().is_empty() {
        return Err(format!(
            "{} did not answer the update check. It may have blocked the request.",
            store.label()
        ));
    }
    let check = parse_update_check(&text).map_err(|_| {
        format!(
            "{} did not return an update for that extension.",
            store.label()
        )
    })?;
    if check.status.eq_ignore_ascii_case("noupdate") {
        return Ok(None);
    }
    if !check.status.eq_ignore_ascii_case("ok") || check.version.is_empty() {
        return Err(format!(
            "{} did not return an update for that extension.",
            store.label()
        ));
    }
    if version_is_newer(&check.version, current) {
        Ok(Some(check.version))
    } else {
        Ok(None)
    }
}

/// Plain zip bytes pass through. A CRX3 is verified and returned as its zip plus the public key.
/// A CRX2 is unpacked without a key, matching the previous installer.
pub fn decode_archive(bytes: &[u8]) -> Result<DecodedArchive, String> {
    if bytes.len() < 4 || &bytes[..4] != b"Cr24" {
        return Ok(DecodedArchive {
            zip: bytes.to_vec(),
            public_key: None,
        });
    }
    if bytes.len() < 8 {
        return Err("That .crx file is truncated.".into());
    }
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    match version {
        2 => Ok(DecodedArchive {
            zip: crx2_zip(bytes)?,
            public_key: None,
        }),
        3 => {
            let verified = open_crx(bytes)?;
            Ok(DecodedArchive {
                zip: verified.zip,
                public_key: Some(verified.public_key),
            })
        }
        version => Err(format!("Unsupported .crx version {version}.")),
    }
}

#[derive(Debug)]
pub(crate) struct VerifiedCrx {
    pub public_key: Vec<u8>,
    pub zip: Vec<u8>,
}

/// Verifies a CRX3 signature and returns the publisher key and zip archive.
pub(crate) fn open_crx(bytes: &[u8]) -> Result<VerifiedCrx, String> {
    let (header, zip) = crx3_parts(bytes)?;
    let (proofs, signed_header) = parse_crx_header(&header)?;
    if proofs.is_empty() || signed_header.is_empty() {
        return Err("That extension package has no signature.".into());
    }
    let crx_id = parse_signed_id(&signed_header)?;
    let payload = signed_bytes(&signed_header, &zip);
    for proof in proofs {
        if verify_proof(&proof, &payload).is_err() {
            continue;
        }
        let digest = Sha256::digest(&proof.public_key);
        if digest[..16] != crx_id[..] {
            continue;
        }
        return Ok(VerifiedCrx {
            public_key: proof.public_key,
            zip,
        });
    }
    Err("That extension package has a bad signature.".into())
}

pub fn extension_id(public_key: &[u8]) -> String {
    let digest = Sha256::digest(public_key);
    id_from_digest(&digest[..16])
}

fn id_from_digest(bytes: &[u8]) -> String {
    let mut id = String::with_capacity(32);
    for byte in bytes.iter().take(16) {
        id.push((b'a' + (byte >> 4)) as char);
        id.push((b'a' + (byte & 0x0f)) as char);
    }
    id
}

fn is_extension_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| (b'a'..=b'p').contains(&byte))
}

fn store_link_error() -> String {
    "That is not a Chrome Web Store or Edge Add-ons link.".into()
}

fn refused(store: Store) -> String {
    format!(
        "{} did not return that extension. It may be unavailable, or the store blocked the download.",
        store.label()
    )
}

fn version_parts(value: &str) -> Vec<u64> {
    value
        .split('.')
        .take(8)
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

fn feed_url(store: Store, id: &str, version: Option<&str>, redirect: bool) -> String {
    let prodversion = webview_version();
    // `installsource=ondemand` is required. Without it the store answers `noupdate` and an empty download.
    let extra = match version.map(str::trim).filter(|version| !version.is_empty()) {
        Some(version) => format!("id={id}&v={version}&installsource=ondemand&uc"),
        None => format!("id={id}&installsource=ondemand&uc"),
    };
    let (base, prod) = match store {
        Store::Chrome => (
            "https://clients2.google.com/service/update2/crx",
            "chromiumcrx",
        ),
        Store::Edge => (
            "https://edge.microsoft.com/extensionwebstorebase/v1/crx",
            "edgecrx",
        ),
    };
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    if redirect {
        serializer.append_pair("response", "redirect");
    }
    serializer
        .append_pair("os", "win")
        .append_pair("arch", "x64")
        .append_pair("os_arch", "x86_64")
        .append_pair("nacl_arch", "x86-64")
        .append_pair("prod", prod)
        .append_pair("prodchannel", "stable")
        .append_pair("prodversion", &prodversion)
        .append_pair("lang", "en-US")
        .append_pair("acceptformat", "crx3")
        .append_pair("x", &extra);
    format!("{base}?{}", serializer.finish())
}

fn webview_version() -> String {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION
        .get_or_init(|| {
            registry_version()
                .or_else(folder_version)
                .unwrap_or_else(|| FALLBACK_VERSION.to_string())
        })
        .clone()
}

fn registry_version() -> Option<String> {
    for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        for subkey in [WEBVIEW_CLIENT, WEBVIEW_CLIENT_NATIVE] {
            if let Some(version) = registry_string(root, subkey, "pv") {
                return Some(version);
            }
        }
    }
    None
}

fn registry_string(
    root: windows::Win32::System::Registry::HKEY,
    subkey: &str,
    value: &str,
) -> Option<String> {
    let subkey = HSTRING::from(subkey);
    let value = HSTRING::from(value);
    let mut size = 0u32;
    let status = unsafe {
        RegGetValueW(
            root,
            &subkey,
            &value,
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS || size < 2 {
        return None;
    }
    let mut buffer = vec![0u16; size as usize / 2 + 1];
    let status = unsafe {
        RegGetValueW(
            root,
            &subkey,
            &value,
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let len = (size as usize / 2).saturating_sub(1);
    let text = String::from_utf16_lossy(&buffer[..len])
        .trim()
        .trim_end_matches('\0')
        .to_string();
    if text.is_empty() { None } else { Some(text) }
}

fn folder_version() -> Option<String> {
    let mut best: Option<String> = None;
    for root in [
        Path::new(r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application"),
        Path::new(r"C:\Program Files\Microsoft\EdgeWebView\Application"),
    ] {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let version = name
                .split('.')
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()));
            if !version {
                continue;
            }
            if best
                .as_ref()
                .is_none_or(|current| version_is_newer(&name, current))
            {
                best = Some(name);
            }
        }
    }
    best
}

fn user_agent() -> String {
    let version = webview_version();
    let chrome = version.split('.').next().unwrap_or("131");
    format!(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{chrome}.0.0.0 Safari/537.36 Edg/{version}"
    )
}

fn http_bytes(url: &str, timeout: Duration) -> Result<Vec<u8>, String> {
    let agent = ureq::AgentBuilder::new().redirects(10).build();
    let response = agent
        .get(url)
        .timeout(timeout)
        .set("User-Agent", &user_agent())
        .set("Accept", "*/*")
        .call()
        .map_err(|error| match error {
            ureq::Error::Status(code, _) => format!("The store returned HTTP {code}."),
            other => other.to_string(),
        })?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_EXTENSION_ZIP as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > MAX_EXTENSION_ZIP {
        return Err("That extension is too large to install.".into());
    }
    Ok(bytes)
}

struct UpdateCheck {
    status: String,
    version: String,
}

fn parse_update_check(xml: &str) -> Result<UpdateCheck, String> {
    let lower = xml.to_ascii_lowercase();
    let start = lower
        .find("<updatecheck")
        .ok_or_else(|| "The store response could not be read.".to_string())?;
    let rest = &xml[start..];
    let end = rest
        .find('>')
        .ok_or_else(|| "The store response could not be read.".to_string())?;
    let tag = &rest[..=end];
    Ok(UpdateCheck {
        status: xml_attr(tag, "status").unwrap_or_default(),
        version: xml_attr(tag, "version").unwrap_or_default(),
    })
}

fn xml_attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let needle = format!("{name}=");
    let index = lower.find(&needle)?;
    let after = &tag[index + needle.len()..];
    let quote = after.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    Some(after[quote.len_utf8()..].split(quote).next()?.to_string())
}

fn crx3_parts(bytes: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    if bytes.len() < 12 || &bytes[..4] != b"Cr24" {
        return Err("That is not a .crx file.".into());
    }
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version != 3 {
        return Err(format!("Unsupported .crx version {version}."));
    }
    let header_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    let header_end = 12 + header_len;
    if header_len == 0 || header_end > bytes.len() {
        return Err("That .crx file is truncated.".into());
    }
    Ok((bytes[12..header_end].to_vec(), bytes[header_end..].to_vec()))
}

fn crx2_zip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() < 16 {
        return Err("That .crx file is truncated.".into());
    }
    let read_u32 = |at: usize| {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize
    };
    let offset = 16 + read_u32(8) + read_u32(12);
    if offset >= bytes.len() {
        return Err("That .crx file is truncated.".into());
    }
    Ok(bytes[offset..].to_vec())
}

fn parse_crx_header(header: &[u8]) -> Result<(Vec<Proof>, Vec<u8>), String> {
    let mut at = 0;
    let mut proofs = Vec::new();
    let mut signed_header = Vec::new();
    while at < header.len() {
        let key = read_varint(header, &mut at).map_err(|_| "That .crx file is truncated.")?;
        let field = (key >> 3) as u32;
        let wire = (key & 7) as u32;
        if wire != 2 {
            skip_field(header, &mut at, wire).map_err(|_| "That .crx file is truncated.")?;
            continue;
        }
        let len =
            read_varint(header, &mut at).map_err(|_| "That .crx file is truncated.")? as usize;
        let end = at + len;
        let bytes = header
            .get(at..end)
            .ok_or_else(|| "That .crx file is truncated.".to_string())?;
        at = end;
        match field {
            2 => proofs.push(parse_proof(bytes, ProofKind::Rsa)?),
            3 => proofs.push(parse_proof(bytes, ProofKind::Ecdsa)?),
            10000 => signed_header = bytes.to_vec(),
            _ => {}
        }
    }
    Ok((proofs, signed_header))
}

fn parse_proof(bytes: &[u8], kind: ProofKind) -> Result<Proof, String> {
    let mut at = 0;
    let mut public_key = Vec::new();
    let mut signature = Vec::new();
    while at < bytes.len() {
        let key = read_varint(bytes, &mut at).map_err(|_| "That .crx file is truncated.")?;
        let field = (key >> 3) as u32;
        let wire = (key & 7) as u32;
        if wire != 2 {
            skip_field(bytes, &mut at, wire).map_err(|_| "That .crx file is truncated.")?;
            continue;
        }
        let len = read_varint(bytes, &mut at).map_err(|_| "That .crx file is truncated.")? as usize;
        let end = at + len;
        let value = bytes
            .get(at..end)
            .ok_or_else(|| "That .crx file is truncated.".to_string())?
            .to_vec();
        at = end;
        match field {
            1 => public_key = value,
            2 => signature = value,
            _ => {}
        }
    }
    if public_key.is_empty() || signature.is_empty() {
        return Err("That extension package has no signature.".into());
    }
    Ok(Proof {
        kind,
        public_key,
        signature,
    })
}

fn parse_signed_id(signed_header: &[u8]) -> Result<Vec<u8>, String> {
    let mut at = 0;
    while at < signed_header.len() {
        let key =
            read_varint(signed_header, &mut at).map_err(|_| "That .crx file is truncated.")?;
        let field = (key >> 3) as u32;
        let wire = (key & 7) as u32;
        if wire != 2 {
            skip_field(signed_header, &mut at, wire).map_err(|_| "That .crx file is truncated.")?;
            continue;
        }
        let len = read_varint(signed_header, &mut at).map_err(|_| "That .crx file is truncated.")?
            as usize;
        let end = at + len;
        let value = signed_header
            .get(at..end)
            .ok_or_else(|| "That .crx file is truncated.".to_string())?;
        at = end;
        if field == 1 {
            if value.len() != 16 {
                return Err("That extension package has a bad signature.".into());
            }
            return Ok(value.to_vec());
        }
    }
    Err("That extension package has a bad signature.".into())
}

fn signed_bytes(signed_header: &[u8], zip: &[u8]) -> Vec<u8> {
    let mut payload =
        Vec::with_capacity(SIGNED_CONTEXT.len() + 4 + signed_header.len() + zip.len());
    payload.extend(SIGNED_CONTEXT);
    payload.extend(&(signed_header.len() as u32).to_le_bytes());
    payload.extend(signed_header);
    payload.extend(zip);
    payload
}

fn read_varint(data: &[u8], at: &mut usize) -> Result<u64, ()> {
    let mut value = 0u64;
    let mut shift = 0;
    loop {
        if *at >= data.len() || shift > 63 {
            return Err(());
        }
        let byte = data[*at];
        *at += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
    }
}

fn skip_field(data: &[u8], at: &mut usize, wire: u32) -> Result<(), ()> {
    match wire {
        0 => {
            read_varint(data, at)?;
        }
        1 => {
            *at += 8;
        }
        2 => {
            let len = read_varint(data, at)? as usize;
            *at += len;
        }
        5 => {
            *at += 4;
        }
        _ => return Err(()),
    }
    if *at > data.len() {
        return Err(());
    }
    Ok(())
}

fn verify_proof(proof: &Proof, payload: &[u8]) -> Result<(), String> {
    match proof.kind {
        ProofKind::Rsa => verify_rsa(&proof.public_key, &proof.signature, payload),
        ProofKind::Ecdsa => verify_ecdsa(&proof.public_key, &proof.signature, payload),
    }
}

fn nt_ok(status: NTSTATUS) -> bool {
    status.0 >= 0
}

struct Algorithm(BCRYPT_ALG_HANDLE);

impl Drop for Algorithm {
    fn drop(&mut self) {
        unsafe {
            let _ = BCryptCloseAlgorithmProvider(self.0, 0);
        }
    }
}

struct Key(BCRYPT_KEY_HANDLE);

impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = BCryptDestroyKey(self.0);
        }
    }
}

fn open_algorithm(id: PCWSTR) -> Result<Algorithm, String> {
    let mut handle = BCRYPT_ALG_HANDLE::default();
    let status =
        unsafe { BCryptOpenAlgorithmProvider(&mut handle, id, PCWSTR::null(), Default::default()) };
    if !nt_ok(status) {
        return Err("Windows could not verify the extension signature.".into());
    }
    Ok(Algorithm(handle))
}

fn import_key(algorithm: &Algorithm, kind: PCWSTR, blob: &[u8]) -> Result<Key, String> {
    let mut handle = BCRYPT_KEY_HANDLE::default();
    let status = unsafe { BCryptImportKeyPair(algorithm.0, None, kind, &mut handle, blob, 0) };
    if !nt_ok(status) {
        return Err("That extension package has an unsupported signature.".into());
    }
    Ok(Key(handle))
}

fn verify_rsa(spki: &[u8], signature: &[u8], payload: &[u8]) -> Result<(), String> {
    let blob = rsa_public_blob(spki)?;
    let algorithm = open_algorithm(BCRYPT_RSA_ALGORITHM)?;
    let key = import_key(&algorithm, BCRYPT_RSAPUBLIC_BLOB, &blob)?;
    let hash = Sha256::digest(payload);
    let padding = BCRYPT_PKCS1_PADDING_INFO {
        pszAlgId: BCRYPT_SHA256_ALGORITHM,
    };
    let status = unsafe {
        BCryptVerifySignature(
            key.0,
            Some(std::ptr::from_ref(&padding).cast()),
            hash.as_slice(),
            signature,
            BCRYPT_PAD_PKCS1,
        )
    };
    if nt_ok(status) {
        Ok(())
    } else {
        Err("That extension package has a bad signature.".into())
    }
}

fn verify_ecdsa(spki: &[u8], signature: &[u8], payload: &[u8]) -> Result<(), String> {
    let blob = ecdsa_public_blob(spki)?;
    let raw = ecdsa_raw_signature(signature)?;
    let algorithm = open_algorithm(BCRYPT_ECDSA_P256_ALGORITHM)?;
    let key = import_key(&algorithm, BCRYPT_ECCPUBLIC_BLOB, &blob)?;
    let hash = Sha256::digest(payload);
    let status =
        unsafe { BCryptVerifySignature(key.0, None, hash.as_slice(), &raw, Default::default()) };
    if nt_ok(status) {
        Ok(())
    } else {
        Err("That extension package has a bad signature.".into())
    }
}

fn rsa_public_blob(spki: &[u8]) -> Result<Vec<u8>, String> {
    let mut outer = Der::new(spki);
    let body = outer
        .read_tlv(0x30)
        .map_err(|_| "That extension package has an unsupported signature.")?;
    let mut body = Der::new(body);
    let _algorithm = body
        .read_tlv(0x30)
        .map_err(|_| "That extension package has an unsupported signature.")?;
    let bits = body
        .read_tlv(0x03)
        .map_err(|_| "That extension package has an unsupported signature.")?;
    let Some((0, pkcs)) = bits.split_first() else {
        return Err("That extension package has an unsupported signature.".into());
    };
    let mut rsa = Der::new(pkcs);
    let seq = rsa
        .read_tlv(0x30)
        .map_err(|_| "That extension package has an unsupported signature.")?;
    let mut seq = Der::new(seq);
    let modulus = strip_integer(
        seq.read_tlv(0x02)
            .map_err(|_| "That extension package has an unsupported signature.")?,
    );
    let exponent = strip_integer(
        seq.read_tlv(0x02)
            .map_err(|_| "That extension package has an unsupported signature.")?,
    );
    if modulus.is_empty() || exponent.is_empty() {
        return Err("That extension package has an unsupported signature.".into());
    }
    let header = BCRYPT_RSAKEY_BLOB {
        Magic: BCRYPT_RSAPUBLIC_MAGIC,
        BitLength: (modulus.len() * 8) as u32,
        cbPublicExp: exponent.len() as u32,
        cbModulus: modulus.len() as u32,
        cbPrime1: 0,
        cbPrime2: 0,
    };
    let mut blob = Vec::with_capacity(24 + exponent.len() + modulus.len());
    blob.extend(header.Magic.0.to_le_bytes());
    blob.extend(header.BitLength.to_le_bytes());
    blob.extend(header.cbPublicExp.to_le_bytes());
    blob.extend(header.cbModulus.to_le_bytes());
    blob.extend(header.cbPrime1.to_le_bytes());
    blob.extend(header.cbPrime2.to_le_bytes());
    blob.extend(exponent);
    blob.extend(modulus);
    Ok(blob)
}

fn ecdsa_public_blob(spki: &[u8]) -> Result<Vec<u8>, String> {
    let mut outer = Der::new(spki);
    let body = outer
        .read_tlv(0x30)
        .map_err(|_| "That extension package has an unsupported signature.")?;
    let mut body = Der::new(body);
    let _algorithm = body
        .read_tlv(0x30)
        .map_err(|_| "That extension package has an unsupported signature.")?;
    let bits = body
        .read_tlv(0x03)
        .map_err(|_| "That extension package has an unsupported signature.")?;
    let Some((0, point)) = bits.split_first() else {
        return Err("That extension package has an unsupported signature.".into());
    };
    let Some((4, coords)) = point.split_first() else {
        return Err("That extension package has an unsupported signature.".into());
    };
    if coords.len() != 64 {
        return Err("That extension package has an unsupported signature.".into());
    }
    let header = BCRYPT_ECCKEY_BLOB {
        dwMagic: BCRYPT_ECDSA_PUBLIC_P256_MAGIC,
        cbKey: 32,
    };
    let mut blob = Vec::with_capacity(8 + 64);
    blob.extend(header.dwMagic.to_le_bytes());
    blob.extend(header.cbKey.to_le_bytes());
    blob.extend(coords);
    Ok(blob)
}

fn ecdsa_raw_signature(signature: &[u8]) -> Result<Vec<u8>, String> {
    if signature.len() == 64 {
        return Ok(signature.to_vec());
    }
    let mut der = Der::new(signature);
    let seq = der
        .read_tlv(0x30)
        .map_err(|_| "That extension package has an unsupported signature.")?;
    let mut seq = Der::new(seq);
    let r = strip_integer(
        seq.read_tlv(0x02)
            .map_err(|_| "That extension package has an unsupported signature.")?,
    );
    let s = strip_integer(
        seq.read_tlv(0x02)
            .map_err(|_| "That extension package has an unsupported signature.")?,
    );
    Ok([pad_coord(&r)?, pad_coord(&s)?].concat())
}

fn pad_coord(value: &[u8]) -> Result<Vec<u8>, String> {
    if value.len() > 32 {
        return Err("That extension package has an unsupported signature.".into());
    }
    let mut padded = vec![0u8; 32 - value.len()];
    padded.extend(value);
    Ok(padded)
}

fn strip_integer(bytes: &[u8]) -> Vec<u8> {
    let mut bytes = bytes;
    while bytes.len() > 1 && bytes[0] == 0 {
        bytes = &bytes[1..];
    }
    bytes.to_vec()
}

struct Der<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Der<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, at: 0 }
    }

    fn read_tlv(&mut self, tag: u8) -> Result<&'a [u8], ()> {
        let actual = self.take(1)?;
        if actual[0] != tag {
            return Err(());
        }
        let len = self.read_len()?;
        self.take(len)
    }

    fn read_len(&mut self) -> Result<usize, ()> {
        let first = self.take(1)?[0];
        if first < 0x80 {
            return Ok(first as usize);
        }
        let count = (first & 0x7f) as usize;
        if count == 0 || count > 3 {
            return Err(());
        }
        let bytes = self.take(count)?;
        let mut len = 0usize;
        for byte in bytes {
            len = (len << 8) | *byte as usize;
        }
        Ok(len)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], ()> {
        let end = self.at.checked_add(len).ok_or(())?;
        if end > self.data.len() {
            return Err(());
        }
        let slice = &self.data[self.at..end];
        self.at = end;
        Ok(slice)
    }
}

#[cfg(test)]
struct TestKey {
    algorithm: Algorithm,
    key: Key,
    spki: Vec<u8>,
}

#[cfg(test)]
fn test_key() -> TestKey {
    unsafe {
        let algorithm = open_algorithm(BCRYPT_RSA_ALGORITHM).expect("rsa");
        let mut handle = BCRYPT_KEY_HANDLE::default();
        let status = BCryptGenerateKeyPair(algorithm.0, &mut handle, 2048, 0);
        assert!(nt_ok(status), "generate");
        let key = Key(handle);
        let status = BCryptFinalizeKeyPair(key.0, 0);
        assert!(nt_ok(status), "finalize");
        let mut size = 0u32;
        let status = BCryptExportKey(key.0, None, BCRYPT_RSAPUBLIC_BLOB, None, &mut size, 0);
        assert!(nt_ok(status), "export size");
        let mut blob = vec![0u8; size as usize];
        let status = BCryptExportKey(
            key.0,
            None,
            BCRYPT_RSAPUBLIC_BLOB,
            Some(&mut blob),
            &mut size,
            0,
        );
        assert!(nt_ok(status), "export");
        blob.truncate(size as usize);
        let spki = spki_from_rsa_blob(&blob);
        TestKey {
            algorithm,
            key,
            spki,
        }
    }
}

#[cfg(test)]
fn sign_with(key: &TestKey, payload: &[u8]) -> Vec<u8> {
    let _ = &key.algorithm;
    let hash = Sha256::digest(payload);
    let padding = BCRYPT_PKCS1_PADDING_INFO {
        pszAlgId: BCRYPT_SHA256_ALGORITHM,
    };
    let mut sig_len = 0u32;
    let status = unsafe {
        BCryptSignHash(
            key.key.0,
            Some(std::ptr::from_ref(&padding).cast()),
            hash.as_slice(),
            None,
            &mut sig_len,
            BCRYPT_PAD_PKCS1,
        )
    };
    assert!(nt_ok(status), "sign size");
    let mut signature = vec![0u8; sig_len as usize];
    let status = unsafe {
        BCryptSignHash(
            key.key.0,
            Some(std::ptr::from_ref(&padding).cast()),
            hash.as_slice(),
            Some(&mut signature),
            &mut sig_len,
            BCRYPT_PAD_PKCS1,
        )
    };
    assert!(nt_ok(status), "sign");
    signature.truncate(sig_len as usize);
    signature
}

#[cfg(test)]
fn spki_from_rsa_blob(blob: &[u8]) -> Vec<u8> {
    assert!(blob.len() >= 24);
    let exponent_len = u32::from_le_bytes(blob[8..12].try_into().unwrap()) as usize;
    let modulus_len = u32::from_le_bytes(blob[12..16].try_into().unwrap()) as usize;
    let exponent = &blob[24..24 + exponent_len];
    let modulus = &blob[24 + exponent_len..24 + exponent_len + modulus_len];
    let pkcs = der_tlv(
        0x30,
        &[der_integer(modulus), der_integer(exponent)].concat(),
    );
    let algorithm = der_tlv(
        0x30,
        &[
            vec![
                0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01,
            ],
            vec![0x05, 0x00],
        ]
        .concat(),
    );
    let bits = der_tlv(0x03, &[vec![0x00], pkcs].concat());
    der_tlv(0x30, &[algorithm, bits].concat())
}

#[cfg(test)]
fn der_integer(bytes: &[u8]) -> Vec<u8> {
    if bytes.first().is_some_and(|byte| byte & 0x80 != 0) {
        der_tlv(0x02, &[vec![0x00], bytes.to_vec()].concat())
    } else {
        der_tlv(0x02, bytes)
    }
}

#[cfg(test)]
fn der_tlv(tag: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    if body.len() < 0x80 {
        out.push(body.len() as u8);
    } else if body.len() <= 0xff {
        out.extend([0x81, body.len() as u8]);
    } else {
        out.push(0x82);
        out.extend((body.len() as u16).to_be_bytes());
    }
    out.extend(body);
    out
}

#[cfg(test)]
fn proto_bytes(field: u64, data: &[u8]) -> Vec<u8> {
    let mut out = proto_varint((field << 3) | 2);
    out.extend(proto_varint(data.len() as u64));
    out.extend(data);
    out
}

#[cfg(test)]
fn proto_varint(mut value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            return out;
        }
    }
}

#[cfg(test)]
fn sample_zip() -> Vec<u8> {
    use std::io::Write;
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("manifest.json", options).unwrap();
        zip.write_all(br#"{"manifest_version":3,"name":"Sample","version":"1.0.0"}"#)
            .unwrap();
        zip.finish().unwrap();
    }
    cursor.into_inner()
}

#[cfg(test)]
fn sample_crx(zip: &[u8]) -> (Vec<u8>, String) {
    let key = test_key();
    let digest = Sha256::digest(&key.spki);
    let signed_header = proto_bytes(1, &digest[..16]);
    let payload = signed_bytes(&signed_header, zip);
    let signature = sign_with(&key, &payload);
    let proof = [proto_bytes(1, &key.spki), proto_bytes(2, &signature)].concat();
    let header = [proto_bytes(2, &proof), proto_bytes(10000, &signed_header)].concat();
    let mut bytes = Vec::new();
    bytes.extend(b"Cr24");
    bytes.extend(3u32.to_le_bytes());
    bytes.extend((header.len() as u32).to_le_bytes());
    bytes.extend(header);
    bytes.extend(zip);
    (bytes, extension_id(&key.spki))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_id_maps_the_first_16_sha256_bytes() {
        let key: Vec<u8> = (0..32).collect();
        assert_eq!(extension_id(&key), "gdanmncjggmeddggjbbcfeeilllcflep");
    }

    #[test]
    fn version_order_is_numeric() {
        assert!(version_is_newer("1.2.10", "1.2.9"));
        assert!(version_is_newer("1.3", "1.2.9"));
        assert!(!version_is_newer("1.2", "1.2.0"));
        assert!(!version_is_newer("1.2.0", "1.2"));
        assert!(!version_is_newer("", "1.0"));
        assert!(version_is_newer("2", "1.9.9"));
    }

    #[test]
    fn parses_store_listings() {
        let chrome = parse_listing(
            "https://chromewebstore.google.com/detail/ublock-origin/cjpalhdlnbpafiamejdnhcphjbkeiagm?hl=en",
        )
        .unwrap();
        assert_eq!(chrome.store, Store::Chrome);
        assert_eq!(chrome.id, "cjpalhdlnbpafiamejdnhcphjbkeiagm");

        let older = parse_listing(
            "https://chrome.google.com/webstore/detail/name/cjpalhdlnbpafiamejdnhcphjbkeiagm",
        )
        .unwrap();
        assert_eq!(older.store, Store::Chrome);

        let edge = parse_listing(
            "https://microsoftedge.microsoft.com/addons/detail/ublock-origin/odfafepnkmbhccpbejgmiehpchacaeak",
        )
        .unwrap();
        assert_eq!(edge.store, Store::Edge);
        assert_eq!(edge.id, "odfafepnkmbhccpbejgmiehpchacaeak");

        let bare = parse_listing("cjpalhdlnbpafiamejdnhcphjbkeiagm").unwrap();
        assert_eq!(bare.store, Store::Chrome);

        assert!(
            parse_listing("https://example.com/detail/cjpalhdlnbpafiamejdnhcphjbkeiagm").is_err()
        );
        assert!(parse_listing("not a link").is_err());
    }

    #[test]
    fn crx3_round_trip_verifies_and_rejects_a_changed_zip() {
        let zip = sample_zip();
        let (bytes, id) = sample_crx(&zip);
        let verified = open_crx(&bytes).unwrap();
        assert_eq!(extension_id(&verified.public_key), id);
        assert_eq!(verified.zip, zip);
        let decoded = decode_archive(&bytes).unwrap();
        assert_eq!(decoded.public_key.unwrap(), verified.public_key);

        let mut tampered = bytes.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0xff;
        assert!(open_crx(&tampered).is_err());
    }

    #[test]
    fn crx2_returns_the_zip_without_a_key() {
        let zip = sample_zip();
        let mut bytes = Vec::new();
        bytes.extend(b"Cr24");
        bytes.extend(2u32.to_le_bytes());
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(1u32.to_le_bytes());
        bytes.extend([0x11, 0x22]);
        bytes.extend(&zip);
        let decoded = decode_archive(&bytes).unwrap();
        assert!(decoded.public_key.is_none());
        assert_eq!(decoded.zip, zip);
    }

    #[test]
    fn plain_zip_passes_through() {
        let zip = sample_zip();
        let decoded = decode_archive(&zip).unwrap();
        assert!(decoded.public_key.is_none());
        assert_eq!(decoded.zip, zip);
    }

    #[test]
    fn feed_url_requests_an_ondemand_install() {
        let url = feed_url(
            Store::Chrome,
            "cjpalhdlnbpafiamejdnhcphjbkeiagm",
            Some("1.2.3"),
            false,
        );
        assert!(url.contains("clients2.google.com/service/update2/crx"));
        assert!(url.contains("installsource%3Dondemand") || url.contains("installsource=ondemand"));
        assert!(url.contains("cjpalhdlnbpafiamejdnhcphjbkeiagm"));
        let edge = feed_url(Store::Edge, "odfafepnkmbhccpbejgmiehpchacaeak", None, true);
        assert!(edge.contains("edge.microsoft.com/extensionwebstorebase/v1/crx"));
        assert!(edge.contains("response=redirect"));
    }

    #[test]
    fn update_check_xml_reads_version_and_noupdate() {
        let found = parse_update_check(
            r#"<gupdate><app appid="abc" status="ok"><updatecheck _esbAllowlist="true" codebase="https://example.test/a.crx" status="ok" version="1.4.2"/></app></gupdate>"#,
        )
        .unwrap();
        assert_eq!(found.status, "ok");
        assert_eq!(found.version, "1.4.2");
        let none = parse_update_check(r#"<updatecheck status="noupdate"/>"#).unwrap();
        assert_eq!(none.status, "noupdate");
    }
}
