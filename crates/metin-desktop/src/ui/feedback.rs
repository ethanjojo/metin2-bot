use super::*;

pub(super) struct Feedback {
    pub message: &'static str,
    kind: FeedbackKind,
    profile: uuid::Uuid,
    change: u64,
    undo: bool,
    until: std::time::Instant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FeedbackKind {
    Notice,
    Saved,
}

impl MetinApp {
    pub(super) fn show_feedback(&mut self, message: &'static str, undo: bool) {
        self.feedback = Some(Feedback {
            message,
            kind: FeedbackKind::Notice,
            profile: self.editor.profile.id,
            change: self.editor.change_id(),
            undo,
            until: std::time::Instant::now() + std::time::Duration::from_secs(4),
        });
    }
    pub(super) fn show_saved_feedback(&mut self) {
        self.feedback = Some(Feedback {
            message: "Saved",
            kind: FeedbackKind::Saved,
            profile: self.editor.profile.id,
            change: self.editor.change_id(),
            undo: false,
            until: std::time::Instant::now() + std::time::Duration::from_secs(2),
        });
    }
    fn saved_feedback_current(&self) -> bool {
        self.feedback_current()
            && !self.editor.dirty()
            && !self.editor.is_recalibrating()
            && self
                .feedback
                .as_ref()
                .is_some_and(|notice| notice.kind == FeedbackKind::Saved)
    }
    pub(super) fn footer_feedback_current(&self) -> bool {
        self.feedback_current()
            && self
                .feedback
                .as_ref()
                .is_some_and(|notice| notice.kind == FeedbackKind::Notice)
    }
    pub(super) fn save_profile_button(
        &mut self,
        ui: &mut egui::Ui,
        can_commit: bool,
    ) -> egui::Response {
        let saved = self.saved_feedback_current();
        if saved && let Some(notice) = &self.feedback {
            ui.ctx().request_repaint_after(
                notice
                    .until
                    .saturating_duration_since(std::time::Instant::now()),
            );
        }
        let hint = if saved {
            "Saved"
        } else {
            "Save profile changes. Applying changes pauses automation."
        };
        let response = widgets::colored_small_icon_button(
            ui,
            if saved {
                widgets::Icon::Check
            } else {
                widgets::Icon::Save
            },
            saved || can_commit,
            hint,
            if saved {
                widgets::GREEN
            } else {
                egui::Color32::from_gray(238)
            },
        );
        #[cfg(test)]
        ui.ctx().data_mut(|data| {
            data.insert_temp(egui::Id::unique("profile-save"), (response.rect, saved))
        });
        response
    }
    pub(super) fn feedback_current(&self) -> bool {
        self.feedback.as_ref().is_some_and(|notice| {
            notice.profile == self.editor.profile.id
                && notice.change == self.editor.change_id()
                && notice.until > std::time::Instant::now()
        })
    }
    pub(super) fn undo_feedback(&mut self) -> bool {
        if self.feedback_current()
            && self.feedback.as_ref().is_some_and(|notice| notice.undo)
            && self.editor.can_undo()
        {
            self.undo_profile();
            self.show_feedback("Restored", false);
            true
        } else {
            false
        }
    }
    pub(super) fn feedback_view(&mut self, ui: &mut egui::Ui) -> bool {
        if !self.feedback_current() {
            self.feedback = None;
            return false;
        }
        let notice = self.feedback.as_ref().expect("current feedback");
        let message = notice.message;
        let undo = notice.undo && self.editor.can_undo();
        let until = notice.until;
        ui.label(egui::RichText::new(message).small().color(widgets::GREEN));
        if undo
            && widgets::compact_text_button(ui, "Undo")
                .on_hover_text("Restore only the edit identified by this message.")
                .clicked()
        {
            self.undo_feedback();
        }
        ui.ctx()
            .request_repaint_after(until.saturating_duration_since(std::time::Instant::now()));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_indicator_expires_after_two_seconds_and_retires_when_the_draft_changes() {
        let directory = tempfile::tempdir().unwrap();
        let context = egui::Context::default();
        let mut app = MetinApp::from_context(&context, directory.path().to_owned()).unwrap();
        app.show_saved_feedback();
        let until = app.feedback.as_ref().unwrap().until;
        let remaining = until.saturating_duration_since(std::time::Instant::now());
        assert!(remaining <= std::time::Duration::from_secs(2));
        assert!(remaining > std::time::Duration::from_millis(1900));
        assert!(app.saved_feedback_current());
        assert!(!app.footer_feedback_current());
        app.feedback.as_mut().unwrap().until =
            std::time::Instant::now() - std::time::Duration::from_secs(1);
        assert!(!app.saved_feedback_current());
        app.show_saved_feedback();
        app.editor.profile.name.push_str(" changed");
        app.editor.record();
        assert!(!app.saved_feedback_current());
        app.show_feedback("Area renamed", true);
        assert!(app.footer_feedback_current());
        app.runtime.shutdown().unwrap();
    }
}
