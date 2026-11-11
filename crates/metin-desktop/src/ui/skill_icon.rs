use super::{catalog, forms, skin, widgets};
use eframe::egui;
use metin_core::{BuffSkill, SkillIconId};
use std::collections::HashMap;

pub(super) fn glyph(
    ui: &egui::Ui,
    skill: &BuffSkill,
    textures: &mut HashMap<String, egui::TextureHandle>,
) -> widgets::Glyph {
    if let Some(id) = skill.display_icon {
        widgets::Glyph::Skill(id)
    } else if let Some(texture) = skill
        .icon
        .as_ref()
        .and_then(|icon| forms::icon_texture(ui, icon, textures))
    {
        widgets::Glyph::Capture(texture.id())
    } else {
        catalog::Artwork::Skills.into()
    }
}

/// A presentation choice, never a replacement for captured recognition images.
pub(super) fn picker(ui: &mut egui::Ui, selected: &mut Option<SkillIconId>) -> bool {
    let mut changed = false;
    widgets::field(ui, "Display icon", |ui| {
        let label = selected
            .map(|id| catalog::skill(id).name)
            .unwrap_or("Automatic");
        let response =
            widgets::combo_trigger(ui, egui::Id::unique("skill-icon-picker"), label, 200.);
        let popup = egui::Popup::from_toggle_button_response(&response)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .width(340.)
            .frame(skin::popup_frame(ui));
        #[cfg(feature = "ui-smoke")]
        let popup = if ui
            .ctx()
            .data(|d| d.get_temp::<bool>(egui::Id::unique("buff-icon-picker-fixture")))
            .unwrap_or(false)
        {
            popup.open(true)
        } else {
            popup
        };
        popup.show(|ui| {
            let class_id = ui.make_persistent_id("skill-class");
            let default_class = selected.and_then(|id| catalog::CLASSES.iter().position(|c| c.skills.iter().any(|s| s.id == id))).unwrap_or(0);
            let mut class = ui.ctx().data_mut(|d| *d.get_temp_mut_or(class_id, default_class));
            ui.horizontal(|ui| {
                for (i, entry) in catalog::CLASSES.iter().enumerate() {
                    let width = (ui.available_width() - ui.spacing().item_spacing.x * (3-i) as f32) / (4-i) as f32;
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 48.), egui::Sense::hover());
                    let response = ui.interact(rect, ui.make_persistent_id(("skill-class",entry.id)), egui::Sense::click());
                    skin::bevel(ui, rect, class == i || response.has_focus());
                    skin::sprite(ui, entry.portrait, egui::Rect::from_center_size(rect.center_top()+egui::vec2(0.,16.),egui::Vec2::splat(26.)));
                    ui.painter().text(rect.center_bottom()-egui::vec2(0.,8.),egui::Align2::CENTER_CENTER,entry.name,egui::FontId::proportional(12.),ui.visuals().text_color());
                    response.widget_info(|| { let mut info = egui::WidgetInfo::labeled(egui::Role::RadioButton,true,entry.name); info.selected=Some(class==i); info });
                    if response.on_hover_text(format!("{} · both specializations",entry.name)).clicked() { class=i; }
                }
            });
            ui.ctx().data_mut(|d| d.insert_temp(class_id, class));
            ui.add_space(4.);
            egui::Grid::new("skill-icon-grid").num_columns(6).spacing(egui::vec2(8.,8.)).show(ui, |ui| {
                for (i, entry) in catalog::CLASSES[class].skills.iter().enumerate() {
                    let (rect,response)=ui.allocate_exact_size(egui::Vec2::splat(40.),egui::Sense::click());
                    skin::bevel(ui,rect,*selected==Some(entry.id) || response.hovered() || response.has_focus());
                    catalog::paint_skill(ui,entry.id,egui::Rect::from_center_size(rect.center(),egui::Vec2::splat(32.)));
                    response.widget_info(|| { let mut info=egui::WidgetInfo::labeled(egui::Role::RadioButton,true,entry.name);info.selected=Some(*selected==Some(entry.id));info });
                    #[cfg(test)]
                    ui.ctx().data_mut(|d|d.insert_temp(egui::Id::unique(("skill-icon",entry.id.get())),rect));
                    if response.on_hover_text(entry.name).clicked() { changed |= selected.replace(entry.id)!=Some(entry.id); ui.close(); }
                    if (i+1)%6==0 { ui.end_row(); }
                }
            });
            ui.separator();
            if widgets::plain_button(ui,"Automatic icon").on_hover_text("Use the captured Active icon when available, otherwise the default skill symbol. Only the display changes.").clicked() {
                changed |= selected.take().is_some();ui.close();
            }
            ui.weak("Display only · recognition uses your captures.");
        });
    });
    changed
}
