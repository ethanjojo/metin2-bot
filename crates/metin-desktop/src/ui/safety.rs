use super::*;
use widgets::section;

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
                            "Safety",
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
        section(ui, "Stuck recovery", |ui| {
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
            ui.small("Attempts reset only after a confirmed target end or a manual Start. Failed recovery or an exhausted limit pauses automation.");
        });

        if changed {
            self.runtime.stop();
            self.editor.record();
        }
    }

    pub(super) fn safety_rules(&mut self, ui: &mut egui::Ui) {
        use widgets::{Icon, button};
        section(ui, "Rules", |ui| {
            ui.small("Capture what to recognize, choose how long it must persist, then choose the reaction. All rules start disabled.");
            if button(ui, Icon::Plus, "Add rule", self.editor.profile.safety.rules.len() < 8, "Create a disabled visual rule. Capture a distinctive reference before enabling it.").clicked() {
                self.runtime.stop();
                self.editor.profile.safety.rules.push(metin_core::SafetyRule::new("New visual event"));
                self.selected_safety = self.editor.profile.safety.rules.len() - 1;
                self.editor.record();
            }
            if self.editor.profile.safety.rules.is_empty() {
                ui.label("No visual rules yet. Use Add rule to create one.");
                return;
            }
            let frame_size = self.editor.profile.frame_size;
            let can_enable = |rule: &metin_core::SafetyRule| {
                rule.ready()
                    && rule.reaction.any()
                    && (!rule.reaction.execute
                        || rule.execution.input.validate(true, frame_size).is_ok())
            };
            let mut remove = None;
            for (index, rule) in self.editor.profile.safety.rules.iter().enumerate() {
                ui.horizontal(|ui| {
                    widgets::readiness_mark(ui, can_enable(rule));
                    if ui
                        .selectable_label(
                            self.selected_safety == index,
                            format!(
                                "{} · {}",
                                rule.name,
                                if rule.enabled { "On" } else { "Off" }
                            ),
                        )
                        .clicked()
                    {
                        self.selected_safety = index;
                        self.snapshot_score = None;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if button(
                            ui,
                            Icon::Trash,
                            "",
                            true,
                            &format!("Delete {}. Undo restores this rule.", rule.name),
                        )
                        .clicked()
                        {
                            remove = Some(index);
                        }
                    });
                });
            }
            if let Some(index) = remove {
                self.runtime.stop();
                self.editor.profile.safety.rules.remove(index);
                self.binding = None;
                self.mode = EditMode::World;
                self.return_to_safety = false;
                self.editor.record();
            }
            if self.editor.profile.safety.rules.is_empty() {
                return;
            }
            self.selected_safety = self
                .selected_safety
                .min(self.editor.profile.safety.rules.len() - 1);
            let index = self.selected_safety;
            let mut changed = false;
            let mut rule = self.editor.profile.safety.rules[index].clone();
            let id = rule.id;
            let reference = rule.reference.clone();
            let recognition = |app: &mut Self,
                               ui: &mut egui::Ui,
                               rule: &mut metin_core::SafetyRule| {
                let mut changed = false;
                let ready = can_enable(rule);
                changed |= ui
                    .add_enabled(
                        ready,
                        egui::Checkbox::new(&mut rule.enabled, "Enable this rule"),
                    )
                    .on_disabled_hover_text(
                        "Capture its reference and configure its input before enabling this rule.",
                    )
                    .changed();
                ui.label("Event name");
                changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut rule.name)
                            .hint_text("e.g. Connection lost")
                            .char_limit(60),
                    )
                    .changed();
                changed |= crate::ui_controls::numeric(
                    ui,
                    "Similarity threshold",
                    "threshold",
                    &mut rule.threshold,
                );
                changed |= crate::ui_controls::numeric(
                    ui,
                    "Must persist for (s)",
                    "safety_duration_seconds",
                    &mut rule.duration_seconds,
                );
                ui.label("Reactions");
                ui.horizontal_wrapped(|ui| {
                    changed |= ui.checkbox(&mut rule.reaction.notify,"Notify").on_hover_text("Notify when this event is recognized, and about action retries or failure. Uses the sound/system notification options in Safety. Event history is always recorded.").changed();
                    changed |= ui.add_enabled(!rule.reaction.execute,egui::Checkbox::new(&mut rule.reaction.pause,"Pause immediately")).on_hover_text("Pause immediately after recognition without performing input.").on_disabled_hover_text("Uncheck Perform action first. To pause after an action, use After completion instead.").changed();
                    changed |= ui.add_enabled(!rule.reaction.pause,egui::Checkbox::new(&mut rule.reaction.execute,"Perform action")).on_hover_text("Release ongoing input, perform the configured key or click, then check the result. Notify is independent.").on_disabled_hover_text("Uncheck Pause immediately first. You can still choose a pause after completing the action.").changed();
                });
                if !rule.reaction.any() {
                    ui.colored_label(
                        widgets::RED,
                        "Choose at least one reaction before enabling this rule.",
                    );
                }
                if rule.reaction.notify
                    && !app.editor.profile.safety.sound
                    && !app.editor.profile.safety.system_notification
                {
                    ui.small("Sound and system notifications are off.");
                    ui.horizontal(|ui| {
                        if ui.small_button("Configure notifications").on_hover_text("Open Safety notifications. This rule's edits are kept; Save applies them.").clicked() {
                            app.navigate(Tab::Safety);app.safety_page=SafetyPage::Notifications;
                        }
                    });
                }
                ui.horizontal_wrapped(|ui| {
                if button(ui,Icon::Camera,"Capture reference",true,"Open Calibration with this rule selected. Capture a distinctive 5–128 px fixed part of the event, inspect it and return manually.").clicked(){app.edit_area(EditMode::SafetyReference(index));}
                if button(ui,Icon::Target,"Test on snapshot",reference.as_ref().is_some_and(|r|r.region.is_some())&&app.editor.frozen.is_some(),"Compare this reference against the current frozen snapshot. Test against both an event frame and a normal frame.").clicked()
                    && let (Some(reference),Some(frame))=(reference.as_ref(),app.editor.frozen.as_ref())
                    && let Some(region)=reference.region {
                        let result=reference.png_bytes().map_err(|e|e.to_string()).and_then(|bytes|metin_vision::decode_png(&bytes).map_err(|e|e.to_string())).and_then(|template|metin_vision::panel_score(frame,&template,region).map_err(|e|e.to_string()));
                        match result {Ok(score)=>app.snapshot_score=Some((id,score)),Err(e)=>app.error=Some(e)}
                    }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {if button(ui,Icon::Trash,"",reference.is_some(),"Clear this reference and disable its rule. Undo restores it.").clicked(){rule.reference = None; rule.enabled = false; changed = true;}});
            });
                if let Some(reference) = reference.clone() {
                    app.icon_thumbnail(ui, &reference);
                } else {
                    ui.small("Reference missing. No client-specific image is assumed.");
                }
                if let Some((tested, score)) = app.snapshot_score
                    && tested == id
                {
                    ui.label(format!("Snapshot match: {score:.3}"));
                }
                if let Some((_, score)) = app
                    .snapshot
                    .safety_scores
                    .iter()
                    .find(|(rule, _)| *rule == id)
                {
                    ui.label(format!("Applied live match: {score:.3}"));
                }
                changed
            };
            let execution = |app: &mut Self,
                             ui: &mut egui::Ui,
                             rule: &mut metin_core::SafetyRule| {
                let mut changed = false;
                if rule.reaction.execute {
                    ui.group(|ui| {
                changed |= app.action_input_editor(ui, &mut rule.execution.input, Binding::Visual(rule.id), EditMode::VisualClick(index));
                ui.label("After input");
                ui.horizontal_wrapped(|ui| {
                changed |= ui.radio_value(&mut rule.execution.verification, metin_core::Verification::ReferenceGone, "Verify reference disappears").on_hover_text("Wait for this rule's reference to disappear. Retry only after the timeout, up to the configured total number of attempts. Exhaustion pauses automation.").changed();
                changed |= ui.radio_value(&mut rule.execution.verification, metin_core::Verification::Delay, "Wait only").on_hover_text("Elapsed delay does not prove that the game action succeeded.").changed();
                });
                changed |= crate::ui_controls::numeric(ui, "Wait after input (s)", "wait_seconds", &mut rule.execution.wait_seconds);
                let timeout_label=if rule.execution.verification==metin_core::Verification::ReferenceGone {"Verification timeout (s)"} else {"Event confirmation timeout (s)"};
                changed |= crate::ui_controls::numeric(ui, timeout_label, "visual_timeout_seconds", &mut rule.execution.timeout_seconds);
                ui.add_enabled_ui(rule.execution.verification == metin_core::Verification::ReferenceGone, |ui| {
                    changed |= crate::ui_controls::numeric_u32(ui, "Maximum attempts", "visual_attempts", &mut rule.execution.attempts);
                });
                ui.label("After completion");
                ui.horizontal_wrapped(|ui| {
                changed |= ui.radio_value(&mut rule.execution.after_success, metin_core::AfterSuccess::Pause, "Pause automation").changed();
                changed |= ui.radio_value(&mut rule.execution.after_success, metin_core::AfterSuccess::Continue, "Continue from a fresh frame").changed();
                });
            });
                }
                if !rule.reaction.execute {
                    ui.small("Choose Perform action to configure a key or click and its result.");
                }
                changed
            };
            if ui.available_width() >= 900. {
                ui.columns(2, |columns| {
                    changed |= recognition(self, &mut columns[0], &mut rule);
                    changed |= execution(self, &mut columns[1], &mut rule);
                });
            } else {
                changed |= recognition(self, ui, &mut rule);
                ui.separator();
                changed |= execution(self, ui, &mut rule);
            }
            if !rule.reaction.any()
                || (rule.reaction.execute
                    && matches!(
                        rule.execution.input,
                        metin_core::ActionInput::Click { region: None }
                    ))
            {
                rule.enabled = false;
            }
            self.editor.profile.safety.rules[index] = rule;
            if changed {
                self.runtime.stop();
                self.editor.record();
            }
        });
        if let Err(error) = self.editor.profile.validate_visual_actions() {
            ui.colored_label(widgets::RED, error.to_string());
        }
    }
    fn notification_settings(&mut self, ui: &mut egui::Ui) {
        section(ui, "Local notifications", |ui| {
            let config = &mut self.editor.profile.safety;
            let mut changed=ui.checkbox(&mut config.sound,"Play a sound").on_hover_text("For triggered rules, automatic stops and runtime errors. Linux sound-only alerts require canberra-gtk-play; unavailable services are reported.").changed();
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
            self.actions_page = if periodic {
                ActionsPage::Periodic
            } else {
                ActionsPage::VisualRules
            };
        }
        if let EditMode::SafetyReference(index) = mode {
            ui.small("Show the event and take a fresh Snapshot. Draw around a distinctive fixed icon or dialog detail (5–128 source pixels).");
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
            ui.small("Show the intended control and take a fresh Snapshot. Draw a small rectangle inside the button. Its center is the click location in the original game pixels.");
            self.click_target_preview(ui, self.editor.region(mode));
        }
    }
    fn schedule_view(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("safety-scroll").show(ui, |ui| {
            widgets::heading(ui, "Safety", "Set limits for this profile. Changes apply on Save.");
            let mut changed = false;
            let config = &mut self.editor.profile.safety;
            section(ui, "Automatic stop", |ui| {
                changed |= ui.checkbox(&mut config.stop_enabled, "Stop after active time")
                    .on_hover_text("Pauses and scheduled breaks do not count. Reset statistics does not reset this limit. Stops even during combat or a held shortcut.").changed();
                ui.add_enabled_ui(config.stop_enabled, |ui| {
                    changed |= minutes(ui, "Active time (minutes)", "stop_after_seconds", &mut config.stop_after_seconds);
                });
                changed |= ui.checkbox(&mut config.clock_enabled, "Stop at local time")
                    .on_hover_text("Uses the next occurrence of this local time when a new run starts, including tomorrow. The fixed deadline continues during pauses and breaks. If both limits are enabled, the earlier one wins.").changed();
                ui.add_enabled_ui(config.clock_enabled, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Local time");
                        let mut hour = config.stop_at_minute / 60;
                        let mut minute = config.stop_at_minute % 60;
                        let h = ui.add(egui::DragValue::new(&mut hour).range(0..=23).min_decimals(0).max_decimals(0));
                        ui.label(":");
                        let m = ui.add(egui::DragValue::new(&mut minute).range(0..=59).min_decimals(0).max_decimals(0));
                        if h.changed() || m.changed() { config.stop_at_minute = hour*60+minute; changed = true; }
                        ui.small(format!("{:02}:{:02}", hour, minute));
                    });
                });
                ui.small("Reaching a limit pauses automation. Press Start or F7 explicitly to begin a new run.");
            });
            section(ui, "Scheduled breaks", |ui| {
                changed |= ui.checkbox(&mut config.breaks_enabled, "Take regular breaks")
                    .on_hover_text("A due break waits until the current target, pickup and settling finish. No keys or clicks are sent during a break. Automatic continuation requires the same game window to remain focused.").changed();
                ui.add_enabled_ui(config.breaks_enabled, |ui| {
                    changed |= minutes(ui, "Active time between breaks (minutes)", "break_every_seconds", &mut config.break_every_seconds);
                    changed |= minutes(ui, "Break duration (minutes)", "break_seconds", &mut config.break_seconds);
                });
                ui.small("Pause or F8 cancels automatic continuation. The next manual Start begins a fresh break interval.");
            });
            if changed { self.runtime.stop(); self.editor.record(); }
            section(ui, "Applied schedule", |ui| {
                if self.editor.dirty() {
                    widgets::badge(ui, "Unsaved changes", widgets::AMBER);
                    ui.label("Save to apply these limits. Saving changed safety settings starts a fresh safety budget.");
                } else if self.snapshot.key != self.key() || self.key().is_none() {
                    ui.label("Select a game window in Calibration to see the applied schedule.");
                } else {
                    schedule_labels(ui, &self.snapshot.schedule, self.snapshot.running);
                }
                ui.small("Statistics and history can be cleared independently; neither changes the applied safety budget.");
            });
        });
    }
}

fn minutes(ui: &mut egui::Ui, label: &str, rule: &str, seconds: &mut f64) -> bool {
    let rule = metin_core::validation::NUMBER_RULES
        .iter()
        .find(|r| r.name == rule)
        .expect("numeric rule");
    let mut value = *seconds / 60.;
    let changed = ui
        .horizontal(|ui| {
            ui.label(label).on_hover_text(format!(
                "Allowed range: {:.1}–{:.0} minutes.",
                rule.low / 60.,
                rule.high / 60.
            ));
            ui.add(
                egui::DragValue::new(&mut value)
                    .range(rule.low / 60. ..=rule.high / 60.)
                    .speed(rule.step / 60.)
                    .max_decimals(2),
            )
            .changed()
        })
        .inner;
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
