//! Reusable detection settings are copied into the main profile on explicit Apply.
use super::*;
use crate::{
    storage::MetinPreset,
    ui_controls::{numeric, numeric_u32},
};
use metin_core::ColorConfig;
use uuid::Uuid;
use widgets::Icon;

pub(super) struct PresetUi {
    pub catalog: Vec<MetinPreset>,
    pub draft: Option<PresetDraft>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DraftKind {
    Custom,
    Named(Uuid),
    New,
}

pub(super) struct PresetDraft {
    pub profile: Uuid,
    pub change_id: u64,
    pub target: Option<TargetToken>,
    pub kind: DraftKind,
    pub name: String,
    pub color: ColorConfig,
    pub sampled_size: Option<[u32; 2]>,
    pub error: Option<String>,
    delete_confirmation: bool,
}

impl PresetUi {
    pub fn new(catalog: Vec<MetinPreset>) -> Self {
        Self {
            catalog,
            draft: None,
        }
    }
    pub fn active(&self, profile: &Profile) -> Option<&MetinPreset> {
        self.catalog.iter().find(|preset| {
            Some(preset.id) == profile.color_preset_id
                && profile.color.as_ref() == Some(&preset.color)
        })
    }
}

impl MetinApp {
    pub(super) fn commit_preset_color(
        &mut self,
        capture: crate::editor::CaptureEdit,
        region: Option<metin_core::Region>,
        point: [u32; 2],
    ) -> Result<(), String> {
        let draft = self
            .metin_presets
            .draft
            .as_mut()
            .ok_or("The Metin detection editor has closed.")?;
        if draft.profile != self.editor.profile.id
            || draft.change_id != self.editor.change_id()
            || draft.target != self.target
        {
            return Err("The profile or game window changed during selection.".into());
        }
        let mut sampled = Editor::new(self.editor.profile.clone(), self.editor.template.clone());
        capture.commit(&mut sampled, region, point)?;
        draft.color.hsv = sampled
            .profile
            .color
            .as_ref()
            .ok_or("No color was sampled.")?
            .hsv;
        draft.sampled_size = sampled.profile.frame_size;
        draft.error = None;
        self.overlay_frame_id = 0;
        self.overlay_preview = None;
        Ok(())
    }
    pub(super) fn open_metin_preset(&mut self, new: bool) {
        self.close_recognition_check();
        self.cancel_overlay_capture();
        self.runtime.stop();
        self.binding = None;
        let active = self.metin_presets.active(&self.editor.profile);
        self.metin_presets.draft = Some(PresetDraft {
            profile: self.editor.profile.id,
            change_id: self.editor.change_id(),
            target: self.target,
            kind: if new {
                DraftKind::New
            } else {
                active.map_or(DraftKind::Custom, |preset| DraftKind::Named(preset.id))
            },
            name: if new {
                String::new()
            } else {
                active.map_or_else(String::new, |preset| preset.name.clone())
            },
            color: self.editor.profile.color.clone().unwrap_or_default(),
            sampled_size: None,
            error: None,
            delete_confirmation: false,
        });
        self.overlay_frame_id = 0;
    }

    pub(super) fn apply_metin_preset(&mut self, draft: &PresetDraft) -> Result<(), String> {
        if draft.profile != self.editor.profile.id
            || draft.change_id != self.editor.change_id()
            || draft.target != self.target
        {
            return Err("The profile changed. Cancel this editor and open it again.".into());
        }
        draft.color.validate().map_err(|error| error.to_string())?;
        let provenance = match draft.kind {
            DraftKind::Custom => None,
            DraftKind::New | DraftKind::Named(_) => {
                let id = match draft.kind {
                    DraftKind::Named(id) => Some(id),
                    _ => None,
                };
                let saved = self
                    .store
                    .save_metin_preset(id, &draft.name, &draft.color)
                    .map_err(|error| error.to_string())?;
                if let Some(previous) = self
                    .metin_presets
                    .catalog
                    .iter_mut()
                    .find(|p| p.id == saved.id)
                {
                    *previous = saved.clone();
                } else {
                    self.metin_presets.catalog.push(saved.clone());
                }
                Some(saved.id)
            }
        };
        self.runtime.stop();
        self.editor.profile.color = Some(draft.color.clone());
        self.editor.profile.color_preset_id = provenance;
        if self.editor.profile.frame_size.is_none() && draft.sampled_size.is_some() {
            self.editor.profile.frame_size = draft.sampled_size;
        }
        self.editor.record();
        self.overlay_preview = None;
        self.overlay_frame_id = 0;
        Ok(())
    }

    fn select_metin_preset(&mut self, id: Uuid) {
        let Some(preset) = self.metin_presets.catalog.iter().find(|p| p.id == id) else {
            return;
        };
        self.runtime.stop();
        self.editor.profile.color = Some(preset.color.clone());
        self.editor.profile.color_preset_id = Some(id);
        self.editor.record();
        self.overlay_frame_id = 0;
    }

    pub(super) fn metin_detection_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            widgets::readiness_mark(ui, self.editor.profile.color.is_some());
            if ui.selectable_label(self.editing && self.mode == EditMode::Color, "2. Metin detection").on_hover_text("Pick a representative target pixel directly in the frozen game image, or use the preset selector and pencil below.").clicked() { self.edit_area(EditMode::Color); }
            crate::ui_controls::help(ui, "Choose a saved detection preset or pick a target color. The pencil edits color, tolerances, shape limits and stable frames together. Each main profile retains its own copy; Save applies profile changes.");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::small_icon_button(ui, Icon::Trash, self.editor.profile.color.is_some(), "Clear this profile's Metin detection color. Saved presets remain available.").clicked() { self.delete_area(EditMode::Color); }
                if widgets::plain_button(ui, "Pick").on_hover_text("Sample a target pixel over a fresh frozen game image. This creates a custom profile edit.").clicked() { self.edit_area(EditMode::Color); }
            });
        });
        let active = self
            .metin_presets
            .active(&self.editor.profile)
            .map(|p| p.id);
        let label = self.metin_presets.active(&self.editor.profile).map_or_else(
            || {
                if self.editor.profile.color.is_some() {
                    "Custom"
                } else {
                    "Not configured"
                }
                .to_owned()
            },
            |p| p.name.clone(),
        );
        let mut selected = None;
        let mut custom = false;
        ui.horizontal(|ui| {
            color_swatch(ui, self.editor.profile.color.as_ref());
            let width = (ui.available_width() - 64.).max(80.);
            widgets::ComboBox::from_id_salt("metin-preset-select")
                .width(width)
                .selected_text(label)
                .show_ui(ui, |ui| {
                    if select::option_row(ui, active.is_none(), "Custom · current settings")
                        .on_hover_text(
                            "Keep this profile's values and detach the saved preset name.",
                        )
                        .clicked()
                    {
                        custom = true;
                        ui.close();
                    }
                    for preset in &self.metin_presets.catalog {
                        ui.horizontal(|ui| {
                            color_swatch(ui, Some(&preset.color));
                            if select::option_row(ui, active == Some(preset.id), &preset.name)
                                .clicked()
                            {
                                selected = Some(preset.id);
                                ui.close();
                            }
                        });
                    }
                    if self.metin_presets.catalog.is_empty() {
                        ui.small("No saved presets yet. Use + to create one.");
                    }
                });
            if widgets::small_icon_button(
                ui,
                Icon::Edit,
                true,
                "Edit this detection preset or the current custom settings.",
            )
            .clicked()
            {
                self.open_metin_preset(false);
            }
            if widgets::small_icon_button(
                ui,
                Icon::Plus,
                self.metin_presets.catalog.len() < 128,
                "Create a named Metin detection preset from the current settings.",
            )
            .clicked()
            {
                self.open_metin_preset(true);
            }
        });
        if let Some(id) = selected {
            self.select_metin_preset(id);
        }
        if custom && self.editor.profile.color_preset_id.take().is_some() {
            self.runtime.stop();
            self.editor.record();
        }
    }

    pub(super) fn metin_preset_dialog(&mut self, ctx: &egui::Context) {
        if self.pending_overlay.is_some() {
            return;
        }
        let Some(mut draft) = self.metin_presets.draft.take() else {
            return;
        };
        let previous_color = draft.color.clone();
        let mut apply = false;
        let mut cancel = false;
        let mut pick = false;
        let mut delete = false;
        let width = (ctx.content_rect().width() - 36.).clamp(220., 420.);
        let body_height = (ctx.content_rect().height() - 210.).max(80.);
        let frame = egui::Frame::new()
            .fill(ctx.global_style().visuals.window_fill)
            .stroke(egui::Stroke::new(1., widgets::BRONZE))
            .inner_margin(10.);
        let modal = egui::Modal::new(egui::Id::unique("metin-preset-editor")).frame(frame).show(ctx, |ui| {
            ui.set_width(width);
            ui.strong(match draft.kind { DraftKind::Custom => "Custom Metin detection", DraftKind::Named(_) => "Edit Metin preset", DraftKind::New => "New Metin preset" });
            ui.separator();
            select::menu_scroll(ui, "metin-preset-body", body_height, |ui| {
                if draft.kind != DraftKind::Custom {
                    widgets::field(ui, "Name", |ui| { ui.add(egui::TextEdit::singleline(&mut draft.name).char_limit(80).desired_width(ui.available_width()).hint_text("Metin preset name")); });
                }
                ui.horizontal(|ui| {
                    ui.label("Color");
                    let mut rgb = crate::ui_controls::sampled_hsv_to_srgb(draft.color.hsv);
                    if ui.color_edit_button_srgb(&mut rgb).changed() { draft.color.hsv = crate::ui_controls::sampled_srgb_to_hsv(rgb); }
                    pick = widgets::button(ui, Icon::Target, "Pick in game", self.target.is_some(), "Sample a pixel directly in the game. The result stays in this draft until Apply.").clicked();
                });
                ui.small(format!("HSV {}, {}, {}", draft.color.hsv[0], draft.color.hsv[1], draft.color.hsv[2]));
                let mut preview = self.run_live && self.show_detections;
                if ui.add_enabled(self.target.is_some(), egui::Checkbox::new(&mut preview, "Preview detections in game")).on_hover_text("Read-only outlines over the connected game while it is foreground. Enabling this turns on Game overlay and Detected targets; it never starts automation.").changed() {
                    self.run_live = preview;
                    if preview { self.show_detections = true; }
                }
                ui.separator();
                ui.strong("Color tolerance");
                for (label, key, value) in [
                    ("Hue", "hue_tolerance", &mut draft.color.hue_tolerance),
                    ("Saturation", "saturation_tolerance", &mut draft.color.saturation_tolerance),
                    ("Brightness", "value_tolerance", &mut draft.color.value_tolerance),
                ] { numeric_u32(ui, label, key, value); }
                ui.separator();
                ui.strong("Candidate shape");
                for (label, key, value) in [
                    ("Minimum area (px²)", "min_area", &mut draft.color.min_area),
                    ("Minimum width (px)", "min_width", &mut draft.color.min_width),
                    ("Minimum height (px)", "min_height", &mut draft.color.min_height),
                ] { numeric_u32(ui, label, key, value); }
                numeric(ui, "Maximum frame area ratio", "max_area_ratio", &mut draft.color.max_area_ratio);
                numeric_u32(ui, "Stable detections (frames)", "stable_frames", &mut draft.color.stable_frames);
                if let DraftKind::Named(_) = draft.kind {
                    ui.separator();
                    if draft.delete_confirmation {
                        ui.label("Delete this saved preset? Profiles keep their current detection values.");
                        ui.horizontal(|ui| {
                            delete = widgets::plain_button(ui, "Delete preset").clicked();
                            if widgets::plain_button(ui, "Keep preset").clicked() { draft.delete_confirmation = false; }
                        });
                    } else if widgets::button(ui, Icon::Trash, "Delete saved preset", true, "Remove it from the preset library. Main profiles keep their copied values.").clicked() { draft.delete_confirmation = true; }
                }
            });
            if let Some(error) = &draft.error {
                egui::ScrollArea::vertical().id_salt("metin-preset-error").max_height(64.).show(ui, |ui| { ui.add(egui::Label::new(egui::RichText::new(error).color(widgets::RED)).wrap()).on_hover_text(error); });
            }
            ui.separator();
            ui.horizontal(|ui| {
                cancel = widgets::plain_button(ui, "Cancel").clicked();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    apply = widgets::button(ui, Icon::Check, if draft.kind == DraftKind::Custom { "Apply" } else { "Save & apply" }, true, "Apply these values to this profile's draft. Use the header Save to apply profile edits to runtime.").clicked();
                });
            });
        });
        cancel |= modal.should_close();
        if cancel {
            self.overlay_frame_id = 0;
            self.overlay_preview = None;
            return;
        }
        if delete && let DraftKind::Named(id) = draft.kind {
            match self.store.delete_metin_preset(id) {
                Ok(()) => {
                    self.metin_presets.catalog.retain(|p| p.id != id);
                    self.overlay_frame_id = 0;
                    return;
                }
                Err(error) => draft.error = Some(error.to_string()),
            }
        }
        if apply {
            match self.apply_metin_preset(&draft) {
                Ok(()) => return,
                Err(error) => draft.error = Some(error),
            }
        }
        self.metin_presets.draft = Some(draft);
        if self
            .metin_presets
            .draft
            .as_ref()
            .is_some_and(|draft| draft.color != previous_color)
        {
            self.overlay_frame_id = 0;
        }
        if pick {
            self.request_capture(overlay::CapturePurpose::PresetColor);
        }
    }
}

fn color_swatch(ui: &mut egui::Ui, color: Option<&ColorConfig>) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(18., 20.), egui::Sense::hover());
    let fill = color.map_or(egui::Color32::TRANSPARENT, |c| {
        let [r, g, b] = crate::ui_controls::sampled_hsv_to_srgb(c.hsv);
        egui::Color32::from_rgb(r, g, b)
    });
    ui.painter().rect_filled(rect.shrink(1.), 0., fill);
    ui.painter().rect_stroke(
        rect.shrink(1.),
        0.,
        egui::Stroke::new(1., widgets::BRONZE),
        egui::StrokeKind::Inside,
    );
    response.on_hover_text(color.map_or("No Metin color selected".into(), |c| {
        format!("HSV {}, {}, {}", c.hsv[0], c.hsv[1], c.hsv[2])
    }));
}
