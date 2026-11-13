use super::*;

/// OS DPI scales native dimensions; UI zoom only changes their egui coordinate representation.
pub(super) fn expanded_minimum(ctx: &egui::Context) -> egui::Vec2 {
    let dpi = (ctx.native_pixels_per_point().unwrap_or(1.) * 96.).round() as u32;
    let [width, height] =
        crate::placement::PanelSize::MINIMUM.ui_points(dpi, ctx.pixels_per_point());
    egui::vec2(width, height)
}

#[derive(Default)]
pub(super) struct PanelSizeState {
    target: Option<TargetToken>,
    sample: Option<([u32; 2], u32)>,
    pending: Option<([u32; 2], std::time::Instant)>,
    resizing: bool,
    work_area: Option<[i32; 4]>,
}
impl PanelSizeState {
    fn user_size(
        &mut self,
        size: [u32; 2],
        dpi: u32,
        user_resizing: bool,
    ) -> Option<crate::placement::PanelSize> {
        let previous = self.sample.replace((size, dpi));
        let was_resizing = std::mem::replace(&mut self.resizing, user_resizing);
        if let Some((expected, until)) = self.pending {
            if expected == size {
                self.pending = None;
            } else if std::time::Instant::now() < until {
                return None;
            } else {
                self.pending = None;
            }
            return None;
        }
        ((user_resizing || was_resizing)
            && previous.is_some_and(|(old, old_dpi)| old != size && old_dpi == dpi))
        .then(|| crate::placement::PanelSize::from_physical(size, dpi))
    }
}

impl MetinApp {
    /// Called on every presentation frame; only a real native sizing loop learns extents.
    pub(super) fn observe_expanded_panel_size(&mut self, ctx: &egui::Context) {
        #[cfg(any(test, feature = "ui-smoke"))]
        if self.window_fixture.is_some() {
            // Synthetic QA owns its exact viewport bounds and has no live native target.
            self.panel_size_state = Default::default();
            if !self.collapsed && !self.window_chooser.open {
                self.expanded_size = ctx.content_rect().size();
            }
            return;
        }
        if self.window_chooser.open || self.collapsed || self.startup_update_open {
            self.panel_size_state = Default::default();
            return;
        }
        let Some(target) = self.target else {
            self.panel_size_state = Default::default();
            return;
        };
        #[cfg(windows)]
        {
            use windows_sys::Win32::{
                Graphics::Gdi::*,
                UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
            };
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            let Some(window) = &self.native_window else {
                return;
            };
            let Ok(handle) = window.window_handle() else {
                return;
            };
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return;
            };
            let hwnd = handle.hwnd.get() as _;
            // SAFETY: only inspect our live winit-owned window and its UI thread.
            let (size, dpi, user_resizing, work) = unsafe {
                let mut rect = std::mem::zeroed();
                if GetWindowRect(hwnd, &mut rect) == 0 {
                    return;
                }
                let mut thread: GUITHREADINFO = std::mem::zeroed();
                thread.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
                let tid = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
                let resizing = GetGUIThreadInfo(tid, &mut thread) != 0
                    && thread.flags & GUI_INMOVESIZE != 0
                    && thread.hwndMoveSize == hwnd;
                let mut monitor: MONITORINFO = std::mem::zeroed();
                monitor.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
                if GetMonitorInfoW(
                    MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
                    &mut monitor,
                ) == 0
                {
                    return;
                }
                (
                    [
                        (rect.right - rect.left).max(1) as u32,
                        (rect.bottom - rect.top).max(1) as u32,
                    ],
                    GetDpiForWindow(hwnd).max(96),
                    resizing,
                    [
                        monitor.rcWork.left,
                        monitor.rcWork.top,
                        monitor.rcWork.right,
                        monitor.rcWork.bottom,
                    ],
                )
            };
            let attaching = self.panel_size_state.target != Some(target);
            if attaching {
                self.panel_size_state = Default::default();
                self.panel_size_state.target = Some(target);
                self.placement_key =
                    crate::placement::placement_key(target, &self.snapshot.targets)
                        .map(|key| (target, key));
            }
            let preferred = crate::placement::preferred_size(
                &self.settings.panel_positions,
                self.placement_key.as_ref().map(|(_, key)| key),
            );
            let preferred_pixels = preferred.physical(dpi);
            self.expanded_size = egui::vec2(preferred_pixels[0] as f32, preferred_pixels[1] as f32)
                / ctx.pixels_per_point();
            let environment_changed = self.panel_size_state.work_area != Some(work)
                || self
                    .panel_size_state
                    .sample
                    .is_some_and(|(_, previous_dpi)| previous_dpi != dpi);
            self.panel_size_state.work_area = Some(work);
            if attaching || environment_changed {
                let requested = preferred.restored_physical(dpi, work);
                ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(
                    expanded_minimum(ctx).min(
                        egui::vec2(
                            (work[2] - work[0]).max(1) as f32,
                            (work[3] - work[1]).max(1) as f32,
                        ) / ctx.pixels_per_point(),
                    ),
                ));
                if requested != size {
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                        egui::vec2(requested[0] as f32, requested[1] as f32)
                            / ctx.pixels_per_point(),
                    ));
                    self.panel_size_state.pending = Some((
                        requested,
                        std::time::Instant::now() + std::time::Duration::from_millis(500),
                    ));
                }
                self.panel_size_state.sample = Some((size, dpi));
                return;
            }
            if let Some(preferred) = self.panel_size_state.user_size(size, dpi, user_resizing) {
                self.expanded_size = ctx.content_rect().size();
                if let Some((_, key)) = &self.placement_key
                    && crate::placement::remember_size(
                        &mut self.settings.panel_positions,
                        key.clone(),
                        preferred,
                    )
                {
                    self.placement_save_due =
                        Some(std::time::Instant::now() + std::time::Duration::from_millis(500));
                }
            }
        }
        #[cfg(not(windows))]
        {
            let _ = (target, ctx);
        }
    }

    pub(super) fn reset_panel_placement(&mut self, ctx: &egui::Context) {
        let preferred = crate::placement::PanelSize::default();
        if let Some((_, key)) = &self.placement_key {
            crate::placement::remember_position(
                &mut self.settings.panel_positions,
                key.clone(),
                Default::default(),
            );
            crate::placement::remember_size(
                &mut self.settings.panel_positions,
                key.clone(),
                preferred,
            );
            self.placement_save_due = Some(std::time::Instant::now());
        }
        if let Some(overlay) = &self.overlay
            && let Err(error) = overlay.reset_panel_position()
        {
            self.error = Some(error);
        }
        #[cfg(windows)]
        if !self.pinned
            && let (Some(window), Some(target)) = (&self.native_window, self.target)
            && let Err(error) = super::native_chrome::reanchor_panel(window, target, preferred)
        {
            self.error = Some(error);
        }
        self.panel_size_state = Default::default();
        self.expanded_size = egui::vec2(preferred.width as f32, preferred.height as f32);
        if !self.collapsed {
            if self.target.is_some() {
                self.observe_expanded_panel_size(ctx);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(self.expanded_size));
            }
        }
        self.flush_panel_position(true);
    }

    pub(super) fn remember_latest_panel_position(&mut self) {
        if let Some((target, offset)) = self
            .overlay
            .as_ref()
            .and_then(|overlay| overlay.panel_position())
        {
            self.remember_panel_position(target, offset);
        }
    }
    pub(super) fn remember_panel_position(
        &mut self,
        target: TargetToken,
        offset: crate::placement::PanelOffset,
    ) {
        if self.target != Some(target) || self.window_chooser.open {
            return;
        }
        let Some((key_target, key)) = &self.placement_key else {
            return;
        };
        if *key_target != target {
            return;
        }
        if crate::placement::remember_position(
            &mut self.settings.panel_positions,
            key.clone(),
            offset,
        ) {
            self.placement_save_due =
                Some(std::time::Instant::now() + std::time::Duration::from_millis(500));
        }
    }
    pub(super) fn flush_panel_position(&mut self, immediate: bool) {
        if self
            .placement_save_due
            .is_some_and(|due| immediate || std::time::Instant::now() >= due)
        {
            match self.settings.save(&self.root) {
                Ok(()) => self.placement_save_due = None,
                Err(error) => {
                    self.placement_save_due = None;
                    self.error = Some(format!("Could not save panel position: {error}"));
                }
            }
        }
    }
}

#[cfg(test)]
mod size_tests {
    use super::*;
    #[test]
    fn synthetic_window_fixture_keeps_requested_viewport_without_learning_or_restoring_target_size()
    {
        let dir = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        ctx.set_zoom_factor(1.5);
        let mut app = MetinApp::from_context(&ctx, dir.path().to_owned()).unwrap();
        app.window_fixture = Some(super::super::window_selection::WindowFixture {
            snapshot: app.snapshot.clone(),
        });
        app.target = Some(TargetToken {
            handle: 100,
            pid: 1201,
            created: 1,
        });
        app.window_chooser.open = false;
        app.expanded_size = egui::vec2(700., 800.);
        let requested = egui::vec2(400., 480.);
        // egui applies a zoom change at the frame boundary; settle the fixture before
        // comparing logical content coordinates with the observer's temporary size.
        for _ in 0..4 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, requested)),
                    ..Default::default()
                },
                |ui| app.observe_expanded_panel_size(ui.ctx()),
            );
            output.textures_delta.clear();
            assert!(
                output
                    .viewport_output
                    .values()
                    .flat_map(|viewport| &viewport.commands)
                    .all(|command| !matches!(
                        command,
                        egui::ViewportCommand::InnerSize(_)
                            | egui::ViewportCommand::MinInnerSize(_)
                    ))
            );
        }
        let logical_bounds = ctx.content_rect().size();
        assert_eq!(app.expanded_size, logical_bounds);
        assert!(logical_bounds.x <= requested.x && logical_bounds.y <= requested.y);
        assert!(app.settings.panel_positions.is_empty());
        app.runtime.shutdown().unwrap();
    }

    #[test]
    fn expanded_minimum_command_stays_400_by_480_physical_at_96dpi_with_150percent_zoom() {
        let ctx = egui::Context::default();
        ctx.set_zoom_factor(1.5);
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let minimum = expanded_minimum(ui.ctx()) * ui.ctx().pixels_per_point();
            assert!((minimum.x - 400.).abs() < 0.01 && (minimum.y - 480.).abs() < 0.01);
        });
        output.textures_delta.clear();
    }

    #[test]
    fn only_user_size_changes_at_unchanged_dpi_are_learned() {
        let mut state = PanelSizeState::default();
        assert!(state.user_size([460, 620], 96, false).is_none());
        assert!(
            state.user_size([400, 480], 96, false).is_none(),
            "work-area clamp"
        );
        assert!(
            state.user_size([400, 480], 96, true).is_none(),
            "moving a clamped surface"
        );
        assert_eq!(
            state.user_size([500, 640], 96, true),
            Some(crate::placement::PanelSize {
                width: 500,
                height: 640
            })
        );
        assert!(
            state.user_size([750, 960], 144, true).is_none(),
            "DPI transition"
        );
        state.pending = Some((
            [600, 720],
            std::time::Instant::now() + std::time::Duration::from_secs(1),
        ));
        assert!(
            state.user_size([640, 800], 144, true).is_none(),
            "restore not yet acknowledged"
        );
        assert!(
            state.user_size([600, 720], 144, true).is_none(),
            "programmatic restore"
        );
    }
}
