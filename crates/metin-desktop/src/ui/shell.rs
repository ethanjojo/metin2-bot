use super::*;
use widgets::Icon;

impl MetinApp {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        // The title and profile row share one header; avoid an extra widget gap.
        let row_spacing = ui.spacing().item_spacing.y;
        ui.spacing_mut().item_spacing.y = 0.;
        let _title = egui::Frame::new()
            .inner_margin(egui::Margin { left: 10, right: 10, top: 4, bottom: 0 })
            .show(ui, |ui| {
                let title = skin::title(ui);
                let content = title.content;
                let control = |index: usize| {
                    egui::Rect::from_center_size(
                        egui::pos2(
                            title.close_rect.center().x - (3 - index) as f32 * 29.,
                            content.center().y,
                        ),
                        egui::vec2(26., 26.),
                    )
                };
                let drag_rect = egui::Rect::from_min_max(
                    content.min,
                    egui::pos2(control(0).left() - 3., content.bottom()),
                );
                let response = ui.interact(
                    drag_rect,
                    ui.make_persistent_id("title-drag"),
                    egui::Sense::DRAG,
                );
                let title_color = egui::Color32::from_rgb(245, 235, 214);
                let galley = ui.painter().layout_no_wrap(
                    crate::PRODUCT_NAME.into(),
                    egui::FontId::proportional(13.),
                    title_color,
                );
                // Center the visible letters, not the font's ascent/descent box.
                // The left ornament occupies the first part of the source cap.
                let position = egui::pos2(
                    content.left() + 24. - galley.mesh_bounds.left(),
                    content.center().y - galley.mesh_bounds.center().y,
                );
                let pixels_per_point = ui.ctx().pixels_per_point();
                let position = egui::pos2(
                    (position.x * pixels_per_point).round() / pixels_per_point,
                    (position.y * pixels_per_point).round() / pixels_per_point,
                );
                ui.painter().galley(position, galley, title_color);
                if response.drag_started() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if icons::header_button(
                    ui,
                    control(0),
                    Icon::Window,
                    false,
                    "Change the connected game window. Automation pauses before the window chooser opens.",
                ).clicked() {
                    self.open_window_chooser();
                }
                if icons::header_button(
                    ui,
                    control(1),
                    Icon::Pin,
                    self.pinned,
                    if self.pinned {
                        "Unpin the panel from the selected game window"
                    } else {
                        "Pin the panel to the selected game window"
                    },
                ).clicked() {
                    self.pinned = !self.pinned;
                }
                if skin::native_control_at(
                    ui,
                    "minimize_button",
                    control(2),
                    if self.collapsed { "Expand panel" } else { "Collapse panel" },
                ).clicked() {
                    self.collapsed = !self.collapsed;
                    if self.collapsed {
                        self.expanded_size = ui.ctx().content_rect().size();
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(320., 42.)));
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(self.expanded_size.x, 42.)));
                    } else {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(400., 480.)));
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(self.expanded_size));
                    }
                }
                #[cfg(windows)]
                self.sync_title_clip(ui.ctx(), self.collapsed.then(|| title.clip(ui.ctx().pixels_per_point())));
                if title.close(ui, "Close Companion").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        ui.spacing_mut().item_spacing.y = row_spacing;
        #[cfg(test)]
        ui.ctx().data_mut(|data| {
            data.insert_temp(egui::Id::unique("shell-header"), _title.response.rect)
        });
        if self.collapsed {
            return;
        }
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: 6,
                bottom: 5,
            })
            .show(ui, |ui| self.profile_select(ui));
    }
    pub(super) fn navigation(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.;
            let width = ui.available_width() / 5.;
            for (tab, label, hint) in [
                (
                    Tab::Run,
                    "Run",
                    "Current state, readiness and session statistics.",
                ),
                (
                    Tab::Actions,
                    "Actions",
                    "Configure buffs, loot and mount behavior.",
                ),
                (
                    Tab::Calibration,
                    "Recognition",
                    "Capture recognition areas directly over the connected game.",
                ),
                (
                    Tab::Safety,
                    "Safety",
                    "Automatic stopping and scheduled breaks.",
                ),
                (
                    Tab::Settings,
                    "Settings",
                    "Recognition tolerances, timing and platform diagnostics.",
                ),
            ] {
                let selected = self.tab == tab;
                let response = widgets::tab(ui, label, width, selected, hint);
                if response.clicked() {
                    self.history_open = false;
                    self.navigate(tab);
                }
            }
        });
    }
    pub(super) fn footer(&mut self, ui: &mut egui::Ui) {
        self.status_bar(ui);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.;
            let busy = self.snapshot.running || self.snapshot.arming;
            let blocker = self.start_blocker();
            let dirty = self.has_unsaved_profile();
            let hint = if busy {
                "Pause immediately and release held inputs. F8 also pauses; while starting it cancels the countdown."
            } else if self.pending_overlay.is_some() {
                "Finish or cancel the selection in the game before starting. F8 cancels capture."
            } else if let Some(blocker) = &blocker {
                blocker.message.as_str()
            } else if dirty {
                "Save all profile edits and start after the three-second countdown. F7 performs the same Save & Start action."
            } else {
                "Start after the three-second countdown. F7 also starts."
            };
            let label = if self.snapshot.arming { "Cancel start" } else if busy { "Pause · F8" }
                else if dirty { "Save & Start" } else { "Start · F7" };
            let size = egui::vec2((ui.available_width() - 52.).max(140.), 44.);
            let response=widgets::sized_button(ui,if busy{Icon::Pause}else{Icon::Play},label,
                size,busy||(blocker.is_none()&&self.pending_overlay.is_none()),false,hint);
            #[cfg(test)]
            ui.ctx().data_mut(|data| data.insert_temp(egui::Id::unique("shell-primary"), response.rect));
            if response.clicked() {
                if busy { self.runtime.stop(); }
                else {
                    if let Err(error) = self.start_profile() { self.error = Some(error); }
                }
            }
            self.footer_mount_switch(ui);
        });
    }
    pub(super) fn footer_mount_switch(&mut self, ui: &mut egui::Ui) {
        let declared = self.declared_mount();
        let connected = self.key().is_some()
            && self.snapshot.key == self.key()
            && self.snapshot.revision == self.revision;
        let reviewing = self.editor.is_recalibrating();
        let enabled = connected && self.pending_overlay.is_none() && !reviewing;
        let hint = if reviewing {
            "Finish or cancel the calibration review before changing the mount state."
        } else if !enabled {
            "Connect a game window first and finish any open capture. Mount confirmation needs no snapshot and sends no game input."
        } else {
            match declared {
                Some(true) => {
                    "Mounted. Click to confirm On foot. Pauses automation; sends no game input."
                }
                Some(false) => {
                    "On foot. Click to confirm Mounted. Pauses automation; sends no game input."
                }
                None => {
                    "Mount state unknown. Choose On foot or Mounted. Pauses automation; sends no game input."
                }
            }
        };
        let response = ui
            .add_enabled_ui(enabled, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(44., 44.), egui::Sense::click());
                // The sprite already has a native slot edge; do not add a second frame.
                catalog::paint(
                    ui,
                    match declared {
                        Some(true) => catalog::Artwork::MountMounted,
                        Some(false) => catalog::Artwork::MountOnFoot,
                        None => catalog::Artwork::MountUnknown,
                    },
                    egui::Rect::from_center_size(rect.center(), egui::vec2(32., 32.)),
                );
                if response.has_focus() {
                    ui.painter().rect_stroke(
                        rect.shrink(1.),
                        0.,
                        egui::Stroke::new(1., widgets::BRONZE),
                        egui::StrokeKind::Inside,
                    );
                }
                response.widget_info(|| {
                    let mut info =
                        egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), hint);
                    info.selected = declared;
                    info
                });
                #[cfg(test)]
                ui.ctx().data_mut(|d| {
                    d.insert_temp(egui::Id::unique(("control", hint)), rect);
                    d.insert_temp(egui::Id::unique("mount-toggle"), rect);
                    d.insert_temp(egui::Id::unique("mount-state"), declared);
                });
                response
            })
            .inner
            .on_hover_text(hint)
            .on_disabled_hover_text(hint);
        if response.clicked()
            && let Some(mounted) = declared
        {
            self.confirm_mount(Some(!mounted));
        }
        if declared.is_none() && enabled {
            egui::Popup::from_toggle_button_response(&response)
                .frame(skin::popup_frame(ui))
                .show(|ui| {
                    ui.strong("Confirm the current state");
                    for (value, label) in [(false, "On foot"), (true, "Mounted")] {
                        if widgets::plain_button(ui, label).clicked() {
                            self.confirm_mount(Some(value));
                            ui.close();
                        }
                    }
                });
        }
    }
    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
        if self.error.is_none() && !self.feedback_current() {
            self.feedback = None;
            return;
        }
        let width = ui.available_width();
        ui.allocate_ui_with_layout(
            egui::vec2(width, 20.),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_width(width);
                ui.set_min_height(20.);
                if let Some(error) = self.error.clone() {
                    let text_width = (width - 58.).max(1.);
                    ui.allocate_ui_with_layout(
                        egui::vec2(text_width, 18.),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_width(text_width);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&error).small().color(widgets::RED),
                                )
                                .truncate(),
                            )
                            .on_hover_text(&error);
                        },
                    );
                    if widgets::plain_button(ui, "Dismiss")
                        .on_hover_text(
                            "Hide this message; resolve the underlying condition before starting.",
                        )
                        .clicked()
                    {
                        self.error = None;
                    }
                } else {
                    self.feedback_view(ui);
                }
            },
        );
    }
    pub(super) fn window_frame(&mut self, ui: &mut egui::Ui) {
        let bounds = ui.ctx().content_rect().shrink(1.);
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::unique("companion-resize"),
        ));
        if !self.collapsed {
            let grip = egui::Rect::from_min_max(
                bounds.right_bottom() - egui::vec2(14., 14.),
                bounds.right_bottom(),
            );
            let response = ui
                .interact(
                    grip,
                    egui::Id::unique("companion-resize"),
                    egui::Sense::drag(),
                )
                .on_hover_cursor(egui::CursorIcon::ResizeNwSe)
                .on_hover_text("Resize panel");
            for offset in [4., 8., 12.] {
                painter.line_segment(
                    [
                        bounds.right_bottom() - egui::vec2(offset, 1.),
                        bounds.right_bottom() - egui::vec2(1., offset),
                    ],
                    egui::Stroke::new(1., widgets::BRONZE),
                );
            }
            if response.drag_started() {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::BeginResize(
                        egui::ResizeDirection::SouthEast,
                    ));
            }
        }
    }
}
