use super::*;
use metin_core::ExcludedArea;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct Rename {
    index: usize,
    original: ExcludedArea,
    name: String,
    focus: bool,
    error: Option<String>,
}

#[derive(Clone)]
struct Highlight {
    area: ExcludedArea,
    until: Instant,
}

impl MetinApp {
    fn exclusion_state_id(&self, kind: &str) -> egui::Id {
        egui::Id::unique((kind, self.editor.profile.id))
    }

    pub(in crate::ui) fn highlighted_exclusion(
        &self,
        context: &egui::Context,
    ) -> Option<(metin_core::Region, String)> {
        let highlight = context.data(|data| {
            data.get_temp::<Highlight>(self.exclusion_state_id("excluded-highlight"))
        })?;
        (highlight.until > Instant::now()
            && self.tab == Tab::Calibration
            && self.editor.profile.masks.contains(&highlight.area))
        .then_some((highlight.area.region, highlight.area.name))
    }

    pub(in crate::ui) fn show_excluded_area(
        &mut self,
        context: &egui::Context,
        area: ExcludedArea,
    ) {
        context.data_mut(|data| {
            data.insert_temp(
                self.exclusion_state_id("excluded-highlight"),
                Highlight {
                    area,
                    until: Instant::now() + Duration::from_secs(3),
                },
            )
        });
        self.overlay_frame_id = 0;
        context.request_repaint_after(Duration::from_secs(3));
    }

    pub(super) fn excluded_areas(&mut self, ui: &mut egui::Ui) {
        let id = self.exclusion_state_id("excluded-rename");
        let mut rename = ui.ctx().data(|data| data.get_temp::<Rename>(id));
        // An undo, import or another edit must not retarget an open rename draft.
        if rename
            .as_ref()
            .is_some_and(|edit| self.editor.profile.masks.get(edit.index) != Some(&edit.original))
        {
            rename = None;
        }
        let can_edit = self.pending_overlay.is_none() && !self.editor.is_recalibrating();
        let can_show = self.run_live && self.key().is_some() && self.pending_overlay.is_none();
        let mut removed = None;
        for index in 0..self.editor.profile.masks.len() {
            let area = self.editor.profile.masks[index].clone();
            let mut begin_rename = false;
            let mut confirm = false;
            let mut cancel = false;
            let mut edit_area = false;
            let mut show = false;
            ui.push_id(("excluded-area", index), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.;
                    let editing = rename.as_ref().is_some_and(|edit| edit.index == index);
                    let controls_width = if editing { 60. } else {
                        3. * 26. + 4. * 4. + 38. + ui.painter().layout_no_wrap("Edit".into(), egui::FontId::proportional(13.), egui::Color32::WHITE).size().x
                    };
                    let width = (ui.available_width() - controls_width).max(50.);
                    if editing {
                        let draft = rename.as_mut().expect("editing rename");
                        let input = ui.add_sized([width, 28.], egui::TextEdit::singleline(&mut draft.name)
                            .char_limit(80).hint_text("Area name").id(egui::Id::unique(("excluded-name", self.editor.profile.id, index))));
                        if draft.focus { input.request_focus(); draft.focus = false; }
                        if input.changed() { draft.error = None; }
                        let mut candidate = area.clone();
                        candidate.name = draft.name.trim().to_owned();
                        let valid = candidate.validate().is_ok();
                        confirm = widgets::small_icon_button(ui, Icon::Check, can_edit && valid, "Save excluded area name (Enter)")
                            .on_disabled_hover_text("Enter 1–80 characters without control characters.").clicked();
                        cancel = widgets::small_icon_button(ui, Icon::Cross, true, "Cancel excluded area rename (Escape)").clicked();
                        if input.has_focus() || input.lost_focus() {
                            confirm |= valid && can_edit && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                            cancel |= ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
                        }
                    } else {
                        let response = ui.add_sized([width, 28.], egui::Button::selectable(self.mode == EditMode::Mask(Some(index)), &area.name).truncate());
                        if response.clicked() {
                            self.mode = EditMode::Mask(Some(index));
                            show = can_show;
                        }
                        response.on_hover_text("Select this area. Use Edit to move or resize it over a frozen game image.");
                        begin_rename = widgets::small_icon_button(ui, Icon::Edit, can_edit, &format!("Rename excluded area: {}", area.name)).clicked();
                        show |= widgets::small_icon_button(ui, Icon::Target, can_show, &format!("Show {} in game for 3 seconds", area.name))
                            .on_disabled_hover_text("Connect a game window and enable Game overlay first. No game input is sent.").clicked();
                        edit_area = widgets::button(ui, Icon::Edit, "Edit", can_edit, &format!("Move or resize {} over a frozen game image", area.name)).clicked();
                        if widgets::small_icon_button(ui, Icon::Trash, can_edit, "Delete this excluded area. Undo restores it.").clicked() { removed = Some(index); }
                    }
                });
            });
            if begin_rename {
                rename = Some(Rename {
                    index,
                    original: area.clone(),
                    name: area.name.clone(),
                    focus: true,
                    error: None,
                });
            }
            if show {
                self.show_excluded_area(ui.ctx(), area.clone());
            }
            if edit_area {
                rename = None;
                self.edit_area(EditMode::Mask(Some(index)));
            }
            if cancel {
                rename = None;
            } else if confirm {
                let draft = rename.as_ref().expect("rename confirmation");
                match self.editor.rename_excluded_area(index, &draft.name) {
                    Ok(()) => {
                        self.runtime.stop();
                        rename = None;
                        self.overlay_frame_id = 0;
                        self.show_feedback("Area renamed", true);
                    }
                    Err(error) => {
                        if let Some(draft) = &mut rename {
                            draft.error = Some(error);
                        }
                    }
                }
            }
            if let Some(error) = rename
                .as_ref()
                .filter(|edit| edit.index == index)
                .and_then(|edit| edit.error.as_ref())
            {
                ui.label(egui::RichText::new(error).small().color(widgets::RED));
            }
        }
        if let Some(index) = removed {
            rename = None;
            self.delete_area(EditMode::Mask(Some(index)));
        }
        ui.ctx().data_mut(|data| {
            if let Some(rename) = rename {
                data.insert_temp(id, rename);
            } else {
                data.remove::<Rename>(id);
            }
        });
    }
}
