//! Microphone and system-audio loopback, mixed to 48 kHz stereo PCM.
//!
//! Each device is read on its own thread so a stalled microphone does not stall the picture.
//! Pause throws samples away: the gap is not written into the file.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use windows::Win32::Media::Audio::{
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, IAudioCaptureClient, IAudioClient,
    IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, WAVEFORMATEX, eCapture, eConsole, eRender,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
};
use windows::core::HSTRING;

const RATE: u32 = 48_000;

pub struct Mix {
    stop: Arc<AtomicBool>,
    mic: Arc<Mutex<Vec<f32>>>,
    system: Arc<Mutex<Vec<f32>>>,
    /// Latest microphone peak, 0 to 1000.
    mic_level: Arc<AtomicU32>,
    system_level: Arc<AtomicU32>,
}

impl Mix {
    pub fn start(microphone: bool, system: bool, device_id: &str) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let mic = Arc::new(Mutex::new(Vec::new()));
        let system_queue = Arc::new(Mutex::new(Vec::new()));
        let mic_level = Arc::new(AtomicU32::new(0));
        let system_level = Arc::new(AtomicU32::new(0));
        if microphone {
            spawn(
                stop.clone(),
                mic.clone(),
                device_id.to_string(),
                false,
                Some(mic_level.clone()),
            );
        }
        if system {
            spawn(
                stop.clone(),
                system_queue.clone(),
                String::new(),
                true,
                Some(system_level.clone()),
            );
        }
        Self {
            stop,
            mic,
            system: system_queue,
            mic_level,
            system_level,
        }
    }

    /// Louder of the microphone and system audio, from 0 to 1000.
    pub fn mic_level(&self) -> u32 {
        self.mic_level
            .load(Ordering::Relaxed)
            .max(self.system_level.load(Ordering::Relaxed))
            .min(1000)
    }

    /// `frames` is the number of stereo frames. Mute drops the microphone without letting it pile up.
    pub fn pull(&self, frames: usize, mic_on: bool) -> Vec<u8> {
        let mic = take(&self.mic, frames);
        let system = take(&self.system, frames);
        mix_channels(&mic, &system, if mic_on { 1.0 } else { 0.0 }, 0.85)
    }

    pub fn discard(&self) {
        self.mic.lock().expect("mic").clear();
        self.system.lock().expect("system").clear();
    }
}

impl Drop for Mix {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn spawn(
    stop: Arc<AtomicBool>,
    queue: Arc<Mutex<Vec<f32>>>,
    device_id: String,
    loopback: bool,
    level: Option<Arc<AtomicU32>>,
) {
    thread::Builder::new()
        .name(if loopback {
            "recording-loopback".into()
        } else {
            "recording-mic".into()
        })
        .spawn(move || capture_loop(stop, queue, device_id, loopback, level))
        .ok();
}

fn capture_loop(
    stop: Arc<AtomicBool>,
    queue: Arc<Mutex<Vec<f32>>>,
    device_id: String,
    loopback: bool,
    level: Option<Arc<AtomicU32>>,
) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let Ok(capture) = open(&device_id, loopback) else {
        return;
    };
    while !stop.load(Ordering::Relaxed) {
        if let Some(samples) = capture.read() {
            if let Some(level) = level.as_ref() {
                note_level(level, &samples);
            }
            push(&queue, samples);
        } else if let Some(level) = level.as_ref() {
            decay_level(level);
        }
        thread::sleep(Duration::from_millis(8));
    }
}

fn note_level(level: &AtomicU32, samples: &[f32]) {
    let peak = samples
        .iter()
        .fold(0.0f32, |loudest, sample| loudest.max(sample.abs()));
    let previous = level.load(Ordering::Relaxed) as f32 / 1000.0;
    // Speech sits well below full scale, so the meter is opened up until it clips at 1.
    let next = (peak * 5.0).clamp(0.0, 1.0).max(previous * 0.72);
    level.store((next * 1000.0) as u32, Ordering::Relaxed);
}

fn decay_level(level: &AtomicU32) {
    let previous = level.load(Ordering::Relaxed);
    level.store((previous as f32 * 0.8) as u32, Ordering::Relaxed);
}

fn push(queue: &Mutex<Vec<f32>>, samples: Vec<f32>) {
    let mut guard = queue.lock().expect("audio");
    guard.extend(samples);
    let cap = RATE as usize * 2 * 3;
    if guard.len() > cap {
        let extra = guard.len() - cap;
        guard.drain(0..extra);
    }
}

fn take(queue: &Mutex<Vec<f32>>, frames: usize) -> Vec<f32> {
    let need = frames.saturating_mul(2);
    let mut guard = queue.lock().expect("audio");
    if guard.len() >= need {
        guard.drain(0..need).collect()
    } else {
        let mut samples = std::mem::take(&mut *guard);
        samples.resize(need, 0.0);
        samples
    }
}

pub fn mix_channels(mic: &[f32], system: &[f32], mic_gain: f32, system_gain: f32) -> Vec<u8> {
    let count = mic.len().max(system.len());
    let mut pcm = Vec::with_capacity(count * 2);
    for index in 0..count {
        let mixed = mic.get(index).copied().unwrap_or(0.0) * mic_gain
            + system.get(index).copied().unwrap_or(0.0) * system_gain;
        let sample = (mixed.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        pcm.extend(sample.to_le_bytes());
    }
    pcm
}

struct Capture {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    channels: u16,
    rate: u32,
    bits: u16,
    block: u16,
    float_samples: bool,
}

impl Capture {
    fn read(&self) -> Option<Vec<f32>> {
        let mut decoded = Vec::new();
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
                let silent = flags & 2 != 0 || data.is_null();
                if silent {
                    decoded.extend(std::iter::repeat_n(0.0, frames as usize * 2));
                } else if self.block > 0 {
                    let bytes = (frames as usize).saturating_mul(self.block as usize);
                    let slice = std::slice::from_raw_parts(data, bytes);
                    decode(
                        slice,
                        self.channels,
                        self.bits,
                        self.float_samples,
                        &mut decoded,
                    );
                }
                self.capture.ReleaseBuffer(frames).ok()?;
            }
        }
        Some(resample(decoded, self.rate))
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}

fn open(device_id: &str, loopback: bool) -> Result<Capture, String> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|error| error.to_string())?;
        let device = if !loopback && !device_id.is_empty() {
            enumerator
                .GetDevice(&HSTRING::from(device_id))
                .or_else(|_| enumerator.GetDefaultAudioEndpoint(eCapture, eConsole))
                .map_err(|error| error.to_string())?
        } else if loopback {
            enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|error| error.to_string())?
        } else {
            enumerator
                .GetDefaultAudioEndpoint(eCapture, eConsole)
                .map_err(|error| error.to_string())?
        };
        open_device(&device, loopback)
    }
}

fn open_device(device: &IMMDevice, loopback: bool) -> Result<Capture, String> {
    unsafe {
        let client: IAudioClient = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|error| error.to_string())?;
        let format = client.GetMixFormat().map_err(|error| error.to_string())?;
        let channels = read_u16(format, 2).max(1);
        let rate = read_u32(format, 4).max(1);
        let block = read_u16(format, 12);
        let bits = read_u16(format, 14);
        let float_samples = float_format(format);
        let flags = if loopback {
            AUDCLNT_STREAMFLAGS_LOOPBACK
        } else {
            0
        };
        let init = client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 1_000_000, 0, format, None);
        CoTaskMemFree(Some(format as *const core::ffi::c_void));
        init.map_err(|error| error.to_string())?;
        let capture = client.GetService().map_err(|error| error.to_string())?;
        client.Start().map_err(|error| error.to_string())?;
        Ok(Capture {
            client,
            capture,
            channels,
            rate,
            bits,
            block,
            float_samples,
        })
    }
}

fn decode(data: &[u8], channels: u16, bits: u16, float_samples: bool, out: &mut Vec<f32>) {
    let width = (bits / 8) as usize;
    if width == 0 || channels == 0 {
        return;
    }
    let frame = width * channels as usize;
    if frame == 0 {
        return;
    }
    for chunk in data.chunks(frame) {
        if chunk.len() < frame {
            break;
        }
        let left = sample(&chunk[..width], bits, float_samples);
        let right = if channels > 1 {
            sample(&chunk[width..width * 2], bits, float_samples)
        } else {
            left
        };
        out.push(left);
        out.push(right);
    }
}

fn sample(bytes: &[u8], bits: u16, float_samples: bool) -> f32 {
    if float_samples && bytes.len() >= 4 {
        return f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    }
    match bits {
        16 if bytes.len() >= 2 => i16::from_le_bytes([bytes[0], bytes[1]]) as f32 / 32768.0,
        32 if bytes.len() >= 4 => {
            i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f32 / 2147483648.0
        }
        _ => 0.0,
    }
}

fn resample(stereo: Vec<f32>, rate: u32) -> Vec<f32> {
    if rate == RATE || rate == 0 || stereo.len() < 2 {
        return stereo;
    }
    let source_frames = stereo.len() / 2;
    let dest_frames = ((source_frames as u64) * u64::from(RATE) / u64::from(rate)).max(1) as usize;
    let mut out = Vec::with_capacity(dest_frames * 2);
    for index in 0..dest_frames {
        let position = index as f64 * f64::from(rate) / f64::from(RATE);
        let left_index = (position.floor() as usize).min(source_frames - 1);
        let right_index = (left_index + 1).min(source_frames - 1);
        let fraction = (position - left_index as f64) as f32;
        for channel in 0..2 {
            let start = stereo[left_index * 2 + channel];
            let end = stereo[right_index * 2 + channel];
            out.push(start + (end - start) * fraction);
        }
    }
    out
}

fn read_u16(format: *const WAVEFORMATEX, offset: usize) -> u16 {
    unsafe { std::ptr::read_unaligned((format as *const u8).add(offset) as *const u16) }
}

fn read_u32(format: *const WAVEFORMATEX, offset: usize) -> u32 {
    unsafe { std::ptr::read_unaligned((format as *const u8).add(offset) as *const u32) }
}

fn float_format(format: *const WAVEFORMATEX) -> bool {
    let tag = read_u16(format, 0);
    if tag == 3 {
        return true;
    }
    if tag != 0xFFFE || read_u16(format, 16) < 22 {
        return false;
    }
    unsafe {
        let mut guid = [0u8; 4];
        std::ptr::copy_nonoverlapping((format as *const u8).add(24), guid.as_mut_ptr(), 4);
        guid == [3, 0, 0, 0]
    }
}

#[cfg(test)]
mod tests {
    use super::mix_channels;

    #[test]
    fn mixed_samples_stay_in_range() {
        let pcm = mix_channels(&[1.0, 1.0], &[1.0, 1.0], 1.0, 1.0);
        assert_eq!(pcm, 32767i16.to_le_bytes().repeat(2));
    }
}
