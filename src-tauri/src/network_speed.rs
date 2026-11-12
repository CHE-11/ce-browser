//! Live adapter throughput, Wi-Fi signal, and gateway latency.
//!
//! Rates are the whole adapter, not only this browser. A sample newer than
//! [`FRESH`] is reused so the toolbar and the dropdown can poll together.

use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use windows::Win32::Foundation::{ERROR_SUCCESS, HANDLE};
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfTable2, GetIpForwardTable2, ICMP_ECHO_REPLY, IcmpCloseHandle,
    IcmpCreateFile, IcmpSendEcho, MIB_IF_ROW2, MIB_IF_TABLE2, MIB_IPFORWARD_ROW2,
    MIB_IPFORWARD_TABLE2,
};
use windows::Win32::NetworkManagement::Ndis::{
    IfOperStatusUp, MediaConnectStateConnected, NdisPhysicalMediumNative802_11,
    NdisPhysicalMediumWirelessLan,
};
use windows::Win32::NetworkManagement::WiFi::{
    WlanCloseHandle, WlanEnumInterfaces, WlanFreeMemory, WlanGetNetworkBssList, WlanOpenHandle,
    WlanQueryInterface, WLAN_BSS_LIST, WLAN_CONNECTION_ATTRIBUTES, WLAN_INTERFACE_INFO_LIST,
    dot11_BSS_type_any, wlan_intf_opcode_current_connection,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6, SOCKADDR_INET};
use windows::core::GUID;

const FRESH: Duration = Duration::from_millis(700);
const PING_EVERY: Duration = Duration::from_secs(3);
const PING_TIMEOUT_MS: u32 = 1000;
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
};

static STATE: Mutex<State> = Mutex::new(State {
    snapshot: OFFLINE,
    taken: None,
    counters: None,
    gateway: None,
    rtt_ms: None,
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
    }
    speed
}

struct Reading {
    adapter: Option<Adapter>,
    gateway: Option<u32>,
}

fn sample_link() -> Reading {
    let adapters = interfaces();
    let routes = default_routes();
    match choose(&adapters, &routes) {
        Some((adapter, gateway)) => Reading {
            adapter: Some(adapter),
            gateway,
        },
        None => Reading {
            adapter: None,
            gateway: None,
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
        speed
    }

    fn apply(&mut self, reading: Reading) {
        let Some(adapter) = reading.adapter else {
            self.counters = None;
            self.gateway = None;
            self.rtt_ms = None;
            self.wifi_guid = None;
            self.snapshot = OFFLINE;
            self.taken = Some(Instant::now());
            return;
        };
        if self.gateway != reading.gateway {
            self.rtt_ms = None;
        }
        self.gateway = reading.gateway;
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
        let same_wifi =
            adapter.kind == NetworkKind::Wifi && self.wifi_guid == Some(adapter.guid);
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
        };
        self.taken = Some(now);
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
        let gateway = STATE.lock().expect("network speed").gateway;
        let rtt = gateway.and_then(probe);
        if let Ok(mut state) = STATE.lock() {
            if state.gateway == gateway {
                state.rtt_ms = rtt;
            }
        }
        thread::sleep(PING_EVERY);
    }
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
