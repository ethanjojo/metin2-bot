use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ui-smoke", derive(clap::ValueEnum))]
pub enum SafetyPage {
    Schedule,
    Recovery,
    Notifications,
}
impl MetinApp {
    pub(super) fn safety_view(&mut self, ui: &mut egui::Ui) {
        if widgets::subtabs(
            ui,
            &mut self.safety_page,
            &[
                (
                    SafetyPage::Schedule,
                    "Schedule",
                    "Automatic stops and scheduled breaks.",
                ),
                (
                    SafetyPage::Recovery,
                    "Recovery",
                    "Limited recovery after exhausted retries.",
                ),
                (
                    SafetyPage::Notifications,
                    "Notifications",
                    "Local sound and system alerts.",
                ),
            ],
        ) {
            self.binding = None;
        }
        match self.safety_page {
            SafetyPage::Schedule => self.schedule_view(ui),
            _ => {
                egui::ScrollArea::vertical()
                    .id_salt(("safety-page", self.safety_page as u8))
                    .show(ui, |ui| {
                        widgets::heading(
                            ui,
                            if self.safety_page == SafetyPage::Recovery {
                                "Recovery"
                            } else {
                                "Notifications"
                            },
                            "Profile-specific protections. Changes apply on Save.",
                        );
                        match self.safety_page {
                            SafetyPage::Recovery => self.recovery_settings(ui),
                            SafetyPage::Notifications => self.notification_settings(ui),
                            _ => {}
                        }
                    });
            }
        }
    }
    fn recovery_settings(&mut self, ui: &mut egui::Ui) {
        let config = &mut self.editor.profile.safety;
        let mut changed = false;
        widgets::first_section(ui, "Stuck recovery", |ui| {
            changed|=ui.checkbox(&mut config.recovery_enabled,"Attempt limited recovery").on_hover_text("After attack, click-confirmation or search retries are exhausted: clear target with Escape, rotate the camera, then check a fresh frame. Never counts recovery as a kill or triggers loot.").changed();
            ui.add_enabled_ui(config.recovery_enabled, |ui| {
                changed |= crate::ui_controls::numeric_u32(
                    ui,
                    "Maximum recovery attempts",
                    "recovery_attempts",
                    &mut config.recovery_attempts,
                );
                changed |= crate::ui_controls::numeric(
                    ui,
                    "Wait after recovery (s)",
                    "wait_seconds",
                    &mut config.recovery_wait_seconds,
                );
            });
        });

        if changed {
            self.runtime.stop();
            self.editor.record();
        }
    }

    pub(super) fn safety_rules(&mut self, ui: &mut egui::Ui) {
        use widgets::{Icon, button};
        if !self.visual_detail {
            widgets::icon_heading(
                ui,
                catalog::Artwork::Camera,
                "Visual rules",
                "Capture an event image and choose its reaction. New rules start disabled; there are no supplied game images.",
            );
            if button(
                ui,
                Icon::Plus,
                "Add rule",
                self.editor.profile.safety.rules.len() < 8,
                "Create a disabled visual rule.",
            )
            .clicked()
            {
                self.runtime.stop();
                self.editor
                    .profile
                    .safety
                    .rules
                    .push(metin_core::SafetyRule::new("New visual event"));
                self.selected_safety = self.editor.profile.safety.rules.len() - 1;
                self.visual_detail = true;
                self.editor.record();
            }
            let mut remove = None;
            for (index, rule) in self.editor.profile.safety.rules.iter().enumerate() {
                ui.horizontal_top(|ui| {
                    let width = (ui.available_width() - 38.).max(80.);
                    ui.allocate_ui_with_layout(
                        egui::vec2(width, 44.),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            if widgets::catalog_row(
                                ui,
                                Icon::Target,
                                &rule.name,
                                if rule.enabled {
                                    "On"
                                } else if rule.ready() {
                                    "Off · reference captured"
                                } else {
                                    "Off · capture a reference"
                                },
                            )
                            .clicked()
                            {
                                self.selected_safety = index;
                                self.visual_detail = true;
                                self.snapshot_score = None;
                            }
                        },
                    );
                    if button(
                        ui,
                        Icon::Trash,
                        "",
                        true,
                        &format!("Delete {}. Undo restores it.", rule.name),
                    )
                    .clicked()
                    {
                        remove = Some(index);
                    }
                });
            }
            if let Some(index) = remove {
                self.runtime.stop();
                self.editor.profile.safety.rules.remove(index);
                self.binding = None;
                self.return_to_safety = false;
                self.editor.record();
            }
            if self.editor.profile.safety.rules.is_empty() {
                ui.small("No visual rules yet. Add a rule to begin.");
            }
            return;
        }
        if self.editor.profile.safety.rules.is_empty() {
            self.visual_detail = false;
            return;
        }
        if button(
            ui,
            Icon::Back,
            "Back to Visual rules",
            true,
            "Return to the rule list without discarding edits.",
        )
        .clicked()
        {
            self.visual_detail = false;
            self.binding = None;
            return;
        }
        self.selected_safety = self
            .selected_safety
            .min(self.editor.profile.safety.rules.len() - 1);
        let index = self.selected_safety;
        let mut rule = self.editor.profile.safety.rules[index].clone();
        let id = rule.id;
        widgets::icon_heading(
            ui,
            catalog::Artwork::Camera,
            &rule.name,
            "Match a captured event, then notify, pause or perform an input. Changes apply on Save.",
        );
        ui.strong("When");
        let mut changed = widgets::field(ui, "Event name", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut rule.name)
                    .char_limit(60)
                    .desired_width(f32::INFINITY),
            )
            .changed()
        });
        ui.horizontal(|ui| {
            if let Some(reference) = &rule.reference {
                self.icon_thumbnail(ui, reference);
            }
            if button(
                ui,
                Icon::Camera,
                "Capture reference",
                true,
                "Select a distinctive fixed 5–128 px event detail over a fresh game image.",
            )
            .clicked()
            {
                self.edit_area(EditMode::SafetyReference(index));
            }
            if widgets::small_icon_button(
                ui,
                Icon::Trash,
                rule.reference.is_some(),
                "Clear this reference and disable the rule. Undo restores it.",
            )
            .clicked()
            {
                rule.reference = None;
                rule.enabled = false;
                changed = true;
            }
        });
        widgets::collapsing_summary(
            ui,
            "Advanced recognition",
            ("visual-recognition", id),
            format!(
                "{:.0}% match · {} s persistence",
                rule.threshold * 100.,
                rule.duration_seconds
            ),
            |ui| {
                changed |= crate::ui_controls::numeric(
                    ui,
                    "Must persist for (s)",
                    "safety_duration_seconds",
                    &mut rule.duration_seconds,
                );
                changed |= crate::ui_controls::numeric(
                    ui,
                    "Similarity threshold",
                    "threshold",
                    &mut rule.threshold,
                );
            },
        );
        let reference = rule.reference.clone();
        if button(ui, Icon::Target, "Test on snapshot", reference.as_ref().is_some_and(|r| r.region.is_some()) && self.editor.frozen.is_some(), "Compare against retained frozen pixels; check both normal and event frames before enabling.").clicked()
            && let (Some(reference), Some(frame)) = (reference.as_ref(), self.editor.frozen.as_ref())
            && let Some(region) = reference.region
        {
            let result = reference.png_bytes().map_err(|e| e.to_string())
                .and_then(|bytes| metin_vision::decode_png(&bytes).map_err(|e| e.to_string()))
                .and_then(|template| metin_vision::region_template_score(frame, &template, region).map_err(|e| e.to_string()));
            match result { Ok(score) => self.snapshot_score = Some((id, score)), Err(error) => self.error = Some(error) }
        }
        if let Some((tested, score)) = self.snapshot_score
            && tested == id
        {
            ui.small(format!("Snapshot match: {score:.3}"));
        }
        if let Some((_, score)) = self
            .snapshot
            .safety_scores
            .iter()
            .find(|(rule, _)| *rule == id)
        {
            ui.small(format!("Applied live match: {score:.3}"));
        }
        ui.strong("Do");
        ui.horizontal_wrapped(|ui| {
            changed|=ui.checkbox(&mut rule.reaction.notify,"Notify").on_hover_text("Use the sound/system notification options in Safety. History is always recorded.").changed();
            changed|=ui.add_enabled(!rule.reaction.execute,egui::Checkbox::new(&mut rule.reaction.pause,"Pause immediately")).on_disabled_hover_text("Uncheck Perform action first. To pause after an action, use After completion.").changed();
            changed|=ui.add_enabled(!rule.reaction.pause,egui::Checkbox::new(&mut rule.reaction.execute,"Perform action")).on_disabled_hover_text("Uncheck Pause immediately first.").changed();
        });
        if !rule.reaction.any() {
            ui.small("Choose at least one reaction to enable.");
        }
        let frame_size = self.editor.profile.frame_size;
        let can_enable = |rule: &metin_core::SafetyRule| {
            rule.ready()
                && rule.reaction.any()
                && (!rule.reaction.execute
                    || rule.execution.input.validate(true, frame_size).is_ok())
        };
        let ready = can_enable(&rule);
        ui.horizontal(|ui| {
            changed |= ui
                .add_enabled(
                    ready,
                    egui::Checkbox::new(&mut rule.enabled, "Enable this rule"),
                )
                .on_disabled_hover_text(
                    "Capture a reference and configure the reaction/input first.",
                )
                .changed();
            if rule.reaction.notify
                && !self.editor.profile.safety.sound
                && !self.editor.profile.safety.system_notification
                && widgets::plain_button(ui, "Configure notifications")
                    .on_hover_text(
                        "Sound and system notifications are off. Open Safety to enable them.",
                    )
                    .clicked()
            {
                self.navigate(Tab::Safety);
                self.safety_page = SafetyPage::Notifications;
            }
        });
        if rule.reference.is_none() {
            ui.small("Capture a reference before enabling.");
        }
        if rule.reaction.execute {
            changed |= self.action_input_editor(
                ui,
                &mut rule.execution.input,
                Binding::Visual(id),
                EditMode::VisualClick(index),
            );
            ui.strong("Then");
            widgets::field(ui, "After input", |ui| {
                changed |= widgets::segmented_choice(
                    ui,
                    ("visual-verification", id),
                    &mut rule.execution.verification,
                    &[
                        (
                            metin_core::Verification::ReferenceGone,
                            "Verify",
                            "Verify that the reference disappears in fresh consecutive frames, within the timeout and maximum attempts.",
                        ),
                        (
                            metin_core::Verification::Delay,
                            "Wait",
                            "Wait after input without verifying its success. Event confirmation is still bounded by the timeout.",
                        ),
                    ],
                );
            });
            changed |= crate::ui_controls::numeric(
                ui,
                "Wait after input (s)",
                "wait_seconds",
                &mut rule.execution.wait_seconds,
            );
            widgets::field(ui, "After completion", |ui| {
                changed |= widgets::segmented_choice(
                    ui,
                    ("visual-completion", id),
                    &mut rule.execution.after_success,
                    &[
                        (
                            metin_core::AfterSuccess::Continue,
                            "Continue",
                            "Continue automation from a fresh frame after the action finishes.",
                        ),
                        (
                            metin_core::AfterSuccess::Pause,
                            "Pause",
                            "Pause automation after the action finishes. This is independent of Notify.",
                        ),
                    ],
                );
            });
            let execution_summary =
                if rule.execution.verification == metin_core::Verification::ReferenceGone {
                    format!(
                        "{} s timeout · {} attempts",
                        rule.execution.timeout_seconds, rule.execution.attempts
                    )
                } else {
                    format!("{} s event timeout", rule.execution.timeout_seconds)
                };
            widgets::collapsing_summary(
                ui,
                "Advanced execution",
                ("visual-execution", id),
                &execution_summary,
                |ui| {
                    let label =
                        if rule.execution.verification == metin_core::Verification::ReferenceGone {
                            "Verification timeout (s)"
                        } else {
                            "Event confirmation timeout (s)"
                        };
                    changed |= crate::ui_controls::numeric(
                        ui,
                        label,
                        "visual_timeout_seconds",
                        &mut rule.execution.timeout_seconds,
                    );
                    ui.add_enabled_ui(
                        rule.execution.verification == metin_core::Verification::ReferenceGone,
                        |ui| {
                            changed |= crate::ui_controls::numeric_u32(
                                ui,
                                "Maximum attempts",
                                "visual_attempts",
                                &mut rule.execution.attempts,
                            );
                        },
                    );
                    crate::ui_controls::help(
                        ui,
                        "Reference disappearance is verified with fresh consecutive frames and bounded attempts. Wait only is an unverified delay. Pause/F8, focus or geometry changes cancel execution.",
                    );
                },
            );
        }
        if !can_enable(&rule) {
            rule.enabled = false;
        }
        self.editor.profile.safety.rules[index] = rule;
        if changed {
            self.runtime.stop();
            self.editor.record();
        }
        if let Err(error) = self.editor.profile.validate_visual_actions() {
            ui.colored_label(widgets::RED, error.to_string());
        }
    }
    fn notification_settings(&mut self, ui: &mut egui::Ui) {
        widgets::first_section(ui, "Local notifications", |ui| {
            let config = &mut self.editor.profile.safety;
            let mut changed=ui.checkbox(&mut config.sound,"Play a sound").on_hover_text("For triggered rules, automatic stops and runtime errors. Unavailable services are reported.").changed();
            changed|=ui.checkbox(&mut config.system_notification,"Show a system notification").on_hover_text("Local OS notification. Delivery depends on desktop permissions and Do Not Disturb. No external messaging service is contacted.").changed();
            let (sound, system) = (config.sound, config.system_notification);
            if changed {
                self.runtime.stop();
                self.editor.record();
            }
            if widgets::button(
                ui,
                widgets::Icon::Play,
                "Test notification",
                sound || system,
                "Send one local test using the selected options. No game input is sent.",
            )
            .clicked()
            {
                self.send(Command::TestNotification { sound, system });
            }
            ui.small(&self.snapshot.notification_status);
        });
    }
    pub(super) fn safety_capture_controls(&mut self, ui: &mut egui::Ui) {
        let mode = self.mode;
        let periodic = matches!(mode, EditMode::PeriodicClick(_));
        let name = match mode {
            EditMode::PeriodicClick(id) => self
                .editor
                .profile
                .periodic_actions
                .iter()
                .find(|a| a.id == id)
                .map(|a| a.name.clone()),
            EditMode::SafetyReference(index) | EditMode::VisualClick(index) => self
                .editor
                .profile
                .safety
                .rules
                .get(index)
                .map(|r| r.name.clone()),
            _ => None,
        };
        ui.strong(name.unwrap_or_else(|| "Action capture".into()));
        if widgets::button(
            ui,
            widgets::Icon::Back,
            "Done & back to Actions",
            true,
            "Keep this capture as an unsaved edit and return to the action. Save applies it.",
        )
        .clicked()
        {
            self.return_to_safety = false;
            self.navigate(Tab::Actions);
            self.actions_detail = true;
            self.actions_page = if periodic {
                ActionsPage::Periodic
            } else {
                ActionsPage::VisualRules
            };
            if let EditMode::PeriodicClick(id) = mode {
                self.selected_periodic = Some(id);
            } else {
                self.visual_detail = true;
            }
        }
        if let EditMode::SafetyReference(index) = mode {
            ui.small("Choose the capture tool, then select a distinctive fixed event detail over a fresh game image (5–128 source pixels).");
            if let Some(reference) = self
                .editor
                .profile
                .safety
                .rules
                .get(index)
                .and_then(|r| r.reference.clone())
            {
                self.icon_thumbnail(ui, &reference);
                ui.label("Captured · profile edit");
            } else {
                ui.label("Not captured yet");
            }
        } else {
            ui.small("Show the intended control, choose Capture and select a small rectangle inside the button. Its center is the click location in the original game pixels.");
            self.click_target_preview(ui, self.editor.region(mode));
        }
    }
    fn schedule_view(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("safety-scroll").show(ui,|ui| {
            widgets::heading(ui,"Schedule","Set per-profile limits. Saving stop/break changes starts a fresh budget. Clearing statistics or history does not reset it.");
            let mut changed=false;
            let config=&mut self.editor.profile.safety;
            changed|=ui.checkbox(&mut config.stop_enabled,"Stop after active time").on_hover_text("Counts active work only. Interrupts held input immediately, including during combat.").changed();
            ui.add_enabled_ui(config.stop_enabled,|ui|{changed|=minutes(ui,"Active time (minutes)","stop_after_seconds",&mut config.stop_after_seconds);});
            changed|=ui.checkbox(&mut config.clock_enabled,"Stop at local time").on_hover_text("Uses the next occurrence of this local time, including tomorrow. The fixed deadline continues during pauses and breaks.").changed();
            ui.add_enabled_ui(config.clock_enabled,|ui| {
                widgets::field(ui,"Local time",|ui| {
                    let mut hour=config.stop_at_minute/60;let mut minute=config.stop_at_minute%60;
                    let m=ui.add(egui::DragValue::new(&mut minute).range(0..=59).min_decimals(0).max_decimals(0));ui.label(":");
                    let h=ui.add(egui::DragValue::new(&mut hour).range(0..=23).min_decimals(0).max_decimals(0));
                    if h.changed()||m.changed(){config.stop_at_minute=hour*60+minute;changed=true;}
                });
            });
            ui.separator();
            changed|=ui.checkbox(&mut config.breaks_enabled,"Take regular breaks").on_hover_text("A due break waits for the current target, loot and settling. Keep the game focused for automatic continuation; Pause/F8 cancels it.").changed();
            ui.add_enabled_ui(config.breaks_enabled,|ui| {
                changed|=minutes(ui,"Break every (minutes)","break_every_seconds",&mut config.break_every_seconds);
                changed|=minutes(ui,"Break duration (minutes)","break_seconds",&mut config.break_seconds);
            });
            if changed{self.runtime.stop();self.editor.record();}
            ui.small("Live countdowns are in Run. Save to apply schedule edits.");
        });
    }
}

fn minutes(ui: &mut egui::Ui, label: &str, rule: &str, seconds: &mut f64) -> bool {
    let rule = metin_core::validation::NUMBER_RULES
        .iter()
        .find(|r| r.name == rule)
        .expect("numeric rule");
    let mut value = *seconds / 60.;
    let changed = widgets::field(ui, label, |ui| {
        ui.add(
            egui::DragValue::new(&mut value)
                .range(rule.low / 60. ..=rule.high / 60.)
                .speed(rule.step / 60.)
                .max_decimals(2),
        )
        .on_hover_text(format!(
            "Allowed range: {:.1}–{:.0} minutes.",
            rule.low / 60.,
            rule.high / 60.
        ))
        .changed()
    });
    if changed {
        *seconds = value * 60.;
    }
    changed
}

pub(super) fn schedule_labels(
    ui: &mut egui::Ui,
    schedule: &metin_core::ScheduleSnapshot,
    running: bool,
) {
    if let Some(left) = schedule.break_ends_in {
        ui.colored_label(
            widgets::AMBER,
            format!("Break ends in {}", metin_core::duration(left.ceil())),
        );
    } else if let Some(left) = schedule.break_remaining {
        ui.label(if left == 0. {
            "Break due — waiting for the current cycle to finish".into()
        } else {
            format!(
                "Next break in {} of active time",
                metin_core::duration(left.ceil())
            )
        });
    }
    if let Some(left) = schedule.stop_remaining {
        ui.label(format!(
            "Automatic stop in {} of active time",
            metin_core::duration(left.ceil())
        ));
    }
    if let Some(left) = schedule.clock_remaining {
        ui.label(format!(
            "Local-time stop in {}",
            metin_core::duration(left.ceil())
        ));
    } else if let Some(minute) = schedule.clock_at_minute {
        ui.label(format!(
            "Local-time stop: {:02}:{:02}; deadline set on Start",
            minute / 60,
            minute % 60
        ));
    }
    if schedule.stop_remaining.is_none()
        && schedule.clock_remaining.is_none()
        && schedule.clock_at_minute.is_none()
        && schedule.break_remaining.is_none()
    {
        ui.label("No automatic stop or breaks configured.");
    } else if !running {
        ui.small("Paused — active-time countdowns are frozen.");
    }
}
