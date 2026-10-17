use super::*;
use widgets::{Icon, button, section};
enum ScheduleLink {
    Event(metin_core::EventDestination, Option<uuid::Uuid>),
    Skill(usize),
    Actions,
}

impl MetinApp {
    pub(super) fn run_view(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("run-scroll")
            .show(ui, |ui| {
                self.run_activity(ui);
                ui.add_space(10.);
                if ui.available_width() >= 900. {
                    let gap = 14.;
                    let width = ui.available_width();
                    let side = (width * 0.31).clamp(270., 360.);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        ui.allocate_ui_with_layout(
                            egui::vec2(width - side - gap, 0.),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                self.run_preview(ui);
                            },
                        );
                        ui.allocate_ui_with_layout(
                            egui::vec2(side, 0.),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                self.run_session(ui);
                                self.run_schedule(ui);
                            },
                        );
                    });
                } else {
                    self.run_preview(ui);
                    self.run_session(ui);
                    self.run_schedule(ui);
                }
                ui.add_space(6.);
                self.run_history(ui);
            });
    }
    fn run_activity(&mut self, ui: &mut egui::Ui) {
        section(ui, "Current activity", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(&self.snapshot.state)
                        .size(28.)
                        .strong()
                        .color(if self.snapshot.schedule.break_ends_in.is_some() {
                            widgets::AMBER
                        } else if self.snapshot.running {
                            widgets::GREEN
                        } else {
                            ui.visuals().text_color()
                        }),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button(format!(
                            "View event history ({})",
                            self.snapshot.events.len()
                        ))
                        .on_hover_text("Open full history in a dialog without moving this page.")
                        .clicked()
                    {
                        self.history_open = true;
                    }
                });
            });
            ui.label(&self.snapshot.reason);
            if let Some(left) = self.snapshot.attack_remaining {
                ui.strong(format!(
                    "Attack key releases in {}",
                    metin_core::duration(left.ceil())
                ));
            }

            if let Some(left) = self.snapshot.schedule.break_ends_in {
                ui.strong(format!(
                    "Continues in {}",
                    metin_core::duration(left.ceil())
                ));
            }
            let steps = if self.snapshot.running {
                Vec::new()
            } else {
                self.run_setup_steps()
                    .into_iter()
                    .filter(|step| step.action != SetupAction::Mount)
                    .collect()
            };
            if !steps.is_empty() {
                ui.add_space(8.);
                widgets::badge(ui, "Before you start", widgets::AMBER);
                for step in steps {
                    ui.horizontal_wrapped(|ui| {
                        ui.strong(step.title);
                        ui.label(&step.detail);
                        let (label, icon) = match step.action {
                            SetupAction::Window => ("Select window", Icon::Camera),
                            SetupAction::Calibration => ("Calibrate", Icon::Target),
                            SetupAction::Mount => ("Confirm mount", Icon::Check),
                            SetupAction::Actions(_) => ("Review actions", Icon::Buff),
                            SetupAction::Safety => ("Review safety", Icon::Settings),
                            SetupAction::VisualRules => ("Review visual rules", Icon::Target),
                            SetupAction::Settings => ("View settings", Icon::Settings),
                        };
                        if button(
                            ui,
                            icon,
                            label,
                            true,
                            "Open the setup step that needs attention.",
                        )
                        .clicked()
                        {
                            self.open_setup(step.action);
                        }
                    });
                }
            } else if self.snapshot.running || self.start_blocker().is_none() {
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
            ui.add_space(6.);
            self.run_mount_state(ui);
            if self.editor.dirty() {
                ui.small("Save & Start applies these edits before starting. F7 uses the currently applied runtime profile.");
            }
        });
    }
    fn run_session(&mut self, ui: &mut egui::Ui) {
        section(ui, "Session", |ui| {
            egui::Grid::new("run-session-statistics")
                .num_columns(2)
                .spacing(egui::vec2(18., 8.))
                .show(ui, |ui| {
                    for (label, value, hint) in [
                        ("Active time", metin_core::duration(self.snapshot.statistics.active_seconds), "Active time excludes pauses."),
                        ("Targets ended", self.snapshot.statistics.completed.to_string(), "Confirmed target-panel disappearances. This is not independent proof of a kill."),
                        ("Clicks", self.snapshot.statistics.clicks.to_string(), "Target-selection clicks sent during this session."),
                        ("Unconfirmed", self.snapshot.statistics.failed.to_string(), "Attempts that did not complete confirmation."),
                    ] {
                        ui.label(label).on_hover_text(hint);
                        ui.label(egui::RichText::new(value).size(20.).strong()).on_hover_text(hint);
                        ui.end_row();
                    }
                });
            ui.add_space(8.);
            if button(
                ui,
                Icon::Refresh,
                "Reset statistics",
                self.key().is_some(),
                "Reset session counters without changing skill timers or safety limits.",
            )
            .clicked()
                && let Some(key) = self.key()
            {
                self.send(Command::ResetStatistics { key });
            }
        });
    }
    fn run_schedule(&mut self, ui: &mut egui::Ui) {
        use metin_core::EventDestination as D;
        let mut open = None;
        section(ui, "Action schedule", |ui| {
            for timer in &self.snapshot.action_timers {
                let puller = timer.key == metin_core::ScheduledKey::Puller;
                let text = match timer.remaining_seconds {
                    None if puller => "Waiting for the next confirmed combat".into(),
                    None => "Waiting for runtime".into(),
                    Some(0.) => "Due in next allowed state".into(),
                    Some(s) => format!(
                        "in {}{}",
                        metin_core::duration(s.ceil()),
                        if puller { "" } else { " of active time" }
                    ),
                };
                ui.push_id(timer.key, |ui| {
                    if schedule_entry(ui, &timer.name, &text) {
                        open = Some(match timer.key {
                            metin_core::ScheduledKey::Puller => {
                                ScheduleLink::Event(D::Puller, None)
                            }
                            metin_core::ScheduledKey::Periodic(id) => {
                                ScheduleLink::Event(D::Periodic, Some(id))
                            }
                        });
                    }
                });
            }
            if self.snapshot.recoveries > 0
                && schedule_entry(
                    ui,
                    "Recovery",
                    &format!("{} attempts", self.snapshot.recoveries),
                )
            {
                open = Some(ScheduleLink::Event(D::Recovery, None));
            }
            if self.snapshot.key == self.key() && self.key().is_some() {
                let schedule = ui
                    .scope(|ui| {
                        if ui
                            .link("Safety schedule")
                            .on_hover_text("Configure breaks and automatic stopping.")
                            .clicked()
                        {
                            open = Some(ScheduleLink::Event(D::Safety, None));
                        }
                        safety::schedule_labels(ui, &self.snapshot.schedule, self.snapshot.running);
                    })
                    .response
                    .interact(egui::Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text("Open Safety > Schedule to adjust breaks and stopping limits.");
                if schedule.clicked() {
                    open = Some(ScheduleLink::Event(D::Safety, None));
                }
                ui.separator();
            }
            if self.editor.profile.loot.enabled
                && schedule_entry(
                    ui,
                    "Loot",
                    &format!(
                        "{} presses of {} after each confirmed target end",
                        self.editor.profile.loot.presses, self.editor.profile.loot.shortcut
                    ),
                )
            {
                open = Some(ScheduleLink::Event(D::Loot, None));
            }
            if !self.editor.profile.buffs.enabled {
                if schedule_entry(ui, "Buffs", "Off") {
                    open = Some(ScheduleLink::Event(D::Buffs, None));
                }
            } else if self.editor.dirty() || self.snapshot.key != self.key() || self.key().is_none()
            {
                if schedule_entry(ui, "Buffs", "Save and connect to see applied skill timers") {
                    open = Some(ScheduleLink::Event(D::Buffs, None));
                }
            } else {
                for (i, skill) in self.editor.profile.buffs.skills.iter().enumerate() {
                    let name = if skill.name.is_empty() {
                        skill.shortcut.as_str()
                    } else {
                        skill.name.as_str()
                    };
                    let time = if skill.mode == metin_core::BuffMode::Inactive {
                        "Checked between targets".into()
                    } else {
                        self.snapshot
                            .timers
                            .get(i)
                            .and_then(|v| *v)
                            .map(metin_core::duration)
                            .map_or("Waiting for runtime".into(), |t| format!("Next use in {t}"))
                    };
                    ui.push_id(("scheduled-skill", i), |ui| {
                        if schedule_entry(ui, name, &format!("{} · {time}", skill.shortcut)) {
                            open = Some(ScheduleLink::Skill(i));
                        }
                    });
                }
            }
            ui.add_space(6.);
            if ui.link("Configure actions").on_hover_text("Open Actions to configure buffs, loot, mount, periodic actions, visual rules and combat helpers.").clicked() {
                open = Some(ScheduleLink::Actions);
            }
        });
        match open {
            Some(ScheduleLink::Event(target, subject)) => self.open_event(target, subject),
            Some(ScheduleLink::Skill(index)) => {
                self.open_setup(SetupAction::Actions(ActionsPage::Buffs));
                self.selected_skill = index;
            }
            Some(ScheduleLink::Actions) => self.navigate(Tab::Actions),
            None => {}
        }
    }
    fn run_history(&mut self, ui: &mut egui::Ui) {
        let mut destination = None;
        section(ui, "Latest events", |ui| {
            if ui.small_button("Show full history").on_hover_text("Open the latest 200 events, warning filter and clear controls in a separate dialog.").clicked() { self.history_open = true; }
            destination = self.event_rows(ui, 5, false);
        });
        if let Some((target, subject)) = destination {
            self.open_event(target, subject);
        }
    }
    pub(super) fn run_history_dialog(&mut self, ctx: &egui::Context) {
        if !self.history_open {
            return;
        }
        let mut destination = None;
        let bounds = ctx.content_rect();
        let response = egui::Modal::new(egui::Id::unique("run-event-history")).show(ctx, |ui| {
            ui.set_width((bounds.width() - 64.).clamp(240., 760.));
            ui.horizontal(|ui| {
                ui.heading("Event history");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if button(ui, Icon::Cross, "", true, "Close history (Escape)").clicked() {
                        ui.close();
                    }
                });
            });
            ui.small(format!(
                "{} · latest 200 events · newest first",
                self.editor.profile.name
            ));
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.warnings_only, "Warnings only")
                    .on_hover_text("Filter this dialog. All events remain in history.");
                if button(
                    ui,
                    Icon::Trash,
                    "Clear history",
                    self.key().is_some() && !self.snapshot.events.is_empty(),
                    "Clear this session's in-memory history. Full events remain in logs/run.log.",
                )
                .clicked()
                    && let Some(key) = self.key()
                {
                    self.send(Command::ClearHistory { key });
                }
            });
            ui.separator();
            let height = (bounds.height() - 220.).clamp(140., 480.);
            egui::ScrollArea::vertical()
                .id_salt((
                    "event-dialog-scroll",
                    self.editor.profile.id,
                    self.target.map(|t| t.handle),
                ))
                .max_height(height)
                .min_scrolled_height(height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    destination = self.event_rows(ui, 200, self.warnings_only);
                });
            ui.separator();
            ui.small("Times since application startup. Full history: logs/run.log.");
            if ui.button("Close history").clicked() {
                ui.close();
            }
        });
        if response.should_close() {
            self.history_open = false;
        }
        if let Some((target, subject)) = destination {
            self.history_open = false;
            self.open_event(target, subject);
        }
    }
    fn event_rows(
        &self,
        ui: &mut egui::Ui,
        limit: usize,
        warnings_only: bool,
    ) -> Option<(metin_core::EventDestination, Option<uuid::Uuid>)> {
        let mut destination = None;
        let mut any = false;
        for event in self
            .snapshot
            .events
            .iter()
            .rev()
            .filter(|event| !warnings_only || event.level == metin_core::EventLevel::Warning)
            .take(limit)
        {
            any = true;
            ui.horizontal_wrapped(|ui| {
                ui.monospace(metin_core::duration(event.at_seconds));
                ui.colored_label(
                    if event.level == metin_core::EventLevel::Warning {
                        widgets::AMBER
                    } else {
                        ui.visuals().text_color()
                    },
                    &event.activity,
                );
                ui.label(&event.message);
                if event.destination != metin_core::EventDestination::Run
                    && ui
                        .small_button("Open setup")
                        .on_hover_text("Open the settings for this event.")
                        .clicked()
                {
                    destination = Some((event.destination, event.subject));
                }
            });
            ui.separator();
        }
        if !any {
            ui.label(if warnings_only {
                "No warnings in this session."
            } else {
                "No events yet. Starts, pauses, target changes and actions appear here."
            });
        }
        destination
    }
    fn open_event(&mut self, target: metin_core::EventDestination, subject: Option<uuid::Uuid>) {
        use metin_core::EventDestination as D;
        match target {
            D::Run => self.navigate(Tab::Run),
            D::Safety => {
                if subject.is_some() {
                    self.navigate(Tab::Actions);
                    self.actions_page = ActionsPage::VisualRules;
                } else {
                    self.navigate(Tab::Safety);
                    self.safety_page = SafetyPage::Schedule;
                }
                if let Some(index) = self
                    .editor
                    .profile
                    .safety
                    .rules
                    .iter()
                    .position(|r| Some(r.id) == subject)
                {
                    self.selected_safety = index;
                }
            }
            D::Calibration => self.navigate(Tab::Calibration),
            D::Recovery => {
                self.navigate(Tab::Safety);
                self.safety_page = SafetyPage::Recovery;
            }
            D::Settings => self.navigate(Tab::Settings),
            _ => {
                self.navigate(Tab::Actions);
                self.actions_page = match target {
                    D::Loot => ActionsPage::Loot,
                    D::Mount => ActionsPage::Mount,
                    D::Puller => ActionsPage::Puller,
                    D::OutOfTarget => ActionsPage::OutOfTarget,
                    D::Periodic => ActionsPage::Periodic,
                    _ => ActionsPage::Buffs,
                };
                if target == D::Periodic {
                    self.selected_periodic = subject;
                }
            }
        }
    }
    fn run_preview(&mut self, ui: &mut egui::Ui) {
        let preview = self.presentation_snapshot();
        section(ui, "Game preview", |ui| {
            ui.checkbox(&mut self.run_live, "Live preview")
                .on_hover_text("Show the selected game window. When paused, turning this off stops preview capture. Active automation continues detection and safety checks at the configured interval.");
            if !self.run_live {
                widgets::badge(ui, "Preview off", ui.visuals().weak_text_color());
                ui.label("Enable Live preview to see game capture and target detections.");
                return;
            }
            let live = preview.key == self.key() && self.key().is_some() && preview.frame.is_some();
            let frame = if live {
                preview.frame.clone()
            } else {
                self.editor.frozen.clone()
            };
            let Some(frame) = frame else {
                ui.label(if self.target.is_some() {
                    "Waiting for the selected game window. If no frame arrives, check capture in Calibration."
                } else {
                    "Select a game window in Calibration to see its live view here."
                });
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
                let stale = preview.captured_at.is_none_or(|t| {
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
            // Leave room for activity, mount controls and preview labels at
            // the common window size; keep the full image within that canvas.
            let max_height = (ui.ctx().content_rect().height() - 480.).clamp(180., 500.);
            let height = (width * frame.height as f32 / frame.width as f32).clamp(180., max_height);
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
                for candidate in &preview.candidates {
                    let current = preview.selected_target == Some(candidate.point);
                    let next = preview.next_target == Some(candidate.point);
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
                if let Some(point) = preview.selected_target {
                    let p = image.min + egui::vec2(point[0] as f32, point[1] as f32) * scale;
                    painter.circle_stroke(p, 7., egui::Stroke::new(2., widgets::AMBER));
                }
            }
        });
    }
}

fn schedule_entry(ui: &mut egui::Ui, name: &str, detail: &str) -> bool {
    ui.horizontal_wrapped(|ui| {
        let clicked = ui
            .link(name)
            .on_hover_text(format!("Open settings for {name}."))
            .clicked();
        ui.label(detail);
        clicked
    })
    .inner
}
