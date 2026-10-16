//! HWND-only capture. COM objects and the D3D immediate context stay on the
//! backend worker; no desktop pixels or alternative capture path are used.
use crate::{PlatformError, Result, StopToken, TargetState, TargetToken};
use std::{
    mem::size_of,
    time::{Duration, Instant},
};
use windows::{
    Graphics::{
        Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession},
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
    },
    Win32::{
        Foundation::{HMODULE, HWND, RECT},
        Graphics::{
            Direct3D::D3D_DRIVER_TYPE_HARDWARE,
            Direct3D11::*,
            Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
            Dxgi::{Common::DXGI_FORMAT_B8G8R8A8_UNORM, IDXGIDevice},
        },
        System::{
            Com::{CO_MTA_USAGE_COOKIE, CoDecrementMTAUsage, CoIncrementMTAUsage},
            Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
            WinRT::{
                Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess},
                Graphics::Capture::IGraphicsCaptureItemInterop,
            },
        },
    },
    core::{Interface, factory},
};

pub(super) const MAX_FRAME_AGE: Duration = Duration::from_millis(250);

fn error(e: windows::core::Error) -> PlatformError {
    PlatformError(format!(
        "Window capture failed: {e}. The selected window must support Windows Graphics Capture; desktop capture is never substituted."
    ))
}
// MTA usage cookies may be balanced on another thread (Microsoft contract).
struct Apartment(usize);
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            let _ = CoDecrementMTAUsage(CO_MTA_USAGE_COOKIE(self.0 as _));
        }
    }
}
pub(super) struct WindowCapture {
    target: TargetToken,
    session: GraphicsCaptureSession,
    pool: Direct3D11CaptureFramePool,
    _item: GraphicsCaptureItem,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    size: windows::Graphics::SizeInt32,
    // Declared last so interfaces are released before COM is uninitialized.
    _apartment: Apartment,
}
impl Drop for WindowCapture {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}
impl WindowCapture {
    pub(super) fn new(target: TargetToken) -> Result<Self> {
        let apartment = Apartment(unsafe { CoIncrementMTAUsage().map_err(error)?.0 as usize });
        if !GraphicsCaptureSession::IsSupported().map_err(error)? {
            return Err(PlatformError(
                "Windows Graphics Capture is unavailable on this system.".into(),
            ));
        }
        let interop: IGraphicsCaptureItemInterop =
            factory::<GraphicsCaptureItem, _>().map_err(error)?;
        let item: GraphicsCaptureItem = unsafe {
            interop
                .CreateForWindow(HWND(target.handle as _))
                .map_err(error)?
        };
        let (mut device, mut context) = (None, None);
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
            .map_err(error)?;
        }
        let device =
            device.ok_or_else(|| PlatformError("Capture device was not created.".into()))?;
        let context =
            context.ok_or_else(|| PlatformError("Capture context was not created.".into()))?;
        let dxgi: IDXGIDevice = device.cast().map_err(error)?;
        let winrt_device: IDirect3DDevice = unsafe {
            CreateDirect3D11DeviceFromDXGIDevice(&dxgi)
                .map_err(error)?
                .cast()
                .map_err(error)?
        };
        let size = item.Size().map_err(error)?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &winrt_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            size,
        )
        .map_err(error)?;
        let session = pool.CreateCaptureSession(&item).map_err(error)?;
        session.SetIsCursorCaptureEnabled(false).map_err(error)?;
        session.StartCapture().map_err(error)?;
        Ok(Self {
            target,
            session,
            pool,
            _item: item,
            device,
            context,
            size,
            _apartment: apartment,
        })
    }
    pub(super) fn frame(
        &mut self,
        geometry: TargetState,
        requested: Instant,
        deadline: Instant,
        stop: &StopToken,
        generation: u64,
    ) -> Result<(Vec<u8>, Instant)> {
        if geometry.target != self.target {
            return Err(PlatformError("Capture target changed.".into()));
        }
        loop {
            if stop.cancelled(generation) || Instant::now() >= deadline {
                return Err(PlatformError("Window capture cancelled or no fresh window frame arrived before the deadline.".into()));
            }
            let mut frame = match next_frame(&self.pool).map_err(error)? {
                Some(frame) => frame,
                None => {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                }
            };
            while let Some(newer) = next_frame(&self.pool).map_err(error)? {
                frame.Close().map_err(error)?;
                frame = newer;
                if stop.cancelled(generation) || Instant::now() >= deadline {
                    return Err(PlatformError(
                        "Capture cancelled while draining frames.".into(),
                    ));
                }
            }
            let ticks = frame.SystemRelativeTime().map_err(error)?.Duration;
            let mut clock = clock_now()?;
            if ticks > clock.1 {
                // DWM may timestamp the scheduled presentation a few ms ahead.
                // Wait for that instant; never stamp future pixels as "now".
                if ticks - clock.1 > 1_000_000 {
                    return Err(PlatformError(
                        "Window-frame presentation timestamp is too far in the future.".into(),
                    ));
                }
                while ticks > clock.1 {
                    if stop.cancelled(generation) || Instant::now() >= deadline {
                        return Err(PlatformError(
                            "Capture cancelled before frame presentation.".into(),
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(1));
                    clock = clock_now()?;
                }
            }
            let captured_at = source_instant(ticks, clock)?;
            if captured_at < requested || captured_at.elapsed() > MAX_FRAME_AGE {
                frame.Close().map_err(error)?;
                continue;
            }
            let content = frame.ContentSize().map_err(error)?;
            if content != self.size {
                frame.Close().map_err(error)?;
                self.resize(content)?;
                continue;
            }
            let result = self.read_client(
                &frame.Surface().map_err(error)?,
                geometry,
                deadline,
                stop,
                generation,
            );
            frame.Close().map_err(error)?;
            return result.map(|pixels| (pixels, captured_at));
        }
    }
    fn resize(&mut self, size: windows::Graphics::SizeInt32) -> Result<()> {
        let dxgi: IDXGIDevice = self.device.cast().map_err(error)?;
        let device: IDirect3DDevice = unsafe {
            CreateDirect3D11DeviceFromDXGIDevice(&dxgi)
                .map_err(error)?
                .cast()
                .map_err(error)?
        };
        self.pool
            .Recreate(&device, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, size)
            .map_err(error)?;
        self.size = size;
        Ok(())
    }
    fn read_client(
        &self,
        surface: &windows::Graphics::DirectX::Direct3D11::IDirect3DSurface,
        geometry: TargetState,
        deadline: Instant,
        stop: &StopToken,
        generation: u64,
    ) -> Result<Vec<u8>> {
        let access: IDirect3DDxgiInterfaceAccess = surface.cast().map_err(error)?;
        let texture: ID3D11Texture2D = unsafe { access.GetInterface().map_err(error)? };
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe {
            texture.GetDesc(&mut desc);
        }
        let mut bounds = RECT::default();
        unsafe {
            DwmGetWindowAttribute(
                HWND(self.target.handle as _),
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut bounds as *mut _ as _,
                size_of::<RECT>() as u32,
            )
            .map_err(error)?;
        }
        let x = geometry.left - bounds.left;
        let y = geometry.top - bounds.top;
        if x < 0
            || y < 0
            || x as u64 + u64::from(geometry.width) > u64::from(desc.Width)
            || y as u64 + u64::from(geometry.height) > u64::from(desc.Height)
        {
            return Err(PlatformError(
                "Window frame and client geometry do not match. Retry the capture.".into(),
            ));
        }
        let staging_desc = D3D11_TEXTURE2D_DESC {
            Width: geometry.width,
            Height: geometry.height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            ..Default::default()
        };
        let mut staging = None;
        unsafe {
            self.device
                .CreateTexture2D(&staging_desc, None, Some(&mut staging))
                .map_err(error)?;
        }
        let staging = staging
            .ok_or_else(|| PlatformError("Capture readback texture was not created.".into()))?;
        let crop = D3D11_BOX {
            left: x as u32,
            top: y as u32,
            right: x as u32 + geometry.width,
            bottom: y as u32 + geometry.height,
            front: 0,
            back: 1,
        };
        unsafe {
            self.context
                .CopySubresourceRegion(&staging, 0, 0, 0, 0, &texture, 0, Some(&crop));
        }
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        loop {
            if stop.cancelled(generation) || Instant::now() >= deadline {
                return Err(PlatformError(
                    "Window readback cancelled or timed out.".into(),
                ));
            }
            match unsafe {
                self.context.Map(
                    &staging,
                    0,
                    D3D11_MAP_READ,
                    D3D11_MAP_FLAG_DO_NOT_WAIT.0 as u32,
                    Some(&mut mapped),
                )
            } {
                Ok(()) => break,
                Err(e) if e.code() == windows::core::HRESULT(0x887A000Au32 as i32) => {
                    std::thread::sleep(Duration::from_millis(1))
                }
                Err(e) => return Err(error(e)),
            }
        }
        let mut pixels = Vec::with_capacity(geometry.width as usize * geometry.height as usize * 3);
        // SAFETY: mapped staging texture is CPU-readable. Respect RowPitch;
        // bounds were checked before the GPU copy. Unmap before returning.
        unsafe {
            for row in 0..geometry.height as usize {
                let bytes = std::slice::from_raw_parts(
                    (mapped.pData as *const u8).add(row * mapped.RowPitch as usize),
                    geometry.width as usize * 4,
                );
                pixels.extend(
                    bytes
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .flat_map(|p| [p[0], p[1], p[2]]),
                );
            }
            self.context.Unmap(&staging, 0);
        }
        Ok(pixels)
    }
}

fn next_frame(
    pool: &Direct3D11CaptureFramePool,
) -> windows::core::Result<Option<windows::Graphics::Capture::Direct3D11CaptureFrame>> {
    // The WinRT method legitimately returns S_OK + null when no frame is
    // available. The generated non-optional wrapper turns null into an error.
    // Preserve this documented nullable result and transfer its owned reference.
    unsafe {
        let mut raw = std::ptr::null_mut();
        (Interface::vtable(pool).TryGetNextFrame)(pool.as_raw(), &mut raw).ok()?;
        Ok(if raw.is_null() {
            None
        } else {
            Some(windows::Graphics::Capture::Direct3D11CaptureFrame::from_raw(raw))
        })
    }
}

fn clock_now() -> Result<(Instant, i64)> {
    let now = Instant::now();
    let (mut ticks, mut frequency) = (0, 0);
    unsafe {
        QueryPerformanceCounter(&mut ticks).map_err(error)?;
        QueryPerformanceFrequency(&mut frequency).map_err(error)?;
    }
    if frequency <= 0 {
        return Err(PlatformError("Invalid capture clock frequency.".into()));
    }
    Ok((
        now,
        (i128::from(ticks) * 10_000_000 / i128::from(frequency)) as i64,
    ))
}
fn source_instant(ticks: i64, (now, current): (Instant, i64)) -> Result<Instant> {
    let age = current
        .checked_sub(ticks)
        .filter(|age| *age >= 0)
        .ok_or_else(|| {
            PlatformError(format!(
                "Invalid future window-frame timestamp: source={ticks}, current={current}."
            ))
        })?;
    now.checked_sub(Duration::from_nanos(
        (age as u64)
            .checked_mul(100)
            .ok_or_else(|| PlatformError("Window-frame age overflow.".into()))?,
    ))
    .ok_or_else(|| PlatformError("Invalid window-frame timestamp.".into()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_time_preserves_age_instead_of_restamping_queued_pixels() {
        let now = Instant::now();
        let old = source_instant(5_000_000, (now, 10_000_000)).unwrap();
        assert_eq!(now.duration_since(old), Duration::from_millis(500));
        assert!(source_instant(10_000_001, (now, 10_000_000)).is_err());
    }
}
