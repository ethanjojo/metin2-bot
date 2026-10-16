use super::*;
use widgets::{Icon, button};
impl MetinApp {
    pub(super) fn calibration_view(&mut self, ui: &mut egui::Ui) {
        if !self.calibration_controls {
            self.preview(ui);
            return;
        }
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
                    ui.heading("Calibration");
                    ui.label("Prepare recognition on a frozen game snapshot."); ui.add_space(8.);
                    ui.label("Profile name");
                    if ui.add(egui::TextEdit::singleline(&mut self.editor.profile.name).hint_text("e.g. Warrior · forest").char_limit(80).desired_width(f32::INFINITY)).on_hover_text("A descriptive name for these saved calibration and buff settings.").changed() {self.runtime.stop();self.editor.record();}
                    self.window_picker(ui);
                    ui.separator();
                    if self.return_to_buffs {
                        if button(ui,Icon::Back,"Back to buffs",true,"Return to the selected skill. Captured areas are kept as profile edits.").clicked() {self.return_to_buffs=false;self.navigate(Tab::Buffs);}
                        ui.strong(match self.mode {EditMode::BuffArea=>"Select the buff detection area",EditMode::BuffIcon{inactive:true,..}=>"Select the inactive icon",_=>"Select the active icon"});
                        ui.label("Drag on the frozen image. Press Escape to cancel a drag. Take a Snapshot if no frozen image is available.");
                    } else {
                        ui.strong("Recognition checklist (4 steps)");
                        for (number,mode,label,hint) in [
                            (1,EditMode::World,"Search area","Drag around the game area where Metins may appear. Exclude interface elements below."),
                            (2,EditMode::Color,"Metin color","Click a representative Metin pixel in the frozen image."),
                            (3,EditMode::Panel,"Target panel","Draw around the target information panel while it is visible."),
                            (4,EditMode::Template,"Reference crop","Draw a small, distinctive fixed part inside the target panel. Avoid changing HP or text.")
                        ] {
                            let ready=self.calibration_ready(mode);
                            let text=format!("{number}. {label}    {}",if ready{"Ready"}else{"Needed"});
                            let response=ui.selectable_label(self.mode==mode,text).on_hover_text(hint);
                            if response.hovered(){self.hover_mode=Some(mode);}
                            if response.clicked(){self.edit_area(mode);}
                            if self.mode==mode {ui.small(hint);}
                        }
                        ui.separator();
                        ui.strong("Excluded areas");
                        if self.editor.profile.masks.is_empty(){ui.small("Optional: hide chat, minimap or other false detections.");}
                        for i in 0..self.editor.profile.masks.len() {
                            let mode=EditMode::Mask(Some(i));
                            let response=ui.selectable_label(self.mode==mode,format!("Excluded area {}",i+1)).on_hover_text("Select this excluded rectangle to move, resize or delete it.");
                            if response.hovered(){self.hover_mode=Some(mode);}
                            if response.clicked(){self.edit_area(mode);}
                        }
                        if button(ui,Icon::Plus,"Excluded area",self.editor.profile.masks.len()<30,"Draw an area that target detection should ignore.").clicked(){self.edit_area(EditMode::Mask(None));}
                    }
                    let can_delete=self.calibration_ready(self.mode);
                    if button(ui,Icon::Trash,"Delete selected",can_delete,if can_delete {"Delete the selected area, sampled color or icon. Undo restores the edit."} else {"Nothing is selected to delete. Select a captured area, sampled color or icon first."}).clicked(){self.delete_selected();}
                    ui.add_space(8.);
                    let ready=[EditMode::World,EditMode::Color,EditMode::Panel,EditMode::Template].iter().all(|m|self.calibration_ready(*m));
                    widgets::badge(ui,if ready{"Recognition areas ready"}else{"Complete the needed areas"},if ready{widgets::GREEN}else{widgets::AMBER});
                    ui.small("Full start readiness also checks the window, buffs and numeric settings.");
                });
            });
            ui.separator();
            ui.allocate_ui_with_layout(egui::vec2(canvas_width,height),egui::Layout::top_down(egui::Align::Min),|ui| {ui.set_width(canvas_width); self.preview(ui);});
        });
    }
}
