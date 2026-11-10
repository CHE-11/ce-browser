//! H.264 + AAC MP4 through the Windows Media Foundation sink writer.
//!
//! A trimmed export copies the file when the whole take is kept. A shorter range is decoded and
//! written again so the cut can start on any frame, not only a keyframe.

use std::path::Path;
use std::sync::OnceLock;

use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D11::{D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice};
use windows::Win32::Media::MediaFoundation::{
    MFAudioFormat_AAC, MFAudioFormat_PCM, MFCreateDXGIDeviceManager, MFCreateMediaType, MFCreateMemoryBuffer, MFCreateSample,
    MFCreateSinkWriterFromURL, MFCreateSourceReaderFromURL, MFMediaType_Audio, MFMediaType_Video,
    MFStartup, MFVideoFormat_H264, MFVideoFormat_RGB32, MFVideoInterlace_Progressive, MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
    MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_BLOCK_ALIGNMENT, MF_MT_AUDIO_NUM_CHANNELS,
    MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_AVG_BITRATE, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE,
    MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE,
    MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READER_FIRST_AUDIO_STREAM, MF_SOURCE_READER_FIRST_VIDEO_STREAM,
    MF_VERSION, MFSTARTUP_NOSOCKET, IMFDXGIDeviceManager, IMFMediaBuffer, IMFSample, IMFSinkWriter, IMFSourceReader,
};
use windows::Win32::Foundation::HMODULE;
use windows::core::{IUnknown, HSTRING};

use super::output_size;

const FPS: u32 = 30;
const FRAME_HNS: i64 = 10_000_000 / FPS as i64;
const AAC_PAYLOAD: windows::core::GUID = windows::core::GUID::from_u128(0xbfbabe79_7434_4d1c_94f0_72a3b9e17188);

pub struct Writer {
    writer: IMFSinkWriter,
    video: u32,
    audio: Option<u32>,
    width: u32,
    height: u32,
    /// Kept alive so a hardware encoder can keep using the device.
    _manager: Option<windows::Win32::Media::MediaFoundation::IMFDXGIDeviceManager>,
}

impl Writer {
    pub fn open(path: &Path, width: u32, height: u32) -> Result<Self, String> {
        mf_startup()?;
        let (width, height) = output_size(width, height, width.max(height));
        if let Ok(writer) = open_configured(path, width, height, true) {
            return Ok(writer);
        }
        open_configured(path, width, height, false)
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn video(&self, bgra: &[u8], time: i64, duration: i64) -> Result<(), String> {
        let bytes = (self.width as usize) * (self.height as usize) * 4;
        if bgra.len() < bytes {
            return Err("The video frame is the wrong size.".into());
        }
        unsafe {
            let sample = sample_from(&bgra[..bytes], time, duration.max(1))?;
            self.writer.WriteSample(self.video, &sample).map_err(text)
        }
    }

    pub fn audio(&self, pcm: &[u8], time: i64) -> Result<(), String> {
        let Some(index) = self.audio else {
            return Ok(());
        };
        if pcm.is_empty() {
            return Ok(());
        }
        let duration = pcm.len() as i64 * 10_000_000 / (48_000 * 4);
        unsafe {
            let sample = sample_from(pcm, time, duration.max(1))?;
            self.writer.WriteSample(index, &sample).map_err(text)
        }
    }

    pub fn finish(self) -> Result<(), String> {
        unsafe { self.writer.Finalize().map_err(text) }
    }
}

pub fn export_video(src: &Path, dest: &Path, start_ms: u64, end_ms: u64, duration_ms: u64) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("Couldn't export the recording. {error}"))?;
    }
    let covers_all = start_ms == 0 && (end_ms == 0 || end_ms + 80 >= duration_ms);
    if covers_all {
        std::fs::copy(src, dest).map_err(|error| format!("Couldn't export the recording. {error}"))?;
        return Ok(());
    }
    mf_startup()?;
    trim(src, dest, start_ms, end_ms.max(start_ms + 1))
}

fn trim(src: &Path, dest: &Path, start_ms: u64, end_ms: u64) -> Result<(), String> {
    let start = ms_to_hns(start_ms);
    let end = ms_to_hns(end_ms);
    unsafe {
        let wide = HSTRING::from(src.to_string_lossy().as_ref());
        let reader = MFCreateSourceReaderFromURL(&wide, None).map_err(text)?;
        let video_index = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
        let wanted = MFCreateMediaType().map_err(text)?;
        wanted.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(text)?;
        wanted.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).map_err(text)?;
        reader.SetCurrentMediaType(video_index, None, &wanted).map_err(text)?;
        let current = reader.GetCurrentMediaType(video_index).map_err(text)?;
        let packed = current.GetUINT64(&MF_MT_FRAME_SIZE).unwrap_or(0);
        let width = (packed >> 32) as u32;
        let height = (packed & 0xffff_ffff) as u32;
        if width == 0 || height == 0 {
            return Err("The recording has no picture.".into());
        }
        let stride = current.GetUINT32(&MF_MT_DEFAULT_STRIDE).unwrap_or(width.saturating_mul(4)) as i32;
        let audio_index = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
        let audio_ok = {
            let pcm = MFCreateMediaType().map_err(text)?;
            pcm.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(text)?;
            pcm.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM).map_err(text)?;
            pcm.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16).map_err(text)?;
            pcm.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000).map_err(text)?;
            pcm.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2).map_err(text)?;
            pcm.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4).map_err(text)?;
            pcm.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 192_000).map_err(text)?;
            reader.SetCurrentMediaType(audio_index, None, &pcm).is_ok()
        };
        let writer = Writer::open(dest, width, height)?;
        write_stream(&reader, &writer, video_index, start, end, true, width, height, stride)?;
        if audio_ok && writer.audio.is_some() {
            let _ = write_stream(&reader, &writer, audio_index, start, end, false, width, height, 0);
        }
        writer.finish()
    }
}

fn write_stream(
    reader: &IMFSourceReader,
    writer: &Writer,
    stream: u32,
    start: i64,
    end: i64,
    picture: bool,
    width: u32,
    height: u32,
    stride: i32,
) -> Result<(), String> {
    let mut fallback = 0i64;
    loop {
        let mut flags = 0u32;
        let mut time = 0i64;
        let mut sample = None;
        unsafe {
            reader
                .ReadSample(stream, 0, None, Some(&mut flags), Some(&mut time), Some(&mut sample))
                .map_err(text)?;
        }
        if flags & (MF_SOURCE_READERF_ENDOFSTREAM.0 as u32) != 0 {
            break;
        }
        let Some(sample) = sample else {
            continue;
        };
        if time < start {
            continue;
        }
        if time > end {
            break;
        }
        let bytes = bytes_of(&sample)?;
        let when = time - start;
        if picture {
            let packed = unpack_bgra(&bytes, width, height, stride);
            writer.video(&packed, when, FRAME_HNS)?;
        } else {
            writer.audio(&bytes, if when == 0 { fallback } else { when })?;
        }
        fallback = when + FRAME_HNS;
    }
    Ok(())
}

fn unpack_bgra(data: &[u8], width: u32, height: u32, stride: i32) -> Vec<u8> {
    let row = (width as usize) * 4;
    let pitch = if stride == 0 { row as i32 } else { stride };
    let abs = pitch.unsigned_abs() as usize;
    let mut packed = vec![0u8; row * height as usize];
    for y in 0..height as usize {
        let source_row = if pitch < 0 { height as usize - 1 - y } else { y };
        let start = source_row * abs;
        let end = start + row;
        let dest = y * row;
        if end <= data.len() && dest + row <= packed.len() {
            packed[dest..dest + row].copy_from_slice(&data[start..end]);
        }
    }
    packed
}

fn add_video(writer: &IMFSinkWriter, width: u32, height: u32) -> Result<u32, String> {
    unsafe {
        let output = video_type(width, height, true)?;
        let index = writer.AddStream(&output).map_err(text)?;
        let input = video_type(width, height, false)?;
        writer.SetInputMediaType(index, &input, None).map_err(text)?;
        Ok(index)
    }
}

fn video_type(width: u32, height: u32, encoded: bool) -> Result<windows::Win32::Media::MediaFoundation::IMFMediaType, String> {
    unsafe {
        let kind = MFCreateMediaType().map_err(text)?;
        kind.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(text)?;
        kind.SetGUID(&MF_MT_SUBTYPE, if encoded { &MFVideoFormat_H264 } else { &MFVideoFormat_RGB32 })
            .map_err(text)?;
        kind.SetUINT64(&MF_MT_FRAME_SIZE, pack(width, height)).map_err(text)?;
        kind.SetUINT64(&MF_MT_FRAME_RATE, pack(FPS, 1)).map_err(text)?;
        kind.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1)).map_err(text)?;
        kind.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            .map_err(text)?;
        if encoded {
            let pixels = u64::from(width) * u64::from(height);
            // Screen text falls apart under a low rate. About 8 bits per pixel per second.
            let bitrate = pixels.saturating_mul(8).clamp(12_000_000, 40_000_000) as u32;
            kind.SetUINT32(&MF_MT_AVG_BITRATE, bitrate).map_err(text)?;
        } else {
            kind.SetUINT32(&MF_MT_DEFAULT_STRIDE, width.saturating_mul(4)).map_err(text)?;
        }
        Ok(kind)
    }
}

fn add_audio(writer: &IMFSinkWriter) -> Result<u32, String> {
    unsafe {
        let output = MFCreateMediaType().map_err(text)?;
        output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(text)?;
        output.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC).map_err(text)?;
        output.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16).map_err(text)?;
        output.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000).map_err(text)?;
        output.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2).map_err(text)?;
        output.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 16_000).map_err(text)?;
        output.SetUINT32(&AAC_PAYLOAD, 0).map_err(text)?;
        let index = writer.AddStream(&output).map_err(text)?;
        let input = MFCreateMediaType().map_err(text)?;
        input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(text)?;
        input.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM).map_err(text)?;
        input.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16).map_err(text)?;
        input.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000).map_err(text)?;
        input.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2).map_err(text)?;
        input.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4).map_err(text)?;
        input.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 192_000).map_err(text)?;
        writer.SetInputMediaType(index, &input, None).map_err(text)?;
        Ok(index)
    }
}

fn sample_from(bytes: &[u8], time: i64, duration: i64) -> Result<IMFSample, String> {
    unsafe {
        let buffer: IMFMediaBuffer = MFCreateMemoryBuffer(bytes.len() as u32).map_err(text)?;
        let mut data = std::ptr::null_mut();
        buffer.Lock(&mut data, None, None).map_err(text)?;
        if !data.is_null() {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
        }
        buffer.SetCurrentLength(bytes.len() as u32).map_err(text)?;
        buffer.Unlock().map_err(text)?;
        let sample: IMFSample = MFCreateSample().map_err(text)?;
        sample.AddBuffer(&buffer).map_err(text)?;
        sample.SetSampleTime(time.max(0)).map_err(text)?;
        sample.SetSampleDuration(duration.max(1)).map_err(text)?;
        Ok(sample)
    }
}

fn bytes_of(sample: &IMFSample) -> Result<Vec<u8>, String> {
    unsafe {
        let buffer = sample.ConvertToContiguousBuffer().map_err(text)?;
        let mut data = std::ptr::null_mut();
        let mut length = 0u32;
        buffer.Lock(&mut data, None, Some(&mut length)).map_err(text)?;
        let bytes = if data.is_null() {
            Vec::new()
        } else {
            std::slice::from_raw_parts(data, length as usize).to_vec()
        };
        buffer.Unlock().map_err(text)?;
        Ok(bytes)
    }
}

fn pack(high: u32, low: u32) -> u64 {
    (u64::from(high) << 32) | u64::from(low)
}

fn ms_to_hns(ms: u64) -> i64 {
    (ms as i64).saturating_mul(10_000)
}

fn open_configured(path: &Path, width: u32, height: u32, hardware: bool) -> Result<Writer, String> {
    unsafe {
        let mut attrs = None;
        windows::Win32::Media::MediaFoundation::MFCreateAttributes(&mut attrs, 8).map_err(text)?;
        let manager = hardware.then(hardware_manager).flatten();
        if let Some(attrs) = attrs.as_ref() {
            let _ = attrs.SetUINT32(
                &windows::Win32::Media::MediaFoundation::MF_SINK_WRITER_DISABLE_THROTTLING,
                1,
            );
            let _ = attrs.SetUINT32(&windows::Win32::Media::MediaFoundation::MF_LOW_LATENCY, 1);
            if hardware {
                let _ = attrs.SetUINT32(
                    &windows::Win32::Media::MediaFoundation::MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS,
                    1,
                );
                if let Some(manager) = manager.as_ref() {
                    let _ = attrs.SetUnknown(
                        &windows::Win32::Media::MediaFoundation::MF_SINK_WRITER_D3D_MANAGER,
                        manager,
                    );
                }
            }
        }
        let wide = HSTRING::from(path.to_string_lossy().as_ref());
        let writer = MFCreateSinkWriterFromURL(&wide, None, attrs.as_ref()).map_err(text)?;
        let video = add_video(&writer, width, height)?;
        let audio = add_audio(&writer).ok();
        writer.BeginWriting().map_err(text)?;
        Ok(Writer { writer, video, audio, width, height, _manager: manager })
    }
}

fn hardware_manager() -> Option<IMFDXGIDeviceManager> {
    unsafe {
        let mut token = 0u32;
        let mut manager = None;
        MFCreateDXGIDeviceManager(&mut token, &mut manager).ok()?;
        let manager = manager?;
        let mut device = None;
        let mut context = None;
        let levels = [D3D_FEATURE_LEVEL_11_0];
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE(std::ptr::null_mut()),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&levels),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .ok()?;
        let device = device?;
        let unknown: IUnknown = windows::core::Interface::cast(&device).ok()?;
        manager.ResetDevice(&unknown, token).ok()?;
        Some(manager)
    }
}

fn mf_startup() -> Result<(), String> {
    static READY: OnceLock<Result<(), String>> = OnceLock::new();
    READY
        .get_or_init(|| unsafe { MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).map_err(text) })
        .clone()
}

fn text(error: windows::core::Error) -> String {
    error.to_string()
}
