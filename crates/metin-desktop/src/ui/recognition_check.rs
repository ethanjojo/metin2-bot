//! Read-only inspection of one fresh captured client image. This path never
//! owns an input executor and cannot change recognition thresholds or timers.
use super::*;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecognitionSubject {
    HpPanel,
    Buff(usize),
}
#[derive(Clone)]
struct CheckContext {
    subject: RecognitionSubject,
    key: SessionKey,
    revision: u64,
    change: u64,
}
impl CheckContext {
    fn matches(&self, app: &MetinApp) -> bool {
        app.key() == Some(self.key)
            && app.revision == self.revision
            && app.editor.change_id() == self.change
    }
}
struct PendingCheck {
    context: CheckContext,
    after_frame: u64,
    requested_at: Instant,
}
struct Report {
    context: CheckContext,
    captured_at: Instant,
    message: String,
    scores: Vec<String>,
    images: Vec<(&'static str, egui::TextureHandle)>,
}
#[derive(Default)]
pub(super) struct RecognitionCheck {
    pending: Option<PendingCheck>,
    report: Option<Report>,
    failure: Option<(RecognitionSubject, String)>,
    focus_report: u8,
}
impl RecognitionCheck {
    pub(super) fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
}

fn fresh_check_frame(
    snapshot: &RuntimeSnapshot,
    key: SessionKey,
    revision: u64,
    after_frame: u64,
    requested_at: Instant,
) -> Option<&metin_platform::Frame> {
    (snapshot.key == Some(key)
        && snapshot.revision == revision
        && snapshot.frame_id > after_frame
        && snapshot.geometry.is_some_and(|geometry| {
            geometry.target == key.target
                && geometry.visible
                && snapshot.frame.as_ref().is_some_and(|frame| {
                    frame.width == geometry.width && frame.height == geometry.height
                })
        })
        && snapshot.captured_at.is_some_and(|captured| {
            captured >= requested_at
                && Instant::now()
                    .checked_duration_since(captured)
                    .is_some_and(|age| age <= Duration::from_millis(500))
        }))
    .then_some(snapshot.frame.as_ref())
    .flatten()
}

impl MetinApp {
    pub(super) fn recognition_check_controls(
        &mut self,
        ui: &mut egui::Ui,
        subject: RecognitionSubject,
    ) {
        self.poll_recognition_check(ui.ctx());
        let pending = self
            .recognition_check
            .pending
            .as_ref()
            .is_some_and(|p| p.context.subject == subject);
        let available = self.key().is_some()
            && !self.snapshot.running
            && !self.snapshot.arming
            && self.pending_overlay.is_none()
            && !self.editor.is_recalibrating();
        if widgets::button(ui, widgets::Icon::Target, if pending {"Checking…"} else {"Check recognition"}, available && !pending,
            "Capture and compare one fresh game image. No click or shortcut is sent; profile settings and buff timers are unchanged.")
            .on_disabled_hover_text("Connect a window and pause automation; finish any open selection or calibration review first.").clicked()
            && let Some(key) = self.key()
        {
            let context = CheckContext {subject,key,revision:self.revision,change:self.editor.change_id()};
            let requested_at = Instant::now();
            match self.runtime.send(Command::Snapshot {key}) {
                Ok(()) => {
                    self.recognition_check.report = None;
                    self.recognition_check.failure = None;
                    self.recognition_check.pending = Some(PendingCheck {context,after_frame:self.snapshot.frame_id,requested_at});
                }
                Err(error) => self.recognition_check.failure = Some((subject,error)),
            }
        }
        if let RecognitionSubject::Buff(index) = subject {
            self.buff_recognition_evidence(ui, index);
        }
        if pending {
            ui.small("Waiting for a fresh image from the selected window…");
            ui.ctx().request_repaint_after(Duration::from_millis(50));
        }
        if let Some((failed, message)) = &self.recognition_check.failure
            && *failed == subject
        {
            ui.add(egui::Label::new(egui::RichText::new(message).color(widgets::AMBER)).wrap());
        }
        if let Some(report) = &self.recognition_check.report
            && report.context.subject == subject
            && report.context.matches(self)
        {
            let result = ui
                .vertical(|ui| {
                    widgets::section(ui, "Snapshot check", |ui| {
                        ui.small(format!(
                            "Captured {:.1} s ago · read-only result",
                            report.captured_at.elapsed().as_secs_f64()
                        ));
                        ui.add(egui::Label::new(&report.message).wrap());
                        for score in &report.scores {
                            ui.add(egui::Label::new(egui::RichText::new(score).small()).wrap());
                        }
                        ui.horizontal_wrapped(|ui| {
                            for (title, texture) in &report.images {
                                ui.vertical(|ui| {
                                    ui.small(*title);
                                    ui.add(
                                        egui::Image::new(texture).max_size(egui::vec2(112., 72.)),
                                    );
                                });
                            }
                        });
                    });
                })
                .response;
            if self.recognition_check.focus_report > 0 {
                result.scroll_to_me(Some(egui::Align::Min));
                self.recognition_check.focus_report -= 1;
            }
        }
    }
    fn buff_recognition_evidence(&self, ui: &mut egui::Ui, index: usize) {
        let fresh = self.snapshot.key == self.key()
            && self.snapshot.revision == self.revision
            && self.snapshot.applied_revision == Some(self.revision)
            && !self.editor.dirty()
            && self.snapshot.geometry.is_some()
            && self
                .snapshot
                .captured_at
                .is_some_and(|at| at.elapsed() <= Duration::from_millis(500));
        let presence = fresh
            .then(|| self.snapshot.buff_presence.get(index))
            .flatten()
            .copied();
        let message = match presence {
            Some(metin_core::Presence::Active) => "Live state: Active",
            Some(metin_core::Presence::Absent) => "Live state: Inactive",
            Some(metin_core::Presence::Unknown) => {
                "Unknown · References did not confidently distinguish the current icon"
            }
            None if self.editor.dirty() => {
                "Live state applies after Save; snapshot checks use your current edits"
            }
            None if self.snapshot.applied_revision != Some(self.revision) => {
                "Apply the profile to see live state; snapshot checks remain available"
            }
            None => "Unknown · No fresh image",
        };
        ui.add(egui::Label::new(egui::RichText::new(message).small()).wrap());
        if fresh
            && let Some(age) = self
                .snapshot
                .buff_last_active_age_seconds
                .get(index)
                .copied()
                .flatten()
        {
            let since_frame = self
                .snapshot
                .captured_at
                .map_or(0., |at| at.elapsed().as_secs_f64());
            ui.small(format!(
                "Last confident active match: {:.1} s ago",
                age + since_frame
            ));
        }
    }
    pub(super) fn poll_recognition_check(&mut self, context: &egui::Context) {
        if self
            .recognition_check
            .report
            .as_ref()
            .is_some_and(|report| !report.context.matches(self))
        {
            self.recognition_check.report = None;
        }
        let Some(pending) = self.recognition_check.pending.take() else {
            return;
        };
        if !pending.context.matches(self) || self.snapshot.running || self.snapshot.arming {
            self.recognition_check.failure = Some((
                pending.context.subject,
                "Check cancelled because the profile, window or running state changed.".into(),
            ));
            return;
        }
        let Some(frame) = fresh_check_frame(
            &self.snapshot,
            pending.context.key,
            pending.context.revision,
            pending.after_frame,
            pending.requested_at,
        ) else {
            if pending.requested_at.elapsed() > Duration::from_secs(3) {
                let reason=self.snapshot.pause_info.as_ref().map_or("No fresh image arrived. Check that the selected game is visible and responding.",|info|info.details.as_str());
                self.recognition_check.failure = Some((pending.context.subject, reason.to_owned()));
            } else {
                self.recognition_check.pending = Some(pending);
            }
            return;
        };
        let evaluated = evaluate(
            frame,
            &self.editor.profile,
            self.editor.template.as_ref(),
            pending.context.subject,
        );
        match evaluated {
            Err(error) => self.recognition_check.failure = Some((pending.context.subject, error)),
            Ok(evaluated) => {
                self.recognition_check.report = Some(make_report(
                    context,
                    pending.context,
                    self.snapshot.captured_at.expect("fresh frame"),
                    self.snapshot.frame_id,
                    evaluated,
                ));
                self.recognition_check.focus_report = 6;
            }
        }
    }
    #[cfg(feature = "ui-smoke")]
    pub fn prepare_recognition_check_fixture(
        &mut self,
        context: &egui::Context,
    ) -> Result<(), String> {
        let subject = if self.actions_page == ActionsPage::Buffs && self.buff_detail {
            RecognitionSubject::Buff(self.selected_skill)
        } else {
            RecognitionSubject::HpPanel
        };
        let frame = self
            .editor
            .frozen
            .as_ref()
            .or(self.snapshot.frame.as_ref())
            .ok_or("The recognition fixture requires an image.")?;
        let evaluated = evaluate(
            frame,
            &self.editor.profile,
            self.editor.template.as_ref(),
            subject,
        )?;
        let check = CheckContext {
            subject,
            key: self
                .key()
                .ok_or("The recognition fixture requires a selected synthetic window.")?,
            revision: self.revision,
            change: self.editor.change_id(),
        };
        self.recognition_check.report = Some(make_report(
            context,
            check,
            Instant::now(),
            self.snapshot.frame_id,
            evaluated,
        ));
        self.recognition_check.focus_report = 6;
        Ok(())
    }
}
type Evaluation = (
    String,
    Vec<String>,
    Vec<(&'static str, metin_platform::Frame)>,
);
fn make_report(
    context: &egui::Context,
    check: CheckContext,
    captured_at: Instant,
    frame_id: u64,
    (message, scores, images): Evaluation,
) -> Report {
    let images = images
        .into_iter()
        .enumerate()
        .map(|(index, (title, frame))| {
            let rgb = frame
                .bgr
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|pixel| [pixel[2], pixel[1], pixel[0]])
                .collect::<Vec<_>>();
            let texture = context.load_texture(
                format!("recognition-check-{frame_id}-{index}"),
                egui::ColorImage::from_rgb([frame.width as usize, frame.height as usize], &rgb),
                egui::TextureOptions::LINEAR,
            );
            (title, texture)
        })
        .collect();
    Report {
        context: check,
        captured_at,
        message,
        scores,
        images,
    }
}
fn evaluate(
    frame: &metin_platform::Frame,
    profile: &Profile,
    template: Option<&metin_platform::Frame>,
    subject: RecognitionSubject,
) -> Result<Evaluation, String> {
    if profile.frame_size != Some([frame.width, frame.height]) {
        return Err(
            "The game image size differs from this profile. Review recognition before checking it."
                .into(),
        );
    }
    match subject {
        RecognitionSubject::HpPanel => {
            let template =
                template.ok_or("Capture a fixed detail of the target HP panel first.")?;
            let found = metin_vision::panel_match(frame, template).map_err(|e| e.to_string())?;
            let current = metin_vision::crop(frame, found.region(frame.width, frame.height))
                .map_err(|e| e.to_string())?;
            Ok((
                if found.score >= profile.panel.threshold {
                    "HP reference matches this image"
                } else {
                    "HP reference does not meet the match threshold"
                }
                .into(),
                vec![
                    format!(
                        "Match {:.3} · required {:.3}",
                        found.score, profile.panel.threshold
                    ),
                    "A snapshot comparison does not replace consecutive live confirmations.".into(),
                ],
                vec![
                    ("Current best match", current),
                    ("HP reference", template.clone()),
                ],
            ))
        }
        RecognitionSubject::Buff(index) => {
            let diagnostic = metin_vision::inspect_buff(frame, &profile.buffs, index)
                .map_err(|e| e.to_string())?;
            let mut images = vec![("Current best match", diagnostic.current)];
            if let Some(active) = diagnostic.active {
                images.push(("Active", active));
            }
            if let Some(inactive) = diagnostic.inactive {
                images.push(("Inactive", inactive));
            }
            let scores=vec![format!("Active {} · Inactive {} · required {:.2}",diagnostic.active_score.map_or("not available".into(),|v|format!("{v:.3}")),diagnostic.inactive_score.map_or("not available".into(),|v|format!("{v:.3}")),metin_vision::BUFF_MATCH_THRESHOLD),"Live checks also require contrast, a distinct match and stable inactive evidence. Active hold timing is unchanged.".into()];
            Ok((diagnostic.message, scores, images))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_check_rejects_previous_frames_stale_images_and_other_targets() {
        let target = TargetToken {
            handle: 1,
            pid: 2,
            created: 3,
        };
        let key = SessionKey {
            profile_id: uuid::Uuid::new_v4(),
            target,
        };
        let now = Instant::now();
        let mut snapshot = RuntimeSnapshot {
            key: Some(key),
            frame_id: 2,
            captured_at: Some(now),
            geometry: Some(metin_platform::TargetState {
                target,
                left: 0,
                top: 0,
                width: 10,
                height: 10,
                visible: true,
                foreground: false,
            }),
            frame: Some(metin_platform::Frame::new(10, 10, vec![0; 300]).unwrap()),
            ..Default::default()
        };
        assert!(fresh_check_frame(&snapshot, key, 0, 1, now - Duration::from_millis(1)).is_some());
        assert!(fresh_check_frame(&snapshot, key, 1, 1, now).is_none());
        assert!(fresh_check_frame(&snapshot, key, 0, 2, now).is_none());
        assert!(fresh_check_frame(&snapshot, key, 0, 1, now + Duration::from_millis(1)).is_none());
        assert!(
            fresh_check_frame(
                &snapshot,
                SessionKey {
                    profile_id: uuid::Uuid::new_v4(),
                    target
                },
                0,
                1,
                now
            )
            .is_none()
        );
        snapshot.captured_at = Some(now - Duration::from_secs(1));
        assert!(fresh_check_frame(&snapshot, key, 0, 1, now - Duration::from_secs(2)).is_none());
        snapshot.captured_at = Some(Instant::now() + Duration::from_secs(1));
        assert!(fresh_check_frame(&snapshot, key, 0, 1, now).is_none());
        snapshot.captured_at = Some(now);
        snapshot.geometry.as_mut().unwrap().target.created += 1;
        assert!(fresh_check_frame(&snapshot, key, 0, 1, now).is_none());
    }
}
