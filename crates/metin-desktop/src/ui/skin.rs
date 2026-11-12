//! Rendering-only skin. Input, focus and accessible state remain owned by egui.
use eframe::egui::{self, Color32, Rect, Vec2};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Deserialize)]
struct Region {
    rect: [u32; 4],
}
#[derive(Deserialize)]
struct Style {
    #[serde(default)]
    parts: BTreeMap<String, String>,
    #[serde(default)]
    states: BTreeMap<String, String>,
    borders_ltrb_px: Option<[u32; 4]>,
    center_rgba8: Option<[u8; 4]>,
    cap_widths_px: Option<[u32; 2]>,
    logical_height_px: Option<u32>,
    paint_height_px: Option<u32>,
    fill_region: Option<String>,
    fill_insets_ltrb_px: Option<[u32; 4]>,
}
#[derive(Deserialize)]
struct Manifest {
    size: [u32; 2],
    regions: BTreeMap<String, Region>,
    styles: BTreeMap<String, Style>,
}
struct Skin {
    manifest: Manifest,
    texture: egui::TextureHandle,
    title_alpha: [AlphaTile; 3],
}
struct AlphaTile {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}
impl AlphaTile {
    fn sample(&self, x: f32, y: f32) -> u8 {
        self.pixels[(y as u32 % self.height * self.width + x as u32 % self.width) as usize]
    }
}

fn get(ctx: &egui::Context) -> Arc<Skin> {
    let id = egui::Id::unique("metin-widget-skin");
    if let Some(skin) = ctx.data_mut(|data| data.get_temp::<Arc<Skin>>(id)) {
        return skin;
    }
    let manifest: Manifest =
        serde_json::from_slice(include_bytes!("../../assets/metin-ui/manifest.json"))
            .expect("validated embedded skin manifest");
    let icon =
        eframe::icon_data::from_png_bytes(include_bytes!("../../assets/metin-ui/widgets.png"))
            .expect("validated embedded skin PNG");
    assert_eq!([icon.width, icon.height], manifest.size);
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [icon.width as usize, icon.height as usize],
        &icon.rgba,
    );
    let texture = ctx.load_texture("Metin UI atlas", image, egui::TextureOptions::NEAREST);
    let title_alpha = ["left", "center", "right"].map(|part| {
        let name = &manifest.styles["titlebar"].parts[part];
        let [x, y, width, height] = manifest.regions[name].rect;
        let pixels = (y..y + height)
            .flat_map(|py| (x..x + width).map(move |px| ((py * icon.width + px) * 4 + 3) as usize))
            .map(|offset| icon.rgba[offset])
            .collect();
        AlphaTile {
            width,
            height,
            pixels,
        }
    });
    let skin = Arc::new(Skin {
        manifest,
        texture,
        title_alpha,
    });
    ctx.data_mut(|data| data.insert_temp(id, skin.clone()));
    skin
}

fn tiles(destination: Rect, source: Rect, size: Vec2) -> Vec<(Rect, Rect)> {
    if !destination.is_finite()
        || !destination.is_positive()
        || !size.is_finite()
        || size.x <= 0.
        || size.y <= 0.
    {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut y = destination.top();
    while y < destination.bottom() {
        let height = size.y.min(destination.bottom() - y);
        let mut x = destination.left();
        while x < destination.right() {
            let width = size.x.min(destination.right() - x);
            let rect = Rect::from_min_size(egui::pos2(x, y), egui::vec2(width, height));
            let uv = Rect::from_min_size(
                source.min,
                source.size() * egui::vec2(width / size.x, height / size.y),
            );
            result.push((rect, uv));
            x += width;
        }
        y += height;
    }
    result
}

impl Skin {
    fn source(&self, name: &str) -> (Rect, Vec2) {
        let [x, y, w, h] = self.manifest.regions[name].rect;
        let size = egui::vec2(self.manifest.size[0] as f32, self.manifest.size[1] as f32);
        (
            Rect::from_min_max(
                (egui::vec2(x as f32, y as f32) / size).to_pos2(),
                (egui::vec2((x + w) as f32, (y + h) as f32) / size).to_pos2(),
            ),
            egui::vec2(w as f32, h as f32),
        )
    }
    fn sprite(&self, painter: &egui::Painter, name: &str, rect: Rect, color: Color32) {
        if rect.is_positive() && painter.clip_rect().intersects(rect) {
            painter.image(self.texture.id(), rect, self.source(name).0, color);
        }
    }
    fn tiled(&self, painter: &egui::Painter, name: &str, rect: Rect, scale: Vec2, color: Color32) {
        if !rect.is_positive() || !painter.clip_rect().intersects(rect) {
            return;
        }
        let (uv, size) = self.source(name);
        let mut mesh = egui::Mesh::with_texture(self.texture.id());
        for (rect, uv) in tiles(rect, uv, size * scale) {
            mesh.add_rect_with_uv(rect, uv, color);
        }
        painter.add(egui::Shape::mesh(mesh));
    }
    fn panel_shape(&self, style: &str, rect: Rect) -> egui::Shape {
        let style = &self.manifest.styles[style];
        let [l, t, r, b] = style
            .borders_ltrb_px
            .expect("panel borders")
            .map(|v| v as f32);
        if rect.width() < l + r || rect.height() < t + b {
            return egui::Shape::Noop;
        }
        let mut mesh = egui::Mesh::with_texture(self.texture.id());
        let mut shapes = Vec::new();
        let inner = Rect::from_min_max(rect.min + egui::vec2(l, t), rect.max - egui::vec2(r, b));
        if let Some(center) = style.parts.get("center") {
            let (uv, size) = self.source(center);
            for (d, uv) in tiles(inner, uv, size) {
                mesh.add_rect_with_uv(d, uv, Color32::WHITE);
            }
        } else if let Some([r, g, b, a]) = style.center_rgba8 {
            shapes.push(egui::Shape::rect_filled(
                inner,
                0.,
                Color32::from_rgba_unmultiplied(r, g, b, a),
            ));
        }
        for (name, point, size) in [
            ("top_left", rect.min, egui::vec2(l, t)),
            (
                "top_right",
                egui::pos2(rect.right() - r, rect.top()),
                egui::vec2(r, t),
            ),
            (
                "bottom_left",
                egui::pos2(rect.left(), rect.bottom() - b),
                egui::vec2(l, b),
            ),
            (
                "bottom_right",
                rect.max - egui::vec2(r, b),
                egui::vec2(r, b),
            ),
            (
                "top",
                rect.min + egui::vec2(l, 0.),
                egui::vec2(rect.width() - l - r, t),
            ),
            (
                "bottom",
                egui::pos2(rect.left() + l, rect.bottom() - b),
                egui::vec2(rect.width() - l - r, b),
            ),
            (
                "left",
                rect.min + egui::vec2(0., t),
                egui::vec2(l, rect.height() - t - b),
            ),
            (
                "right",
                egui::pos2(rect.right() - r, rect.top() + t),
                egui::vec2(r, rect.height() - t - b),
            ),
        ] {
            let (uv, tile_size) = self.source(&style.parts[name]);
            for (d, uv) in tiles(Rect::from_min_size(point, size), uv, tile_size) {
                mesh.add_rect_with_uv(d, uv, Color32::WHITE);
            }
        }
        shapes.push(egui::Shape::mesh(mesh));
        egui::Shape::Vec(shapes)
    }
}

pub fn background(ui: &egui::Ui, collapsed: bool) {
    if collapsed {
        return;
    }
    let ctx = ui.ctx();
    let painter = ctx.layer_painter(egui::LayerId::background());
    let rect = ctx.content_rect();
    painter.rect_filled(rect, 0., ui.visuals().window_fill);
    let skin = get(ctx);
    painter.add(skin.panel_shape("board", rect));
    painter.rect_filled(rect.shrink(12.), 0., Color32::from_black_alpha(120));
}

/// The source right cap includes its own X. Its close action paints that cap once.
pub struct TitleBar {
    pub content: Rect,
    pub close_rect: Rect,
    right_cap: Rect,
    paint: Rect,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) struct TitleClip {
    rect: Rect,
    pixels_per_point: f32,
}

impl TitleBar {
    pub(super) fn clip(&self, pixels_per_point: f32) -> TitleClip {
        TitleClip {
            rect: self.paint,
            pixels_per_point,
        }
    }
    pub fn close(self, ui: &mut egui::Ui, hint: &str) -> egui::Response {
        let response = control_response(
            ui,
            self.close_rect,
            ui.make_persistent_id("title-close"),
            hint,
        );
        let tint = if response.is_pointer_button_down_on() {
            Color32::from_gray(170)
        } else if response.hovered() {
            Color32::from_rgb(255, 235, 185)
        } else {
            Color32::WHITE
        };
        let skin = get(ui.ctx());
        skin.sprite(
            ui.painter(),
            &skin.manifest.styles["titlebar"].parts["right"],
            self.right_cap,
            tint,
        );
        control_focus(ui, &response);
        response
    }
}

/// Reserve the native logical strip; the original 32 px artwork paints beyond it.
pub fn title(ui: &mut egui::Ui) -> TitleBar {
    let skin = get(ui.ctx());
    let style = &skin.manifest.styles["titlebar"];
    let [l, r] = style.cap_widths_px.unwrap().map(|v| v as f32);
    let height = style.paint_height_px.unwrap() as f32;
    let logical_height = style.logical_height_px.unwrap() as f32;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), logical_height),
        egui::Sense::hover(),
    );
    let content = rect;
    let title = TitleBar {
        content,
        paint: Rect::from_min_size(rect.min, egui::vec2(rect.width(), height)),
        // In the 32 px source cap the integrated X is centered at x=21.
        close_rect: Rect::from_center_size(
            egui::pos2(rect.right() - 11., content.center().y),
            Vec2::splat(26.),
        ),
        right_cap: Rect::from_min_size(
            egui::pos2(rect.right() - r, rect.top()),
            egui::vec2(r, height),
        ),
    };
    if rect.width() < l + r {
        return title;
    }
    for (name, x, w) in [
        ("left", rect.left(), l),
        ("center", rect.left() + l, rect.width() - l - r),
    ] {
        skin.tiled(
            ui.painter(),
            &style.parts[name],
            Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(w, height)),
            Vec2::splat(1.),
            Color32::WHITE,
        );
    }
    title
}

/// Shared compact native title for the startup and independent update window.
pub(crate) fn simple_title(ui: &mut egui::Ui, text: &str) -> bool {
    let bar = title(ui);
    let color = Color32::from_rgb(245, 235, 214);
    let galley = ui
        .painter()
        .layout_no_wrap(text.into(), egui::FontId::proportional(13.), color);
    let position = egui::pos2(
        bar.content.left() + 24. - galley.mesh_bounds.left(),
        bar.content.center().y - galley.mesh_bounds.center().y,
    );
    let scale = ui.ctx().pixels_per_point();
    ui.painter().galley(
        egui::pos2(
            (position.x * scale).round() / scale,
            (position.y * scale).round() / scale,
        ),
        galley,
        color,
    );
    let drag = Rect::from_min_max(
        bar.content.min,
        egui::pos2(bar.close_rect.left(), bar.content.bottom()),
    );
    if ui
        .interact(
            drag,
            ui.make_persistent_id("simple-title-drag"),
            egui::Sense::drag(),
        )
        .drag_started()
    {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    bar.close(ui, "Close Companion").clicked()
}

/// Pixel runs from the same atlas that paints the title, including DPI and tiled center.
/// Windows clips the collapsed native surface to these runs, independent of GPU alpha support.
pub(super) fn title_clip_runs(ctx: &egui::Context, clip: TitleClip) -> Vec<[i32; 4]> {
    let skin = get(ctx);
    let [left_cap, right_cap] = skin.manifest.styles["titlebar"]
        .cap_widths_px
        .unwrap()
        .map(|v| v as f32);
    let rect = clip.rect;
    let scale = clip.pixels_per_point;
    let mut runs = Vec::new();
    for py in (rect.top() * scale).floor() as i32..(rect.bottom() * scale).ceil() as i32 {
        let y = (py as f32 + 0.5) / scale - rect.top();
        let mut start = None;
        let end = (rect.right() * scale).ceil() as i32;
        for px in (rect.left() * scale).floor() as i32..=end {
            let x = (px as f32 + 0.5) / scale - rect.left();
            let alpha = if px == end || x < 0. || x >= rect.width() || y < 0. || y >= rect.height()
            {
                0
            } else if x < left_cap {
                skin.title_alpha[0].sample(x, y)
            } else if x < rect.width() - right_cap {
                skin.title_alpha[1].sample(x - left_cap, y)
            } else {
                let tile = &skin.title_alpha[2];
                tile.sample(
                    (x - rect.width() + right_cap) * tile.width as f32 / right_cap,
                    y * tile.height as f32 / rect.height(),
                )
            };
            if alpha >= 128 {
                start.get_or_insert(px);
            } else if let Some(left) = start.take() {
                runs.push([left, py, px, py + 1]);
            }
        }
    }
    runs
}

pub fn sprite(ui: &egui::Ui, name: &str, rect: Rect) {
    let color = Color32::WHITE;
    get(ui.ctx()).sprite(ui.painter(), name, rect, color);
}

pub fn control(ui: &egui::Ui, response: &egui::Response, style: &str, selected: bool) {
    let skin = get(ui.ctx());
    let style = &skin.manifest.styles[style];
    let state = if selected || response.is_pointer_button_down_on() {
        "pressed"
    } else if response.hovered() {
        "hovered"
    } else {
        "normal"
    };
    let source = &style.states[state];
    let (uv, size) = skin.source(source);
    let rect = response.rect;
    let tint = Color32::WHITE;
    // Explicit scalable application variant: fixed-height end caps and tiled center.
    let scale = rect.height() / size.y;
    let cap = (size.x * 0.22).min(12.);
    let edge = (cap * scale).min(rect.width() / 2.);
    for (dst, src) in [
        (
            Rect::from_min_size(rect.min, egui::vec2(edge, rect.height())),
            Rect::from_min_max(
                uv.min,
                egui::pos2(uv.left() + uv.width() * cap / size.x, uv.bottom()),
            ),
        ),
        (
            Rect::from_min_size(
                egui::pos2(rect.right() - edge, rect.top()),
                egui::vec2(edge, rect.height()),
            ),
            Rect::from_min_max(
                egui::pos2(uv.right() - uv.width() * cap / size.x, uv.top()),
                uv.max,
            ),
        ),
    ] {
        ui.painter().image(skin.texture.id(), dst, src, tint);
    }
    let center = Rect::from_min_max(
        egui::pos2(uv.left() + uv.width() * cap / size.x, uv.top()),
        egui::pos2(uv.right() - uv.width() * cap / size.x, uv.bottom()),
    );
    let dst = Rect::from_min_max(
        rect.min + egui::vec2(edge, 0.),
        rect.max - egui::vec2(edge, 0.),
    );
    let mut mesh = egui::Mesh::with_texture(skin.texture.id());
    for (d, uv) in tiles(
        dst,
        center,
        egui::vec2((size.x - 2. * cap) * scale, rect.height()),
    ) {
        mesh.add_rect_with_uv(d, uv, tint);
    }
    ui.painter().add(egui::Shape::mesh(mesh));
    if response.has_focus() || selected {
        ui.painter().rect_stroke(
            rect.shrink(1.),
            0.,
            egui::Stroke::new(1., super::widgets::BRONZE),
            egui::StrokeKind::Inside,
        );
    }
}

pub fn native_sprite(ui: &egui::Ui, style: &str, response: &egui::Response, rect: Rect) {
    let skin = get(ui.ctx());
    skin.sprite(
        ui.painter(),
        native_region(&skin, style, response),
        rect,
        Color32::WHITE,
    );
}

fn native_region<'a>(skin: &'a Skin, style: &str, response: &egui::Response) -> &'a str {
    let state = if response.is_pointer_button_down_on() {
        "pressed"
    } else if response.hovered() {
        "hovered"
    } else {
        "normal"
    };
    &skin.manifest.styles[style].states[state]
}

pub fn native_control(ui: &mut egui::Ui, style: &str, hint: &str) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(26., 26.), egui::Sense::hover());
    native_control_at(ui, style, rect, hint)
}

/// Native source dimensions and a larger hit target are independent.
pub fn native_control_at(ui: &mut egui::Ui, style: &str, rect: Rect, hint: &str) -> egui::Response {
    let response = control_response(
        ui,
        rect,
        ui.make_persistent_id(("native-control", style)),
        hint,
    );
    let skin = get(ui.ctx());
    let source = native_region(&skin, style, &response);
    skin.sprite(
        ui.painter(),
        source,
        Rect::from_center_size(rect.center(), skin.source(source).1),
        Color32::WHITE,
    );
    control_focus(ui, &response);
    response
}

fn control_focus(ui: &egui::Ui, response: &egui::Response) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect,
            0.,
            egui::Stroke::new(1., super::widgets::BRONZE),
            egui::StrokeKind::Inside,
        );
    }
}

fn control_response(ui: &mut egui::Ui, rect: Rect, id: egui::Id, hint: &str) -> egui::Response {
    let response = ui.interact(rect, id, egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), hint));
    #[cfg(test)]
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::unique(("control", hint)), rect));
    response.on_hover_text(hint)
}

pub fn bevel(ui: &egui::Ui, rect: Rect, focused: bool) {
    ui.painter()
        .rect_filled(rect, 0., ui.visuals().extreme_bg_color);
    let top = if focused {
        super::widgets::BRONZE
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke.color
    };
    let bottom = Color32::from_gray(145);
    ui.painter().line_segment(
        [rect.left_bottom(), rect.left_top()],
        egui::Stroke::new(1., top),
    );
    ui.painter().line_segment(
        [rect.left_top(), rect.right_top()],
        egui::Stroke::new(1., top),
    );
    ui.painter().line_segment(
        [rect.right_top(), rect.right_bottom()],
        egui::Stroke::new(1., bottom),
    );
    ui.painter().line_segment(
        [rect.right_bottom(), rect.left_bottom()],
        egui::Stroke::new(1., bottom),
    );
}

pub fn gauge(ui: &egui::Ui, rect: Rect, fraction: Option<f32>) {
    if rect.width() < 48. {
        return;
    }
    let skin = get(ui.ctx());
    let style = &skin.manifest.styles["gauge_blue"];
    let y = rect.center().y - 4.;
    for (name, x, w) in [
        ("left", rect.left(), 16.),
        ("center", rect.left() + 16., rect.width() - 32.),
        ("right", rect.right() - 16., 16.),
    ] {
        skin.tiled(
            ui.painter(),
            &style.parts[name],
            Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, 8.)),
            Vec2::splat(1.),
            Color32::WHITE,
        );
    }
    if let Some(f) = fraction.filter(|f| f.is_finite()) {
        let [l, t, r, _] = style.fill_insets_ltrb_px.unwrap().map(|v| v as f32);
        skin.tiled(
            ui.painter(),
            style.fill_region.as_deref().unwrap(),
            Rect::from_min_size(
                egui::pos2(rect.left() + l, y + t),
                egui::vec2((rect.width() - l - r) * f.clamp(0., 1.), 8.),
            ),
            Vec2::splat(1.),
            Color32::WHITE,
        );
    }
}

pub fn popup_frame(ui: &egui::Ui) -> egui::Frame {
    egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(egui::Stroke::new(1., super::widgets::BRONZE))
        .inner_margin(8.)
}

/// A replaceable background shape lets a dynamic panel paint behind its content.
pub fn panel_shape(ui: &egui::Ui, rect: Rect) -> egui::Shape {
    if rect.width() < 32. || rect.height() < 32. {
        return egui::Shape::rect_filled(rect, 0., ui.visuals().extreme_bg_color);
    }
    get(ui.ctx()).panel_shape("thinboard", rect)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_skin_regions_and_styles_are_complete_and_upload_once_per_context() {
        let ctx = egui::Context::default();
        let first = get(&ctx);
        let second = get(&ctx);
        assert!(Arc::ptr_eq(&first, &second));
        for region in first.manifest.regions.values() {
            let [x, y, w, h] = region.rect;
            assert!(
                w > 0
                    && h > 0
                    && x + w <= first.manifest.size[0]
                    && y + h <= first.manifest.size[1]
            );
        }
        for style in first.manifest.styles.values() {
            for name in style
                .parts
                .values()
                .chain(style.states.values())
                .chain(style.fill_region.iter())
            {
                assert!(
                    first.manifest.regions.contains_key(name),
                    "Missing sprite {name}"
                );
            }
        }
        let output = ctx.run_ui(Default::default(), |ui| {
            assert!(matches!(
                first.panel_shape(
                    "board",
                    Rect::from_min_size(egui::Pos2::ZERO, Vec2::splat(30.))
                ),
                egui::Shape::Noop
            ));
            let shape = first.panel_shape(
                "board",
                Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400., 620.)),
            );
            ui.painter().add(shape);
        });
        let mut output = output;
        output.textures_delta.clear();
    }
    #[test]
    fn partial_atlas_tiles_cover_destination_without_sampling_adjacent_regions() {
        let destination = Rect::from_min_size(egui::pos2(3., 7.), egui::vec2(35., 19.));
        let source = Rect::from_min_max(egui::pos2(0.25, 0.5), egui::pos2(0.5, 0.75));
        let result = tiles(destination, source, egui::vec2(16., 16.));
        assert_eq!(result.len(), 6);
        let area: f32 = result.iter().map(|(d, _)| d.area()).sum();
        assert_eq!(area, destination.area());
        for (d, uv) in &result {
            assert!(destination.contains_rect(*d));
            assert!(source.contains_rect(*uv));
        }
        let last = result.last().unwrap();
        assert_eq!(last.0.size(), egui::vec2(3., 3.));
        assert!((last.1.width() - source.width() * 3. / 16.).abs() < 0.00001);
    }
    #[test]
    fn invalid_or_empty_tile_destinations_produce_no_geometry() {
        let source = Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.));
        for size in [Vec2::ZERO, egui::vec2(-1., 5.), egui::vec2(f32::NAN, 2.)] {
            assert!(tiles(source, source, size).is_empty());
        }
        assert!(tiles(Rect::NOTHING, source, Vec2::splat(16.)).is_empty());
    }
}
