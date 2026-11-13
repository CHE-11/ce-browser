//! Live adapter throughput, Wi-Fi signal, gateway latency, and a public ping.
//!
//! Rates are the whole adapter, not only this browser. A sample newer than
//! [`FRESH`] is reused so the toolbar and the dropdown can poll together.
//! Ping and jitter come from echo requests to 1.1.1.1. The local address is the
//! adapter's preferred unicast address. The external address is what a public
//! server sees, refreshed about twice a minute.

use std::ffi::c_void;
use std::io::Read;
use std::mem::{size_of, zeroed};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE};
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfTable2, GetIpForwardTable2, GetUnicastIpAddressTable, ICMP_ECHO_REPLY,
    IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, MIB_IF_ROW2, MIB_IF_TABLE2, MIB_IPFORWARD_ROW2,
    MIB_IPFORWARD_TABLE2, MIB_UNICASTIPADDRESS_ROW, MIB_UNICASTIPADDRESS_TABLE,
};
use windows::Win32::NetworkManagement::Ndis::{
    IfOperStatusUp, MediaConnectStateConnected, NdisPhysicalMediumNative802_11,
    NdisPhysicalMediumWirelessLan,
};
use windows::Win32::NetworkManagement::WiFi::{
    WLAN_BSS_LIST, WLAN_CONNECTION_ATTRIBUTES, WLAN_INTERFACE_INFO_LIST, WlanCloseHandle,
    WlanEnumInterfaces, WlanFreeMemory, WlanGetNetworkBssList, WlanOpenHandle, WlanQueryInterface,
    dot11_BSS_type_any, wlan_intf_opcode_current_connection,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6, IpDadStatePreferred, SOCKADDR_INET};
use windows::core::GUID;

const FRESH: Duration = Duration::from_millis(700);
const PING_EVERY: Duration = Duration::from_secs(3);
const PING_TIMEOUT_MS: u32 = 1000;
/// How often a settled connection asks again what address the internet sees.
const EXTERNAL_EVERY: Duration = Duration::from_secs(30);
const EXTERNAL_TIMEOUT: Duration = Duration::from_secs(5);
const TRACE_URL: &str = "https://cloudflare.com/cdn-cgi/trace";
const PING_WINDOW: usize = 8;
/// 1.1.1.1 in the same network-order `u32` `IcmpSendEcho` already uses for the gateway.
const INTERNET_V4: u32 = u32::from_ne_bytes([1, 1, 1, 1]);
/// `IF_TYPE_SOFTWARE_LOOPBACK`.
const IF_TYPE_LOOPBACK: u32 = 24;
/// `IF_TYPE_ETHERNET_CSMACD`.
const IF_TYPE_ETHERNET: u32 = 6;
/// `IF_TYPE_IEEE80211`.
const IF_TYPE_IEEE80211: u32 = 71;

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NetworkKind {
    Wifi,
    Ethernet,
    Other,
    Offline,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSpeed {
    pub online: bool,
    pub name: String,
    pub kind: NetworkKind,
    /// Bytes per second since the previous sample.
    pub down_bps: f64,
    /// Bytes per second since the previous sample.
    pub up_bps: f64,
    /// Negotiated link speed, bits per second. Zero when unknown.
    pub link_bps: f64,
    /// Wi-Fi signal, 0–100. Absent on a wired link or when the query fails.
    pub signal: Option<u32>,
    /// Round trip to the default gateway, milliseconds.
    pub rtt_ms: Option<u32>,
    /// Round trip to 1.1.1.1, milliseconds.
    pub ping_ms: Option<u32>,
    /// Mean gap between consecutive public pings, milliseconds.
    pub jitter_ms: Option<u32>,
    /// IPv4 of the connected adapter, or IPv6 when it has no IPv4.
    pub ip_address: Option<String>,
    /// Address a public server sees for this connection.
    pub external_ip: Option<String>,
}

#[derive(Clone)]
struct Adapter {
    index: u32,
    guid: GUID,
    name: String,
    kind: NetworkKind,
    down: u64,
    up: u64,
    link: u64,
}

struct Route {
    index: u32,
    metric: u32,
    gateway_v4: Option<u32>,
}

struct Counters {
    at: Instant,
    index: u32,
    down: u64,
    up: u64,
}

struct State {
    snapshot: NetworkSpeed,
    taken: Option<Instant>,
    counters: Option<Counters>,
    gateway: Option<u32>,
    rtt_ms: Option<u32>,
    adapter_index: Option<u32>,
    ping_ms: Option<u32>,
    jitter_ms: Option<u32>,
    ping_samples: [u32; PING_WINDOW],
    ping_count: u8,
    ping_next: u8,
    external_ip: Option<String>,
    wifi_guid: Option<GUID>,
    background_started: bool,
}

const OFFLINE: NetworkSpeed = NetworkSpeed {
    online: false,
    name: String::new(),
    kind: NetworkKind::Offline,
    down_bps: 0.0,
    up_bps: 0.0,
    link_bps: 0.0,
    signal: None,
    rtt_ms: None,
    ping_ms: None,
    jitter_ms: None,
    ip_address: None,
    external_ip: None,
};

static STATE: Mutex<State> = Mutex::new(State {
    snapshot: OFFLINE,
    taken: None,
    counters: None,
    gateway: None,
    rtt_ms: None,
    adapter_index: None,
    ping_ms: None,
    jitter_ms: None,
    ping_samples: [0; PING_WINDOW],
    ping_count: 0,
    ping_next: 0,
    external_ip: None,
    wifi_guid: None,
    background_started: false,
});

pub fn snapshot() -> NetworkSpeed {
    {
        let state = STATE.lock().expect("network speed");
        if state.fresh() {
            return state.view();
        }
    }
    let reading = sample_link();
    let (speed, start) = {
        let mut state = STATE.lock().expect("network speed");
        if !state.fresh() {
            state.apply(reading);
        }
        let start = !state.background_started;
        state.background_started = true;
        (state.view(), start)
    };
    if start {
        let _ = thread::Builder::new()
            .name("network-speed-ping".into())
            .spawn(ping_loop);
        let _ = thread::Builder::new()
            .name("network-speed-wifi".into())
            .spawn(signal_loop);
        let _ = thread::Builder::new()
            .name("network-speed-external".into())
            .spawn(external_loop);
    }
    speed
}

struct Reading {
    adapter: Option<Adapter>,
    gateway: Option<u32>,
    ip: Option<String>,
}

fn sample_link() -> Reading {
    let adapters = interfaces();
    let routes = default_routes();
    match choose(&adapters, &routes) {
        Some((adapter, gateway)) => {
            let ip = adapter_address(adapter.index);
            Reading {
                adapter: Some(adapter),
                gateway,
                ip,
            }
        }
        None => Reading {
            adapter: None,
            gateway: None,
            ip: None,
        },
    }
}

impl State {
    fn fresh(&self) -> bool {
        self.taken.is_some_and(|taken| taken.elapsed() < FRESH)
    }

    fn view(&self) -> NetworkSpeed {
        let mut speed = self.snapshot.clone();
        speed.rtt_ms = self.rtt_ms;
        speed.ping_ms = self.ping_ms;
        speed.jitter_ms = self.ping_ms.and(self.jitter_ms);
        speed.external_ip = self.external_ip.clone();
        speed
    }

    fn apply(&mut self, reading: Reading) {
        let Some(adapter) = reading.adapter else {
            self.counters = None;
            self.gateway = None;
            self.rtt_ms = None;
            self.wifi_guid = None;
            self.adapter_index = None;
            self.clear_ping();
            self.external_ip = None;
            self.snapshot = OFFLINE;
            self.taken = Some(Instant::now());
            return;
        };
        if self.gateway != reading.gateway {
            self.rtt_ms = None;
        }
        if self.adapter_index != Some(adapter.index) {
            self.clear_ping();
            self.external_ip = None;
        }
        self.gateway = reading.gateway;
        self.adapter_index = Some(adapter.index);
        let now = Instant::now();
        let (down_bps, up_bps) = match &self.counters {
            Some(prev) if prev.index == adapter.index => (
                per_second(
                    prev.down,
                    adapter.down,
                    now.saturating_duration_since(prev.at),
                ),
                per_second(prev.up, adapter.up, now.saturating_duration_since(prev.at)),
            ),
            _ => (0.0, 0.0),
        };
        let same_wifi = adapter.kind == NetworkKind::Wifi && self.wifi_guid == Some(adapter.guid);
        let signal = if same_wifi {
            self.snapshot.signal
        } else {
            None
        };
        self.wifi_guid = if adapter.kind == NetworkKind::Wifi {
            Some(adapter.guid)
        } else {
            None
        };
        self.counters = Some(Counters {
            at: now,
            index: adapter.index,
            down: adapter.down,
            up: adapter.up,
        });
        self.snapshot = NetworkSpeed {
            online: true,
            name: adapter.name,
            kind: adapter.kind,
            down_bps,
            up_bps,
            link_bps: adapter.link as f64,
            signal,
            rtt_ms: self.rtt_ms,
            ping_ms: self.ping_ms,
            jitter_ms: self.jitter_ms,
            ip_address: reading.ip,
            external_ip: self.external_ip.clone(),
        };
        self.taken = Some(now);
    }

    fn clear_ping(&mut self) {
        self.ping_ms = None;
        self.jitter_ms = None;
        self.ping_count = 0;
        self.ping_next = 0;
    }
}

fn per_second(before: u64, after: u64, elapsed: Duration) -> f64 {
    let secs = elapsed.as_secs_f64();
    if secs <= 0.0 || after < before {
        return 0.0;
    }
    (after - before) as f64 / secs
}

fn choose(adapters: &[Adapter], routes: &[Route]) -> Option<(Adapter, Option<u32>)> {
    for route in routes {
        if let Some(adapter) = adapters.iter().find(|item| item.index == route.index) {
            let gateway = routes.iter().find_map(|item| {
                if item.index == adapter.index {
                    item.gateway_v4
                } else {
                    None
                }
            });
            return Some((adapter.clone(), gateway));
        }
    }
    let adapter = adapters
        .iter()
        .max_by_key(|item| item.down.saturating_add(item.up))?;
    Some((adapter.clone(), None))
}

fn interfaces() -> Vec<Adapter> {
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    if unsafe { GetIfTable2(&mut table) } != ERROR_SUCCESS || table.is_null() {
        return Vec::new();
    }
    let rows = unsafe {
        std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize)
    };
    let adapters = rows.iter().filter_map(adapter_from).collect();
    unsafe { FreeMibTable(table.cast()) };
    adapters
}

fn adapter_from(row: &MIB_IF_ROW2) -> Option<Adapter> {
    if row.OperStatus != IfOperStatusUp || row.MediaConnectState != MediaConnectStateConnected {
        return None;
    }
    if row.Type == IF_TYPE_LOOPBACK {
        return None;
    }
    let name = wide(&row.Alias);
    if name.is_empty() {
        return None;
    }
    let link = if row.ReceiveLinkSpeed > 0 {
        row.ReceiveLinkSpeed
    } else {
        row.TransmitLinkSpeed
    };
    Some(Adapter {
        index: row.InterfaceIndex,
        guid: row.InterfaceGuid,
        name,
        kind: kind_of(row),
        down: row.InOctets,
        up: row.OutOctets,
        link,
    })
}

fn kind_of(row: &MIB_IF_ROW2) -> NetworkKind {
    let medium = row.PhysicalMediumType;
    if row.Type == IF_TYPE_IEEE80211
        || medium == NdisPhysicalMediumWirelessLan
        || medium == NdisPhysicalMediumNative802_11
    {
        NetworkKind::Wifi
    } else if row.Type == IF_TYPE_ETHERNET {
        NetworkKind::Ethernet
    } else {
        NetworkKind::Other
    }
}

fn default_routes() -> Vec<Route> {
    let mut routes = routes_for(AF_INET);
    routes.extend(routes_for(AF_INET6));
    routes.sort_by_key(|route| route.metric);
    routes
}

fn routes_for(family: windows::Win32::Networking::WinSock::ADDRESS_FAMILY) -> Vec<Route> {
    let mut table: *mut MIB_IPFORWARD_TABLE2 = std::ptr::null_mut();
    if unsafe { GetIpForwardTable2(family, &mut table) } != ERROR_SUCCESS || table.is_null() {
        return Vec::new();
    }
    let rows = unsafe {
        std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize)
    };
    let routes = rows.iter().filter_map(route_from).collect();
    unsafe { FreeMibTable(table.cast()) };
    routes
}

fn route_from(row: &MIB_IPFORWARD_ROW2) -> Option<Route> {
    if row.DestinationPrefix.PrefixLength != 0 || row.Loopback {
        return None;
    }
    Some(Route {
        index: row.InterfaceIndex,
        metric: row.Metric,
        gateway_v4: gateway_v4(&row.NextHop),
    })
}

fn gateway_v4(next: &SOCKADDR_INET) -> Option<u32> {
    unsafe {
        if next.Ipv4.sin_family != AF_INET {
            return None;
        }
        let addr = next.Ipv4.sin_addr.S_un.S_addr;
        if addr == 0 { None } else { Some(addr) }
    }
}

fn wide(units: &[u16]) -> String {
    let end = units
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end]).trim().to_string()
}

fn wifi_signal(guid: GUID) -> Option<u32> {
    unsafe {
        let mut version = 0u32;
        let mut client = HANDLE::default();
        if WlanOpenHandle(2, None, &mut version, &mut client) != 0 {
            return None;
        }
        let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
        let status = WlanEnumInterfaces(client, None, &mut list);
        let quality = if status == 0 && !list.is_null() {
            let found = signal_from_list(client, list, guid);
            WlanFreeMemory(list.cast());
            found
        } else {
            None
        };
        let _ = WlanCloseHandle(client, None);
        quality
    }
}

fn signal_from_list(
    client: HANDLE,
    list: *const WLAN_INTERFACE_INFO_LIST,
    guid: GUID,
) -> Option<u32> {
    let rows = unsafe {
        std::slice::from_raw_parts(
            (*list).InterfaceInfo.as_ptr(),
            (*list).dwNumberOfItems as usize,
        )
    };
    let mut only = None;
    let mut connected = 0u32;
    for info in rows {
        let Some(quality) = connection_quality(client, &info.InterfaceGuid) else {
            continue;
        };
        connected += 1;
        only = Some(quality);
        if info.InterfaceGuid == guid {
            return Some(quality);
        }
    }
    if connected == 1 { only } else { None }
}

fn connection_quality(client: HANDLE, guid: *const GUID) -> Option<u32> {
    unsafe {
        let mut size = 0u32;
        let mut data: *mut c_void = std::ptr::null_mut();
        let status = WlanQueryInterface(
            client,
            guid,
            wlan_intf_opcode_current_connection,
            None,
            &mut size,
            &mut data,
            None,
        );
        if status == 0 && !data.is_null() {
            let attrs = &*(data as *const WLAN_CONNECTION_ATTRIBUTES);
            let quality = attrs.wlanAssociationAttributes.wlanSignalQuality.min(100);
            WlanFreeMemory(data);
            return Some(quality);
        }
        bss_quality(client, guid)
    }
}

/// Strongest visible network. Used when the connection query is denied.
fn bss_quality(client: HANDLE, guid: *const GUID) -> Option<u32> {
    let mut best: Option<u32> = None;
    for secured in [true, false] {
        let quality = bss_list_quality(client, guid, secured);
        best = match (best, quality) {
            (Some(current), Some(next)) => Some(current.max(next)),
            (None, next) => next,
            (current, None) => current,
        };
    }
    best
}

fn bss_list_quality(client: HANDLE, guid: *const GUID, secured: bool) -> Option<u32> {
    unsafe {
        let mut list: *mut WLAN_BSS_LIST = std::ptr::null_mut();
        let status = WlanGetNetworkBssList(
            client,
            guid,
            None,
            dot11_BSS_type_any,
            secured,
            None,
            &mut list,
        );
        if status != 0 || list.is_null() {
            return None;
        }
        let entries = std::slice::from_raw_parts(
            (*list).wlanBssEntries.as_ptr(),
            (*list).dwNumberOfItems as usize,
        );
        let mut best = None;
        for entry in entries {
            let quality = entry.uLinkQuality.min(100);
            best = Some(best.map_or(quality, |current: u32| current.max(quality)));
        }
        WlanFreeMemory(list.cast());
        best
    }
}

fn signal_loop() {
    loop {
        let guid = STATE.lock().expect("network speed").wifi_guid;
        let signal = guid.and_then(wifi_signal);
        if let Ok(mut state) = STATE.lock() {
            if state.wifi_guid == guid {
                state.snapshot.signal = signal;
            }
        }
        thread::sleep(PING_EVERY);
    }
}

fn ping_loop() {
    loop {
        let (gateway, index) = {
            let state = STATE.lock().expect("network speed");
            (state.gateway, state.adapter_index)
        };
        let rtt = gateway.and_then(probe);
        let ping = if index.is_some() {
            probe(INTERNET_V4)
        } else {
            None
        };
        if let Ok(mut state) = STATE.lock() {
            if state.gateway == gateway {
                state.rtt_ms = rtt;
            }
            if state.adapter_index == index {
                match ping {
                    Some(ms) => record_ping(&mut state, ms),
                    None => state.ping_ms = None,
                }
            }
        }
        thread::sleep(PING_EVERY);
    }
}

fn record_ping(state: &mut State, rtt: u32) {
    let next = state.ping_next as usize;
    state.ping_samples[next] = rtt;
    state.ping_next = ((next + 1) % PING_WINDOW) as u8;
    if (state.ping_count as usize) < PING_WINDOW {
        state.ping_count += 1;
    }
    state.ping_ms = Some(rtt);
    state.jitter_ms = ping_jitter(&state.ping_samples, state.ping_count, state.ping_next);
}

/// Mean absolute gap between consecutive samples, oldest first.
fn ping_jitter(samples: &[u32; PING_WINDOW], count: u8, next: u8) -> Option<u32> {
    let count = count as usize;
    if count < 2 {
        return None;
    }
    let start = (next as usize + PING_WINDOW - count) % PING_WINDOW;
    let mut previous = samples[start];
    let mut sum = 0u32;
    for step in 1..count {
        let current = samples[(start + step) % PING_WINDOW];
        sum += current.abs_diff(previous);
        previous = current;
    }
    Some(sum / (count as u32 - 1))
}

fn probe(dest: u32) -> Option<u32> {
    #[repr(C)]
    struct Reply {
        echo: ICMP_ECHO_REPLY,
        extra: [u8; 64],
    }

    unsafe {
        let handle = IcmpCreateFile().ok()?;
        let payload = [0u8; 32];
        let mut reply: Reply = zeroed();
        let replies = IcmpSendEcho(
            handle,
            dest,
            payload.as_ptr().cast(),
            payload.len() as u16,
            None,
            (&mut reply as *mut Reply).cast(),
            size_of::<Reply>() as u32,
            PING_TIMEOUT_MS,
        );
        let _ = IcmpCloseHandle(handle);
        if replies == 0 || reply.echo.Status != 0 {
            return None;
        }
        Some(reply.echo.RoundTripTime)
    }
}

fn adapter_address(index: u32) -> Option<String> {
    unicast_address(index, AF_INET).or_else(|| unicast_address(index, AF_INET6))
}

fn unicast_address(
    index: u32,
    family: windows::Win32::Networking::WinSock::ADDRESS_FAMILY,
) -> Option<String> {
    let mut table: *mut MIB_UNICASTIPADDRESS_TABLE = std::ptr::null_mut();
    if unsafe { GetUnicastIpAddressTable(family, &mut table) } != ERROR_SUCCESS || table.is_null() {
        return None;
    }
    let rows = unsafe {
        std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize)
    };
    let mut best: Option<(u8, String)> = None;
    for row in rows {
        if row.InterfaceIndex != index {
            continue;
        }
        let Some(text) = format_address(row) else {
            continue;
        };
        let score = address_score(row);
        if best.as_ref().is_none_or(|(current, _)| score > *current) {
            best = Some((score, text));
        }
    }
    unsafe { FreeMibTable(table.cast()) };
    best.map(|(_, text)| text)
}

fn address_score(row: &MIB_UNICASTIPADDRESS_ROW) -> u8 {
    u8::from(row.DadState == IpDadStatePreferred) * 2 + u8::from(!row.SkipAsSource)
}

fn format_address(row: &MIB_UNICASTIPADDRESS_ROW) -> Option<String> {
    unsafe {
        let family = row.Address.si_family;
        if family == AF_INET {
            let bytes = row.Address.Ipv4.sin_addr.S_un.S_un_b;
            if bytes.s_b1 == 0 || bytes.s_b1 == 127 || (bytes.s_b1 == 169 && bytes.s_b2 == 254) {
                return None;
            }
            return Some(format!(
                "{}.{}.{}.{}",
                bytes.s_b1, bytes.s_b2, bytes.s_b3, bytes.s_b4
            ));
        }
        if family == AF_INET6 {
            let bytes = row.Address.Ipv6.sin6_addr.u.Byte;
            if skipped_v6(&bytes) {
                return None;
            }
            return Some(format_v6(&bytes));
        }
        None
    }
}

fn skipped_v6(bytes: &[u8; 16]) -> bool {
    let unspecified = bytes.iter().all(|byte| *byte == 0);
    let multicast = bytes[0] == 0xff;
    let link_local = bytes[0] == 0xfe && bytes[1] & 0xc0 == 0x80;
    unspecified || multicast || link_local
}

/// RFC 5952 text form. The longest run of zeros, leftmost on a tie, collapses to `::`.
fn format_v6(bytes: &[u8; 16]) -> String {
    let words: [u16; 8] =
        std::array::from_fn(|index| u16::from_be_bytes([bytes[index * 2], bytes[index * 2 + 1]]));
    let mut best_at = 0usize;
    let mut best_len = 0usize;
    let mut index = 0usize;
    while index < words.len() {
        if words[index] != 0 {
            index += 1;
            continue;
        }
        let start = index;
        while index < words.len() && words[index] == 0 {
            index += 1;
        }
        let len = index - start;
        if len > best_len {
            best_at = start;
            best_len = len;
        }
    }
    if best_len < 2 {
        best_len = 0;
    }
    let mut out = String::new();
    index = 0;
    while index < words.len() {
        if best_len >= 2 && index == best_at {
            out.push_str("::");
            index += best_len;
            continue;
        }
        if !out.is_empty() && !out.ends_with(':') {
            out.push(':');
        }
        out.push_str(&format!("{:x}", words[index]));
        index += 1;
    }
    if out.is_empty() {
        "::".to_string()
    } else {
        out
    }
}

fn external_loop() {
    loop {
        let index = STATE.lock().expect("network speed").adapter_index;
        let Some(index) = index else {
            if let Ok(mut state) = STATE.lock() {
                if state.adapter_index.is_none() {
                    state.external_ip = None;
                }
            }
            thread::sleep(Duration::from_secs(2));
            continue;
        };
        let ip = public_ip();
        if let Ok(mut state) = STATE.lock() {
            if state.adapter_index == Some(index) {
                if let Some(ip) = ip {
                    state.external_ip = Some(ip);
                }
            }
        }
        // A network change should ask again without waiting out the full interval.
        for _ in 0..15 {
            thread::sleep(EXTERNAL_EVERY / 15);
            let current = STATE.lock().expect("network speed").adapter_index;
            if current != Some(index) {
                break;
            }
        }
    }
}

fn public_ip() -> Option<String> {
    let response = ureq::get(TRACE_URL)
        .timeout(EXTERNAL_TIMEOUT)
        .set("User-Agent", "ce-browser")
        .call()
        .ok()?;
    let mut body = String::new();
    response
        .into_reader()
        .take(4096)
        .read_to_string(&mut body)
        .ok()?;
    parse_trace_ip(&body)
}

/// `ip=` line from a Cloudflare trace body.
fn parse_trace_ip(body: &str) -> Option<String> {
    let value = body.lines().find_map(|line| line.strip_prefix("ip="))?;
    let ip = value.trim();
    if ip.parse::<std::net::IpAddr>().is_ok() {
        Some(ip.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{format_v6, parse_trace_ip, ping_jitter};

    #[test]
    fn ipv6_compresses_the_longest_zero_run() {
        let mut bytes = [0u8; 16];
        bytes[0] = 0x20;
        bytes[1] = 0x01;
        bytes[2] = 0x0d;
        bytes[3] = 0xb8;
        bytes[15] = 1;
        assert_eq!(format_v6(&bytes), "2001:db8::1");
    }

    #[test]
    fn ipv6_loopback_is_double_colon_one() {
        let mut bytes = [0u8; 16];
        bytes[15] = 1;
        assert_eq!(format_v6(&bytes), "::1");
    }

    #[test]
    fn trace_body_yields_the_public_address() {
        let body = "fl=abc\nip=203.0.113.8\nts=1\nloc=US\n";
        assert_eq!(parse_trace_ip(body).as_deref(), Some("203.0.113.8"));
        assert_eq!(parse_trace_ip("fl=abc\nip=not-an-ip\n"), None);
    }

    #[test]
    fn jitter_is_the_mean_gap_between_pings() {
        let samples = [10, 12, 11, 20, 0, 0, 0, 0];
        assert_eq!(ping_jitter(&samples, 4, 4), Some(4));
    }

    #[test]
    fn jitter_reads_a_wrapped_window_in_order() {
        // Oldest sample sits at index 2. Each step is 4 ms.
        let samples = [34, 38, 10, 14, 18, 22, 26, 30];
        assert_eq!(ping_jitter(&samples, 8, 2), Some(4));
    }
}
