use super::*;

impl MetinApp {
    /// A declaration of the selected character's state, never a game input.
    /// Run and Actions use the same acknowledged runtime value and command.
    pub(super) fn mount_state_control(&mut self, ui: &mut egui::Ui) {
        let connected = self.key().is_some() && self.snapshot.key == self.key();
        let declared = self.declared_mount();
        let mut mounted = declared;
        ui.horizontal_wrapped(|ui| {
            ui.strong("Mount state");
            crate::ui_controls::help(ui, "Confirm what you currently see in the selected game window. This only records its state and pauses automation; it never presses the mount key. No Snapshot is needed.");
            self.mount_choices(ui, &mut mounted);
            if declared.is_none() && self.key().is_some() {
                widgets::badge(ui, "Unknown", widgets::AMBER);
            }
        });
        if self.key().is_none() {
            ui.small("Select a game window to confirm the character's current mount state.");
        } else if !connected {
            ui.small("Connecting to the selected game window. Mount confirmation becomes available once connected.");
        } else if declared.is_none() {
            ui.small("Mount state unknown: check the game. Choose On foot or Mounted; no Snapshot needed.");
        }
        self.confirm_mount(mounted);
    }
    pub(super) fn run_mount_state(&mut self, ui: &mut egui::Ui) {
        let declared = self.declared_mount();
        let mut mounted = declared;
        ui.horizontal_wrapped(|ui| {
            ui.strong("Mount");
            crate::ui_controls::help(ui, "Confirm the character's current state. No Snapshot is required. Confirmation pauses automation and sends no game input. Fight mounted and the mount shortcut are configured in Actions > Mount.");
            if let Some(value) = declared && !self.run_mount_editing {
                ui.label(if value { "Mounted" } else { "On foot" });
                if ui.small_button("Change").on_hover_text("Correct the declared state. Opening these choices does not pause automation or press a key.").clicked() {
                    self.run_mount_editing = true;
                }
            } else {
                if declared.is_none() { widgets::badge(ui, "Unknown", widgets::AMBER); }
                self.mount_choices(ui, &mut mounted);
                if declared.is_some() && ui.small_button("Cancel").clicked() {
                    self.run_mount_editing = false;
                }
                if self.key().is_none() { ui.small("Select a game window first."); }
                else if self.snapshot.key != self.key() { ui.small("Connecting…"); }
            }
        });
        if self.confirm_mount(mounted) {
            self.run_mount_editing = false;
        }
    }
    fn mount_choices(&self, ui: &mut egui::Ui, mounted: &mut Option<bool>) {
        let connected = self.key().is_some() && self.snapshot.key == self.key();
        ui.add_enabled_ui(connected, |ui| {
            ui.selectable_value(mounted, Some(false), "On foot")
                .on_hover_text("Confirm this character is on foot. Pauses automation; sends no game input.");
            ui.selectable_value(mounted, Some(true), "Mounted")
                .on_hover_text("Confirm this character is mounted. Pauses automation; sends no game input.");
        }).response.on_disabled_hover_text("Select a game window and wait for it to connect. Calibration and a Snapshot are not required.");
    }
    fn confirm_mount(&mut self, mounted: Option<bool>) -> bool {
        if mounted != self.declared_mount()
            && let Some(mounted) = mounted
            && let Some(key) = self.key()
            && self.snapshot.key == Some(key)
        {
            self.runtime.stop();
            self.revision += 1;
            self.send(Command::DeclareMount {
                key,
                mounted,
                revision: self.revision,
            });
            true
        } else {
            false
        }
    }
}
