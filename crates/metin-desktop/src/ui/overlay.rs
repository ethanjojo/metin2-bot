//! UI orchestration for the single native capture/annotation adapter.
use super::*;
use crate::game_overlay::{
    GameOverlay, OverlayEvent, OverlayScene, SelectionKind, SelectionRequest,
};
use std::time::{Duration, Instant};

/// Only unsaved recognition edits need a presentation result separate from the
/// applied controller. It never feeds actions or weakens runtime verification.
pub(super) struct RecognitionPreview {
    profile: Profile,
    template: Option<metin_platform::Frame>,
    frame: metin_platform::Frame,
    candidates: Vec<metin_core::Candidate>,
    panel: Option<metin_vision::TemplateMatch>,
}

fn same_image(a: &Option<metin_platform::Frame>, b: &Option<metin_platform::Frame>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(&a.bgr, &b.bgr),
        (None, None) => true,
        _ => false,
    }
}

#[derive(Clone, Copy)]
pub(super) enum CapturePurpose {
    Edit(EditMode),
    SizeReview(EditMode),
    PresetColor,
}

pub(super) struct PendingCapture {
    id: u64,
    key: SessionKey,
    revision: u64,
    generation: u64,
    requested: Instant,
    purpose: CapturePurpose,
    launched: bool,
    capture: Option<crate::editor::CaptureEdit>,
}

impl PendingCapture {
    fn timed_out(&self) -> bool {
        !self.launched && self.requested.elapsed() > Duration::from_secs(4)
    }
    fn current(&self, app: &MetinApp) -> bool {
        app.key() == Some(self.key)
            && (!matches!(self.purpose, CapturePurpose::PresetColor)
                || app.metin_presets.draft.as_ref().is_some_and(|draft| {
                    draft.profile == app.editor.profile.id
                        && draft.change_id == app.editor.change_id()
                        && draft.target == app.target
                }))
            && app.revision == self.revision
            && app.runtime.stop_generation() == self.generation
            && (!self.launched
                || self
                    .capture
                    .as_ref()
                    .is_some_and(|capture| capture.current(&app.editor)))
    }
    fn ready(&self, snapshot: &RuntimeSnapshot) -> bool {
        snapshot.key == Some(self.key)
            && !snapshot.running
            && !snapshot.arming
            && snapshot.released_generation == Some(self.generation)
            && snapshot.captured_at.is_some_and(|time| {
                time >= self.requested && time.elapsed() < Duration::from_millis(500)
            })
            && snapshot.geometry.is_some_and(|geometry| {
                geometry.target == self.key.target
                    && geometry.visible
                    && snapshot.frame.as_ref().is_some_and(|frame| {
                        frame.width == geometry.width && frame.height == geometry.height
                    })
            })
    }
}

impl MetinApp {
    fn report_capture_error(&mut self, purpose: CapturePurpose, error: String) {
        if matches!(purpose, CapturePurpose::PresetColor)
            && let Some(draft) = &mut self.metin_presets.draft
        {
            draft.error = Some(error);
        } else {
            self.error = Some(error);
        }
    }
    pub(super) fn cancel_overlay_capture(&mut self) {
        if self.pending_overlay.take().is_some() {
            // Retire an accepted one-shot as well as queued capture commands.
            self.runtime.stop();
        }
        self.overlay_frame_id = 0;
        self.overlay_preview = None;
        if let Some(overlay) = &self.overlay {
            overlay.hide();
        }
    }

    pub(super) fn request_overlay_capture(&mut self, mode: EditMode) {
        self.request_capture(CapturePurpose::Edit(mode));
    }
    pub(super) fn request_capture(&mut self, purpose: CapturePurpose) {
        // Headless egui tests and synthetic fixtures have no native host window.
        #[cfg(windows)]
        if self.native_window.is_none() {
            return;
        }
        #[cfg(any(test, feature = "ui-smoke"))]
        if self.window_fixture.is_some() {
            return;
        }
        let Some(key) = self.key() else { return };
        self.cancel_overlay_capture();
        self.runtime.stop();
        let requested = Instant::now();
        match self.runtime.send(Command::Snapshot {
            key,
            purpose: crate::runtime::SnapshotPurpose::UpdateLiveEvidence,
        }) {
            Ok(()) => {
                self.overlay_request_id = self.overlay_request_id.wrapping_add(1);
                self.pending_overlay = Some(PendingCapture {
                    id: self.overlay_request_id,
                    key,
                    revision: self.revision,
                    generation: self.runtime.stop_generation(),
                    requested,
                    purpose,
                    launched: false,
                    capture: None,
                });
            }
            Err(error) => self.report_capture_error(purpose, error),
        }
    }

    fn ensure_overlay(&mut self) -> Result<(), String> {
        if self.overlay.is_none() {
            self.overlay = Some(GameOverlay::new()?);
        }
        Ok(())
    }

    pub(super) fn sync_game_overlay(&mut self, ctx: &egui::Context) {
        #[cfg(windows)]
        if self.native_window.is_none() {
            return;
        }
        #[cfg(any(test, feature = "ui-smoke"))]
        if self.window_fixture.is_some() {
            self.cancel_overlay_capture();
            return;
        }
        if self.settings.pin_to_game != self.pinned {
            let previous = self.settings.pin_to_game;
            self.settings.pin_to_game = self.pinned;
            if let Err(error) = self.settings.save(&self.root) {
                self.settings.pin_to_game = previous;
                self.pinned = previous;
                self.error = Some(error.to_string());
            }
        }
        if self
            .pending_overlay
            .as_ref()
            .is_some_and(|pending| !pending.current(self))
        {
            self.cancel_overlay_capture();
        }
        let mut events = Vec::new();
        if let Some(overlay) = &self.overlay {
            for _ in 0..16 {
                let Some(event) = overlay.poll() else { break };
                events.push(event);
            }
        }
        for event in events {
            match event {
                OverlayEvent::PanelMoved { target, offset } => {
                    self.remember_panel_position(target, offset);
                }
                OverlayEvent::Selected { id, region, point } => {
                    let Some(pending) = self.pending_overlay.take() else {
                        continue;
                    };
                    if pending.id != id {
                        self.pending_overlay = Some(pending);
                        continue;
                    }
                    if !pending.current(self) {
                        self.cancel_overlay_capture();
                        continue;
                    }
                    let result = pending
                        .capture
                        .ok_or("The capture source is no longer available.".to_owned())
                        .and_then(|capture| {
                            if matches!(pending.purpose, CapturePurpose::PresetColor) {
                                self.commit_preset_color(capture, region, point)
                            } else {
                                capture.commit(&mut self.editor, region, point)
                            }
                        });
                    match result {
                        Ok(()) if !matches!(pending.purpose, CapturePurpose::PresetColor) => {
                            self.show_feedback("Selection updated", true)
                        }
                        Ok(()) => {}
                        Err(error) => self.report_capture_error(pending.purpose, error),
                    }
                    self.editing = false;
                    self.overlay_frame_id = 0;
                }
                OverlayEvent::Cancelled { id } => {
                    if self
                        .pending_overlay
                        .as_ref()
                        .is_some_and(|pending| pending.id == id)
                    {
                        self.cancel_overlay_capture();
                        if self.editor.is_recalibrating() {
                            self.cancel_size_review();
                        }
                        self.editing = false;
                    }
                }
                OverlayEvent::Failed { id, message } => {
                    if id.is_none()
                        || self
                            .pending_overlay
                            .as_ref()
                            .is_some_and(|p| Some(p.id) == id)
                    {
                        let purpose = self
                            .pending_overlay
                            .as_ref()
                            .map_or(CapturePurpose::Edit(self.mode), |pending| pending.purpose);
                        self.report_capture_error(purpose, message);
                        self.cancel_overlay_capture();
                    }
                }
            }
        }
        if self.pending_overlay.is_some() {
            self.launch_pending_capture();
            ctx.request_repaint_after(Duration::from_millis(40));
            return;
        }
        self.sync_pinned_panel();
        let safe = self.snapshot.key == self.key()
            && self.snapshot.revision == self.revision
            && self
                .snapshot
                .captured_at
                .is_some_and(|time| time.elapsed() < Duration::from_millis(500))
            && self
                .snapshot
                .geometry
                .is_some_and(|g| g.visible && g.foreground);
        let demanded = self.run_live && !self.window_chooser.open;
        if !safe || !demanded || self.snapshot.frame.is_none() {
            if let Some(overlay) = &self.overlay {
                overlay.hide();
            }
            self.overlay_frame_id = 0;
            return;
        }
        if self.overlay_frame_id == self.snapshot.frame_id {
            return;
        }
        let (candidates, panel) = self.annotation_recognition();
        let regions = if self.editor.is_recalibrating() {
            self.editor
                .review_regions()
                .into_iter()
                .map(|area| (area.region, area.label))
                .collect()
        } else if let Some(area) = self.highlighted_exclusion(ctx) {
            vec![area]
        } else if self.show_regions && self.tab == Tab::Calibration {
            let region = if self.mode == EditMode::Template {
                panel
                    .as_ref()
                    .filter(|found| found.score >= self.editor.profile.panel.threshold)
                    .and_then(|found| {
                        self.snapshot
                            .frame
                            .as_ref()
                            .map(|frame| found.region(frame.width, frame.height))
                    })
            } else {
                self.editor.region(self.mode)
            };
            let label = match self.mode {
                EditMode::Mask(Some(index)) => self
                    .editor
                    .profile
                    .masks
                    .get(index)
                    .map(|area| area.name.clone())
                    .unwrap_or_else(|| tool_name(self.mode).into()),
                _ => tool_name(self.mode).into(),
            };
            region
                .map(|region| vec![(region, label)])
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let result = self.ensure_overlay().and_then(|()| {
            self.overlay
                .as_ref()
                .expect("created overlay")
                .show_annotations(OverlayScene {
                    geometry: self.snapshot.geometry.expect("safe geometry"),
                    frame: self.snapshot.frame.clone().expect("checked frame"),
                    captured_at: self.snapshot.captured_at.expect("safe timestamp"),
                    candidates: if self.show_detections {
                        candidates
                    } else {
                        vec![]
                    },
                    current: self
                        .snapshot
                        .selected_target
                        .filter(|_| self.show_detections),
                    next: self.snapshot.next_target.filter(|_| self.show_detections),
                    regions,
                    show_sizes: self.show_sizes,
                    redraw_interval: Duration::from_millis(u64::from(if self.settings.lite_mode {
                        self.settings.preview_refresh_ms.max(1000)
                    } else {
                        self.settings.preview_refresh_ms
                    })),
                })
        });
        match result {
            Ok(()) => self.overlay_frame_id = self.snapshot.frame_id,
            Err(error) => {
                self.error = Some(error);
                self.overlay_frame_id = self.snapshot.frame_id;
            }
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn annotation_recognition(
        &mut self,
    ) -> (
        Vec<metin_core::Candidate>,
        Option<metin_vision::TemplateMatch>,
    ) {
        if self.snapshot.running
            || self.snapshot.arming
            || (!self.editor.dirty()
                && self.metin_presets.draft.is_none()
                && self.snapshot.recognition_evidence_matches_frame())
        {
            self.overlay_preview = None;
            return (self.snapshot.candidates.clone(), self.snapshot.panel_match);
        }
        let Some(frame) = self.snapshot.frame.clone() else {
            return (Vec::new(), None);
        };
        let mut profile = self.editor.profile.clone();
        if let Some(draft) = &self.metin_presets.draft {
            profile.color = Some(draft.color.clone());
        }
        let valid = self.overlay_preview.as_ref().is_some_and(|cache| {
            cache.profile == profile
                && Arc::ptr_eq(&cache.frame.bgr, &frame.bgr)
                && same_image(&cache.template, &self.editor.template)
        });
        if !valid {
            let panel = self
                .editor
                .template
                .as_ref()
                .and_then(|template| metin_vision::panel_match(&frame, template).ok());
            match metin_vision::detect_candidates_excluding(
                &frame,
                &profile,
                panel
                    .as_ref()
                    .filter(|found| found.score >= self.editor.profile.panel.threshold),
            ) {
                Ok(candidates) => {
                    self.overlay_preview = Some(RecognitionPreview {
                        profile,
                        template: self.editor.template.clone(),
                        frame,
                        candidates,
                        panel,
                    })
                }
                Err(error) => {
                    self.error = Some(format!("Overlay preview: {error}"));
                    self.overlay_preview = None;
                }
            }
        }
        self.overlay_preview
            .as_ref()
            .map_or((Vec::new(), None), |cache| {
                (cache.candidates.clone(), cache.panel)
            })
    }

    fn launch_pending_capture(&mut self) {
        let Some(pending) = &self.pending_overlay else {
            return;
        };
        if pending.launched {
            return;
        }
        if pending.timed_out() {
            self.report_capture_error(pending.purpose, "No fresh game frame or input-release confirmation arrived. Check the selected window and retry.".into());
            self.cancel_overlay_capture();
            return;
        }
        if !pending.ready(&self.snapshot) {
            return;
        }
        let (Some(frame), Some(geometry)) = (self.snapshot.frame.clone(), self.snapshot.geometry)
        else {
            return;
        };
        let id = pending.id;
        let purpose = pending.purpose;
        if let CapturePurpose::SizeReview(mode) = purpose {
            self.pending_overlay = None;
            if self.editor.profile.frame_size == Some([frame.width, frame.height]) {
                self.size_change = None;
                self.edit_area(mode);
            } else if let Err(error) = self.begin_size_review_frame(frame, mode) {
                self.error = Some(error);
            }
            return;
        }
        let mode = match purpose {
            CapturePurpose::Edit(mode) => mode,
            CapturePurpose::PresetColor => EditMode::Color,
            CapturePurpose::SizeReview(_) => unreachable!(),
        };
        if self
            .editor
            .profile
            .frame_size
            .is_some_and(|previous| previous != [frame.width, frame.height])
        {
            self.record_size_change(&frame, mode);
            if matches!(purpose, CapturePurpose::PresetColor) {
                self.report_capture_error(purpose, "The game size changed. Cancel this editor and use Review calibration before sampling again.".into());
            }
            self.cancel_overlay_capture();
            return;
        }
        let kind = if mode == EditMode::Color {
            SelectionKind::Point
        } else {
            SelectionKind::Region
        };
        let label = self.capture_label(mode);
        let capture = match self.editor.begin_capture(mode, frame.clone()) {
            Ok(capture) => capture,
            Err(error) => {
                self.report_capture_error(purpose, error);
                self.cancel_overlay_capture();
                return;
            }
        };
        let initial_region = capture.initial_region();
        let color_preview = (mode == EditMode::Color).then(|| {
            let mut profile = capture.profile().clone();
            if matches!(purpose, CapturePurpose::PresetColor)
                && let Some(draft) = &self.metin_presets.draft
            {
                profile.color = Some(draft.color.clone());
            }
            Box::new(profile)
        });
        let result = self.ensure_overlay().and_then(|()| {
            self.overlay
                .as_ref()
                .expect("created overlay")
                .begin_selection_after_release(SelectionRequest {
                    id,
                    geometry,
                    frame: frame.clone(),
                    kind,
                    label,
                    initial_region,
                    context_regions: if self.editor.is_recalibrating() {
                        self.editor
                            .review_regions()
                            .into_iter()
                            .map(|r| (r.region, r.label))
                            .collect()
                    } else {
                        vec![]
                    },
                    color_preview,
                })
        });
        match result {
            Ok(()) => {
                let pending = self.pending_overlay.as_mut().expect("pending capture");
                pending.launched = true;
                pending.capture = Some(capture);
            }
            Err(error) => {
                self.report_capture_error(purpose, error);
                self.cancel_overlay_capture();
            }
        }
    }

    fn capture_label(&self, mode: EditMode) -> String {
        match mode {
            EditMode::Mask(Some(index)) => self
                .editor
                .profile
                .masks
                .get(index)
                .map(|area| area.name.clone())
                .unwrap_or_else(|| tool_name(mode).into()),
            EditMode::BuffIcon { index, inactive } => {
                let name = self
                    .editor
                    .profile
                    .buffs
                    .skills
                    .get(index)
                    .map(|skill| skill.name.as_str())
                    .filter(|name| !name.is_empty())
                    .unwrap_or("Buff");
                format!(
                    "{name} — {} icon",
                    if inactive { "Inactive" } else { "Active" }
                )
            }
            EditMode::SafetyReference(index) | EditMode::VisualClick(index) => {
                let name = self
                    .editor
                    .profile
                    .safety
                    .rules
                    .get(index)
                    .map(|rule| rule.name.as_str())
                    .unwrap_or("Visual rule");
                format!("{name} — {}", tool_name(mode))
            }
            _ => tool_name(mode).into(),
        }
    }

    #[cfg(windows)]
    fn sync_pinned_panel(&mut self) {
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
        let mut desired = self
            .target
            .filter(|_| self.pinned && !self.window_chooser.open)
            .map(|target| crate::game_overlay::ControlPanelLink {
                window: handle.hwnd.get() as usize,
                target,
                offset: None,
            });
        if self.panel_link.map(|link| (link.window, link.target))
            == desired.map(|link| (link.window, link.target))
        {
            return;
        }
        self.remember_latest_panel_position();
        if let Some(link) = &mut desired {
            if self.placement_key.as_ref().map(|(target, _)| *target) != Some(link.target) {
                self.placement_key =
                    crate::placement::placement_key(link.target, &self.snapshot.targets)
                        .map(|key| (link.target, key));
            }
            link.offset = self
                .placement_key
                .as_ref()
                .and_then(|(_, key)| {
                    self.settings
                        .panel_positions
                        .iter()
                        .find(|position| &position.key == key)
                })
                .map(|position| position.offset);
        }
        if desired.is_some()
            && let Err(error) = self.ensure_overlay()
        {
            self.error = Some(error);
            return;
        }
        if let Some(overlay) = &self.overlay {
            match overlay.set_control_panel(desired) {
                Ok(()) => self.panel_link = desired,
                Err(error) => self.error = Some(error),
            }
        }
    }
    #[cfg(not(windows))]
    fn sync_pinned_panel(&mut self) {}
}

pub(super) fn tool_name(mode: EditMode) -> &'static str {
    match mode {
        EditMode::SafetyReference(_) => "Visual reference",
        EditMode::VisualClick(_) | EditMode::PeriodicClick(_) => "Click target",
        EditMode::World => "Search area",
        EditMode::Color => "Metin color - click a pixel",
        EditMode::Template => "Reference crop",
        EditMode::BuffArea => "Buff detection area",
        EditMode::BuffIcon { inactive: true, .. } => "Inactive icon",
        EditMode::BuffIcon { .. } => "Active icon",
        EditMode::Mask(None) => "Draw excluded area",
        EditMode::Mask(Some(_)) => "Excluded area",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capture() -> (PendingCapture, RuntimeSnapshot) {
        let key = SessionKey {
            profile_id: uuid::Uuid::new_v4(),
            target: metin_platform::TargetToken {
                handle: 7,
                pid: 9,
                created: 11,
            },
        };
        let pending = PendingCapture {
            id: 1,
            key,
            revision: 3,
            generation: 5,
            requested: Instant::now() - Duration::from_millis(1),
            purpose: CapturePurpose::Edit(EditMode::Template),
            launched: false,
            capture: None,
        };
        let snapshot = RuntimeSnapshot {
            key: Some(key),
            revision: 3,
            released_generation: Some(5),
            captured_at: Some(Instant::now()),
            geometry: Some(metin_platform::TargetState {
                target: key.target,
                left: -100,
                top: 20,
                width: 10,
                height: 10,
                visible: true,
                foreground: false,
            }),
            frame: Some(metin_platform::Frame::new(10, 10, vec![100; 300]).unwrap()),
            ..Default::default()
        };
        (pending, snapshot)
    }

    #[test]
    fn capture_requires_current_release_ack_before_opening_native_selection() {
        let (pending, mut snapshot) = capture();
        assert!(pending.ready(&snapshot));
        for generation in [None, Some(4), Some(6)] {
            snapshot.released_generation = generation;
            assert!(!pending.ready(&snapshot));
        }
        snapshot.released_generation = Some(5);
        snapshot.running = true;
        assert!(!pending.ready(&snapshot));
        snapshot.running = false;
        snapshot.arming = true;
        assert!(!pending.ready(&snapshot));
    }

    #[test]
    fn capture_rejects_old_frames_replaced_targets_and_mismatched_dimensions() {
        let (pending, mut snapshot) = capture();
        snapshot.captured_at = Some(pending.requested - Duration::from_millis(1));
        assert!(!pending.ready(&snapshot));
        snapshot.captured_at = Some(Instant::now() - Duration::from_secs(1));
        assert!(!pending.ready(&snapshot));
        snapshot.captured_at = Some(Instant::now());
        snapshot.geometry.as_mut().unwrap().target.created += 1;
        assert!(!pending.ready(&snapshot));
        snapshot.geometry.as_mut().unwrap().target = pending.key.target;
        snapshot.geometry.as_mut().unwrap().width = 11;
        assert!(!pending.ready(&snapshot));
        snapshot.geometry.as_mut().unwrap().width = 10;
        snapshot.key = None;
        assert!(!pending.ready(&snapshot));
    }

    #[test]
    fn pending_capture_times_out_but_operator_selection_does_not() {
        let (mut pending, _) = capture();
        assert!(!pending.timed_out());
        pending.requested = Instant::now() - Duration::from_secs(5);
        assert!(pending.timed_out());
        pending.launched = true;
        assert!(!pending.timed_out());
    }

    #[test]
    fn draft_annotation_preview_uses_editor_color_but_running_uses_applied_results() {
        let root = tempfile::tempdir().unwrap();
        let mut app =
            MetinApp::from_context(&egui::Context::default(), root.path().to_owned()).unwrap();
        let mut pixels = vec![0; 100 * 100 * 3];
        for y in 20..60 {
            for x in 20..60 {
                pixels[(y * 100 + x) * 3 + 1] = 255;
            }
        }
        let frame = metin_platform::Frame::new(100, 100, pixels).unwrap();
        app.editor.freeze(frame.clone()).unwrap();
        app.editor.profile.region = Some(metin_core::Region {
            left: 0.,
            top: 0.,
            right: 1.,
            bottom: 1.,
        });
        app.editor.color([30, 30]).unwrap();
        app.editor.profile.color.as_mut().unwrap().max_area_ratio = 0.5;
        app.snapshot = Arc::new(RuntimeSnapshot {
            frame: Some(frame.clone()),
            ..Default::default()
        });
        let expected = metin_vision::detect_candidates(&frame, &app.editor.profile).unwrap();
        assert!(!expected.is_empty());
        assert_eq!(app.annotation_recognition().0, expected);
        app.editor.profile.color.as_mut().unwrap().hsv = [120, 255, 255];
        app.editor.record();
        assert!(app.annotation_recognition().0.is_empty());
        let mut running = (*app.snapshot).clone();
        running.running = true;
        running.candidates = expected.clone();
        app.snapshot = Arc::new(running);
        assert_eq!(app.annotation_recognition().0, expected);
        assert!(app.overlay_preview.is_none());
        app.runtime.shutdown().unwrap();
    }
    #[test]
    fn clean_paused_annotations_do_not_reuse_hp_bounds_from_an_older_raw_frame() {
        let directory = tempfile::tempdir().unwrap();
        let mut app =
            MetinApp::from_context(&egui::Context::default(), directory.path().to_owned()).unwrap();
        app.editor.mark_saved(app.editor.profile.clone());
        app.target = Some(metin_platform::TargetToken {
            handle: 7,
            pid: 8,
            created: 9,
        });
        let key = app.key().unwrap();
        let now = Instant::now();
        let geometry = metin_platform::TargetState {
            target: key.target,
            left: 0,
            top: 0,
            width: 20,
            height: 20,
            visible: true,
            foreground: false,
        };
        app.snapshot = Arc::new(RuntimeSnapshot {
            key: Some(key),
            revision: app.revision,
            geometry: Some(geometry),
            frame_id: 2,
            frame: Some(metin_platform::Frame::new(20, 20, vec![0; 1200]).unwrap()),
            captured_at: Some(now),
            panel_match: Some(metin_vision::TemplateMatch {
                score: 1.,
                position: [8, 8],
                width: 5,
                height: 5,
            }),
            candidates: vec![metin_core::Candidate {
                x: 8,
                y: 8,
                width: 5,
                height: 5,
                point: [10, 10],
            }],
            recognition_evidence: Some(crate::runtime::RecognitionEvidenceSource {
                key,
                revision: app.revision,
                frame_id: 1,
                captured_at: now - Duration::from_millis(10),
                geometry,
            }),
            ..Default::default()
        });
        let annotations = app.annotation_recognition();
        assert!(
            annotations.0.is_empty() && annotations.1.is_none(),
            "An older frame's target/HP bounds must not be drawn over new raw diagnostic pixels"
        );
        assert!(
            app.overlay_preview.is_some(),
            "Use the existing stateless paused-preview evaluator"
        );
        let mut matching = (*app.snapshot).clone();
        matching.recognition_evidence.as_mut().unwrap().frame_id = matching.frame_id;
        matching.recognition_evidence.as_mut().unwrap().captured_at = now;
        app.snapshot = Arc::new(matching);
        assert!(
            app.annotation_recognition().1.is_some(),
            "Same-frame derived evidence retains its fast reuse path"
        );
        assert!(app.overlay_preview.is_none());
        app.runtime.shutdown().unwrap();
    }
    #[test]
    fn keeping_original_cancels_a_queued_size_review_before_any_frame_can_start_it() {
        let context = egui::Context::default();
        let directory = tempfile::tempdir().unwrap();
        let mut app = MetinApp::from_context(&context, directory.path().to_owned()).unwrap();
        let (mut pending, _) = capture();
        pending.purpose = CapturePurpose::SizeReview(EditMode::World);
        app.target = Some(pending.key.target);
        app.editor.profile.id = pending.key.profile_id;
        app.revision = pending.revision;
        app.editor.profile.frame_size = Some([200, 200]);
        app.record_size_change(
            &metin_platform::Frame::new(200, 185, vec![40; 200 * 185 * 3]).unwrap(),
            EditMode::World,
        );
        app.pending_overlay = Some(pending);
        let mut draw = |events: Vec<egui::Event>| {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(460., 620.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.size_change_view(ui),
            );
            output.textures_delta.clear();
        };
        draw(vec![]);
        let rect: egui::Rect = context
            .data(|d| d.get_temp(egui::Id::unique(("control", "Keep original"))))
            .unwrap();
        for pressed in [true, false] {
            draw(vec![
                egui::Event::PointerMoved(rect.center()),
                egui::Event::PointerButton {
                    pos: rect.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ]);
        }
        drop(draw);
        assert!(
            app.pending_overlay.is_none(),
            "Keep original must retire an already queued size review"
        );
        assert!(app.size_change.is_none());
        assert!(!app.editor.is_recalibrating());
        app.runtime.shutdown().unwrap();
    }
}
