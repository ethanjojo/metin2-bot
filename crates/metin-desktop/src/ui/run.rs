use super::*;
use widgets::{Icon, button, section};
impl MetinApp {
    pub(super) fn run_view(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("run-scroll")
            .show(ui, |ui| {
                widgets::heading(
                    ui,
                    "Run overview",
                    "One place to see what is happening and what needs your attention.",
                );
                if ui.available_width() >= 1000. {
                    ui.columns(2, |cols| {
                        self.run_activity(&mut cols[0]);
                        self.run_connection(&mut cols[0]);
                        self.run_session(&mut cols[1]);
                        self.run_schedule(&mut cols[1]);
                    });
                } else {
                    self.run_activity(ui);
                    self.run_session(ui);
                    self.run_schedule(ui);
                    self.run_connection(ui);
                }
            });
    }
    fn run_activity(&mut self, ui: &mut egui::Ui) {
        section(ui, "Current activity", |ui| {
            ui.label(
                egui::RichText::new(&self.snapshot.state)
                    .size(28.)
                    .strong()
                    .color(if self.snapshot.running {
                        widgets::GREEN
                    } else {
                        ui.visuals().text_color()
                    }),
            );
            ui.label(&self.snapshot.reason);
            if let Some(blocker) = self.start_blocker() {
                ui.add_space(8.);
                widgets::badge(ui, "Before you start", widgets::AMBER);
                ui.label(&blocker.message);
                let label = match blocker.tab {
                    Tab::Calibration => "Open calibration",
                    Tab::Buffs => "Complete buff setup",
                    _ => "View platform & settings",
                };
                if button(
                    ui,
                    if blocker.tab == Tab::Settings {
                        Icon::Settings
                    } else {
                        Icon::Target
                    },
                    label,
                    true,
                    "Open the page related to this condition.",
                )
                .clicked()
                {
                    self.navigate(blocker.tab);
                }
            } else {
                widgets::badge(
                    ui,
                    if self.snapshot.running {
                        "Profile checks passed"
                    } else {
                        "Ready to start"
                    },
                    widgets::GREEN,
                );
            }
            if self.editor.dirty() {
                ui.small("Save & Start applies these edits before starting. F7 uses the currently applied runtime profile.");
            }
        });
    }
    fn run_session(&mut self, ui: &mut egui::Ui) {
        section(ui, "Session", |ui| {
            ui.horizontal_wrapped(|ui| {
                    for (label,value,hint) in [("Active time",metin_core::duration(self.snapshot.statistics.active_seconds),"Active time excludes pauses."),("Targets ended",self.snapshot.statistics.completed.to_string(),"Confirmed target-panel disappearances. This is not independent proof of a kill."),("Clicks",self.snapshot.statistics.clicks.to_string(),"Target-selection clicks sent during this session."),("Unconfirmed",self.snapshot.statistics.failed.to_string(),"Attempts that did not complete confirmation.")] {
                        ui.vertical(|ui| {ui.label(label);ui.label(egui::RichText::new(value).size(24.).strong()).on_hover_text(hint);}); ui.add_space(18.);
                    }
                });
            if button(
                ui,
                Icon::Refresh,
                "Reset statistics",
                self.key().is_some(),
                "Reset session counters without resetting skill timers.",
            )
            .clicked()
                && let Some(key) = self.key()
            {
                self.send(Command::ResetStatistics { key });
            }
        });
    }
    fn run_schedule(&mut self, ui: &mut egui::Ui) {
        section(ui, "Buff schedule", |ui| {
            if !self.editor.profile.buffs.enabled {
                ui.label("Automatic buffs are off.");
            } else if self.editor.dirty() || self.snapshot.key != self.key() || self.key().is_none()
            {
                ui.label("Save and connect this profile to see its applied skill timers.");
            } else {
                for (i, skill) in self.editor.profile.buffs.skills.iter().enumerate() {
                    ui.horizontal_wrapped(|ui| {
                        ui.strong(if skill.name.is_empty() {
                            skill.shortcut.as_str()
                        } else {
                            skill.name.as_str()
                        });
                        ui.label(&skill.shortcut);
                        ui.label(if skill.mode == metin_core::BuffMode::Inactive {
                            "Checked between targets".into()
                        } else {
                            self.snapshot
                                .timers
                                .get(i)
                                .and_then(|v| *v)
                                .map(metin_core::duration)
                                .map_or("Waiting for runtime".into(), |t| {
                                    format!("Next use in {t}")
                                })
                        });
                    });
                }
            }
            if button(
                ui,
                Icon::Buff,
                "Edit buffs",
                true,
                "Configure skill timing, recognition and mount behavior.",
            )
            .clicked()
            {
                self.navigate(Tab::Buffs);
            }
        });
    }
    fn run_connection(&mut self, ui: &mut egui::Ui) {
        section(ui, "Connected profile", |ui| {
            ui.strong(&self.editor.profile.name);
            self.window_picker(ui);
        });
    }
}
