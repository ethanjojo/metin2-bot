use super::*;
use widgets::{Icon, button};
impl MetinApp {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(5.);
        ui.horizontal_top(|ui| {
            let controls_width = (ui.available_width() - 40.).max(100.);
            ui.allocate_ui_with_layout(egui::vec2(controls_width,32.),egui::Layout::top_down(egui::Align::Min),|ui| {
                ui.set_width(controls_width);
                ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("MetinBot").size(20.).strong());
            ui.separator();
            ui.label("Profile");
            let mut selected=self.editor.profile.id;
            egui::ComboBox::from_id_salt("profile").width(165.).truncate().selected_text(&self.editor.profile.name).show_ui(ui,|ui| {
                for p in &self.profiles { ui.selectable_value(&mut selected,p.id,&p.name); }
            }).response.on_hover_text("Switch saved profiles. Unsaved changes are checked before switching.");
            if selected!=self.editor.profile.id { self.switch_profile(selected); }
            if button(ui,Icon::Plus,"",true,"New profile. You can save or discard your current edits first.").clicked() && self.discard_or_save() {
                self.replace_profile(Profile::create("New profile"),None); self.navigate(Tab::Calibration);
            }
            if button(ui,Icon::Save,"Save",self.has_unsaved_profile(),"Save all profile changes. Applying a changed profile pauses automation.").clicked() && let Err(e)=self.save() { self.error=Some(e); }
            let blocker=self.start_blocker();
            let dirty=self.has_unsaved_profile();
            let hint=if self.snapshot.arming {
                "Start requested. Waiting for stable game focus and the 3-second delay. Use Pause or F8 to cancel."
            } else if self.snapshot.running {
                "Already running. Use Pause or F8 to pause immediately."
            } else if let Some(blocker)=&blocker {
                blocker.message.as_str()
            } else if dirty {
                "Save these edits, activate the selected game window and start after the countdown. F7 starts the applied profile without saving these edits."
            } else {
                "Activate the selected game window and start after the countdown. F7 also starts."
            };
            let label=if self.snapshot.arming { "Starting..." } else if dirty { "Save & Start" } else { "Start" };
            if widgets::primary_button(ui,Icon::Play,label,blocker.is_none() && !self.snapshot.running && !self.snapshot.arming,hint).clicked() {
                match self.save() { Ok(()) => if let Some(key)=self.key() { self.send(Command::Arm { key }); },Err(e)=>self.error=Some(e) }
            }
            if button(ui,Icon::Pause,"Pause · F8",true,"Pause immediately and release held inputs. F8 remains the emergency pause shortcut.").clicked() { self.runtime.stop(); }
                });
            });
            ui.allocate_ui_with_layout(egui::vec2(32.,32.),egui::Layout::right_to_left(egui::Align::Center),|ui| {
                let dark = self.settings.theme=="dark";
                if button(ui,if dark {Icon::Sun} else {Icon::Moon},"",true,if dark {"Switch to light mode"} else {"Switch to dark mode"}).clicked() {
                    self.toggle_theme(ui.ctx());
                }
            });
        });
        ui.add_space(5.);
    }
    pub(super) fn navigation(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (tab, label, hint) in [
                (
                    Tab::Run,
                    "Run",
                    "Current state, readiness and session statistics.",
                ),
                (
                    Tab::Calibration,
                    "Calibration",
                    "Connect a game window and edit recognition areas on a frozen snapshot.",
                ),
                (
                    Tab::Buffs,
                    "Buffs",
                    "Configure skill order, timing, icons and the declared mount state.",
                ),
                (
                    Tab::Settings,
                    "Settings",
                    "Recognition tolerances, timing and platform diagnostics.",
                ),
            ] {
                if ui
                    .selectable_label(self.tab == tab, egui::RichText::new(label).strong())
                    .on_hover_text(hint)
                    .clicked()
                {
                    self.navigate(tab);
                }
            }
            if self.has_unsaved_profile() {
                widgets::badge(ui, "Unsaved changes", widgets::AMBER);
            }
        });
    }
    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
        if let Some(error) = self.error.clone() {
            ui.horizontal_wrapped(|ui| {
                widgets::badge(ui,"Action needed",widgets::RED); ui.label(error);
                if ui.button("Dismiss").on_hover_text("Hide this message. Dismissing it does not resolve the underlying condition.").clicked() { self.error=None; }
            });
        }
        ui.horizontal_wrapped(|ui| {
            widgets::badge(
                ui,
                &self.snapshot.state,
                if self.snapshot.running {
                    widgets::GREEN
                } else {
                    ui.visuals().text_color()
                },
            );
            ui.label(&self.snapshot.reason)
                .on_hover_text(&self.snapshot.reason);
            ui.separator();
            ui.small("F7 Start   F8 Pause");
        });
    }
    pub(super) fn window_picker(&mut self, ui: &mut egui::Ui) {
        self.window_choice(ui);
        ui.horizontal_wrapped(|ui| {
            if button(ui,Icon::Refresh,"Refresh",true,"Refresh all visible application windows. This does not start the bot.").clicked() {self.send(Command::RefreshTargets);}
            let snapshot_hint=if self.snapshot_requested.is_some() { "Waiting for a fresh frame from the selected game window. Capture times out after 3 seconds." } else if self.target.is_none() { "Select a game window above before taking a Snapshot." } else { "Pause and capture a fresh frame. Show the target panel in the game before capturing." };
            if button(ui,Icon::Camera,if self.snapshot_requested.is_some(){"Capturing..."}else{"Snapshot"},self.target.is_some() && self.snapshot_requested.is_none(),snapshot_hint).clicked() {self.runtime.stop();self.snapshot_requested=Some(std::time::Instant::now());}
        });
    }
}
