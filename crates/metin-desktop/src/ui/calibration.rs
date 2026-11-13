use super::*;
use widgets::{Icon, button};
mod excluded_areas;
impl MetinApp {
    pub(super) fn calibration_view(&mut self, ui: &mut egui::Ui) {
        if self.editor.is_recalibrating() {
            self.size_review_view(ui);
            return;
        }
        self.size_change_view(ui);
        egui::ScrollArea::vertical().id_salt("calibration-controls").show(ui, |ui| {
            widgets::heading(ui, if self.return_to_safety {"Action capture"} else if self.return_to_buffs {"Buff capture"} else {"Recognition"}, "Select directly over a fresh, frozen game image.");
            ui.separator();
            if self.pending_overlay.is_some() {
                ui.horizontal_wrapped(|ui| { ui.spinner(); ui.label("Selecting in game…"); if widgets::plain_button(ui,"Cancel capture").clicked() { self.cancel_overlay_capture(); } });
            }
            if self.return_to_safety { self.safety_capture_controls(ui); }
            else if self.return_to_buffs { self.buff_capture_controls(ui); }
            else {
                for (number, mode, label, hint) in [
                    (1,EditMode::World,"Search area","Select the part of the game where targets may appear. A fresh frame is captured automatically."),
                    (3,EditMode::Template,"HP panel reference","Select one fixed edge or symbol of the target panel. Avoid names, changing HP and the full variable-width panel. The whole connected game client is always searched.")
                ] {
                    if mode==EditMode::Template { self.metin_detection_controls(ui); ui.separator(); }
                    let ready = self.calibration_ready(mode);
                    let mut edit=false; let mut delete=false;
                    ui.horizontal(|ui| {
                        widgets::readiness_mark(ui, ready);
                        let response=ui.selectable_label(self.editing && self.mode==mode, format!("{number}. {label}"));
                        edit|=response.clicked();
                        crate::ui_controls::help(ui,hint);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            delete=widgets::small_icon_button(ui, Icon::Trash, ready, &format!("Clear {label}. Undo restores it.")).clicked();
                        edit |= widgets::plain_button(ui,if mode==EditMode::Color {"Pick"}else{"Edit"}).clicked();
                        });
                        response
                    }).inner.on_hover_text(hint);
                    if edit {self.edit_area(mode);}
                    if delete {self.delete_area(mode);}
                    if mode==EditMode::Template && let Some(image)=self.editor.template.clone() {
                        self.reference_thumbnail(ui, &image);
                        ui.small(format!("Fixed detail · {} × {} px",image.width,image.height));
                        self.recognition_check_controls(ui, recognition_check::RecognitionSubject::HpPanel);
                    }
                    if mode==EditMode::Template {
                        let mut changed=crate::ui_controls::numeric(ui,"Match threshold","threshold",&mut self.editor.profile.panel.threshold);
                        changed|=crate::ui_controls::numeric_u32(ui,"Stable matches (frames)","stable_frames",&mut self.editor.profile.panel.stable_frames);
                        if changed { self.runtime.stop(); self.editor.record(); }
                    }
                }
                ui.separator(); ui.strong("Excluded areas");
                self.excluded_areas(ui);
                if button(ui,Icon::Plus,"Draw excluded area",self.editor.profile.masks.len()<30,"Draw an excluded rectangle directly over a fresh game image.").clicked() {self.edit_area(EditMode::Mask(None));}
                ui.separator();
                let overlay = ui.vertical(|ui| self.overlay_controls(ui)).response;
                #[cfg(feature = "ui-smoke")]
                if self.overlay_controls_fixture {
                    overlay.scroll_to_me(Some(egui::Align::BOTTOM));
                }
                #[cfg(not(feature = "ui-smoke"))]
                let _ = overlay;
            }
        });
    }
    pub(super) fn overlay_controls(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.run_live, "Game overlay")
            .on_hover_text("Show read-only annotations directly over the connected game on every tab. Turning this off does not disable recognition or automation safety.");
        ui.indent("overlay-options", |ui| {
            ui.add_enabled_ui(self.run_live, |ui| {
                ui.checkbox(&mut self.show_detections, "Detected targets")
                    .on_hover_text("Show outlines for recognized targets, including the current and next candidate. This changes presentation only.");
                ui.checkbox(&mut self.show_regions, "Calibration areas")
                    .on_hover_text("Show the selected calibration area's outline in Recognition. HP references follow their recognized position.");
                ui.checkbox(&mut self.show_sizes, "Sizes (px)")
                    .on_hover_text("Show source-pixel dimensions alongside visible target and calibration outlines.");
            }).response.on_disabled_hover_text("Enable Game overlay to change its visible elements. Your choices are retained while it is off.");
        });
    }
    fn reference_thumbnail(&mut self, ui: &mut egui::Ui, frame: &metin_platform::Frame) {
        if self
            .reference_texture
            .as_ref()
            .is_none_or(|(previous, _)| !Arc::ptr_eq(&previous.bgr, &frame.bgr))
        {
            let rgb = frame
                .bgr
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|pixel| [pixel[2], pixel[1], pixel[0]])
                .collect::<Vec<_>>();
            let texture = ui.ctx().load_texture(
                "hp-reference-thumbnail",
                egui::ColorImage::from_rgb([frame.width as usize, frame.height as usize], &rgb),
                egui::TextureOptions::LINEAR,
            );
            self.reference_texture = Some((frame.clone(), texture));
        }
        if let Some((_, texture)) = &self.reference_texture {
            ui.add(egui::Image::new(texture).max_size(egui::vec2(120., 48.)));
        }
    }
    fn buff_capture_controls(&mut self, ui: &mut egui::Ui) {
        let index = self.selected_skill;
        let Some(skill) = self.editor.profile.buffs.skills.get(index) else {
            return;
        };
        let name = if skill.name.is_empty() {
            format!("Buff {}", index + 1)
        } else {
            skill.name.clone()
        };
        ui.strong(name);
        if button(ui, Icon::Back, "Done & back to buffs", true, "Return to this buff's details. Captures are kept as unsaved profile edits; Save applies them.").clicked() {
            self.return_to_buffs = false;
            self.navigate(Tab::Actions);
            self.actions_page = ActionsPage::Buffs;
            self.actions_detail = true;
            self.buff_detail = true;
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
            }).inner.on_hover_text("Select this capture tool. Show the desired state in the game and choose this capture tool before drawing.");
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
}
