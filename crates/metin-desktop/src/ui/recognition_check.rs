//! Read-only inspection of one fresh client image. No input executor or temporal
//! detector state is owned here: one frame is evidence, never live confirmation.
use super::*;
use std::time::{Duration, Instant};

mod dialog;
#[cfg(feature = "ui-smoke")]
mod fixtures;
#[cfg(feature = "ui-smoke")]
pub use fixtures::RecognitionCheckFixture;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecognitionSubject {
    HpPanel,
    Buff(usize),
}
#[derive(Clone)]
struct CheckContext {
    subject: RecognitionSubject,
    title: String,
    key: SessionKey,
    revision: u64,
    change: u64,
    unsaved: bool,
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
#[derive(Debug, PartialEq)]
enum SnapshotEvidence {
    Hp {
        score: f64,
        threshold: f64,
        matched: bool,
        bounds: [u32; 4],
    },
    Buff {
        active_score: Option<f64>,
        inactive_score: Option<f64>,
        presence: metin_core::Presence,
        reason: String,
    },
}
#[derive(Clone, Copy)]
enum ImageRole {
    Current,
    HpReference,
    ActiveReference,
    InactiveReference,
}
impl ImageRole {
    fn title(self) -> &'static str {
        match self {
            Self::Current => "Current best match",
            Self::HpReference => "HP reference",
            Self::ActiveReference => "Active reference",
            Self::InactiveReference => "Inactive reference",
        }
    }
}
struct ReportImage {
    role: ImageRole,
    texture: egui::TextureHandle,
}
struct Report {
    context: CheckContext,
    captured_at: Instant,
    source_size: [u32; 2],
    evidence: SnapshotEvidence,
    images: Vec<ReportImage>,
}
struct CheckFailure {
    context: CheckContext,
    message: String,
}
#[derive(Default)]
enum DialogState {
    #[default]
    Closed,
    Pending(PendingCheck),
    Ready(Report),
    Failure(CheckFailure),
    Invalidated {
        title: String,
    },
}
#[derive(Default)]
pub(super) struct RecognitionCheck {
    state: DialogState,
}
impl RecognitionCheck {
    pub(super) fn is_pending(&self) -> bool {
        matches!(self.state, DialogState::Pending(_))
    }
    pub(super) fn is_open(&self) -> bool {
        !matches!(self.state, DialogState::Closed)
    }
    fn context(&self) -> Option<&CheckContext> {
        match &self.state {
            DialogState::Pending(pending) => Some(&pending.context),
            DialogState::Ready(report) => Some(&report.context),
            DialogState::Failure(failure) => Some(&failure.context),
            DialogState::Closed | DialogState::Invalidated { .. } => None,
        }
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
        if widgets::button(ui, widgets::Icon::Target, "Check recognition", self.recognition_check_available() && !self.recognition_check.is_open(),
            "Compare one fresh game image using current edits. No click or shortcut is sent; settings and buff timers stay unchanged.")
            .on_disabled_hover_text("Connect a window and pause automation; finish any selection or calibration review first.").clicked()
        {
            self.begin_recognition_check(subject);
        }
        if let RecognitionSubject::Buff(index) = subject {
            self.buff_recognition_evidence(ui, index);
        }
    }
    fn recognition_check_available(&self) -> bool {
        self.key().is_some()
            && !self.snapshot.running
            && !self.snapshot.arming
            && self.pending_overlay.is_none()
            && !self.editor.is_recalibrating()
            && !self.window_chooser.open
    }
    fn recognition_context(&self, subject: RecognitionSubject) -> Option<CheckContext> {
        let title = match subject {
            RecognitionSubject::HpPanel => "HP panel".into(),
            RecognitionSubject::Buff(index) => {
                let skill = self.editor.profile.buffs.skills.get(index)?;
                if skill.name.trim().is_empty() {
                    format!("Buff {}", index + 1)
                } else {
                    skill.name.clone()
                }
            }
        };
        Some(CheckContext {
            subject,
            title,
            key: self.key()?,
            revision: self.revision,
            change: self.editor.change_id(),
            unsaved: self.has_unsaved_profile(),
        })
    }
    fn begin_recognition_check(&mut self, subject: RecognitionSubject) {
        if !self.recognition_check_available() {
            return;
        }
        let Some(context) = self.recognition_context(subject) else {
            return;
        };
        let key = context.key;
        self.recognition_check.state = DialogState::Pending(PendingCheck {
            context,
            after_frame: self.snapshot.frame_id,
            requested_at: Instant::now(),
        });
        if let Err(message) = self.runtime.send(Command::Snapshot {
            key,
            purpose: crate::runtime::SnapshotPurpose::ReadOnlyRecognition,
        }) {
            let DialogState::Pending(pending) = std::mem::take(&mut self.recognition_check.state)
            else {
                unreachable!()
            };
            self.recognition_check.state = DialogState::Failure(CheckFailure {
                context: pending.context,
                message,
            });
        }
    }
    pub(super) fn close_recognition_check(&mut self) {
        self.recognition_check.state = DialogState::Closed;
    }
    fn buff_live_evidence_is_fresh(&self) -> bool {
        self.key().is_some()
            && self.snapshot.key == self.key()
            && self.snapshot.revision == self.revision
            && self.snapshot.applied_revision == Some(self.revision)
            && !self.editor.dirty()
            && self.snapshot.recognition_evidence_is_fresh()
    }
    fn buff_recognition_evidence(&self, ui: &mut egui::Ui, index: usize) {
        let fresh = self.buff_live_evidence_is_fresh();
        let presence = fresh
            .then(|| self.snapshot.buff_presence.get(index))
            .flatten()
            .copied();
        let message = match presence {
            Some(metin_core::Presence::Active) => {
                "Live state: Active (may include configured hold)"
            }
            Some(metin_core::Presence::Absent) => "Live state: Inactive",
            Some(metin_core::Presence::Unknown) => {
                "Live state: Unknown · References did not distinguish the icon"
            }
            None if self.editor.dirty() => {
                "Live state applies after Save; snapshot checks use your current edits"
            }
            None if self.snapshot.applied_revision != Some(self.revision) => {
                "Apply the profile to see live state; snapshot checks remain available"
            }
            None => "Live state: Unknown · No fresh image",
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
                .recognition_evidence
                .map_or(0., |source| source.captured_at.elapsed().as_secs_f64());
            ui.small(format!(
                "Last confident active match: {:.1} s ago",
                age + since_frame
            ));
        }
    }
    pub(super) fn poll_recognition_check(&mut self, context: &egui::Context) {
        if let Some(check) = self.recognition_check.context()
            && (!check.matches(self) || self.snapshot.running || self.snapshot.arming)
        {
            let title = check.title.clone();
            self.recognition_check.state = DialogState::Invalidated { title };
            return;
        }
        if !self.recognition_check.is_pending() {
            return;
        }
        let DialogState::Pending(pending) = std::mem::take(&mut self.recognition_check.state)
        else {
            unreachable!()
        };
        let Some(frame) = fresh_check_frame(
            &self.snapshot,
            pending.context.key,
            pending.context.revision,
            pending.after_frame,
            pending.requested_at,
        ) else {
            self.recognition_check.state = if pending.requested_at.elapsed()
                > Duration::from_secs(3)
            {
                let message = "No fresh image arrived. Check that the selected game is visible and responding.".to_owned();
                DialogState::Failure(CheckFailure {
                    context: pending.context,
                    message,
                })
            } else {
                DialogState::Pending(pending)
            };
            return;
        };
        self.recognition_check.state = match evaluate(
            frame,
            &self.editor.profile,
            self.editor.template.as_ref(),
            pending.context.subject,
        ) {
            Err(message) => DialogState::Failure(CheckFailure {
                context: pending.context,
                message,
            }),
            Ok(evaluated) => DialogState::Ready(make_report(
                context,
                pending.context,
                self.snapshot.captured_at.expect("fresh frame"),
                self.snapshot.frame_id,
                evaluated,
            )),
        };
    }
}
struct Evaluation {
    evidence: SnapshotEvidence,
    source_size: [u32; 2],
    images: Vec<(ImageRole, metin_platform::Frame)>,
}
fn make_report(
    context: &egui::Context,
    check: CheckContext,
    captured_at: Instant,
    frame_id: u64,
    evaluated: Evaluation,
) -> Report {
    let images = evaluated
        .images
        .into_iter()
        .enumerate()
        .map(|(index, (role, frame))| ReportImage {
            role,
            texture: forms::frame_texture(
                context,
                format!("recognition-check-{frame_id}-{index}"),
                &frame,
                egui::TextureOptions::NEAREST,
            ),
        })
        .collect();
    Report {
        context: check,
        captured_at,
        source_size: evaluated.source_size,
        evidence: evaluated.evidence,
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
            if !found.score.is_finite() {
                return Err("The HP reference could not produce a valid match score. Capture a distinctive fixed detail.".into());
            }
            let current = metin_vision::crop(frame, found.region(frame.width, frame.height))
                .map_err(|e| e.to_string())?;
            Ok(Evaluation {
                source_size: [frame.width, frame.height],
                evidence: SnapshotEvidence::Hp {
                    score: found.score,
                    threshold: profile.panel.threshold,
                    matched: found.score >= profile.panel.threshold,
                    bounds: found.bounds(),
                },
                images: vec![
                    (ImageRole::Current, current),
                    (ImageRole::HpReference, template.clone()),
                ],
            })
        }
        RecognitionSubject::Buff(index) => {
            let diagnostic = metin_vision::inspect_buff(frame, &profile.buffs, index)
                .map_err(|e| e.to_string())?;
            let presence = if diagnostic.active_match {
                metin_core::Presence::Active
            } else if diagnostic.inactive_match {
                metin_core::Presence::Absent
            } else {
                metin_core::Presence::Unknown
            };
            let mut images = vec![(ImageRole::Current, diagnostic.current)];
            if let Some(active) = diagnostic.active {
                images.push((ImageRole::ActiveReference, active));
            }
            if let Some(inactive) = diagnostic.inactive {
                images.push((ImageRole::InactiveReference, inactive));
            }
            Ok(Evaluation {
                source_size: [frame.width, frame.height],
                evidence: SnapshotEvidence::Buff {
                    active_score: diagnostic.active_score,
                    inactive_score: diagnostic.inactive_score,
                    presence,
                    reason: diagnostic.message,
                },
                images,
            })
        }
    }
}
