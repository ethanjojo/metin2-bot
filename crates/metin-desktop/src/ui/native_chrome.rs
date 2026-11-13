//! Native atlas silhouettes, independent of GPU alpha composition.
use super::*;
use windows_sys::Win32::{Foundation::HWND, Graphics::Gdi::*};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Shared by the main window and the isolated update helper. Successful SetWindowRgn
/// transfers ownership to Windows; every temporary or failed region is deleted here.
pub(crate) fn apply_clip(
    window: HWND,
    ctx: &egui::Context,
    clip: skin::TitleClip,
) -> Result<(), String> {
    let runs = skin::title_clip_runs(ctx, clip);
    if runs.is_empty() {
        return Err("The artwork has an empty silhouette.".into());
    }
    // SAFETY: caller supplies its own live HWND. Successful SetWindowRgn owns
    // the output region; temporary and failed regions are always deleted.
    unsafe {
        let region = CreateRectRgn(0, 0, 0, 0);
        if region.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        for [l, t, r, b] in runs {
            let row = CreateRectRgn(l, t, r, b);
            if row.is_null() {
                DeleteObject(region);
                return Err(std::io::Error::last_os_error().to_string());
            }
            let combined = CombineRgn(region, region, row, RGN_OR);
            DeleteObject(row);
            if combined == ERROR {
                DeleteObject(region);
                return Err("Cannot compose the Companion window shape.".into());
            }
        }
        if SetWindowRgn(window, region, 1) == 0 {
            DeleteObject(region);
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    Ok(())
}

/// Reanchor an unpinned owned panel; target inspection is read-only and identity checked.
pub(super) fn reanchor_panel(
    window: &winit::window::Window,
    target: TargetToken,
    preferred: crate::placement::PanelSize,
) -> Result<(), String> {
    let geometry = crate::game_overlay::inspect_target(target)
        .ok_or("The selected window is unavailable; the saved position was reset.")?;
    let RawWindowHandle::Win32(handle) = window
        .window_handle()
        .map_err(|error| error.to_string())?
        .as_raw()
    else {
        return Err("The Companion window has no Windows handle.".into());
    };
    let hwnd = handle.hwnd.get() as _;
    let target_rect = [
        geometry.left,
        geometry.top,
        geometry.left.saturating_add(geometry.width as i32),
        geometry.top.saturating_add(geometry.height as i32),
    ];
    place_reset_panel(hwnd, target_rect, preferred)
}

fn place_reset_panel(
    hwnd: HWND,
    target_rect: [i32; 4],
    preferred: crate::placement::PanelSize,
) -> Result<(), String> {
    use windows_sys::Win32::UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*};
    // SAFETY: monitor geometry reads are inert; SetWindowPos only changes our HWND.
    unsafe {
        let bounds = windows_sys::Win32::Foundation::RECT {
            left: target_rect[0],
            top: target_rect[1],
            right: target_rect[2],
            bottom: target_rect[3],
        };
        let mut monitor: MONITORINFO = std::mem::zeroed();
        monitor.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(
            MonitorFromRect(&bounds, MONITOR_DEFAULTTONEAREST),
            &mut monitor,
        ) == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let work = [
            monitor.rcWork.left,
            monitor.rcWork.top,
            monitor.rcWork.right,
            monitor.rcWork.bottom,
        ];
        let dpi = GetDpiForWindow(hwnd).max(96);
        let size = preferred.restored_physical(dpi, work);
        let [x, y] =
            crate::placement::place_panel(target_rect, size, Default::default(), dpi, work);
        if SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            x,
            y,
            size[0] as i32,
            size[1] as i32,
            SWP_NOZORDER | SWP_NOACTIVATE,
        ) == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    Ok(())
}

impl MetinApp {
    pub(super) fn sync_title_clip(
        &mut self,
        ctx: &egui::Context,
        desired: Option<skin::TitleClip>,
    ) {
        let desired = desired.unwrap_or_else(|| skin::board_clip(ctx));
        if self.title_clip == Some(desired) {
            return;
        }
        let Some(window) = &self.native_window else {
            return;
        };
        let result = (|| {
            let RawWindowHandle::Win32(handle) =
                window.window_handle().map_err(|e| e.to_string())?.as_raw()
            else {
                return Err("The Companion window has no Windows handle.".to_owned());
            };
            apply_clip(handle.hwnd.get() as _, ctx, desired)
        })();
        match result {
            Ok(()) => self.title_clip = Some(desired),
            Err(error) => {
                self.error = Some(format!("Cannot update the Companion window shape: {error}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    #[test]
    fn owned_native_window_region_tracks_atlas_size_dpi_and_usable_interior() {
        // Hidden STATIC window; no target/game handles, foreground or synthetic input.
        unsafe {
            let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
            let window = CreateWindowExW(
                0,
                class.as_ptr(),
                std::ptr::null(),
                WS_POPUP,
                0,
                0,
                400,
                480,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            );
            assert!(!window.is_null());
            let ctx = egui::Context::default();
            for scale in [1., 1.5] {
                assert_ne!(
                    SetWindowPos(
                        window,
                        std::ptr::null_mut(),
                        0,
                        0,
                        (400. * scale) as i32,
                        (480. * scale) as i32,
                        SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE
                    ),
                    0
                );
                let clip = skin::TitleClip::board(
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400., 480.)),
                    scale,
                );
                apply_clip(window, &ctx, clip).unwrap();
                let region = CreateRectRgn(0, 0, 0, 0);
                assert_ne!(GetWindowRgn(window, region), ERROR);
                assert_eq!(PtInRegion(region, 0, 0), 0);
                assert_ne!(
                    PtInRegion(region, (200. * scale) as i32, (240. * scale) as i32),
                    0
                );
                assert_ne!(
                    PtInRegion(region, (385. * scale) as i32, (465. * scale) as i32),
                    0,
                    "Inset resize grip survives"
                );
                assert_eq!(PtInRegion(region, (400. * scale) as i32, 10), 0);
                DeleteObject(region);
            }
            let foreground = GetForegroundWindow();
            let mut monitor: MONITORINFO = std::mem::zeroed();
            monitor.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            assert_ne!(
                GetMonitorInfoW(
                    MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST),
                    &mut monitor
                ),
                0
            );
            let work = [
                monitor.rcWork.left,
                monitor.rcWork.top,
                monitor.rcWork.right,
                monitor.rcWork.bottom,
            ];
            let preferred = crate::placement::PanelSize {
                width: 460,
                height: 620,
            };
            let dpi = windows_sys::Win32::UI::HiDpi::GetDpiForWindow(window).max(96);
            let size = preferred.restored_physical(dpi, work);
            let position = crate::placement::place_panel(work, size, Default::default(), dpi, work);
            place_reset_panel(window, work, preferred).unwrap();
            let mut rect = std::mem::zeroed();
            assert_ne!(GetWindowRect(window, &mut rect), 0);
            assert_eq!(
                [rect.left, rect.top],
                position,
                "Unpinned reset immediately reanchors the owned HWND"
            );
            assert_eq!(
                [
                    (rect.right - rect.left) as u32,
                    (rect.bottom - rect.top) as u32
                ],
                size
            );
            assert_eq!(
                GetForegroundWindow(),
                foreground,
                "Reset never activates the panel"
            );
            DestroyWindow(window);
        }
    }
}
