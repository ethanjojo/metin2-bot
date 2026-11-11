//! Native outline icons on one 24-unit grid. No font glyphs or theme-specific bitmaps.
use eframe::egui::{self, Color32, Pos2, Stroke};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
    Edit,
    Cross,
    Target,
    Pin,
    Buff,
    Mount,
    Window,
    Revert,
    History,
    Clock,
    Sword,
    Settings,
    Undo,
    Redo,
    Info,
}

pub fn paint(p: &egui::Painter, center: Pos2, kind: Icon, color: Color32) {
    paint_sized(p, center, kind, color, 18.);
}

/// Bare title glyphs retain a full hit target and stable focus across state changes.
pub fn header_button(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    kind: Icon,
    selected: bool,
    hint: &str,
) -> egui::Response {
    let response = ui.interact(
        rect,
        ui.make_persistent_id(("header-glyph", kind)),
        egui::Sense::click(),
    );
    let color = if selected {
        super::widgets::BRONZE
    } else if response.hovered() || response.has_focus() {
        Color32::WHITE
    } else {
        Color32::from_gray(210)
    };
    paint_sized(ui.painter(), rect.center(), kind, color, 18.);
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.),
            0.,
            Stroke::new(1., super::widgets::BRONZE),
            egui::StrokeKind::Inside,
        );
    }
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), hint);
        info.selected = Some(selected);
        info
    });
    #[cfg(test)]
    ui.ctx().data_mut(|d| {
        d.insert_temp(egui::Id::unique(("control", hint)), rect);
        d.insert_temp(egui::Id::unique(("control-selected", hint)), selected);
    });
    response.on_hover_text(hint)
}

/// Keep the complete stroke inside the requested square, including tiny sort buttons.
pub fn paint_sized(p: &egui::Painter, center: Pos2, kind: Icon, color: Color32, size: f32) {
    let scale = size / 24.;
    let pt = |x: f32, y: f32| center + egui::vec2(x - 12., y - 12.) * scale;
    let stroke = Stroke::new(2. * scale, color);
    let line = |points: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            points.iter().map(|&(x, y)| pt(x, y)).collect(),
            stroke,
        ));
    };
    let circle = |x: f32, y: f32, radius: f32| {
        p.circle_stroke(pt(x, y), radius * scale, stroke);
    };
    let curve = |points: [(f32, f32); 4]| {
        p.add(egui::epaint::CubicBezierShape::from_points_stroke(
            points.map(|(x, y)| pt(x, y)),
            false,
            Color32::TRANSPARENT,
            stroke,
        ));
    };
    match kind {
        Icon::Play => {
            p.add(egui::Shape::convex_polygon(
                vec![pt(7., 4.), pt(20., 12.), pt(7., 20.)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Pause => {
            for x in [6., 15.] {
                p.rect_filled(
                    egui::Rect::from_min_max(pt(x, 4.), pt(x + 3., 20.)),
                    scale,
                    color,
                );
            }
        }
        Icon::Plus => {
            line(&[(4., 12.), (20., 12.)]);
            line(&[(12., 4.), (12., 20.)]);
        }

        Icon::Up => {
            line(&[(5., 15.), (12., 8.), (19., 15.)]);
        }
        Icon::Down => {
            line(&[(5., 9.), (12., 16.), (19., 9.)]);
        }
        Icon::Back => {
            line(&[(10., 5.), (3., 12.), (10., 19.)]);
            line(&[(3., 12.), (21., 12.)]);
        }
        Icon::Undo => {
            line(&[(3., 4.), (3., 10.), (9., 10.)]);
            curve([(3., 10.), (8., 2.), (21., 5.), (21., 14.)]);
            curve([(21., 14.), (21., 18.), (18., 21.), (14., 21.)]);
        }
        Icon::Redo => {
            line(&[(21., 4.), (21., 10.), (15., 10.)]);
            curve([(21., 10.), (16., 2.), (3., 5.), (3., 14.)]);
            curve([(3., 14.), (3., 18.), (6., 21.), (10., 21.)]);
        }
        Icon::Check => {
            line(&[(4., 12.), (9., 17.), (20., 6.)]);
        }
        Icon::Edit => {
            line(&[
                (4., 16.),
                (15., 5.),
                (19., 9.),
                (8., 20.),
                (3., 21.),
                (4., 16.),
            ]);
            line(&[(13., 7.), (17., 11.)]);
            line(&[(16., 4.), (18., 2.), (22., 6.), (20., 8.)]);
        }
        Icon::Cross => {
            line(&[(5., 5.), (19., 19.)]);
            line(&[(5., 19.), (19., 5.)]);
        }
        Icon::Trash => {
            line(&[(3., 6.), (21., 6.)]);
            line(&[(9., 6.), (9., 3.), (15., 3.), (15., 6.)]);
            line(&[(5., 6.), (6., 21.), (18., 21.), (19., 6.)]);
            line(&[(10., 10.), (10., 17.)]);
            line(&[(14., 10.), (14., 17.)]);
        }
        Icon::Save => {
            line(&[
                (3., 3.),
                (17., 3.),
                (21., 7.),
                (21., 21.),
                (3., 21.),
                (3., 3.),
            ]);
            line(&[(7., 3.), (7., 9.), (16., 9.), (16., 3.)]);
            line(&[(7., 21.), (7., 14.), (17., 14.), (17., 21.)]);
        }
        Icon::Camera => {
            line(&[
                (3., 7.),
                (7., 7.),
                (9., 4.),
                (15., 4.),
                (17., 7.),
                (21., 7.),
                (21., 20.),
                (3., 20.),
                (3., 7.),
            ]);
            circle(12., 13., 4.);
        }
        Icon::Refresh => {
            curve([(3., 10.), (5., 1.), (18., 1.), (21., 10.)]);
            line(&[(21., 4.), (21., 10.), (15., 10.)]);
            curve([(21., 14.), (19., 23.), (6., 23.), (3., 14.)]);
            line(&[(3., 20.), (3., 14.), (9., 14.)]);
        }
        Icon::Revert | Icon::History => {
            curve([(3., 10.), (6., -1.), (22., 2.), (21., 13.)]);
            curve([(21., 13.), (20., 23.), (6., 24.), (3., 16.)]);
            line(&[(3., 4.), (3., 10.), (9., 10.)]);
            if matches!(kind, Icon::History) {
                line(&[(12., 7.), (12., 12.), (16., 14.)]);
            }
        }
        Icon::Target => {
            circle(12., 12., 7.);
            circle(12., 12., 2.);
            for points in [
                [(12., 1.), (12., 5.)],
                [(12., 19.), (12., 23.)],
                [(1., 12.), (5., 12.)],
                [(19., 12.), (23., 12.)],
            ] {
                line(&points);
            }
        }
        Icon::Pin => {
            line(&[
                (8., 3.),
                (16., 3.),
                (15., 9.),
                (19., 13.),
                (19., 15.),
                (5., 15.),
                (5., 13.),
                (9., 9.),
                (8., 3.),
            ]);
            line(&[(12., 15.), (12., 22.)]);
        }
        Icon::Buff => {
            line(&[
                (13., 2.),
                (4., 14.),
                (11., 14.),
                (10., 22.),
                (20., 9.),
                (13., 9.),
                (13., 2.),
            ]);
        }
        // A horse-head profile reads clearly at 18 px; a miniature four-legged
        // silhouette loses its legs at the footer's actual scale.
        Icon::Mount => {
            line(&[(5., 21.), (19., 21.)]);
            curve([(5., 21.), (5., 16.), (8., 16.), (10., 13.)]);
            line(&[
                (10., 13.),
                (7., 14.),
                (3., 11.),
                (8., 5.),
                (8., 2.),
                (12., 5.),
            ]);
            curve([(12., 5.), (19., 5.), (21., 12.), (19., 21.)]);
            p.circle_filled(pt(10., 8.), scale, color);
            line(&[(5., 9.), (7., 11.)]);
        }

        Icon::Window => {
            line(&[(3., 4.), (21., 4.), (21., 20.), (3., 20.), (3., 4.)]);
            line(&[(3., 9.), (21., 9.)]);
            for x in [6., 9.] {
                p.circle_filled(pt(x, 6.5), 0.7 * scale, color);
            }
        }
        Icon::Clock => {
            circle(12., 12., 9.);
            line(&[(12., 6.), (12., 12.), (16., 14.)]);
        }
        Icon::Sword => {
            line(&[(9., 14.), (18., 3.), (21., 3.), (21., 6.), (10., 15.)]);
            line(&[(6., 11.), (13., 18.)]);
            line(&[(9., 15.), (4., 20.)]);
            line(&[(2., 18.), (6., 22.)]);
        }
        Icon::Settings => {
            for (y, x) in [(5., 8.), (12., 16.), (19., 7.)] {
                line(&[(3., y), (x - 3., y)]);
                line(&[(x + 3., y), (21., y)]);
                circle(x, y, 3.);
            }
        }
        Icon::Info => {
            circle(12., 12., 9.);
            p.circle_filled(pt(12., 7.), scale, color);
            line(&[(12., 11.), (12., 17.)]);
        }
    }
}

#[cfg(feature = "ui-smoke")]
pub fn sheet(ui: &mut egui::Ui) {
    ui.heading("Companion · icon review");
    ui.label("Native size · disabled · 2× detail");
    egui::Grid::new("icon-review")
        .num_columns(3)
        .spacing([12., 8.])
        .show(ui, |ui| {
            for (i, kind) in [
                Icon::Play,
                Icon::Pause,
                Icon::Save,
                Icon::Plus,
                Icon::Up,
                Icon::Down,
                Icon::Trash,
                Icon::Camera,
                Icon::Refresh,
                Icon::Back,
                Icon::Check,
                Icon::Edit,
                Icon::Cross,
                Icon::Target,
                Icon::Pin,
                Icon::Buff,
                Icon::Mount,
                Icon::Window,
                Icon::Revert,
                Icon::History,
                Icon::Clock,
                Icon::Sword,
                Icon::Settings,
                Icon::Undo,
                Icon::Redo,
                Icon::Info,
            ]
            .into_iter()
            .enumerate()
            {
                ui.vertical(|ui| {
                    ui.label(format!("{kind:?}"));
                    let (r, _) =
                        ui.allocate_exact_size(egui::vec2(128., 40.), egui::Sense::hover());
                    let color = ui.visuals().text_color();
                    paint(
                        ui.painter(),
                        r.left_center() + egui::vec2(12., 0.),
                        kind,
                        color,
                    );
                    paint(
                        ui.painter(),
                        r.left_center() + egui::vec2(44., 0.),
                        kind,
                        color.gamma_multiply(ui.visuals().disabled_alpha()),
                    );
                    paint_sized(
                        ui.painter(),
                        r.left_center() + egui::vec2(94., 0.),
                        kind,
                        color,
                        36.,
                    );
                });
                if i % 3 == 2 {
                    ui.end_row();
                }
            }
        });
}
