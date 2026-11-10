//! Windows Graphics Capture of a window or a monitor.
//!
//! Frames stay on this thread. The encoder keeps the capture's own pixels, up to a 2560 long
//! side, so a window resize does not change the file halfway through.

use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_WARP;
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
    D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Device,
    ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::System::WinRT::Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::core::{Interface, factory};
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{HMODULE, HWND};

use super::{Target, output_size};

pub struct Grabber {
    _device: ID3D11Device,
    context: ID3D11DeviceContext,
    _item: GraphicsCaptureItem,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    staging: Option<ID3D11Texture2D>,
    staging_size: (u32, u32),
    output: Option<(u32, u32)>,
    last: Vec<u8>,
    max_long: u32,
}

impl Grabber {
    pub fn open(target: &Target, max_long: u32) -> Result<Self, String> {
        let item = capture_item(target)?;
        let size = item.Size().map_err(text)?;
        if size.Width <= 0 || size.Height <= 0 {
            return Err("The window is too small to record.".into());
        }
        let (device, context) = d3d_device()?;
        let winrt = winrt_device(&device)?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &winrt,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            4,
            size,
        )
        .map_err(text)?;
        let session = pool.CreateCaptureSession(&item).map_err(text)?;
        let _ = session.SetIsCursorCaptureEnabled(true);
        let _ = session.SetIsBorderRequired(false);
        session.StartCapture().map_err(text)?;
        Ok(Self {
            _device: device,
            context,
            _item: item,
            pool,
            session,
            staging: None,
            staging_size: (0, 0),
            output: None,
            last: Vec::new(),
            max_long,
        })
    }

    /// A frame Windows has not handed us before. `None` while the picture is unchanged.
    pub fn poll_new(&mut self) -> Result<Option<&[u8]>, String> {
        let Some((source_w, source_h, pixels)) = self.copy_frame()? else {
            return Ok(None);
        };
        let max_long = self.max_long;
        let (width, height) = *self.output.get_or_insert_with(|| output_size(source_w, source_h, max_long));
        self.last = if source_w == width && source_h == height {
            pixels
        } else if source_w.abs_diff(width) <= 1 && source_h.abs_diff(height) <= 1 {
            crop(&pixels, source_w, width, height)
        } else {
            scale_bilinear(&pixels, source_w, source_h, width, height)
        };
        Ok(Some(self.last.as_slice()))
    }

    /// The latest frame, including the previous one when nothing new arrived.
    pub fn frame(&mut self) -> Result<Option<&[u8]>, String> {
        let arrived = self.poll_new()?.is_some();
        if arrived || !self.last.is_empty() {
            Ok(Some(self.last.as_slice()))
        } else {
            Ok(None)
        }
    }

    pub fn size(&self) -> Option<(u32, u32)> {
        self.output
    }

    /// Windows draws its own pointer into the capture when this is on.
    pub fn set_cursor_visible(&self, visible: bool) {
        let _ = self.session.SetIsCursorCaptureEnabled(visible);
    }

    fn copy_frame(&mut self) -> Result<Option<(u32, u32, Vec<u8>)>, String> {
        let frame = match self.pool.TryGetNextFrame() {
            Ok(frame) => frame,
            Err(_) => return Ok(None),
        };
        let surface = frame.Surface().map_err(text)?;
        let access: IDirect3DDxgiInterfaceAccess = surface.cast().map_err(text)?;
        let texture: ID3D11Texture2D = unsafe { access.GetInterface().map_err(text)? };
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { texture.GetDesc(&mut desc) };
        let (width, height) = (desc.Width.max(1), desc.Height.max(1));
        self.ensure_staging(width, height)?;
        let staging = self.staging.as_ref().ok_or("The capture buffer is missing.")?;
        unsafe {
            self.context.CopyResource(staging, &texture);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(text)?;
            let packed = pack_rows(mapped.pData as *const u8, mapped.RowPitch as usize, width, height);
            self.context.Unmap(staging, 0);
            let _ = frame.Close();
            Ok(Some((width, height, packed)))
        }
    }

    fn ensure_staging(&mut self, width: u32, height: u32) -> Result<(), String> {
        if self.staging_size == (width, height) && self.staging.is_some() {
            return Ok(());
        }
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut texture = None;
        unsafe {
            self._device
                .CreateTexture2D(&desc, None, Some(&mut texture))
                .map_err(text)?;
        }
        self.staging = texture;
        self.staging_size = (width, height);
        Ok(())
    }
}

impl Drop for Grabber {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}

fn capture_item(target: &super::Target) -> Result<GraphicsCaptureItem, String> {
    let interop: IGraphicsCaptureItemInterop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().map_err(text)?;
    unsafe {
        if target.monitor != 0 {
            let monitor = HMONITOR(target.monitor as *mut core::ffi::c_void);
            interop.CreateForMonitor(monitor).map_err(text)
        } else {
            let hwnd = HWND(target.hwnd as *mut core::ffi::c_void);
            interop.CreateForWindow(hwnd).map_err(text)
        }
    }
}

fn d3d_device() -> Result<(ID3D11Device, ID3D11DeviceContext), String> {
    unsafe {
        match create_device(D3D_DRIVER_TYPE_HARDWARE) {
            Ok(device) => Ok(device),
            Err(_) => create_device(D3D_DRIVER_TYPE_WARP),
        }
    }
}

unsafe fn create_device(
    driver: windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE,
) -> Result<(ID3D11Device, ID3D11DeviceContext), String> {
    let mut device = None;
    let mut context = None;
    let levels = [D3D_FEATURE_LEVEL_11_0];
    unsafe {
        D3D11CreateDevice(
            None,
            driver,
            HMODULE(std::ptr::null_mut()),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&levels),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .map_err(text)?;
    }
    Ok((
        device.ok_or("Direct3D did not start.")?,
        context.ok_or("Direct3D did not start.")?,
    ))
}

fn winrt_device(device: &ID3D11Device) -> Result<IDirect3DDevice, String> {
    let dxgi: IDXGIDevice = device.cast().map_err(text)?;
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi).map_err(text)? };
    inspectable.cast().map_err(text)
}

fn pack_rows(data: *const u8, pitch: usize, width: u32, height: u32) -> Vec<u8> {
    let row = width as usize * 4;
    let mut packed = vec![0u8; row * height as usize];
    if data.is_null() || pitch < row {
        return packed;
    }
    for y in 0..height as usize {
        let start = y * row;
        unsafe {
            std::ptr::copy_nonoverlapping(data.add(y * pitch), packed.as_mut_ptr().add(start), row);
        }
    }
    packed
}

/// Drops a leftover pixel so an odd capture becomes even without resampling the whole frame.
fn crop(src: &[u8], source_width: u32, width: u32, height: u32) -> Vec<u8> {
    let row_in = source_width as usize * 4;
    let row_out = width as usize * 4;
    let mut dst = vec![0u8; row_out * height as usize];
    for y in 0..height as usize {
        let from = y * row_in;
        let to = y * row_out;
        if from + row_out <= src.len() && to + row_out <= dst.len() {
            dst[to..to + row_out].copy_from_slice(&src[from..from + row_out]);
        }
    }
    dst
}

fn scale_bilinear(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
    let mut dst = vec![0u8; (dw as usize) * (dh as usize) * 4];
    if sw < 2 || sh < 2 || dw == 0 || dh == 0 {
        return dst;
    }
    let sample = |x: u32, y: u32| -> [u8; 4] {
        let index = ((y * sw + x) * 4) as usize;
        src.get(index..index + 4).map_or([0; 4], |pixel| pixel.try_into().unwrap_or([0; 4]))
    };
    for y in 0..dh {
        let fy = (f64::from(y) + 0.5) * f64::from(sh) / f64::from(dh) - 0.5;
        let y0 = fy.floor().clamp(0.0, f64::from(sh - 1)) as u32;
        let y1 = (y0 + 1).min(sh - 1);
        let ty = (fy - f64::from(y0)).clamp(0.0, 1.0);
        for x in 0..dw {
            let fx = (f64::from(x) + 0.5) * f64::from(sw) / f64::from(dw) - 0.5;
            let x0 = fx.floor().clamp(0.0, f64::from(sw - 1)) as u32;
            let x1 = (x0 + 1).min(sw - 1);
            let tx = (fx - f64::from(x0)).clamp(0.0, 1.0);
            let p00 = sample(x0, y0);
            let p10 = sample(x1, y0);
            let p01 = sample(x0, y1);
            let p11 = sample(x1, y1);
            let to = ((y * dw + x) * 4) as usize;
            for channel in 0..4 {
                let top = f64::from(p00[channel]) * (1.0 - tx) + f64::from(p10[channel]) * tx;
                let bottom = f64::from(p01[channel]) * (1.0 - tx) + f64::from(p11[channel]) * tx;
                dst[to + channel] = (top * (1.0 - ty) + bottom * ty).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    dst
}

fn text(error: windows::core::Error) -> String {
    let message = error.to_string();
    if message.is_empty() {
        "Windows could not capture this window.".into()
    } else {
        message
    }
}
