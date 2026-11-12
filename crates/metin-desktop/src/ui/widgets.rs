use eframe::egui::{self, Color32, Stroke, Vec2};

pub const BRONZE: Color32 = Color32::from_rgb(168, 143, 90);
pub const GREEN: Color32 = Color32::from_rgb(164, 201, 160);
pub const AMBER: Color32 = Color32::from_rgb(213, 188, 131);
pub const RED: Color32 = Color32::from_rgb(211, 145, 128);

pub use super::icons::{Icon, paint as icon};

/// Atlas buttons retain egui hit testing, keyboard activation and accessibility.
fn control(
    ui: &mut egui::Ui,
    kind: Option<Icon>,
    label: &str,
    size: Vec2,
    enabled: bool,
    selected: bool,
    hint: &str,
) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| {
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        response.widget_info(|| {
            let mut info = egui::WidgetInfo::labeled(
                egui::Role::Button,
                ui.is_enabled(),
                if label.is_empty() { hint } else { label },
            );
            info.selected = Some(selected);
            info
        });
        super::skin::control(
            ui,
            &response,
            if size.y >= 38. {
                "button_big"
            } else {
                "button_large"
            },
            selected,
        );
        let color = Color32::from_gray(238);
        let mut job = egui::text::LayoutJob::simple_singleline(
            label.into(),
            egui::FontId::proportional(if size.y >= 38. { 15. } else { 13. }),
            color,
        );
        job.wrap.max_width = (rect.width()
            - if kind.is_some() && !label.is_empty() {
                32.
            } else {
                8.
            })
        .max(1.);
        job.wrap.max_rows = 1;
        let galley = ui.painter().layout_job(job);
        if let Some(kind) = kind {
            let center = if label.is_empty() {
                rect.center()
            } else {
                egui::pos2(
                    rect.center().x - (galley.size().x + 24.) / 2. + 8.,
                    rect.center().y,
                )
            };
            super::icons::paint_sized(
                ui.painter(),
                center,
                kind,
                color,
                (rect.height() - 5.).min(18.),
            );
        }
        if !label.is_empty() {
            let x = rect.center().x - galley.size().x / 2. + if kind.is_some() { 12. } else { 0. };
            ui.painter().galley(
                egui::pos2(x, rect.center().y - galley.size().y / 2.),
                galley,
                color,
            );
        }
        #[cfg(test)]
        let frame = ui.ctx().cumulative_frame_nr();
        #[cfg(test)]
        ui.ctx().data_mut(|data| {
            data.insert_temp(egui::Id::unique(("control", hint)), rect);
            data.insert_temp(egui::Id::unique(("control-clip", hint)), ui.clip_rect());
            data.insert_temp(egui::Id::unique(("control-selected", hint)), selected);
            let registry = data.get_temp_mut_or_default::<(u64, Vec<(String, egui::Rect)>)>(
                egui::Id::unique("skin-control-registry"),
            );
            if registry.0 != frame {
                registry.0 = frame;
                registry.1.clear();
            }
            registry.1.push((hint.to_owned(), rect));
        });
        response.on_hover_text(hint).on_disabled_hover_text(hint)
    })
    .inner
}
pub fn button(
    ui: &mut egui::Ui,
    kind: Icon,
    label: &str,
    enabled: bool,
    hint: &str,
) -> egui::Response {
    let width = ui
        .painter()
        .layout_no_wrap(
            label.into(),
            egui::FontId::proportional(13.),
            Color32::WHITE,
        )
        .size()
        .x;
    control(
        ui,
        Some(kind),
        label,
        egui::vec2(if label.is_empty() { 30. } else { width + 38. }, 28.),
        enabled,
        false,
        hint,
    )
}
pub fn small_icon_button(
    ui: &mut egui::Ui,
    kind: Icon,
    enabled: bool,
    hint: &str,
) -> egui::Response {
    control(
        ui,
        Some(kind),
        "",
        egui::vec2(26., 28.),
        enabled,
        false,
        hint,
    )
}
pub fn sized_button(
    ui: &mut egui::Ui,
    kind: Icon,
    label: &str,
    size: Vec2,
    enabled: bool,
    selected: bool,
    hint: &str,
) -> egui::Response {
    control(ui, Some(kind), label, size, enabled, selected, hint)
}
pub fn compact_icon_button(
    ui: &mut egui::Ui,
    kind: Icon,
    enabled: bool,
    hint: &str,
) -> egui::Response {
    control(
        ui,
        Some(kind),
        "",
        egui::vec2(26., 15.),
        enabled,
        false,
        hint,
    )
}
pub fn plain_button(ui: &mut egui::Ui, label: impl Into<String>) -> egui::Response {
    let label = label.into();
    let width = ui
        .painter()
        .layout_no_wrap(
            label.clone(),
            egui::FontId::proportional(13.),
            Color32::WHITE,
        )
        .size()
        .x;
    control(
        ui,
        None,
        &label,
        egui::vec2((width + 20.).min(ui.available_width()).max(24.), 24.),
        true,
        false,
        &label,
    )
}

/// Direct choices keep their logical order even inside a right-aligned form row.
pub fn segmented_choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    selected: &mut T,
    options: &[(T, &str, &str)],
) -> bool {
    if options.is_empty() {
        return false;
    }
    let before = *selected;
    let width = ui.available_width();
    ui.push_id(id, |ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(width, 28.),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let gap = 3.;
                ui.spacing_mut().item_spacing.x = gap;
                let button_width =
                    ((width - gap * (options.len() - 1) as f32) / options.len() as f32).max(1.);
                let mut move_to = None;
                let mut ids = Vec::with_capacity(options.len());
                for (index, &(value, label, hint)) in options.iter().enumerate() {
                    let response = ui
                        .push_id(index, |ui| {
                            control(
                                ui,
                                None,
                                label,
                                egui::vec2(button_width, 28.),
                                true,
                                *selected == value,
                                hint,
                            )
                        })
                        .inner;
                    ids.push(response.id);
                    if response.clicked() {
                        *selected = value;
                        response.request_focus();
                    }
                    if ui.is_enabled() && response.has_focus() {
                        if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                            move_to = Some(index.saturating_sub(1));
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                            move_to = Some((index + 1).min(options.len() - 1));
                        }
                    }
                }
                if let Some(index) = move_to {
                    *selected = options[index].0;
                    ui.ctx()
                        .memory_mut(|memory| memory.request_focus(ids[index]));
                }
            },
        );
    });
    *selected != before
}

/// Detailed groups expose their configured values without making users open them.
pub fn collapsing_summary<R>(
    ui: &mut egui::Ui,
    title: &str,
    id: impl std::hash::Hash + std::fmt::Debug,
    summary: impl Into<String>,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::collapsing_header::CollapsingResponse<R> {
    let summary = summary.into();
    let mut text = egui::text::LayoutJob::default();
    text.append(
        title,
        0.,
        egui::TextFormat {
            font_id: egui::FontId::proportional(13.),
            color: ui.visuals().text_color(),
            ..Default::default()
        },
    );
    text.append(
        &format!(" · {summary}"),
        0.,
        egui::TextFormat {
            font_id: egui::FontId::proportional(12.),
            color: ui.visuals().weak_text_color(),
            ..Default::default()
        },
    );
    text.wrap.max_rows = 1;
    text.wrap.max_width = (ui.available_width() - ui.spacing().indent).max(1.);
    let result = egui::CollapsingHeader::new(egui::WidgetText::from(text))
        .id_salt(id)
        .show(ui, body);
    result.header_response.clone().on_hover_text(summary);
    result
}
pub fn compact_text_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let width = ui
        .painter()
        .layout_no_wrap(
            label.into(),
            egui::FontId::proportional(13.),
            Color32::WHITE,
        )
        .size()
        .x;
    control(
        ui,
        None,
        label,
        egui::vec2((width + 18.).min(ui.available_width()).max(24.), 20.),
        true,
        false,
        label,
    )
}
pub fn tab(
    ui: &mut egui::Ui,
    label: &str,
    width: f32,
    selected: bool,
    hint: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 26.), egui::Sense::click());
    super::skin::control(ui, &response, "tab_large", selected);
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(egui::Role::RadioButton, ui.is_enabled(), label);
        info.selected = Some(selected);
        info
    });
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(12.),
        Color32::from_gray(238),
    );
    response.on_hover_text(hint)
}

pub use super::select::{ComboBox, combo_trigger};
pub fn readiness_mark(ui: &mut egui::Ui, ready: bool) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(18., 22.), egui::Sense::hover());
    icon(
        ui.painter(),
        rect.center(),
        if ready { Icon::Check } else { Icon::Cross },
        if ready { GREEN } else { RED },
    );
    response.on_hover_text(if ready {
        "Configured"
    } else {
        "Not configured yet"
    });
}
pub fn heading(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.horizontal(|ui| {
        ui.heading(title);
        if !subtitle.is_empty() {
            crate::ui_controls::help(ui, subtitle);
        }
    });
    ui.add_space(5.);
}
pub fn section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    section_body(ui, title, true, body);
}

/// The page navigation already supplies the first separator, above its heading.
pub fn first_section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    section_body(ui, title, false, body);
}

fn section_body(ui: &mut egui::Ui, title: &str, separator: bool, body: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(5.);
    if separator {
        ui.separator();
    }
    ui.label(
        egui::RichText::new(title)
            .small()
            .strong()
            .color(ui.visuals().weak_text_color()),
    );
    let width = ui.available_width();
    ui.scope(|ui| {
        ui.set_width(width);
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        body(ui);
    });
    ui.add_space(5.);
}

/// Consistent primary form rows, with their value aligned to the panel edge.
pub fn field<R>(ui: &mut egui::Ui, label: &str, add_value: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let label_width = (ui.available_width() * 0.43).clamp(110., 185.);
    ui.horizontal(|ui| {
        let label = ui
            .allocate_ui_with_layout(
                egui::vec2(label_width, 26.),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_width(label_width);
                    ui.label(label)
                },
            )
            .inner;
        ui.label_inputs_by(label.id, |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 26.),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    let width = ui.available_width();
                    ui.set_min_width(width);
                    add_value(ui)
                },
            )
            .inner
        })
    })
    .inner
}
pub fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(egui::RichText::new(text).color(color).strong());
}
pub fn theme(ctx: &egui::Context) {
    ctx.add_plugin(ContainedTooltips);
    ctx.set_theme(egui::Theme::Dark);
    let mut visuals = egui::Visuals::dark();
    let stone = Color32::from_rgb(25, 25, 23);
    let raised = Color32::from_rgb(45, 43, 37);
    let recess = Color32::from_rgb(12, 12, 11);
    let edge = Color32::from_rgb(74, 69, 53);
    let text = Color32::from_rgb(236, 228, 210);
    visuals.disabled_alpha = 0.65;
    visuals.panel_fill = Color32::TRANSPARENT;
    visuals.window_fill = stone;
    visuals.faint_bg_color = recess;
    visuals.extreme_bg_color = recess;
    visuals.hyperlink_color = AMBER;
    visuals.override_text_color = Some(text);
    visuals.selection.bg_fill = Color32::from_rgb(28, 38, 82);
    visuals.selection.stroke = Stroke::new(1., BRONZE);
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.bg_fill = raised;
        widget.weak_bg_fill = raised;
        widget.bg_stroke = Stroke::new(1., edge);
        widget.fg_stroke = Stroke::new(1., text);
        widget.corner_radius = egui::CornerRadius::ZERO;
    }
    visuals.widgets.hovered.bg_stroke = Stroke::new(1., BRONZE);
    visuals.widgets.active.bg_fill = recess;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1., edge);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1., Color32::from_rgb(166, 164, 145));
    ctx.set_visuals(visuals);
    ctx.all_styles_mut(|style| {
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(14.));
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(12.));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(18.));
        style.spacing.item_spacing = egui::vec2(6., 6.);
        style.spacing.button_padding = egui::vec2(10., 7.);
        style.spacing.interact_size = egui::vec2(32., 30.);
        style.spacing.combo_width = 160.;
        style.wrap_mode = Some(egui::TextWrapMode::Wrap);
    });
}

/// Ordinary egui hover hints share the viewport's logical width, including zoom.
/// Registering a typed plugin is idempotent when presentation settings are reapplied.
struct ContainedTooltips;

impl egui::plugin::Plugin for ContainedTooltips {
    fn debug_name(&self) -> &'static str {
        "Companion tooltip bounds"
    }

    fn on_begin_pass(&mut self, ui: &mut egui::Ui) {
        // Reserve popup padding, stroke and a small gap from both native edges.
        let width = (ui.ctx().content_rect().width() - 32.).clamp(40., 400.);
        if ui.ctx().global_style().spacing.tooltip_width != width {
            ui.ctx().all_styles_mut(|style| {
                style.spacing.tooltip_width = width;
            });
        }
    }
}

#[cfg(test)]
mod tooltip_layout_tests {
    use super::*;

    fn text_shapes<'a>(shape: &'a egui::Shape, output: &mut Vec<&'a egui::epaint::TextShape>) {
        match shape {
            egui::Shape::Text(text) => output.push(text),
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    text_shapes(shape, output);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn ordinary_tooltips_keep_long_words_inside_viewport_after_resize_and_zoom() {
        let hint = format!(
            "Captured reference: C:\\Companion\\{}\\reference.png. Keep the whole tooltip readable.",
            "DistinctiveReference".repeat(7)
        );
        for zoom in [1., 1.5] {
            let ctx = egui::Context::default();
            theme(&ctx);
            ctx.set_zoom_factor(zoom);
            ctx.all_styles_mut(|style| style.interaction.tooltip_delay = 0.);
            let mut time = 0.;
            for size in [
                egui::vec2(460., 620.),
                egui::vec2(320., 500.),
                egui::vec2(900., 620.),
            ] {
                for right in [false, true] {
                    for bottom in [false, true] {
                        let mut last = None;
                        for _ in 0..6 {
                            time += 0.1;
                            let previous_viewport = ctx.content_rect();
                            let pointer = egui::pos2(
                                if right {
                                    previous_viewport.right() - 16.
                                } else {
                                    16.
                                },
                                if bottom {
                                    previous_viewport.bottom() - 16.
                                } else {
                                    16.
                                },
                            );
                            last = Some(ctx.run_ui(
                                egui::RawInput {
                                    screen_rect: Some(egui::Rect::from_min_size(
                                        egui::Pos2::ZERO,
                                        size,
                                    )),
                                    time: Some(time),
                                    events: vec![egui::Event::PointerMoved(pointer)],
                                    ..Default::default()
                                },
                                |ui| {
                                    let viewport = ui.ctx().content_rect();
                                    let position = egui::pos2(
                                        if right { viewport.right() - 24. } else { 8. },
                                        if bottom { viewport.bottom() - 24. } else { 8. },
                                    );
                                    let response = ui.allocate_rect(
                                        egui::Rect::from_min_size(position, egui::vec2(16., 16.)),
                                        egui::Sense::hover(),
                                    );
                                    response.on_hover_text(&hint);
                                },
                            ));
                            last.as_mut().unwrap().textures_delta.clear();
                        }
                        let output = last.unwrap();
                        let viewport = ctx.content_rect();
                        let mut found = false;
                        for clipped in &output.shapes {
                            let mut text = Vec::new();
                            text_shapes(&clipped.shape, &mut text);
                            for text in text.into_iter().filter(|text| text.galley.job.text == hint)
                            {
                                found = true;
                                let rect = egui::Rect::from_min_size(text.pos, text.galley.size());
                                let glyphs = text.galley.mesh_bounds.translate(text.pos.to_vec2());
                                assert!(!text.galley.elided);
                                assert!(text.galley.rows.len() > 1);
                                assert!(
                                    viewport.expand(0.5).contains_rect(rect),
                                    "tooltip {rect:?} exceeds {viewport:?}, zoom {zoom}, right {right}, bottom {bottom}"
                                );
                                assert!(
                                    clipped.clip_rect.expand(0.5).contains_rect(rect),
                                    "tooltip is clipped: {rect:?} / {:?}",
                                    clipped.clip_rect
                                );
                                assert!(
                                    clipped.clip_rect.expand(0.5).contains_rect(glyphs),
                                    "tooltip glyphs are clipped: {glyphs:?} / {:?}",
                                    clipped.clip_rect
                                );
                            }
                        }
                        assert!(found, "The actual tooltip text must have been painted");
                    }
                }
            }
        }
    }
}

/// Shared secondary navigation; wraps at smaller widths without reserving a sidebar.
pub(super) fn subtabs<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    selected: &mut T,
    pages: &[(T, &str, &str)],
) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        for &(page, label, hint) in pages {
            if tab(
                ui,
                label,
                (ui.painter()
                    .layout_no_wrap(
                        label.into(),
                        egui::FontId::proportional(12.),
                        Color32::WHITE,
                    )
                    .size()
                    .x
                    + 22.)
                    .max(56.),
                *selected == page,
                hint,
            )
            .clicked()
            {
                changed |= *selected != page;
                *selected = page;
            }
        }
    });
    ui.separator();
    changed
}

/// Presentation only: recognition references never depend on the chosen glyph.
#[derive(Clone, Copy)]
pub enum Glyph {
    Vector(Icon),
    Art(super::catalog::Artwork),
    Skill(metin_core::SkillIconId),
    Capture(egui::TextureId),
}
impl From<Icon> for Glyph {
    fn from(value: Icon) -> Self {
        Self::Vector(value)
    }
}
impl From<super::catalog::Artwork> for Glyph {
    fn from(value: super::catalog::Artwork) -> Self {
        Self::Art(value)
    }
}
impl Glyph {
    pub fn paint(self, ui: &egui::Ui, rect: egui::Rect) {
        match self {
            Self::Vector(kind) => super::icons::paint_sized(
                ui.painter(),
                rect.center(),
                kind,
                BRONZE,
                rect.height().min(24.),
            ),
            Self::Art(art) => super::catalog::paint(ui, art, rect),
            Self::Skill(id) => {
                super::skin::sprite(ui, "public/slot_base", rect);
                super::catalog::paint_skill(ui, id, rect.shrink(2.));
            }
            Self::Capture(texture) => {
                super::skin::sprite(ui, "public/slot_base", rect);
                ui.painter().image(
                    texture,
                    rect.shrink(2.),
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                    Color32::WHITE,
                );
            }
        }
    }
}
pub fn icon_heading(ui: &mut egui::Ui, glyph: impl Into<Glyph>, title: &str, hint: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(28.), egui::Sense::hover());
        glyph.into().paint(ui, rect);
        ui.heading(title);
        if !hint.is_empty() {
            crate::ui_controls::help(ui, hint);
        }
    });
    ui.add_space(5.);
}

/// One compact, accessible click target for a catalog entry.
pub fn catalog_row(
    ui: &mut egui::Ui,
    kind: impl Into<Glyph>,
    title: &str,
    summary: &str,
) -> egui::Response {
    catalog_row_inner(ui, kind.into(), title, summary, None)
}

pub fn catalog_row_status(
    ui: &mut egui::Ui,
    kind: impl Into<Glyph>,
    title: &str,
    summary: &str,
    status: &str,
    color: Color32,
) -> egui::Response {
    catalog_row_inner(ui, kind.into(), title, summary, Some((status, color)))
}

fn catalog_row_inner(
    ui: &mut egui::Ui,
    kind: Glyph,
    title: &str,
    summary: &str,
    status: Option<(&str, Color32)>,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 44.), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::Button,
            ui.is_enabled(),
            format!(
                "{title}: {}{summary}",
                status.map_or(String::new(), |(label, _)| format!("{label} · "))
            ),
        )
    });
    let visuals = ui.style().interact(&response);
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(rect, 2., visuals.bg_fill);
    }
    kind.paint(
        ui,
        egui::Rect::from_center_size(rect.left_center() + egui::vec2(16., 0.), Vec2::splat(32.)),
    );
    let left = rect.left() + 36.;
    let max_width = (rect.width() - 62.).max(1.);
    let status_width = status.map_or(0., |(text, color)| {
        let galley =
            ui.painter()
                .layout_no_wrap(text.into(), egui::FontId::proportional(12.), color);
        let width = galley.size().x;
        ui.painter().galley(
            egui::pos2(rect.right() - 24. - width, rect.top() + 5.),
            galley,
            color,
        );
        width + 10.
    });
    for (text, y, size, color) in [
        (title, rect.top() + 5., 14., ui.visuals().text_color()),
        (
            summary,
            rect.top() + 24.,
            12.,
            ui.visuals().weak_text_color(),
        ),
    ] {
        let mut job = egui::text::LayoutJob::simple_singleline(
            text.into(),
            egui::FontId::proportional(size),
            color,
        );
        job.wrap.max_width = if text == title {
            (max_width - status_width).max(1.)
        } else {
            max_width
        };
        job.wrap.max_rows = 1;
        let galley = ui.painter().layout_job(job);
        ui.painter().galley(egui::pos2(left, y), galley, color);
    }
    let c = egui::pos2(rect.right() - 11., rect.center().y);
    ui.painter().add(egui::Shape::line(
        vec![
            c + egui::vec2(-3., -4.),
            c + egui::vec2(1., 0.),
            c + egui::vec2(-3., 4.),
        ],
        Stroke::new(1.4, ui.visuals().weak_text_color()),
    ));
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(summary)
}

pub fn progress_track(ui: &egui::Ui, rect: egui::Rect, fraction: Option<f32>) {
    super::skin::gauge(ui, rect, fraction);
}

/// Fixed-height presentation: unavailable progress never collapses the card.
pub fn timer_bar(
    ui: &mut egui::Ui,
    label: &str,
    elapsed: f64,
    remaining: Option<f64>,
    total: Option<f64>,
) {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 27.), egui::Sense::hover());
    let fraction = total
        .filter(|v| v.is_finite() && *v > 0.)
        .map(|total| (remaining.unwrap_or(0.) / total).clamp(0., 1.) as f32);
    progress_track(
        ui,
        egui::Rect::from_min_max(rect.min + egui::vec2(0., 19.), rect.max),
        fraction,
    );
    let time = remaining
        .map(|v| format!("{} left", metin_core::duration(v.max(0.).ceil())))
        .unwrap_or_else(|| {
            if label.is_empty() {
                String::new()
            } else {
                format!("{} elapsed", metin_core::duration(elapsed.max(0.).floor()))
            }
        });
    if !label.is_empty() {
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::ProgressIndicator,
                ui.is_enabled(),
                format!("{label}: {time}"),
            )
        });
    }
    let mut job = egui::text::LayoutJob::simple_singleline(
        label.into(),
        egui::FontId::proportional(11.),
        ui.visuals().text_color(),
    );
    job.wrap.max_width = (rect.width() - 100.).max(1.);
    job.wrap.max_rows = 1;
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(
        rect.left_top() + egui::vec2(4., 1.),
        galley,
        ui.visuals().text_color(),
    );
    ui.painter().text(
        egui::pos2(rect.right() - 4., rect.top() + 7.),
        egui::Align2::RIGHT_CENTER,
        &time,
        egui::FontId::monospace(11.),
        ui.visuals().text_color(),
    );
    response.on_hover_text(if label.is_empty() {
        "No active countdown".into()
    } else {
        format!("{label}: {time}")
    });
}

/// A quiet heading for dense overview information, independent of form cards.
pub fn compact_heading(
    ui: &mut egui::Ui,
    title: &str,
    action: Option<(&str, &str)>,
) -> Option<egui::Response> {
    ui.add_space(3.);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(title)
                .small()
                .strong()
                .color(ui.visuals().weak_text_color()),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            action.map(|(label, hint)| compact_text_button(ui, label).on_hover_text(hint))
        })
        .inner
    })
    .inner
}

/// Dynamic native thinboard behind a normal egui content region.
pub fn panel<R>(
    ui: &mut egui::Ui,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let background = ui.painter().add(egui::Shape::Noop);
    let result = egui::Frame::new()
        .inner_margin(egui::Margin::same(6))
        .show(ui, body);
    ui.painter().set(
        background,
        super::skin::panel_shape(ui, result.response.rect),
    );
    result
}
