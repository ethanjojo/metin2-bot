//! HWND-only capture. COM objects and the D3D immediate context stay on the
//! backend worker; no desktop pixels or alternative capture path are used.
use crate::{PlatformError, Result, StopToken, TargetState, TargetToken};
use std::{
    mem::size_of,
    sync::OnceLock,
    time::{Duration, Instant},
};
use windows::{
    Foundation::Metadata::ApiInformation,
    Graphics::{
        Capture::{
            Direct3D11CaptureFramePool, GraphicsCaptureAccess, GraphicsCaptureAccessKind,
            GraphicsCaptureItem, GraphicsCaptureSession,
        },
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
    },
    Security::Authorization::AppCapabilityAccess::AppCapabilityAccessStatus,
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

#[derive(Debug)]
enum BorderlessAccess {
    Unsupported,
    Requested(AppCapabilityAccessStatus),
    Unavailable(String),
}

// Runtime capture and chooser previews share the same documented permission
// request. A preview grid must never repeat a system consent prompt per tile.
static BORDERLESS_ACCESS: OnceLock<BorderlessAccess> = OnceLock::new();

fn request_borderless_access() -> BorderlessAccess {
    let present = ApiInformation::IsPropertyPresent(
        &"Windows.Graphics.Capture.GraphicsCaptureSession".into(),
        &"IsBorderRequired".into(),
    );
    match present {
        Ok(false) => BorderlessAccess::Unsupported,
        Err(error) => BorderlessAccess::Unavailable(error.to_string()),
        Ok(true) => {
            match GraphicsCaptureAccess::RequestAccessAsync(GraphicsCaptureAccessKind::Borderless)
                .and_then(|request| request.join())
            {
                Ok(access) => BorderlessAccess::Requested(access),
                Err(error) => BorderlessAccess::Unavailable(error.to_string()),
            }
        }
    }
}

fn configure_capture_border(session: &GraphicsCaptureSession) {
    let access = BORDERLESS_ACCESS.get_or_init(|| {
        let access = request_borderless_access();
        match &access {
            BorderlessAccess::Unsupported => tracing::info!("This Windows version requires a capture border."),
            BorderlessAccess::Requested(status) if *status == AppCapabilityAccessStatus::Allowed => tracing::info!("Windows allowed borderless window capture."),
            BorderlessAccess::Requested(status) => tracing::warn!(status = status.0, "Windows may retain the capture border because borderless access was not granted."),
            BorderlessAccess::Unavailable(error) => tracing::warn!(%error, "Windows borderless capture access could not be requested; system capture policy remains authoritative."),
        }
        access
    });
    if matches!(access, BorderlessAccess::Unsupported) {
        return;
    }
    // Windows ignores this preference when the user or system requires the
    // indicator. Never modify registry/privacy policy or substitute capture.
    if let Err(error) = session.SetIsBorderRequired(false) {
        tracing::warn!(%error, "Windows retained the capture border. Window capture remains available.");
    }
}

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
#[derive(Default)]
struct ReadbackTexture(Option<([u32; 2], ID3D11Texture2D)>);
impl ReadbackTexture {
    fn get(&mut self, device: &ID3D11Device, width: u32, height: u32) -> Result<&ID3D11Texture2D> {
        let size = [width, height];
        if self
            .0
            .as_ref()
            .is_none_or(|(cached_size, _)| *cached_size != size)
        {
            // Drop the previous allocation before replacing it. The owning capture
            // also clears this cache when the frame pool changes shape.
            self.0 = None;
            let desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
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
            let mut texture = None;
            unsafe {
                device
                    .CreateTexture2D(&desc, None, Some(&mut texture))
                    .map_err(error)?;
            }
            let texture = texture
                .ok_or_else(|| PlatformError("Capture readback texture was not created.".into()))?;
            self.0 = Some((size, texture));
        }
        Ok(&self.0.as_ref().expect("readback texture initialized").1)
    }
    fn clear(&mut self) {
        self.0 = None;
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
    // Device and context are fixed for this capture's lifetime, so dropping or
    // recreating the capture also drops its device-specific readback texture.
    readback: ReadbackTexture,
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
        configure_capture_border(&session);
        session.StartCapture().map_err(error)?;
        Ok(Self {
            target,
            session,
            pool,
            _item: item,
            device,
            context,
            size,
            readback: ReadbackTexture::default(),
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
        self.readback.clear();
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
        &mut self,
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
        let staging = self
            .readback
            .get(&self.device, geometry.width, geometry.height)?;
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
                .CopySubresourceRegion(staging, 0, 0, 0, 0, &texture, 0, Some(&crop));
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
                    staging,
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
            self.context.Unmap(staging, 0);
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
    #[ignore = "briefly shows a nonactivating owned window for actual Windows borderless capture QA"]
    fn owned_window_capture_requests_borderless_and_keeps_fresh_capture_working() {
        use windows_sys::Win32::{
            Graphics::Gdi::{
                FillRect, GetDC, GetStockObject, ReleaseDC, UpdateWindow, WHITE_BRUSH,
            },
            System::Threading::GetCurrentProcessId,
            UI::WindowsAndMessaging::{
                CreateWindowExW, DestroyWindow, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
                WS_VISIBLE,
            },
        };
        struct OwnedWindow(windows_sys::Win32::Foundation::HWND);
        impl Drop for OwnedWindow {
            fn drop(&mut self) {
                unsafe {
                    DestroyWindow(self.0);
                }
            }
        }
        unsafe {
            let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
            let title: Vec<u16> = "Companion capture border fixture\0"
                .encode_utf16()
                .collect();
            let window = OwnedWindow(CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                class.as_ptr(),
                title.as_ptr(),
                WS_POPUP | WS_VISIBLE,
                32,
                32,
                240,
                180,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            ));
            assert!(!window.0.is_null(), "create owned capture fixture");
            UpdateWindow(window.0);
            let dc = GetDC(window.0);
            assert!(!dc.is_null());
            let bounds = windows_sys::Win32::Foundation::RECT {
                left: 0,
                top: 0,
                right: 240,
                bottom: 180,
            };
            assert_ne!(FillRect(dc, &bounds, GetStockObject(WHITE_BRUSH) as _), 0);
            ReleaseDC(window.0, dc);
            let pid = GetCurrentProcessId();
            let target = TargetToken {
                handle: window.0 as usize,
                pid,
                created: super::super::process_identity(pid).unwrap(),
            };
            let geometry = super::super::inspect_target(target).unwrap();
            let mut capture = WindowCapture::new(target).unwrap();
            let access = BORDERLESS_ACCESS
                .get()
                .expect("capability checked once for this process");
            eprintln!("Borderless capability result: {access:?}");
            if !matches!(access, BorderlessAccess::Unsupported) {
                assert!(
                    !capture.session.IsBorderRequired().unwrap(),
                    "session requests no border; Windows policy still controls actual visibility"
                );
            }
            let stop = StopToken::default();
            let requested = Instant::now();
            let (pixels, time) = capture
                .frame(
                    geometry,
                    requested,
                    requested + Duration::from_secs(2),
                    &stop,
                    stop.generation(),
                )
                .unwrap();
            assert_eq!((geometry.width, geometry.height), (240, 180));
            assert_eq!(pixels.len(), 240 * 180 * 3);
            assert!(pixels.iter().any(|pixel| *pixel > 0));
            assert!(time >= requested);
            // Session and frame pool close before destroying the source window.
            drop(capture);
            // Every new session receives the preference, even after the one-time
            // permission request has already been cached by a different capture.
            let recreated = WindowCapture::new(target).unwrap();
            if !matches!(access, BorderlessAccess::Unsupported) {
                assert!(!recreated.session.IsBorderRequired().unwrap());
            }
            drop(recreated);
        }
    }

    #[test]
    fn source_time_preserves_age_instead_of_restamping_queued_pixels() {
        let now = Instant::now();
        let old = source_instant(5_000_000, (now, 10_000_000)).unwrap();
        assert_eq!(now.duration_since(old), Duration::from_millis(500));
        assert!(source_instant(10_000_001, (now, 10_000_000)).is_err());
    }
    #[test]
    fn readback_reuses_texture_and_recreates_on_size_or_lifecycle_change() {
        // WARP exercises real D3D resources without a window, capture session,
        // game, hardware-adapter requirement, or elevated privileges.
        let (mut device, mut context) = (None, None);
        unsafe {
            D3D11CreateDevice(
                None,
                windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_WARP,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
            .unwrap();
        }
        let device = device.unwrap();
        let context = context.unwrap();
        let mut cache = ReadbackTexture::default();
        let first = cache.get(&device, 4, 3).unwrap().clone();
        assert_eq!(first.as_raw(), cache.get(&device, 4, 3).unwrap().as_raw());
        let changed = cache.get(&device, 3, 4).unwrap().clone();
        assert_ne!(first.as_raw(), changed.as_raw());
        cache.clear();
        let recreated = cache.get(&device, 3, 4).unwrap().clone();
        assert_ne!(changed.as_raw(), recreated.as_raw());

        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe {
            recreated.GetDesc(&mut desc);
        }
        assert_eq!((desc.Width, desc.Height), (3, 4));
        assert_eq!(desc.Usage, D3D11_USAGE_STAGING);
        assert_eq!(desc.CPUAccessFlags, D3D11_CPU_ACCESS_READ.0 as u32);
        desc.Usage = D3D11_USAGE_DEFAULT;
        desc.CPUAccessFlags = 0;
        let mut source = None;
        let pixels: Vec<u8> = (0..48).collect();
        let initial = D3D11_SUBRESOURCE_DATA {
            pSysMem: pixels.as_ptr().cast(),
            SysMemPitch: 12,
            SysMemSlicePitch: 48,
        };
        unsafe {
            device
                .CreateTexture2D(&desc, Some(&initial), Some(&mut source))
                .unwrap();
            for _ in 0..2 {
                let staging = cache.get(&device, 3, 4).unwrap();
                context.CopyResource(staging, source.as_ref().unwrap());
                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                context
                    .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                    .unwrap();
                let rows: Vec<u8> = (0..4)
                    .flat_map(|row| {
                        std::slice::from_raw_parts(
                            (mapped.pData as *const u8).add(row * mapped.RowPitch as usize),
                            12,
                        )
                        .iter()
                        .copied()
                    })
                    .collect();
                context.Unmap(staging, 0);
                assert_eq!(rows, pixels);
            }
        }
    }
}
