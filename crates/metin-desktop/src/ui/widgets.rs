use eframe::egui::{self, Color32, Pos2, Stroke, Vec2};

pub const BLUE: Color32 = Color32::from_rgb(39, 112, 198);
pub const GREEN: Color32 = Color32::from_rgb(49, 161, 130);
pub const AMBER: Color32 = Color32::from_rgb(190, 126, 30);
pub const RED: Color32 = Color32::from_rgb(204, 78, 78);

#[derive(Clone, Copy)]
pub enum Icon {
    Play,
    Pause,
    Save,
    Plus,
    Up,
    Down,
    Trash,
    Camera,
    Refresh,
    Back,
    Check,
    Cross,
    Target,
    Buff,
    Settings,
    Sun,
    Undo,
    Redo,
    Minus,
}

pub fn icon(p: &egui::Painter, center: Pos2, kind: Icon, color: Color32) {
    let pt = |x: f32, y: f32| center + Vec2::new(x, y);
    let stroke = Stroke::new(1.6, color);
    let line = |points: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            points.iter().map(|&(x, y)| pt(x, y)).collect(),
            stroke,
        ));
    };
    match kind {
        Icon::Play => {
            p.add(egui::Shape::convex_polygon(
                vec![pt(-4., -6.), pt(6., 0.), pt(-4., 6.)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Pause => {
            for x in [-3., 3.] {
                p.line_segment([pt(x, -6.), pt(x, 6.)], Stroke::new(3., color));
            }
        }
        Icon::Plus => {
            line(&[(-6., 0.), (6., 0.)]);
            line(&[(0., -6.), (0., 6.)]);
        }
        Icon::Minus => line(&[(-6., 0.), (6., 0.)]),
        Icon::Up => {
            line(&[(-5., 1.), (0., -4.), (5., 1.)]);
            line(&[(0., -4.), (0., 6.)]);
        }
        Icon::Down => {
            line(&[(-5., -1.), (0., 4.), (5., -1.)]);
            line(&[(0., -6.), (0., 4.)]);
        }
        Icon::Back | Icon::Undo => {
            line(&[(-1., -5.), (-6., 0.), (-1., 5.)]);
            line(&[(-6., 0.), (3., 0.), (6., 3.)]);
        }
        Icon::Redo => {
            line(&[(1., -5.), (6., 0.), (1., 5.)]);
            line(&[(6., 0.), (-3., 0.), (-6., 3.)]);
        }
        Icon::Check => line(&[(-6., 0.), (-2., 4.), (6., -5.)]),
        Icon::Trash => {
            line(&[(-6., -4.), (6., -4.)]);
            line(&[(-4., -4.), (-3., 6.), (3., 6.), (4., -4.)]);
            line(&[(-2., -6.), (2., -6.)]);
        }
        Icon::Save => {
            line(&[
                (-6., -6.),
                (4., -6.),
                (6., -4.),
                (6., 6.),
                (-6., 6.),
                (-6., -6.),
            ]);
            line(&[(-3., -6.), (-3., -1.), (3., -1.), (3., -6.)]);
            line(&[(-3., 6.), (-3., 2.), (3., 2.), (3., 6.)]);
        }
        Icon::Camera => {
            line(&[
                (-7., -4.),
                (-3., -4.),
                (-2., -6.),
                (2., -6.),
                (3., -4.),
                (7., -4.),
                (7., 6.),
                (-7., 6.),
                (-7., -4.),
            ]);
            p.circle_stroke(center + Vec2::new(0., 1.), 3., stroke);
        }
        Icon::Refresh => {
            p.circle_stroke(center, 5.5, stroke);
            line(&[(0., -8.), (5., -5.), (1., -2.)]);
        }
        Icon::Target => {
            p.circle_stroke(center, 5.5, stroke);
            p.circle_stroke(center, 2., stroke);
            line(&[(0., -8.), (0., -5.)]);
            line(&[(5., 0.), (8., 0.)]);
        }
        Icon::Buff => line(&[
            (1., -7.),
            (-5., 1.),
            (0., 1.),
            (-1., 7.),
            (6., -2.),
            (1., -2.),
            (1., -7.),
        ]),
        Icon::Cross => {
            line(&[(-5., -5.), (5., 5.)]);
            line(&[(-5., 5.), (5., -5.)]);
        }
        Icon::Settings => {
            for y in [-5., 0., 5.] {
                line(&[(-7., y), (7., y)]);
            }
            for (x, y) in [(-2., -5.), (3., 0.), (-3., 5.)] {
                p.circle_filled(pt(x, y), 2.5, color);
            }
        }
        Icon::Sun => {
            p.circle_stroke(center, 3.5, stroke);
            for (x, y) in [
                (0., 1.),
                (1., 0.),
                (0., -1.),
                (-1., 0.),
                (0.7, 0.7),
                (-0.7, 0.7),
                (0.7, -0.7),
                (-0.7, -0.7),
            ] {
                p.line_segment([pt(x * 5.5, y * 5.5), pt(x * 8., y * 8.)], stroke);
            }
        }
    }
}

pub fn button(
    ui: &mut egui::Ui,
    kind: Icon,
    label: &str,
    enabled: bool,
    hint: &str,
) -> egui::Response {
    ui.add_enabled(enabled, |ui: &mut egui::Ui| {
        let font = egui::TextStyle::Button.resolve(ui.style());
        let galley = ui
            .painter()
            .layout_no_wrap(label.into(), font, Color32::PLACEHOLDER);
        let width = if label.is_empty() {
            32.
        } else {
            galley.size().x + 44.
        };
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 32.), egui::Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Button,
                ui.is_enabled(),
                if label.is_empty() { hint } else { label },
            )
        });
        let v = ui.style().interact(&response);
        ui.painter()
            .rect(rect, 6., v.bg_fill, v.bg_stroke, egui::StrokeKind::Inside);
        icon(
            ui.painter(),
            egui::pos2(rect.left() + 16., rect.center().y),
            kind,
            v.fg_stroke.color,
        );
        if !label.is_empty() {
            ui.painter().galley(
                egui::pos2(rect.left() + 31., rect.center().y - galley.size().y / 2.),
                galley,
                v.fg_stroke.color,
            );
        }
        response.on_hover_text(hint).on_disabled_hover_text(hint)
    })
}
pub fn small_icon_button(
    ui: &mut egui::Ui,
    kind: Icon,
    enabled: bool,
    hint: &str,
) -> egui::Response {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("").min_size(egui::vec2(26., 28.)),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, hint));
    let color = ui.style().interact(&response).fg_stroke.color;
    let color = if response.enabled() {
        color
    } else {
        color.gamma_multiply(ui.visuals().disabled_alpha())
    };
    icon(ui.painter(), response.rect.center(), kind, color);
    response.on_hover_text(hint).on_disabled_hover_text(hint)
}
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
pub fn primary_button(
    ui: &mut egui::Ui,
    kind: Icon,
    label: &str,
    enabled: bool,
    hint: &str,
) -> egui::Response {
    ui.scope(|ui| {
        ui.visuals_mut().widgets.inactive.bg_fill = BLUE;
        ui.visuals_mut().widgets.inactive.fg_stroke.color = Color32::WHITE;
        ui.visuals_mut().widgets.hovered.bg_fill = Color32::from_rgb(58, 132, 220);
        ui.visuals_mut().widgets.hovered.fg_stroke.color = Color32::WHITE;
        ui.visuals_mut().widgets.active.fg_stroke.color = Color32::WHITE;
        button(ui, kind, label, enabled, hint)
    })
    .inner
}
pub fn heading(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.heading(title);
    ui.label(egui::RichText::new(subtitle).color(ui.visuals().weak_text_color()));
    ui.add_space(12.);
}
pub fn section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(8.)
        .inner_margin(14.)
        .show(ui, |ui| {
            ui.set_min_width((ui.available_width() - 2.).max(1.));
            ui.strong(title);
            ui.add_space(8.);
            body(ui);
        });
    ui.add_space(10.);
}
pub fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    let color = if !ui.visuals().dark_mode {
        if color == AMBER {
            Color32::from_rgb(145, 85, 8)
        } else if color == GREEN {
            Color32::from_rgb(22, 119, 88)
        } else if color == RED {
            Color32::from_rgb(173, 47, 55)
        } else {
            color
        }
    } else {
        color
    };
    ui.label(egui::RichText::new(text).color(color).strong());
}
pub fn theme(ctx: &egui::Context, dark: bool) {
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = if dark {
        Color32::from_rgb(24, 28, 35)
    } else {
        Color32::from_rgb(246, 248, 251)
    };
    visuals.window_fill = visuals.panel_fill;
    visuals.faint_bg_color = if dark {
        Color32::from_rgb(30, 35, 43)
    } else {
        Color32::from_rgb(238, 242, 247)
    };
    visuals.override_text_color = Some(if dark {
        Color32::from_rgb(225, 232, 240)
    } else {
        Color32::from_rgb(33, 44, 58)
    });
    visuals.selection.bg_fill = if dark {
        Color32::from_rgb(35, 71, 108)
    } else {
        Color32::from_rgb(204, 225, 249)
    };
    visuals.selection.stroke = Stroke::new(1., BLUE);
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
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(23.));
        style.spacing.item_spacing = egui::vec2(8., 7.);
        style.spacing.button_padding = egui::vec2(10., 7.);
        style.spacing.interact_size = egui::vec2(32., 30.);
        style.spacing.combo_width = 180.;
    });
}
