use super::*;
use widgets::{Icon, button};
enum ScheduleLink {
    Event(metin_core::EventDestination, Option<uuid::Uuid>),
    Skill(usize),
    Actions,
}

impl MetinApp {
    pub(super) fn run_view(&mut self, ui: &mut egui::Ui) {
        if self.history_open {
            self.run_history_page(ui);
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("run-scroll")
            .show(ui, |ui| {
                self.run_activity(ui);
                self.run_session(ui);
                self.run_schedule(ui);
                self.run_history(ui);
            });
    }
    /// A runtime slot belongs to its applied session, never a draft or another profile.
    pub(super) fn active_skill_presentation(&self) -> Option<usize> {
        (self.snapshot.activity == metin_core::ActivityKind::Buffing
            && self.snapshot.key.is_some()
            && self.snapshot.key == self.key()
            && self.snapshot.revision == self.revision
            && !self.editor.dirty())
        .then_some(self.snapshot.active_skill)
        .flatten()
        .filter(|index| *index < self.editor.profile.buffs.skills.len())
    }
    fn run_activity(&mut self, ui: &mut egui::Ui) {
        let tone = if self.snapshot.activity == metin_core::ActivityKind::Error {
            widgets::RED
        } else if self.snapshot.schedule.break_ends_in.is_some() || self.snapshot.arming {
            widgets::AMBER
        } else if self.snapshot.running {
            widgets::GREEN
        } else {
            widgets::BRONZE
        };
        let icon = if let Some(index) = self.active_skill_presentation() {
            skill_icon::glyph(
                ui,
                &self.editor.profile.buffs.skills[index],
                &mut self.icon_textures,
            )
        } else if self.snapshot.activity == metin_core::ActivityKind::Stopped
            && self.start_blocker().is_none()
        {
            catalog::Artwork::Ready.into()
        } else {
            activity_icon(self.snapshot.activity)
        };
        widgets::panel(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 3.;
            ui.spacing_mut().interact_size.y = 20.;
            ui.spacing_mut().button_padding = egui::vec2(6., 3.);
            ui.horizontal(|ui| {
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(48., 40.), egui::Sense::hover());
                ui.painter()
                    .circle_filled(rect.center(), 19., tone.gamma_multiply(0.12));
                ui.painter()
                    .circle_stroke(rect.center(), 19., egui::Stroke::new(1.2, tone));
                icon.paint(
                    ui,
                    egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(32.)),
                );
                response.on_hover_text(&self.snapshot.reason);
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 40.),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.spacing_mut().item_spacing.y = 2.;
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&self.snapshot.state)
                                    .size(25.)
                                    .strong()
                                    .color(tone),
                            )
                            .truncate(),
                        );
                        let countdown = self
                            .snapshot
                            .schedule
                            .break_ends_in
                            .map(|left| {
                                if self.snapshot.running {
                                    format!("Continues in {}", metin_core::duration(left.ceil()))
                                } else {
                                    format!(
                                        "Break remaining {} · paused",
                                        metin_core::duration(left.ceil())
                                    )
                                }
                            })
                            .or_else(|| {
                                self.snapshot.attack_remaining.map(|left| {
                                    format!(
                                        "Attack key releases in {}",
                                        metin_core::duration(left.ceil())
                                    )
                                })
                            });
                        let subtitle = countdown.as_deref().unwrap_or(&self.snapshot.reason);
                        ui.add(
                            egui::Label::new(egui::RichText::new(subtitle).size(12.)).truncate(),
                        )
                        .on_hover_text(&self.snapshot.reason);
                    },
                );
            });
            ui.horizontal(|ui| {
                if !self.snapshot.running && !self.snapshot.arming && self.start_blocker().is_none() {
                    widgets::badge(ui, "Ready", widgets::GREEN);
                } else if self.snapshot.schedule.break_ends_in.is_none() && self.snapshot.attack_remaining.is_none() {
                    let targets = self.snapshot.candidates.len();
                    ui.label(egui::RichText::new(format!("{targets} found")).small().color(ui.visuals().weak_text_color()))
                        .on_hover_text("Candidates recognized in the current frame. Colored outlines are shown directly over the game window.");
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut self.run_live, "Game overlay").on_hover_text("Show detection outlines directly over the connected game. This never starts automation; safety checks remain active when hidden.");
                });
            });
            if let Some(progress) = &self.snapshot.activity_progress {
                widgets::timer_bar(
                    ui,
                    &progress.label,
                    progress.elapsed_seconds,
                    progress.remaining_seconds,
                    progress.total_seconds,
                );
            } else {
                widgets::timer_bar(ui, "", 0., None, None);
            }
        });
        let steps = if self.snapshot.running || self.snapshot.arming {
            Vec::new()
        } else {
            self.run_setup_steps()
                .into_iter()
                .filter(|step| step.action != SetupAction::Mount)
                .collect::<Vec<_>>()
        };
        if !steps.is_empty() {
            ui.add_space(6.);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Finish setup")
                        .small()
                        .strong()
                        .color(widgets::AMBER),
                );
                ui.label(
                    egui::RichText::new(format!("{} remaining", steps.len()))
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            });
            for step in steps {
                let (icon, label) = match step.action {
                    SetupAction::Window => (Icon::Window, "Choose game window".to_owned()),
                    SetupAction::Calibration => (Icon::Target, "Complete recognition".to_owned()),
                    SetupAction::Mount => (Icon::Mount, "Confirm mount below".to_owned()),
                    SetupAction::Actions(_) => {
                        (Icon::Buff, format!("Review {}", step.title.to_lowercase()))
                    }
                    SetupAction::Safety => (Icon::Settings, "Review safety".to_owned()),
                    SetupAction::VisualRules => (Icon::Target, "Review visual rules".to_owned()),
                    SetupAction::Settings => (Icon::Settings, "Check platform support".to_owned()),
                };
                if setup_entry(ui, icon, &label, &step.detail) {
                    self.open_setup(step.action);
                }
            }
        }
    }
    fn run_session(&mut self, ui: &mut egui::Ui) {
        let can_reset = self.key().is_some()
            && self.snapshot.key == self.key()
            && self.snapshot.revision == self.revision
            && self.pending_overlay.is_none();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Session").small().strong().color(ui.visuals().weak_text_color()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::compact_icon_button(ui, Icon::Refresh, can_reset, "Reset session: pause, release held inputs, clear statistics and history, and make regular buffs due on next Start. Safety limits and scheduled actions keep their current values. Requires a connected session and no open capture.").clicked()
                    && let Some(key) = self.key() { self.send(Command::ResetSession { key }); }
            });
        });
        let values = [
            (
                Icon::Clock,
                metin_core::duration(self.snapshot.statistics.active_seconds),
                "Active time",
                "Active running time; excludes pauses and scheduled breaks.",
            ),
            (
                Icon::Target,
                self.snapshot.statistics.completed.to_string(),
                "Targets ended",
                "Confirmed target-panel disappearances; not independent proof of a kill.",
            ),
            (
                Icon::Sword,
                self.snapshot.statistics.clicks.to_string(),
                "Clicks",
                "Target-selection clicks sent in this session.",
            ),
            (
                Icon::Cross,
                self.snapshot.statistics.failed.to_string(),
                "Unconfirmed",
                "Target attempts that failed confirmation.",
            ),
        ];
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 46.), egui::Sense::hover());
        for (i, (_icon, value, label, hint)) in values.into_iter().enumerate() {
            let tile = egui::Rect::from_min_max(
                egui::pos2(rect.left() + i as f32 * width / 4., rect.top()),
                egui::pos2(rect.left() + (i + 1) as f32 * width / 4., rect.bottom()),
            );
            let response = ui.interact(
                tile,
                ui.scope_id().with(("session-stat", i)),
                egui::Sense::hover(),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Label, true, format!("{label}: {value}"))
            });
            response.on_hover_text(hint);
            if i > 0 {
                ui.painter().vline(
                    tile.left(),
                    (tile.top() + 10.)..=(tile.bottom() - 10.),
                    egui::Stroke::new(1., ui.visuals().widgets.noninteractive.bg_stroke.color),
                );
            }
            ui.painter().text(
                egui::pos2(tile.left() + 9., tile.top() + 2.),
                egui::Align2::LEFT_TOP,
                value,
                egui::FontId::proportional(if i == 0 { 17. } else { 20. }),
                ui.visuals().text_color(),
            );
            ui.painter().text(
                egui::pos2(tile.left() + 9., tile.bottom() - 2.),
                egui::Align2::LEFT_BOTTOM,
                label,
                egui::FontId::proportional(10.),
                ui.visuals().weak_text_color(),
            );
        }
    }
    fn run_schedule(&mut self, ui: &mut egui::Ui) {
        use metin_core::EventDestination as D;
        let mut open = None;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 3.;
            ui.spacing_mut().interact_size.y = 18.;
            if widgets::compact_heading(
                ui,
                "Next actions",
                Some(("Configure actions", "Open all action settings.")),
            )
            .is_some_and(|response| response.clicked())
            {
                open = Some(ScheduleLink::Actions);
            }
            for timer in &self.snapshot.action_timers {
                let puller = timer.key == metin_core::ScheduledKey::Puller;
                let text = match timer.remaining_seconds {
                    None if puller => "Next combat".into(),
                    None => "On Start".into(),
                    Some(0.) => "Due · when allowed".into(),
                    Some(s) => format!(
                        "in {}{}",
                        metin_core::duration(s.ceil()),
                        if puller { "" } else { " active" }
                    ),
                };
                ui.push_id(timer.key, |ui| {
                    if schedule_entry(
                        ui,
                        if puller {
                            catalog::Artwork::Puller.into()
                        } else {
                            Icon::Clock.into()
                        },
                        &timer.name,
                        &text,
                    ) {
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
                    catalog::Artwork::Recover.into(),
                    "Recovery",
                    &format!("{} attempts", self.snapshot.recoveries),
                )
            {
                open = Some(ScheduleLink::Event(D::Recovery, None));
            }
            if self.snapshot.key == self.key() && self.key().is_some() {
                let row = ui.scope(|ui| {
                    schedule_entry(
                        ui,
                        Icon::Clock.into(),
                        "Safety schedule",
                        &schedule_summary(&self.snapshot.schedule),
                    )
                });
                if row.inner {
                    open = Some(ScheduleLink::Event(D::Safety, None));
                }
                row.response.on_hover_ui(|ui| {
                    safety::schedule_labels(ui, &self.snapshot.schedule, self.snapshot.running)
                });
            }

            if self.editor.profile.loot.enabled
                && schedule_entry(
                    ui,
                    catalog::Artwork::Loot.into(),
                    "Loot",
                    &format!(
                        "{} × {} after target",
                        self.editor.profile.loot.presses, self.editor.profile.loot.shortcut
                    ),
                )
            {
                open = Some(ScheduleLink::Event(D::Loot, None));
            }
            if !self.editor.profile.buffs.enabled {
                if schedule_entry(ui, catalog::Artwork::Skills.into(), "Buffs", "Off") {
                    open = Some(ScheduleLink::Event(D::Buffs, None));
                }
            } else if self.editor.dirty() || self.snapshot.key != self.key() || self.key().is_none()
            {
                if schedule_entry(
                    ui,
                    catalog::Artwork::Skills.into(),
                    "Buffs",
                    "Save to apply skill timers",
                ) {
                    open = Some(ScheduleLink::Event(D::Buffs, None));
                }
            } else {
                for (i, skill) in self.editor.profile.buffs.skills.iter().enumerate() {
                    let name = if skill.name.is_empty() {
                        skill.shortcut.as_str()
                    } else {
                        skill.name.as_str()
                    };
                    let (time, fraction) = self.skill_timing(i);
                    let glyph = skill_icon::glyph(ui, skill, &mut self.icon_textures);
                    ui.push_id(("scheduled-skill", i), |ui| {
                        let top = ui.cursor().min;
                        let width = ui.available_width();
                        if schedule_entry(ui, glyph, name, &format!("{} · {time}", skill.shortcut))
                        {
                            open = Some(ScheduleLink::Skill(i));
                        }
                        if skill.mode == metin_core::BuffMode::Refresh {
                            widgets::progress_track(
                                ui,
                                egui::Rect::from_min_size(
                                    top + egui::vec2(27., 20.),
                                    egui::vec2((width - 31.).max(1.), 2.),
                                ),
                                fraction,
                            );
                        }
                    });
                }
            }
        });
        match open {
            Some(ScheduleLink::Event(target, subject)) => self.open_event(target, subject),
            Some(ScheduleLink::Skill(index)) => {
                self.open_setup(SetupAction::Actions(ActionsPage::Buffs));
                self.selected_skill = index;
                self.buff_detail = true;
            }
            Some(ScheduleLink::Actions) => self.navigate(Tab::Actions),
            None => {}
        }
    }
    fn run_history(&mut self, ui: &mut egui::Ui) {
        let mut destination = None;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 3.;
            ui.spacing_mut().interact_size.y = 18.;
            ui.add_space(3.);
            ui.horizontal(|ui| {
                let (icon_rect, _) =
                    ui.allocate_exact_size(egui::vec2(16., 18.), egui::Sense::hover());
                widgets::icon(
                    ui.painter(),
                    icon_rect.center(),
                    Icon::History,
                    widgets::BRONZE,
                );
                ui.label(
                    egui::RichText::new("Recent activity")
                        .small()
                        .strong()
                        .color(ui.visuals().weak_text_color()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::compact_text_button(ui, "History")
                        .on_hover_text("Open full event history, filters and event settings.")
                        .clicked()
                    {
                        self.history_open = true;
                    }
                });
            });
            destination = self.event_rows(ui, 3, false, true);
        });

        if let Some((target, subject)) = destination {
            self.open_event(target, subject);
        }
    }
    pub(super) fn run_history_page(&mut self, ui: &mut egui::Ui) {
        if button(
            ui,
            Icon::Back,
            "Back to Run",
            true,
            "Close history (Escape)",
        )
        .clicked()
            || ui.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.history_open = false;
            return;
        }
        widgets::heading(ui, "Event history", "Latest 200 events · newest first");
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.warnings_only, "Warnings only")
                .on_hover_text("Filter this view. All events remain in history.");
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
        ui.small("Times since application startup. Full history: logs/run.log.");
        ui.separator();
        let history_width = ui.available_width() - ui.spacing().scroll.allocated_width();
        let destination = egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .id_salt((
                "event-history",
                self.editor.profile.id,
                self.target.map(|t| t.handle),
            ))
            .show(ui, |ui| {
                ui.set_width(history_width);
                self.event_rows(ui, 200, self.warnings_only, false)
            })
            .inner;
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
        compact: bool,
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
            if compact {
                let row = ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(metin_core::duration(event.at_seconds))
                            .monospace()
                            .size(11.)
                            .color(ui.visuals().weak_text_color()),
                    );
                    let text = egui::RichText::new(&event.message).size(12.).color(
                        if event.level == metin_core::EventLevel::Warning {
                            widgets::AMBER
                        } else {
                            ui.visuals().text_color()
                        },
                    );
                    let response = ui.add(egui::Label::new(text).truncate().sense(
                        if event.destination == metin_core::EventDestination::Run {
                            egui::Sense::hover()
                        } else {
                            egui::Sense::click()
                        },
                    ));
                    if response.clicked() && event.destination != metin_core::EventDestination::Run
                    {
                        destination = Some((event.destination, event.subject));
                    }
                });
                row.response
                    .on_hover_text(format!("{} · {}", event.activity, event.message));
            } else {
                ui.horizontal(|ui| {
                    ui.monospace(metin_core::duration(event.at_seconds));
                    let linked = event.destination != metin_core::EventDestination::Run;
                    let title_width =
                        (ui.available_width() - if linked { 94. } else { 0. }).max(1.);
                    ui.add_sized(
                        egui::vec2(title_width, 24.),
                        egui::Label::new(egui::RichText::new(&event.activity).color(
                            if event.level == metin_core::EventLevel::Warning {
                                widgets::AMBER
                            } else {
                                ui.visuals().text_color()
                            },
                        ))
                        .truncate(),
                    )
                    .on_hover_text(&event.activity);
                    if linked
                        && widgets::plain_button(ui, "Open setup")
                            .on_hover_text("Open the settings for this event.")
                            .clicked()
                    {
                        destination = Some((event.destination, event.subject));
                    }
                });
                ui.add(egui::Label::new(&event.message).wrap());
            }

            if !compact {
                ui.separator();
            }
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
                    self.actions_detail = true;
                    self.actions_page = ActionsPage::VisualRules;
                    self.visual_detail = true;
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
                self.actions_detail = true;
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
}

fn schedule_summary(schedule: &metin_core::ScheduleSnapshot) -> String {
    let choices = [
        schedule.break_ends_in.map(|time| (time, "Break ends")),
        schedule.break_remaining.map(|time| (time, "Break")),
        schedule.stop_remaining.map(|time| (time, "Stop")),
        schedule.clock_remaining.map(|time| (time, "Local stop")),
    ];
    choices
        .into_iter()
        .flatten()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or_else(
            || {
                if schedule.clock_at_minute.is_some() {
                    "On Start".into()
                } else {
                    "Off".into()
                }
            },
            |(time, label)| format!("{label} in {}", metin_core::duration(time.ceil())),
        )
}

fn activity_icon(kind: metin_core::ActivityKind) -> widgets::Glyph {
    use catalog::Artwork as A;
    use metin_core::ActivityKind as K;
    match kind {
        K::Stopped => Icon::Play.into(),
        K::Paused => Icon::Pause.into(),
        K::Error => Icon::Cross.into(),
        K::Searching | K::VisualRule => A::Camera.into(),
        K::Confirming => Icon::Target.into(),
        K::Attacking => A::Attack.into(),
        K::OutOfTarget => A::AutoAttack.into(),
        K::Looting => A::Loot.into(),
        K::Buffing => A::Skills.into(),
        K::Puller => A::Puller.into(),
        K::Periodic => Icon::Clock.into(),
        K::Starting => Icon::Play.into(),
        K::Waiting | K::Break => A::Wait.into(),
        K::Recovering => A::Recover.into(),
    }
}
fn setup_entry(ui: &mut egui::Ui, icon: Icon, title: &str, detail: &str) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 29.), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, true, format!("{title}: {detail}"))
    });
    let visuals = ui.style().interact(&response);
    let painter = ui.painter_at(rect);
    if response.hovered() || response.has_focus() {
        painter.rect(
            rect,
            3.,
            visuals.bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
    }
    widgets::icon(
        &painter,
        rect.left_center() + egui::vec2(13., 0.),
        icon,
        widgets::AMBER,
    );
    row_text(
        &painter,
        egui::Rect::from_min_max(
            rect.min + egui::vec2(29., 0.),
            rect.max - egui::vec2(30., 0.),
        ),
        title,
        13.,
        ui.visuals().text_color(),
        false,
    );
    widgets::icon(
        &painter,
        rect.right_center() - egui::vec2(13., 0.),
        Icon::Redo,
        ui.visuals().weak_text_color(),
    );
    response
        .on_hover_text(detail)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}
fn schedule_entry(ui: &mut egui::Ui, glyph: widgets::Glyph, name: &str, detail: &str) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.), egui::Sense::click());
    let hint = format!(
        "{name}: {detail}. Open settings. Timers count active running time unless labelled otherwise."
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &hint));
    let painter = ui.painter_at(rect);
    if response.hovered() || response.has_focus() {
        let visuals = ui.style().interact(&response);
        painter.rect(
            rect,
            3.,
            visuals.bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
    }
    glyph.paint(
        ui,
        egui::Rect::from_center_size(
            rect.left_center() + egui::vec2(12., 0.),
            egui::Vec2::splat(20.),
        ),
    );
    let name_rect = egui::Rect::from_min_max(
        rect.min + egui::vec2(27., 0.),
        egui::pos2(rect.left() + rect.width() * 0.47, rect.bottom()),
    );
    row_text(
        &painter,
        name_rect,
        name,
        13.,
        ui.visuals().text_color(),
        false,
    );
    let value_rect = egui::Rect::from_min_max(
        egui::pos2(name_rect.right() + 4., rect.top()),
        rect.max - egui::vec2(5., 0.),
    );
    row_text(
        &painter,
        value_rect,
        detail,
        12.,
        ui.visuals().weak_text_color(),
        true,
    );
    response
        .on_hover_text(hint)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}
fn row_text(
    painter: &egui::Painter,
    rect: egui::Rect,
    text: &str,
    size: f32,
    color: egui::Color32,
    right: bool,
) {
    let mut job = egui::text::LayoutJob::simple_singleline(
        text.into(),
        egui::FontId::proportional(size),
        color,
    );
    job.wrap.max_width = rect.width().max(1.);
    job.wrap.max_rows = 1;
    let galley = painter.layout_job(job);
    let x = if right {
        rect.right() - galley.size().x
    } else {
        rect.left()
    };
    painter.with_clip_rect(rect).galley(
        egui::pos2(x, rect.center().y - galley.size().y / 2.),
        galley,
        color,
    );
}
