//! Capture-device list and a live input meter for the address-bar microphone panel.
//!
//! The page applies the chosen device itself: Chromium device ids are different in every
//! origin, so the saved value is the Windows friendly name.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
use windows::Win32::Media::Audio::{
    AUDCLNT_SHAREMODE_SHARED, DEVICE_STATE_ACTIVE, IAudioCaptureClient, IAudioClient, IMMDevice,
    IMMDeviceEnumerator, MMDeviceEnumerator, WAVEFORMATEX, eCapture, eConsole,
};
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantClear};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, STGM_READ,
};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::core::{HSTRING, PWSTR};

/// Installed at document start. It pins `getUserMedia` to the saved microphone and reports capture.
pub const DOCUMENT_SCRIPT: &str = r#"(() => {
  if (window.__ceMicInstalled) return;
  window.__ceMicInstalled = true;
  const streams = new Set();
  const token = Math.random().toString(36).slice(2) + Date.now().toString(36);
  let reporting = false;
  function post(active) {
    try {
      var host = window.__ceHost || (window.chrome && window.chrome.webview);
      if (host) host.postMessage({ type: "ce-mic", active: active, token: token });
    } catch (error) {}
  }
  function live() {
    for (const stream of streams) {
      if (stream.getAudioTracks().some((track) => track.readyState === "live")) return true;
    }
    return false;
  }
  function sync() {
    const now = live();
    if (now === reporting) return;
    reporting = now;
    post(now);
  }
  function labelOf() {
    if (typeof window.__ceMicLabel === "string") return window.__ceMicLabel;
    try { return localStorage.getItem("ce-microphone") || ""; } catch (error) { return ""; }
  }
  window.__ceStopMic = function () {
    for (const stream of streams) {
      stream.getAudioTracks().forEach((track) => { try { track.stop(); } catch (error) {} });
    }
    streams.clear();
    sync();
  };
  const devices = navigator.mediaDevices;
  if (!devices || !devices.getUserMedia) return;
  const native = devices.getUserMedia.bind(devices);
  devices.getUserMedia = async function (constraints) {
    let next = constraints;
    const audio = constraints && constraints.audio;
    const label = labelOf();
    if (audio && audio !== false && !Array.isArray(audio) && label) {
      try {
        const list = await devices.enumerateDevices();
        const match = list.find((device) => device.kind === "audioinput" && device.label === label);
        if (match && match.deviceId) {
          const base = audio === true ? {} : Object.assign({}, audio);
          base.deviceId = { exact: match.deviceId };
          next = Object.assign({}, constraints, { audio: base });
        }
      } catch (error) {}
    }
    const stream = await native(next);
    if (stream.getAudioTracks().length) {
      streams.add(stream);
      stream.getAudioTracks().forEach((track) => track.addEventListener("ended", sync));
      sync();
    }
    return stream;
  };
})();"#;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicrophoneDevice {
    pub id: String,
    pub label: String,
    pub is_default: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MicrophoneLevel {
    device_id: String,
    level: f32,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MicrophoneCapture {
    tab_id: u32,
    active: bool,
}

pub fn list() -> Result<Vec<MicrophoneDevice>, String> {
    com_init();
    unsafe {
        let enumerator = enumerator()?;
        let default_id = enumerator
            .GetDefaultAudioEndpoint(eCapture, eConsole)
            .ok()
            .as_ref()
            .map(device_id_of)
            .unwrap_or_default();
        let collection = enumerator
            .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)
            .map_err(win_err)?;
        let count = collection.GetCount().map_err(win_err)?;
        let mut devices = Vec::new();
        for index in 0..count {
            let Ok(device) = collection.Item(index) else {
                continue;
            };
            let id = device_id_of(&device);
            if id.is_empty() {
                continue;
            }
            let mut label = friendly_name(&device);
            if label.is_empty() {
                label = "Microphone".into();
            }
            devices.push(MicrophoneDevice {
                is_default: id == default_id,
                id,
                label,
            });
        }
        devices.sort_by(|left, right| {
            right
                .is_default
                .cmp(&left.is_default)
                .then_with(|| left.label.cmp(&right.label))
        });
        Ok(devices)
    }
}

struct WatchHandle {
    stop: Arc<AtomicBool>,
    epoch: u64,
}

/// `epoch` comes from the panel. A late call with an older epoch does not replace a newer watch.
pub fn watch(app: AppHandle, device_id: String, epoch: u64) {
    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut slot = watch_flag().lock().expect("microphone watch");
        if slot.as_ref().is_some_and(|watch| watch.epoch > epoch) {
            return;
        }
        if let Some(previous) = slot.replace(WatchHandle {
            stop: stop.clone(),
            epoch,
        }) {
            previous.stop.store(true, Ordering::Relaxed);
        }
    }
    thread::Builder::new()
        .name("microphone".into())
        .spawn(move || run_watch(app, device_id, stop))
        .ok();
}

pub fn unwatch(epoch: u64) {
    let mut slot = watch_flag().lock().expect("microphone watch");
    if slot.as_ref().is_some_and(|watch| watch.epoch != epoch) {
        return;
    }
    if let Some(watch) = slot.take() {
        watch.stop.store(true, Ordering::Relaxed);
    }
}

pub fn note_capture(app: &AppHandle, tab_id: u32, token: &str, active: bool) {
    let token = token.chars().take(80).collect::<String>();
    if token.is_empty() {
        return;
    }
    let mut map = captures().lock().expect("microphone capture");
    let was_live = map.get(&tab_id).is_some_and(|tokens| !tokens.is_empty());
    if active {
        let tokens = map.entry(tab_id).or_default();
        if tokens.len() < 32 {
            tokens.insert(token);
        }
    } else if let Some(tokens) = map.get_mut(&tab_id) {
        tokens.remove(&token);
    }
    if map.get(&tab_id).is_some_and(|tokens| tokens.is_empty()) {
        map.remove(&tab_id);
    }
    let live = map.get(&tab_id).is_some_and(|tokens| !tokens.is_empty());
    drop(map);
    if was_live != live {
        let _ = app.emit(
            "microphone-capture",
            MicrophoneCapture {
                tab_id,
                active: live,
            },
        );
    }
}

pub fn forget_tab(app: &AppHandle, tab_id: u32) {
    let removed = captures()
        .lock()
        .expect("microphone capture")
        .remove(&tab_id)
        .is_some();
    if removed {
        let _ = app.emit(
            "microphone-capture",
            MicrophoneCapture {
                tab_id,
                active: false,
            },
        );
    }
}

fn run_watch(app: AppHandle, device_id: String, stop: Arc<AtomicBool>) {
    com_init();
    let session = match open_capture(&device_id) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("microphone: {error}");
            return;
        }
    };
    while !stop.load(Ordering::Relaxed) {
        let Some(level) = session.sample() else {
            break;
        };
        let _ = app.emit(
            "microphone-level",
            MicrophoneLevel {
                device_id: session.id.clone(),
                level,
            },
        );
        thread::sleep(Duration::from_millis(66));
    }
}

struct Capture {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    meter: Option<IAudioMeterInformation>,
    bits: u16,
    block: u16,
    float_samples: bool,
    id: String,
}

impl Capture {
    fn sample(&self) -> Option<f32> {
        let mut heard = 0f32;
        unsafe {
            loop {
                let packet = self.capture.GetNextPacketSize().ok()?;
                if packet == 0 {
                    break;
                }
                let mut data = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                self.capture
                    .GetBuffer(&mut data, &mut frames, &mut flags, None, None)
                    .ok()?;
                let silent = flags & 2 != 0;
                if !silent && !data.is_null() && frames > 0 && self.block > 0 {
                    if let Some(bytes) = (frames as usize).checked_mul(self.block as usize) {
                        let slice = std::slice::from_raw_parts(data, bytes);
                        heard = heard.max(pcm_peak(slice, self.bits, self.float_samples));
                    }
                }
                self.capture.ReleaseBuffer(frames).ok()?;
            }
            let meter = self
                .meter
                .as_ref()
                .and_then(|meter| meter.GetPeakValue().ok())
                .unwrap_or(0.0);
            Some(heard.max(meter).clamp(0.0, 1.0))
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}

fn open_capture(device_id: &str) -> Result<Capture, String> {
    unsafe {
        let enumerator = enumerator()?;
        let device = if device_id.is_empty() {
            enumerator
                .GetDefaultAudioEndpoint(eCapture, eConsole)
                .map_err(win_err)?
        } else {
            match enumerator.GetDevice(&HSTRING::from(device_id)) {
                Ok(device) => device,
                Err(_) => enumerator
                    .GetDefaultAudioEndpoint(eCapture, eConsole)
                    .map_err(win_err)?,
            }
        };
        let id = device_id_of(&device);
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(win_err)?;
        let format = client.GetMixFormat().map_err(win_err)?;
        let bits = read_u16(format, 14);
        let block = read_u16(format, 12);
        let float_samples = float_format(format);
        let init = client.Initialize(AUDCLNT_SHAREMODE_SHARED, 0, 1_000_000, 0, format, None);
        CoTaskMemFree(Some(format as *const core::ffi::c_void));
        init.map_err(win_err)?;
        let capture = client.GetService().map_err(win_err)?;
        let meter = device.Activate(CLSCTX_ALL, None).ok();
        client.Start().map_err(win_err)?;
        Ok(Capture {
            client,
            capture,
            meter,
            bits,
            block,
            float_samples,
            id,
        })
    }
}

fn enumerator() -> Result<IMMDeviceEnumerator, String> {
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(win_err) }
}

fn device_id_of(device: &IMMDevice) -> String {
    unsafe { device.GetId().ok().map(pwstr_owned).unwrap_or_default() }
}

fn friendly_name(device: &IMMDevice) -> String {
    unsafe {
        let Ok(store) = device.OpenPropertyStore(STGM_READ) else {
            return String::new();
        };
        let Ok(mut value) = store.GetValue(&PKEY_Device_FriendlyName) else {
            return String::new();
        };
        let name = prop_text(&value).unwrap_or_default();
        let _ = PropVariantClear(&mut value);
        name.chars().take(200).collect()
    }
}

fn prop_text(value: &PROPVARIANT) -> Option<String> {
    unsafe {
        if (*value.Anonymous.Anonymous).vt != VT_LPWSTR {
            return None;
        }
        let raw = (*value.Anonymous.Anonymous).Anonymous.pwszVal;
        if raw.is_null() {
            return None;
        }
        let mut length = 0;
        while *raw.0.add(length) != 0 {
            length += 1;
        }
        let slice = std::slice::from_raw_parts(raw.0, length);
        Some(String::from_utf16_lossy(slice))
    }
}

fn pwstr_owned(raw: PWSTR) -> String {
    if raw.0.is_null() {
        return String::new();
    }
    unsafe {
        let mut length = 0;
        while *raw.0.add(length) != 0 {
            length += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(raw.0, length));
        CoTaskMemFree(Some(raw.0 as *const core::ffi::c_void));
        text
    }
}

fn read_u16(format: *const WAVEFORMATEX, offset: usize) -> u16 {
    unsafe { std::ptr::read_unaligned((format as *const u8).add(offset) as *const u16) }
}

fn float_format(format: *const WAVEFORMATEX) -> bool {
    unsafe {
        let tag = read_u16(format, 0);
        if tag == 3 {
            return true;
        }
        if tag != 0xFFFE || read_u16(format, 16) < 22 {
            return false;
        }
        let mut guid = [0u8; 4];
        std::ptr::copy_nonoverlapping((format as *const u8).add(24), guid.as_mut_ptr(), 4);
        guid == [3, 0, 0, 0]
    }
}

fn pcm_peak(data: &[u8], bits: u16, float_samples: bool) -> f32 {
    let mut max = 0f32;
    if float_samples {
        for chunk in data.chunks_exact(4) {
            let sample = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            if sample.is_finite() {
                max = max.max(sample.abs());
            }
        }
    } else if bits == 16 {
        for chunk in data.chunks_exact(2) {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            max = max.max((sample as f32 / i16::MAX as f32).abs());
        }
    } else if bits == 32 {
        for chunk in data.chunks_exact(4) {
            let sample = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            max = max.max((sample as f32 / i32::MAX as f32).abs());
        }
    }
    max.min(1.0)
}

fn com_init() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

fn win_err(error: windows::core::Error) -> String {
    let message = error.message();
    if message.is_empty() {
        format!("{error}")
    } else {
        message.to_string()
    }
}

fn watch_flag() -> &'static Mutex<Option<WatchHandle>> {
    static FLAG: OnceLock<Mutex<Option<WatchHandle>>> = OnceLock::new();
    FLAG.get_or_init(|| Mutex::new(None))
}

fn captures() -> &'static Mutex<HashMap<u32, HashSet<String>>> {
    static CAPTURES: OnceLock<Mutex<HashMap<u32, HashSet<String>>>> = OnceLock::new();
    CAPTURES.get_or_init(Default::default)
}
