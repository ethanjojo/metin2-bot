use super::*;
use widgets::{Icon, button};
impl MetinApp {
    pub(super) fn calibration_view(&mut self, ui: &mut egui::Ui) {
        let height = ui.available_height();
        let side = if ui.available_width() < 950. {
            250.
        } else {
            290.
        };
        let canvas_width = (ui.available_width() - side - 24.).max(100.);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(side,height),egui::Layout::top_down(egui::Align::Min),|ui| {
                ui.set_max_width(side);
                egui::ScrollArea::vertical().id_salt("calibration-controls").show(ui,|ui| {
                    ui.heading(if self.return_to_buffs { "Buff capture" } else { "Calibration" });
                    ui.label("Prepare recognition on a frozen game snapshot."); ui.add_space(8.);
                    if !self.return_to_buffs {
                    ui.label("Profile name");
                    if ui.add(egui::TextEdit::singleline(&mut self.editor.profile.name).hint_text("e.g. Warrior · forest").char_limit(80).desired_width(f32::INFINITY)).on_hover_text("A descriptive name for these saved calibration and buff settings.").changed() {self.runtime.stop();self.editor.record();}
                    }
                    self.window_picker(ui);
                    ui.separator();
                    if self.return_to_buffs {
                        self.buff_capture_controls(ui);
                    } else {
                        ui.strong("Recognition checklist (4 steps)");
                        for (number,mode,label,hint) in [
                            (1,EditMode::World,"Search area","Drag around the game area where Metins may appear. Exclude interface elements below."),
                            (2,EditMode::Color,"Metin color","Click a representative Metin pixel in the frozen image."),
                            (3,EditMode::Panel,"Target panel","Draw around the target information panel while it is visible."),
                            (4,EditMode::Template,"Reference crop","Draw a small, distinctive fixed part inside the target panel. Avoid changing HP or text.")
                        ] {
                            let ready=self.calibration_ready(mode);
                            let mut delete=false;
                            let response=ui.horizontal(|ui| {
                                widgets::readiness_mark(ui,ready);
                                let response=ui.selectable_label(self.editing && self.mode==mode,format!("{number}. {label}"));
                                crate::ui_controls::help(ui,hint);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    delete=widgets::small_icon_button(ui,Icon::Trash,ready,&format!("Clear {label}. Undo restores the edit.")).clicked();
                                });
                                response
                            }).inner.on_hover_text(hint);
                            if response.hovered(){self.hover_mode=Some(mode);}
                            if response.clicked(){self.edit_area(mode);}
                            if delete {self.delete_area(mode);}
                            if mode==EditMode::Color {self.color_control(ui);}
                        }
                        ui.separator();
                        ui.strong("Excluded areas");
                        if self.editor.profile.masks.is_empty(){ui.small("Optional: hide chat, minimap or other false detections.");}
                        for i in 0..self.editor.profile.masks.len() {
                            let mode=EditMode::Mask(Some(i));
                            let mut delete=false;
                            let response=ui.horizontal(|ui| {
                                let response=ui.selectable_label(self.editing && self.mode==mode,format!("Excluded area {}",i+1));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    delete=widgets::small_icon_button(ui,Icon::Trash,true,&format!("Delete excluded area {}. Undo restores it.",i+1)).clicked();
                                });
                                response
                            }).inner.on_hover_text("Select this excluded rectangle to move or resize it. Use its trash icon to delete it.");
                            if response.hovered(){self.hover_mode=Some(mode);}
                            if response.clicked(){self.edit_area(mode);}
                            if delete {self.delete_area(mode);break;}
                        }
                        if button(ui,Icon::Plus,"Draw excluded area",self.editor.profile.masks.len()<30,"Draw an area that target detection should ignore.").clicked(){self.edit_area(EditMode::Mask(None));}
                    }
                    ui.add_space(8.);
                    let ready=if self.return_to_buffs {
                        self.calibration_ready(EditMode::BuffArea) && self.calibration_ready(EditMode::BuffIcon { index: self.selected_skill, inactive: false })
                    } else {
                        [EditMode::World,EditMode::Color,EditMode::Panel,EditMode::Template].iter().all(|m|self.calibration_ready(*m))
                    };
                    let status=if self.return_to_buffs { if ready {"Skill recognition ready"} else {"Set the detection area and active icon"} } else if ready {"Recognition areas ready"} else {"Complete the needed areas"};
                    widgets::badge(ui,status,if ready{widgets::GREEN}else{widgets::AMBER});
                    ui.small("Full start readiness also checks the window, buffs and numeric settings.");
                });
            });
            ui.separator();
            ui.allocate_ui_with_layout(egui::vec2(canvas_width,height),egui::Layout::top_down(egui::Align::Min),|ui| {ui.set_width(canvas_width); self.preview(ui);});
        });
    }
    fn buff_capture_controls(&mut self, ui: &mut egui::Ui) {
        let index = self.selected_skill;
        let Some(skill) = self.editor.profile.buffs.skills.get(index) else {
            return;
        };
        let name = if skill.name.is_empty() {
            format!("Skill {}", index + 1)
        } else {
            skill.name.clone()
        };
        ui.strong(name);
        if button(ui, Icon::Back, "Done & back to buffs", true, "Return to this skill's details. Captures are kept as unsaved profile edits; Save applies them.").clicked() {
            self.return_to_buffs = false;
            self.navigate(Tab::Buffs);
        }
        for (mode, label) in [
            (EditMode::BuffArea, "Detection area"),
            (
                EditMode::BuffIcon {
                    index,
                    inactive: false,
                },
                "Active icon",
            ),
            (
                EditMode::BuffIcon {
                    index,
                    inactive: true,
                },
                "Inactive icon (optional)",
            ),
        ] {
            let ready = self.calibration_ready(mode);
            let mut delete = false;
            let response = ui.horizontal(|ui| {
                widgets::readiness_mark(ui, ready);
                let response = ui.selectable_label(self.mode == mode, label);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    delete = widgets::small_icon_button(ui, Icon::Trash, ready, &format!("Clear {label}. Undo restores it.")).clicked();
                });
                response
            }).inner.on_hover_text("Select this capture tool. Show the desired state in the game and take a fresh Snapshot before drawing.");
            if response.clicked() {
                self.edit_area(mode);
            }
            if delete {
                self.delete_area(mode);
            }
        }
        ui.separator();
        let icon = match self.mode {
            EditMode::BuffIcon { index, inactive } => self
                .editor
                .profile
                .buffs
                .skills
                .get(index)
                .and_then(|skill| {
                    if inactive {
                        skill.inactive_icon.clone()
                    } else {
                        skill.icon.clone()
                    }
                }),
            _ => None,
        };
        ui.horizontal(|ui| {
            if let Some(icon) = icon { self.icon_thumbnail(ui, &icon); }
            ui.label(if self.calibration_ready(self.mode) { "Captured · profile edit" } else { "Not captured yet" });
            crate::ui_controls::help(ui, "Drag on the frozen image to capture or replace the selection. The preview updates after capture. Return to Buffs when satisfied, then Save the profile.");
        });
    }
    fn color_control(&mut self, ui: &mut egui::Ui) {
        let old = self.editor.profile.color.as_ref().map(|c| c.hsv);
        let hsv = old.unwrap_or([0, 200, 200]);
        let mut picker = crate::ui_controls::sampled_hsv_to_srgb(hsv);
        ui.horizontal_wrapped(|ui| {
            let response=ui.color_edit_button_srgb(&mut picker).on_hover_text("View or adjust the Metin color. You can also select the color tool and sample a frozen pixel.");
            if response.changed() {
                let value=crate::ui_controls::sampled_srgb_to_hsv(picker);
                if old!=Some(value) {self.runtime.stop();self.editor.profile.color.get_or_insert_with(Default::default).hsv=value;self.editor.record();}
            }
            ui.small(old.map_or("Color not set".into(),|v|format!("HSV {}, {}, {}",v[0],v[1],v[2])));
        });
    }
}
