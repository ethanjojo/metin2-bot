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
                        self.run_session(&mut cols[0]);
                        self.run_schedule(&mut cols[0]);
                        self.run_preview(&mut cols[1]);
                    });
                } else {
                    self.run_activity(ui);
                    self.run_preview(ui);
                    self.run_session(ui);
                    self.run_schedule(ui);
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
                    if self.snapshot.arming {
                        "Start requested: Pause or F8 cancels"
                    } else if self.snapshot.running {
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
    fn run_preview(&mut self, ui: &mut egui::Ui) {
        section(ui, "Game preview", |ui| {
            let live = self.snapshot.key == self.key()
                && self.key().is_some()
                && self.snapshot.frame.is_some();
            let frame = if live {
                self.snapshot.frame.clone()
            } else {
                self.editor.frozen.clone()
            };
            let Some(frame) = frame else {
                ui.label("No game frame yet. Select a game window in Calibration to see its live view here.");
                if button(
                    ui,
                    Icon::Camera,
                    "Set up game view",
                    true,
                    "Open Calibration to select the game window and check capture.",
                )
                .clicked()
                {
                    self.navigate(Tab::Calibration);
                }
                return;
            };
            if live {
                let stale = self.snapshot.captured_at.is_none_or(|t| {
                    t.elapsed().as_secs_f64() > self.editor.profile.timing.sample_interval * 2. + 1.
                });
                widgets::badge(
                    ui,
                    if stale {
                        "Waiting for a fresh game frame"
                    } else {
                        "Live selected-window view"
                    },
                    if stale {
                        widgets::AMBER
                    } else {
                        widgets::GREEN
                    },
                );
            } else {
                ui.small("Calibration snapshot - not live");
            }
            ui.small("Read-only view. Amber: current target. Green: next eligible target. Blue: other detections.");
            if self.editor.dirty() {
                ui.small("The live overlay uses the applied profile. Save to apply your edits.");
            }
            self.update_frame_texture(ui.ctx(), &frame);
            let width = ui.available_width();
            let height = (width * frame.height as f32 / frame.width as f32).clamp(180., 420.);
            let (canvas, _) =
                ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
            let scale =
                (canvas.width() / frame.width as f32).min(canvas.height() / frame.height as f32);
            let size = egui::vec2(frame.width as f32 * scale, frame.height as f32 * scale);
            let image = egui::Rect::from_center_size(canvas.center(), size);
            let painter = ui.painter_at(canvas);
            painter.rect_filled(canvas, 4., ui.visuals().extreme_bg_color);
            if let Some(texture) = &self.texture {
                painter.image(
                    texture.id(),
                    image,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                    egui::Color32::WHITE,
                );
            }
            if live {
                for candidate in &self.snapshot.candidates {
                    let current = self.snapshot.selected_target == Some(candidate.point);
                    let next = self.snapshot.next_target == Some(candidate.point);
                    let color = if current {
                        widgets::AMBER
                    } else if next {
                        widgets::GREEN
                    } else {
                        widgets::BLUE
                    };
                    let rect = egui::Rect::from_min_size(
                        image.min + egui::vec2(candidate.x as f32, candidate.y as f32) * scale,
                        egui::vec2(candidate.width as f32, candidate.height as f32) * scale,
                    );
                    painter.rect_stroke(
                        rect,
                        0.,
                        egui::Stroke::new(if current || next { 3. } else { 1.5 }, color),
                        egui::StrokeKind::Inside,
                    );
                }
                // A selected target may no longer pass color detection during
                // combat. Its actual acquisition point is still meaningful.
                if let Some(point) = self.snapshot.selected_target {
                    let p = image.min + egui::vec2(point[0] as f32, point[1] as f32) * scale;
                    painter.circle_stroke(p, 7., egui::Stroke::new(2., widgets::AMBER));
                }
            }
        });
    }
}
