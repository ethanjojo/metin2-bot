//! Product-level review of the settings that Save will persist.
use super::*;
use crate::storage::MetinPreset;
use metin_platform::Frame;
use uuid::Uuid;

mod actions;
mod model;
mod recognition;
mod safety;
use model::*;
#[cfg(test)]
mod tests;

pub(super) fn profile_changes(
    before: &Profile,
    after: &Profile,
    images: [Option<&Frame>; 2],
    presets: &[MetinPreset],
) -> Vec<ProfileChange> {
    let mut rows = Vec::new();
    let geometry = Geometry([before.frame_size, after.frame_size]);
    recognition::compare(&mut rows, before, after, images, presets, geometry);
    actions::compare(&mut rows, before, after, geometry);
    safety::compare(&mut rows, before, after, geometry);
    rows
}

struct Summary {
    rows: Vec<ProfileChange>,
    drafts: Vec<(Uuid, String)>,
    new_profile: Option<String>,
}
impl Summary {
    fn caption(&self) -> String {
        if self.new_profile.is_some() {
            return "New profile".into();
        }
        match self.rows.len() {
            0 if !self.drafts.is_empty() => "Draft".into(),
            1 => "1 change".into(),
            n => format!("{n} changes"),
        }
    }
}
pub(super) struct CachedChanges {
    profile: Uuid,
    revision: (u64, u64),
    unsaved_new: bool,
    preset_names: [Option<String>; 2],
    summary: Arc<Summary>,
}

impl MetinApp {
    fn current_changes(&mut self) -> Arc<Summary> {
        let before = self.editor.saved_profile();
        let after = &self.editor.profile;
        let unsaved_new = !self.profiles.iter().any(|profile| profile.id == after.id);
        let revision = (self.revision, self.editor.change_id());
        let preset_names = [before.color_preset_id, after.color_preset_id].map(|id| {
            id.and_then(|id| self.metin_presets.catalog.iter().find(|p| p.id == id))
                .map(|p| p.name.as_str())
        });
        if let Some(cache) = &self.changes_cache
            && cache.profile == after.id
            && cache.revision == revision
            && cache.unsaved_new == unsaved_new
            && cache.preset_names.each_ref().map(|n| n.as_deref()) == preset_names
        {
            return cache.summary.clone();
        }
        let effective = self.profile_for_save();
        let rows = profile_changes(
            before,
            &effective,
            [self.editor.saved_template(), self.editor.template.as_ref()],
            &self.metin_presets.catalog,
        );
        let drafts = after
            .buffs
            .skills
            .iter()
            .filter(|skill| !effective.buffs.skills.iter().any(|s| s.id == skill.id))
            .map(|skill| (skill.id, actions::buff_name(skill).to_owned()))
            .collect();
        let summary = Arc::new(Summary {
            rows,
            drafts,
            new_profile: unsaved_new.then(|| after.name.clone()),
        });
        self.changes_cache = Some(CachedChanges {
            profile: after.id,
            revision,
            unsaved_new,
            preset_names: preset_names.map(|s| s.map(str::to_owned)),
            summary: summary.clone(),
        });
        summary
    }
    pub(super) fn changes_width(&mut self, ui: &egui::Ui) -> f32 {
        if !self.has_unsaved_profile() {
            return 0.;
        }
        ui.painter()
            .layout_no_wrap(
                self.current_changes().caption(),
                egui::FontId::proportional(13.),
                egui::Color32::WHITE,
            )
            .size()
            .x
            + 20.
            + ui.spacing().item_spacing.x
    }
    pub(super) fn changes_badge(&mut self, ui: &mut egui::Ui) {
        if !self.has_unsaved_profile() {
            return;
        }
        let summary = self.current_changes();
        let caption = summary.caption();
        let response = widgets::plain_button(ui, &caption)
            .on_hover_text("Review pending profile edits and incomplete drafts before Save.");
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Review {caption}"))
        });
        let mut selected = None;
        let width = 360_f32.min((ui.ctx().content_rect().width() - 16.).max(1.));
        let height =
            (ui.ctx().content_rect().height() - response.rect.bottom() - 72.).clamp(72., 350.);
        let frame = skin::popup_frame(ui);
        let popup = egui::Popup::from_toggle_button_response(&response)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .frame(frame)
            .width(width);
        #[cfg(feature = "ui-smoke")]
        let popup = if ui
            .ctx()
            .data(|d| d.get_temp::<bool>(egui::Id::unique("qol-changes-fixture")))
            .unwrap_or(false)
        {
            popup.open(true)
        } else {
            popup
        };
        popup.show(|ui| {
            select::bound_popup_content(ui, width, frame);
            ui.strong(if summary.new_profile.is_some() {
                "New profile"
            } else {
                "Unsaved changes"
            });
            ui.separator();
            select::menu_scroll(
                ui,
                ("profile-changes", self.editor.profile.id),
                height,
                |ui| {
                    if let Some(name) = &summary.new_profile {
                        ui.label(format!(
                            "Save creates the profile “{name}” with its current settings."
                        ));
                    }
                    if !summary.drafts.is_empty() {
                        ui.strong("Incomplete buffs");
                        ui.small("Save omits each buff below until it has a shortcut.");
                        for (id, name) in &summary.drafts {
                            if widgets::plain_button(ui, name).clicked() {
                                selected = Some(Subject::Buff(Some(*id)));
                                ui.close();
                            }
                        }
                    }
                    let mut previous = None;
                    for (i, row) in summary.rows.iter().enumerate() {
                        let group = row.subject.group();
                        if previous != Some(group) {
                            ui.add_space(3.);
                            ui.strong(group);
                            previous = Some(group);
                        }
                        ui.push_id(("change", i), |ui| {
                            if widgets::plain_button(ui, &row.field.label)
                                .on_hover_text(format!("Open {} · {}", group, row.field.label))
                                .clicked()
                            {
                                selected = Some(row.subject);
                                ui.close();
                            }
                            self.show_change_values(ui, &row.field);
                            if !row.details.is_empty() {
                                egui::CollapsingHeader::new("Details")
                                    .id_salt("change-details")
                                    .show(ui, |ui| {
                                        for detail in &row.details {
                                            ui.small(&detail.label);
                                            self.show_change_values(ui, detail);
                                        }
                                    });
                            }
                        });
                    }
                    if summary.rows.is_empty()
                        && summary.new_profile.is_none()
                        && summary.drafts.is_empty()
                    {
                        ui.small("Saved profile settings are unchanged.");
                    }
                },
            );
        });
        if let Some(subject) = selected {
            self.open_change_subject(subject);
        }
    }

    fn show_change_values(&mut self, ui: &mut egui::Ui, field: &Field) {
        ui.columns(2, |columns| {
            for (side, (ui, value)) in columns
                .iter_mut()
                .zip([&field.before, &field.after])
                .enumerate()
            {
                ui.small(if side == 0 { "Saved" } else { "Edited" });
                match &value.preview {
                    Some(Preview::Color(hsv)) => {
                        let [r, g, b] = crate::ui_controls::sampled_hsv_to_srgb(*hsv);
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(32., 16.), egui::Sense::hover());
                        skin::bevel(ui, rect, false);
                        ui.painter().rect_filled(
                            rect.shrink(2.),
                            0.,
                            egui::Color32::from_rgb(r, g, b),
                        );
                    }
                    Some(Preview::Icon(id)) => {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::Vec2::splat(32.), egui::Sense::hover());
                        catalog::paint_skill(ui, *id, rect);
                    }
                    Some(Preview::Capture(icon)) => self.icon_thumbnail(ui, icon),
                    Some(Preview::Frame(frame)) => {
                        let id = ui.make_persistent_id(("hp-change-image", side));
                        let texture = ui
                            .ctx()
                            .data(|d| d.get_temp::<(Frame, egui::TextureHandle)>(id))
                            .filter(|(prev, _)| {
                                prev.width == frame.width
                                    && prev.height == frame.height
                                    && Arc::ptr_eq(&prev.bgr, &frame.bgr)
                            })
                            .map(|(_, texture)| texture)
                            .unwrap_or_else(|| {
                                let texture = forms::frame_texture(
                                    ui.ctx(),
                                    "HP change",
                                    frame,
                                    egui::TextureOptions::NEAREST,
                                );
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(id, (frame.clone(), texture.clone()))
                                });
                                texture
                            });
                        ui.add(
                            egui::Image::new(&texture)
                                .max_size(egui::vec2(ui.available_width(), 48.)),
                        );
                    }
                    None => {}
                }
                ui.add(egui::Label::new(egui::RichText::new(&value.text).small()).wrap());
            }
        });
    }

    #[cfg(test)]
    pub(super) fn open_profile_change(&mut self, row: &ProfileChange) {
        self.open_change_subject(row.subject);
    }

    fn open_change_subject(&mut self, subject: Subject) {
        self.binding = None;
        match subject {
            Subject::Search | Subject::Color | Subject::Hp | Subject::Frame | Subject::Area(_) => {
                self.open_setup(SetupAction::Calibration);
                self.return_to_buffs = false;
                self.return_to_safety = false;
                self.editing = false;
                self.mode =
                    match subject {
                        Subject::Area(id) => EditMode::Mask(id.and_then(|id| {
                            self.editor.profile.masks.iter().position(|a| a.id == id)
                        })),
                        Subject::Color => EditMode::Color,
                        Subject::Hp => EditMode::Template,
                        _ => EditMode::World,
                    };
                if subject == Subject::Color {
                    self.open_metin_preset(false);
                }
            }
            Subject::Buff(id) => {
                self.open_setup(SetupAction::Actions(ActionsPage::Buffs));
                let index = id.and_then(|id| {
                    self.editor
                        .profile
                        .buffs
                        .skills
                        .iter()
                        .position(|v| v.id == id)
                });
                self.buff_detail = index.is_some();
                if let Some(i) = index {
                    self.selected_skill = i;
                }
            }
            Subject::BuffArea => {
                let skill = self
                    .editor
                    .profile
                    .buffs
                    .skills
                    .iter()
                    .find(|v| v.mode == metin_core::BuffMode::Inactive)
                    .map(|v| v.id);
                self.open_change_subject(Subject::Buff(skill));
            }
            Subject::Mount => self.open_setup(SetupAction::Mount),
            Subject::Loot => self.open_setup(SetupAction::Actions(ActionsPage::Loot)),
            Subject::Puller => self.open_setup(SetupAction::Actions(ActionsPage::Puller)),
            Subject::OutOfTarget => self.open_setup(SetupAction::Actions(ActionsPage::OutOfTarget)),
            Subject::Periodic(id) => {
                self.open_setup(SetupAction::Actions(ActionsPage::Periodic));
                self.selected_periodic = id.filter(|id| {
                    self.editor
                        .profile
                        .periodic_actions
                        .iter()
                        .any(|v| v.id == *id)
                });
            }
            Subject::Rule(id) => {
                self.open_setup(SetupAction::VisualRules);
                let index = id.and_then(|id| {
                    self.editor
                        .profile
                        .safety
                        .rules
                        .iter()
                        .position(|v| v.id == id)
                });
                self.visual_detail = index.is_some();
                if let Some(i) = index {
                    self.selected_safety = i;
                }
            }
            Subject::Safety(page) => {
                self.open_setup(SetupAction::Safety);
                self.safety_page = page;
            }
            Subject::Timing => {
                self.open_setup(SetupAction::Settings);
                self.settings_page = SettingsPage::Combat;
            }
        }
    }
}
