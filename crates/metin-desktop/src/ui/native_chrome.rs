//! The collapsed surface follows the painted atlas instead of a GPU-dependent transparent box.
use super::*;
use windows_sys::Win32::Graphics::Gdi::*;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

impl MetinApp {
    pub(super) fn sync_title_clip(
        &mut self,
        ctx: &egui::Context,
        desired: Option<skin::TitleClip>,
    ) {
        if self.title_clip == desired {
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
            let runs = desired.map(|clip| skin::title_clip_runs(ctx, clip));
            // SAFETY: this is our live winit-owned HWND. Successful SetWindowRgn takes
            // ownership; all temporary regions and unsuccessful output regions are deleted.
            unsafe {
                if let Some(runs) = runs {
                    if runs.is_empty() {
                        return Err("The title artwork has an empty silhouette.".into());
                    }
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
                            return Err("Cannot compose the Companion title shape.".into());
                        }
                    }
                    if SetWindowRgn(handle.hwnd.get() as _, region, 1) == 0 {
                        DeleteObject(region);
                        return Err(std::io::Error::last_os_error().to_string());
                    }
                } else if SetWindowRgn(handle.hwnd.get() as _, std::ptr::null_mut(), 1) == 0 {
                    return Err(std::io::Error::last_os_error().to_string());
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => self.title_clip = desired,
            Err(error) => {
                self.error = Some(format!("Cannot update the Companion window shape: {error}"))
            }
        }
    }
}
