use super::*;

pub(super) struct Feedback {
    pub message: &'static str,
    profile: uuid::Uuid,
    change: u64,
    undo: bool,
    until: std::time::Instant,
}

impl MetinApp {
    pub(super) fn show_feedback(&mut self, message: &'static str, undo: bool) {
        self.feedback = Some(Feedback {
            message,
            profile: self.editor.profile.id,
            change: self.editor.change_id(),
            undo,
            until: std::time::Instant::now() + std::time::Duration::from_secs(4),
        });
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
