//! A stable-height name cell with a trailing fixed-width action group.
use eframe::egui;

pub(super) const HEIGHT: f32 = 28.;
pub(super) const ACTION_WIDTH: f32 = 26.;
const GAP: f32 = 5.;

pub(super) fn show<R>(
    ui: &mut egui::Ui,
    actions: usize,
    contents: impl FnOnce(&mut egui::Ui, &mut egui::Ui, f32) -> R,
) -> R {
    let width = ui.available_width();
    let (row, _) = ui.allocate_exact_size(egui::vec2(width, HEIGHT), egui::Sense::hover());
    let controls_width = actions as f32 * ACTION_WIDTH + actions.saturating_sub(1) as f32 * GAP;
    let name_width = (width - controls_width - GAP).max(1.);
    let name_rect = egui::Rect::from_min_size(row.min, egui::vec2(name_width, HEIGHT));
    let actions_rect =
        egui::Rect::from_min_max(egui::pos2(row.right() - controls_width, row.top()), row.max);
    let mut name_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("row-name")
            .max_rect(name_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let mut actions_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("row-actions")
            .max_rect(actions_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    name_ui.set_clip_rect(ui.clip_rect().intersect(name_rect));
    actions_ui.set_clip_rect(ui.clip_rect().intersect(actions_rect));
    name_ui.spacing_mut().interact_size.y = HEIGHT;
    name_ui.spacing_mut().button_padding.y = name_ui.spacing().button_padding.y.min(4.);
    actions_ui.spacing_mut().item_spacing.x = GAP;
    contents(&mut name_ui, &mut actions_ui, name_width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widgets::{self, Icon};

    #[test]
    fn trailing_actions_stay_at_the_right_edge_without_changing_row_height_for_inline_rename() {
        let ctx = egui::Context::default();
        widgets::theme(&ctx);
        ctx.set_zoom_factor(1.5);
        let mut name = "M".repeat(80);
        let mut rows = Vec::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400., 480.),
                )),
                ..Default::default()
            },
            |ui| {
                ui.set_width(400.);
                // A justified container is how popup choices and recognition rows are embedded.
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                    for editing in [false, true] {
                        let edge = ui.available_rect_before_wrap().right();
                        let before = ui.cursor().top();
                        let (text, controls) =
                            show(ui, if editing { 2 } else { 1 }, |ui, actions_ui, width| {
                                let text = if editing {
                                    ui.add_sized(
                                        [width, HEIGHT],
                                        egui::TextEdit::singleline(&mut name),
                                    )
                                } else {
                                    ui.add_sized(
                                        [width, HEIGHT],
                                        egui::Button::new(&name).truncate(),
                                    )
                                };
                                let first = widgets::small_icon_button(
                                    actions_ui,
                                    if editing { Icon::Check } else { Icon::Edit },
                                    true,
                                    "row action",
                                );
                                let last = if editing {
                                    widgets::small_icon_button(
                                        actions_ui,
                                        Icon::Cross,
                                        true,
                                        "cancel",
                                    )
                                } else {
                                    first.clone()
                                };
                                (text, (first.rect, last.rect))
                            });
                        rows.push((edge, before, ui.cursor().top(), text.rect, controls));
                    }
                });
            },
        );
        output.textures_delta.clear();
        for (edge, before, after, text, (first, last)) in &rows {
            assert!(
                (last.right() - edge).abs() < 0.1,
                "Trailing actions must meet the content edge: {last:?} / {edge}"
            );
            assert!(
                text.right() + 4.9 <= first.left(),
                "Editable text cannot push or overlap actions"
            );
            assert!((text.height() - HEIGHT).abs() < 0.1);
            assert!((first.height() - HEIGHT).abs() < 0.1);
            assert!(
                (after - before - HEIGHT - 6.).abs() < 1.,
                "One fixed-height row only"
            );
        }
        assert!((rows[0].2 - rows[0].1 - (rows[1].2 - rows[1].1)).abs() < 0.1);
    }
}
