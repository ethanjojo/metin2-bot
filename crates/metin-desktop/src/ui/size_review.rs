//! Explicit calibration-size review, sharing the normal fresh-frame selector.
use super::*;
use metin_platform::Frame;

#[derive(Clone, Copy)]
pub(super) struct SizeChange {
    pub key: SessionKey,
    pub revision: u64,
    pub previous: [u32; 2],
    pub current: [u32; 2],
    pub mode: EditMode,
}
pub(super) struct ReviewContext {
    pub key: SessionKey,
    pub revision: u64,
    pub size: [u32; 2],
    pub initial_mode: Option<EditMode>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReviewEnd {
    pub key: SessionKey,
    pub revision: u64,
}

/// A failed bounded-queue send never loses the intent to release a review gate.
pub(super) fn deliver_review_end(
    pending: &mut Option<ReviewEnd>,
    send: impl FnOnce(Command) -> Result<(), String>,
) -> bool {
    let Some(end) = *pending else {
        return true;
    };
    if send(Command::CalibrationReview {
        key: end.key,
        revision: end.revision,
        active: false,
    })
    .is_err()
    {
        return false;
    }
    *pending = None;
    true
}

impl MetinApp {
    fn flush_review_end(&mut self) -> bool {
        deliver_review_end(&mut self.pending_review_end, |cmd| self.runtime.send(cmd))
    }
    fn end_runtime_review(&mut self, key: SessionKey, revision: u64) {
        self.pending_review_end = Some(ReviewEnd { key, revision });
        self.flush_review_end();
    }
    #[cfg(feature = "ui-smoke")]
    pub fn prepare_size_review_fixture(&mut self) -> Result<(), String> {
        let frame = self
            .editor
            .frozen
            .clone()
            .ok_or("A review fixture needs source pixels.")?;
        self.set_window_picker_fixture(WindowPickerFixture::Selected);
        self.editor.profile.frame_size = Some([frame.width, frame.height + 15]);
        self.editor.profile.masks.push(metin_core::Region {
            left: 0.1,
            top: 0.96,
            right: 0.3,
            bottom: 0.995,
        });
        self.editor.mark_saved(self.editor.profile.clone());
        let target = self.target.expect("selected fixture");
        let mut snapshot = (*self.window_fixture.as_ref().expect("fixture").snapshot).clone();
        snapshot.key = self.key();
        snapshot.revision = self.revision;
        snapshot.frame = Some(frame.clone());
        snapshot.captured_at = Some(std::time::Instant::now());
        snapshot.geometry = Some(metin_platform::TargetState {
            target,
            left: 0,
            top: 0,
            width: frame.width,
            height: frame.height,
            visible: true,
            foreground: true,
        });
        snapshot.calibration_review = true;
        snapshot.mounted = Some(false);
        self.snapshot = Arc::new(snapshot.clone());
        self.window_fixture.as_mut().expect("fixture").snapshot = Arc::new(snapshot);
        self.begin_size_review_frame(frame, EditMode::World)?;
        self.review_context.as_mut().expect("review").initial_mode = None;
        Ok(())
    }

    pub(super) fn record_size_change(&mut self, frame: &Frame, mode: EditMode) {
        if let (Some(key), Some(previous)) = (self.key(), self.editor.profile.frame_size) {
            self.size_change = Some(SizeChange {
                key,
                revision: self.revision,
                previous,
                current: [frame.width, frame.height],
                mode,
            });
            self.tab = Tab::Calibration;
            self.editing = false;
        }
    }
    pub(super) fn size_change_view(&mut self, ui: &mut egui::Ui) {
        let Some(change) = self.size_change else {
            return;
        };
        widgets::panel(ui, |ui| {
            widgets::badge(ui, "Window size changed", widgets::AMBER);
            ui.label(format!(
                "{} × {} to {} × {} px",
                change.previous[0], change.previous[1], change.current[0], change.current[1]
            ));
            crate::ui_controls::help(
                ui,
                "The saved calibration is unchanged. Review existing areas on a fresh image before accepting the current client size. No automatic scaling or deletion is performed.",
            );
            ui.horizontal_wrapped(|ui| {
                if widgets::plain_button(ui, "Review calibration").clicked() {
                    if self.key() == Some(change.key) && self.revision == change.revision {
                        self.request_capture(overlay::CapturePurpose::SizeReview(change.mode));
                    } else {
                        self.size_change = None;
                    }
                }
                if widgets::plain_button(ui, "Keep original").clicked() {
                    self.cancel_overlay_capture();
                    self.size_change = None;
                }
            });
        });
        ui.add_space(6.);
    }
    pub(super) fn begin_size_review_frame(
        &mut self,
        frame: Frame,
        mode: EditMode,
    ) -> Result<(), String> {
        self.feedback = None;
        if !self.flush_review_end() {
            return Err(
                "Waiting for the previous calibration review to close. Try again shortly.".into(),
            );
        }
        let key = self
            .key()
            .ok_or("Connect a game window before reviewing calibration.")?;
        if self.snapshot.key != Some(key) || self.snapshot.revision != self.revision {
            return Err("Wait for the connected session before reviewing calibration.".into());
        }
        if !self.snapshot.geometry.is_some_and(|g| {
            g.visible
                && g.target == key.target
                && [g.width, g.height] == [frame.width, frame.height]
        }) {
            return Err("The game geometry changed. Request a fresh calibration review.".into());
        }
        self.runtime.send(Command::CalibrationReview {
            key,
            revision: self.revision,
            active: true,
        })?;
        let size = [frame.width, frame.height];
        if let Err(error) = self.editor.begin_size_review(frame) {
            self.end_runtime_review(key, self.revision);
            return Err(error);
        }
        self.review_context = Some(ReviewContext {
            key,
            revision: self.revision,
            size,
            initial_mode: Some(mode),
        });
        self.size_change = None;
        self.overlay_preview = None;
        self.overlay_frame_id = 0;
        self.tab = Tab::Calibration;
        self.return_to_buffs = false;
        self.return_to_safety = false;
        self.editing = false;
        Ok(())
    }
    pub(super) fn cancel_size_review(&mut self) {
        self.feedback = None;
        if let Some(owner) = self.review_context.take() {
            self.cancel_overlay_capture();
            self.end_runtime_review(owner.key, owner.revision);
        }
        self.editor.cancel_size_review();
        self.overlay_preview = None;
        self.overlay_frame_id = 0;
        self.editing = false;
        self.return_to_buffs = false;
        self.return_to_safety = false;
    }
    pub(super) fn size_review_can_confirm(&self) -> bool {
        self.review_context.as_ref().is_some_and(|owner| {
            self.key() == Some(owner.key)
                && self.revision == owner.revision
                && self.snapshot.key == Some(owner.key)
                && self.snapshot.revision == owner.revision
                && self.snapshot.calibration_review
                && self.pending_overlay.is_none()
                && self
                    .snapshot
                    .captured_at
                    .is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(500))
                && self.snapshot.geometry.is_some_and(|g| {
                    g.visible && g.target == owner.key.target && [g.width, g.height] == owner.size
                })
        }) && self.editor.review_issues().is_empty()
    }
    pub(super) fn finish_size_review(&mut self) -> Result<(), String> {
        if !self.size_review_can_confirm() {
            return Err("Finish the active selection, fix highlighted areas and restore the connected game before confirming the review.".into());
        }
        self.editor.finish_size_review()?;
        self.feedback = None;
        if let Some(owner) = self.review_context.take() {
            self.end_runtime_review(owner.key, owner.revision);
        }
        self.overlay_preview = None;
        self.overlay_frame_id = 0;
        self.return_to_buffs = false;
        self.return_to_safety = false;
        Ok(())
    }
    pub(super) fn sync_size_review(&mut self) {
        self.flush_review_end();
        if let Some(owner) = self.review_context.as_ref() {
            let wrong_session = self.key() != Some(owner.key) || self.revision != owner.revision;
            let wrong_size = self.snapshot.key == Some(owner.key)
                && self.snapshot.revision == owner.revision
                && self.snapshot.geometry.is_some_and(|g| {
                    g.target != owner.key.target || [g.width, g.height] != owner.size
                });
            if wrong_session || wrong_size {
                self.cancel_size_review();
                self.error=Some("Calibration review cancelled because the connected window or its size changed. The previous calibration was restored.".into());
                return;
            }
            if self.snapshot.key == Some(owner.key)
                && self.snapshot.revision == owner.revision
                && self.snapshot.calibration_review
                && self.pending_overlay.is_none()
                && let Some(mode) = self
                    .review_context
                    .as_mut()
                    .and_then(|r| r.initial_mode.take())
            {
                self.edit_area(mode);
            }
        }
        if self.size_change.is_some_and(|change| {
            self.key() != Some(change.key) || self.revision != change.revision
        }) {
            self.size_change = None;
        }
    }
    pub(super) fn size_review_view(&mut self, ui: &mut egui::Ui) {
        widgets::heading(
            ui,
            "Review calibration",
            "Review preserved areas on the current client image. Captured icons and reference pixels are retained. Escape cancels the entire review; confirmation is one undoable edit.",
        );
        if let Some((old, new)) = self.editor.reviewing_dimensions() {
            ui.label(format!(
                "{} × {} to {} × {} px",
                old[0], old[1], new[0], new[1]
            ));
        }
        let confirmed = self.size_review_can_confirm();
        ui.horizontal_wrapped(|ui| {
            if widgets::button(ui,widgets::Icon::Check,"Confirm calibration",confirmed,"Accept the current client size and reviewed areas. Save applies the changes; automation stays paused.").clicked()
                && let Err(error)=self.finish_size_review(){self.error=Some(error);}
            if widgets::plain_button(ui,"Cancel review").clicked(){self.cancel_size_review();}
        });
        if !self.editor.is_recalibrating() {
            return;
        }
        let issues = self.editor.review_issues().len();
        if issues > 0 {
            ui.colored_label(
                widgets::RED,
                format!("{issues} area(s) outside the new image. Edit or remove them."),
            );
        }
        if !self.snapshot.calibration_review {
            ui.spinner();
            ui.weak("Pausing and securing the review…");
        }
        if self.pending_overlay.is_some() {
            ui.weak("Selecting in the frozen game image…");
        }
        ui.separator();
        egui::ScrollArea::vertical().id_salt("size-review-areas").show(ui,|ui| {
            let regions=self.editor.review_regions();
            let enabled=self.pending_overlay.is_none() && self.snapshot.calibration_review;
            for area in regions {
                let invalid=area.region.validate().is_err();
                ui.horizontal(|ui| {
                    widgets::readiness_mark(ui,!invalid);
                    ui.add_sized(egui::vec2((ui.available_width()-82.).max(1.),28.),egui::Label::new(&area.label).truncate()).on_hover_text(&area.label);
                    if ui.add_enabled_ui(enabled,|ui|widgets::plain_button(ui,"Edit")).inner.on_hover_text("Adjust this area on a fresh frozen client image; all review outlines remain visible.").clicked(){self.edit_area(area.mode);}
                    if widgets::small_icon_button(ui,widgets::Icon::Trash,enabled,"Remove this area from the review draft. Cancel restores the original.").clicked(){self.delete_area(area.mode);}
                });
            }
            ui.horizontal(|ui| {ui.label("Metin color");if widgets::button(ui,widgets::Icon::Target,"Pick",enabled,"Optional: sample a new target color during review.").clicked(){self.edit_area(EditMode::Color);}});
        });
    }
}
