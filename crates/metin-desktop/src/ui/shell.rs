use super::*;
use widgets::{Icon, button};
impl MetinApp {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(5.);
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
            let hint=if self.snapshot.running { "Already running. Use Pause or F8 to pause immediately." } else { blocker.as_ref().map_or("Save the profile, activate the selected game window and start after the countdown. F7 also starts.",|b| b.message.as_str()) };
            let label=if self.has_unsaved_profile() { "Save & Start" } else { "Start" };
            if widgets::primary_button(ui,Icon::Play,label,blocker.is_none() && !self.snapshot.running,hint).clicked() {
                match self.save() { Ok(()) => if let Some(key)=self.key() { self.send(Command::Arm { key }); },Err(e)=>self.error=Some(e) }
            }
            if button(ui,Icon::Pause,"Pause · F8",true,"Pause immediately and release held inputs. F8 remains the emergency pause shortcut.").clicked() { self.runtime.stop(); }
            if button(ui,Icon::Sun,"",true,"Switch between light and dark appearance.").clicked() {
                self.settings.theme=if self.settings.theme=="dark" {"light"} else {"dark"}.into();
                widgets::theme(ui.ctx(),self.settings.theme=="dark");
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::SetTheme(if self.settings.theme=="dark" {egui::SystemTheme::Dark} else {egui::SystemTheme::Light}));
                if let Err(e)=self.settings.save(&self.root) { self.error=Some(e.to_string()); }
            }
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
        ui.label("Game window");
        let old = self.target;
        let targets = self
            .snapshot
            .targets
            .iter()
            .filter(|w| state::matches_process(&w.process_name, &self.editor.profile.process_name))
            .collect::<Vec<_>>();
        let no_matches = targets.is_empty();
        egui::ComboBox::from_id_salt("game-window").truncate().width(ui.available_width().min(340.)).selected_text(self.snapshot.targets.iter().find(|w| Some(w.token)==self.target).map(|w|w.title.as_str()).unwrap_or("Select a game window")).show_ui(ui,|ui| {
            for window in &targets { ui.selectable_value(&mut self.target,Some(window.token),format!("{} · {}",window.title,window.process_name)); }
        }).response.on_hover_text("Select the exact game window. Its process identity remains the target even if titles change.");
        if self.target != old {
            self.runtime.stop();
            if let Err(e) = self.configure() {
                self.error = Some(e);
            }
            self.snapshot_requested = None;
        }
        ui.horizontal_wrapped(|ui| {
            if button(ui,Icon::Refresh,"Refresh",true,"Refresh the list of matching visible game windows.").clicked() {self.send(Command::RefreshTargets);}
            if button(ui,Icon::Camera,if self.snapshot_requested.is_some(){"Capturing..."}else{"Snapshot"},self.target.is_some() && self.snapshot_requested.is_none(),"Pause and capture a fresh frame. Show the target panel in the game before capturing.").clicked() {self.runtime.stop();self.snapshot_requested=Some(std::time::Instant::now());}
        });
        if no_matches {
            ui.small(if self.snapshot.capabilities.automation_ready() {"No matching windows. Open the client, check the process hint in Settings, then Refresh."} else {"No capture/input backend is available here. Profiles and saved calibration can still be edited."});
        }
    }
}
