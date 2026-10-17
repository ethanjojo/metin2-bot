use super::*;
use metin_core::Region;
pub(super) struct DetectionCache {
    profile: Profile,
    frame: metin_platform::Frame,
    template: Option<metin_platform::Frame>,
    candidates: Vec<metin_core::Candidate>,
    score: f64,
}
fn same_template(a: &Option<metin_platform::Frame>, b: &Option<metin_platform::Frame>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(&a.bgr, &b.bgr),
        (None, None) => true,
        _ => false,
    }
}
impl MetinApp {
    pub(super) fn update_frame_texture(
        &mut self,
        ctx: &egui::Context,
        frame: &metin_platform::Frame,
    ) {
        if self
            .texture_source
            .as_ref()
            .is_none_or(|previous| !Arc::ptr_eq(&previous.bgr, &frame.bgr))
        {
            let rgb = frame
                .bgr
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|p| [p[2], p[1], p[0]])
                .collect::<Vec<_>>();
            let image =
                egui::ColorImage::from_rgb([frame.width as usize, frame.height as usize], &rgb);
            if let Some(texture) = &mut self.texture {
                texture.set(image, egui::TextureOptions::LINEAR);
            } else {
                self.texture =
                    Some(ctx.load_texture("game-frame", image, egui::TextureOptions::LINEAR));
            }
            self.texture_source = Some(frame.clone());
        }
    }
    fn editing_status(&mut self, ui: &mut egui::Ui) {
        ui.add_enabled_ui(self.editing, |ui| {
            if ui.button("Done editing").on_hover_text("Turn off this tool. Completed edits remain in the profile. Escape also turns it off.").clicked() {
                self.editing = false;
                self.gesture = None;
            }
        });
        let instruction = match self.mode {
            EditMode::SafetyReference(_) => {
                "Show the event in the game, take a Snapshot, then capture a distinctive 5–128 px icon or fixed part of its dialog."
            }
            EditMode::VisualClick(_) | EditMode::PeriodicClick(_) => {
                "Draw a small region inside the intended button. A click uses its center in source pixels; keep it clear of adjacent controls."
            }
            EditMode::Color => "Click a Metin pixel in the frozen image.",
            EditMode::Mask(None) => "Drag a rectangle over chat, minimap or false detections.",
            EditMode::BuffArea => "Drag around the area where active buff icons appear.",
            EditMode::BuffIcon { inactive: true, .. } => {
                "Show the inactive state in the game, take a fresh Snapshot, then drag around its icon."
            }
            EditMode::BuffIcon {
                inactive: false, ..
            } => {
                "Show the active state in the game, take a fresh Snapshot, then drag around its icon."
            }
            _ => "Drag to draw or move; use edge handles to resize.",
        };
        crate::ui_controls::help(
            ui,
            &format!("{instruction} Escape cancels. Wheel zooms; middle drag pans."),
        );
        ui.add(
            egui::Label::new(
                egui::RichText::new(if self.editing {
                    format!("Editing: {}", tool_name(self.mode))
                } else {
                    "Select an edit tool".into()
                })
                .color(if self.editing {
                    widgets::BLUE
                } else {
                    ui.visuals().weak_text_color()
                }),
            )
            .truncate(),
        );
    }
    fn preview_view_controls(&mut self, ui: &mut egui::Ui) {
        // Emitted from right to left so the tool group stays flush to the edge.
        if widgets::button(
            ui,
            widgets::Icon::Redo,
            "",
            self.editor.can_redo(),
            "Redo the last undone edit. Ctrl+Y or Ctrl+Shift+Z.",
        )
        .clicked()
        {
            self.redo_profile();
        }
        if widgets::button(
            ui,
            widgets::Icon::Undo,
            "",
            self.editor.can_undo(),
            "Undo the last profile or calibration edit. Ctrl+Z / Cmd+Z.",
        )
        .clicked()
        {
            self.undo_profile();
        }
        ui.label(format!("{:.0}%", self.viewport.zoom * 100.));
        if widgets::button(
            ui,
            widgets::Icon::Plus,
            "",
            true,
            "Zoom in. Hold the middle mouse button to pan.",
        )
        .clicked()
        {
            self.viewport.zoom_at([0., 0.], 1.25);
        }
        if widgets::button(
            ui,
            widgets::Icon::Minus,
            "",
            true,
            "Zoom out. Mouse wheel also zooms around the pointer.",
        )
        .clicked()
        {
            self.viewport.zoom_at([0., 0.], 1. / 1.25);
        }
        if widgets::button(
            ui,
            widgets::Icon::Target,
            "Fit",
            true,
            "Fit the complete frame in the available canvas and reset zoom.",
        )
        .clicked()
        {
            self.viewport.reset();
        }
    }
    fn preview_overlays(&mut self, ui: &mut egui::Ui) {
        if ui
            .checkbox(&mut self.live, "Live preview")
            .on_hover_text("Show only the selected game window. Take a Snapshot before editing.")
            .changed()
            && self.live
        {
            self.editing = false;
            self.gesture = None;
        }
        ui.checkbox(&mut self.show_sizes, "Sizes (px)").on_hover_text("Show source-pixel dimensions above areas and visible detections. Independent of Detections.");
        ui.checkbox(&mut self.show_detections, "Detections")
            .on_hover_text("Show detected Metin candidates on this image.");
    }
    pub(super) fn preview(&mut self, ui: &mut egui::Ui) {
        let preview = self.presentation_snapshot();
        // Both layouts reserve stable space across idle, armed and live states.
        let wide = ui.available_width() >= 850.;
        ui.horizontal(|ui| {
            let editing_width = (ui.available_width() - if wide { 620. } else { 280. }).max(180.);
            ui.allocate_ui_with_layout(
                egui::vec2(editing_width, 32.),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| self.editing_status(ui),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.preview_view_controls(ui);
                if wide {
                    self.preview_overlays(ui);
                }
            });
        });
        if !wide {
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 32.),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| self.preview_overlays(ui),
            );
        }
        let current = if preview.key == self.key() {
            preview.frame.clone()
        } else {
            None
        };
        let frozen = !self.live && self.editor.frozen.is_some();
        let frame = if self.live {
            current
        } else {
            self.editor.frozen.clone()
        };
        let Some(frame) = frame else {
            ui.add_space(32.);
            ui.heading("Calibrate the game view");
            ui.add(
                egui::Label::new(
                    "Select a game window and take a Snapshot with its target panel visible.",
                )
                .wrap(),
            );
            ui.add(
                egui::Label::new(
                    "Then select the search area, Metin color, target panel and fixed crop.",
                )
                .wrap(),
            );
            return;
        };
        let stale = !frozen
            && preview.captured_at.is_none_or(|time| {
                time.elapsed().as_secs_f64() > self.editor.profile.timing.sample_interval * 2. + 1.
            });
        if frozen
            && self.detection_cache.as_ref().is_none_or(|cache| {
                cache.profile != self.editor.profile
                    || !Arc::ptr_eq(&cache.frame.bgr, &frame.bgr)
                    || !same_template(&cache.template, &self.editor.template)
            })
        {
            let candidates = match metin_vision::detect_candidates(&frame, &self.editor.profile) {
                Ok(c) => c,
                Err(error) => {
                    self.error = Some(error.to_string());
                    vec![]
                }
            };
            let score = if let (Some(template), Some(region)) =
                (&self.editor.template, self.editor.profile.panel.region)
            {
                match metin_vision::panel_score(&frame, template, region) {
                    Ok(score) => score,
                    Err(error) => {
                        self.error = Some(error.to_string());
                        0.
                    }
                }
            } else {
                0.
            };
            self.detection_cache = Some(DetectionCache {
                profile: self.editor.profile.clone(),
                frame: frame.clone(),
                template: self.editor.template.clone(),
                candidates,
                score,
            });
        }
        ui.horizontal_wrapped(|ui| {
            ui.strong(if frozen {
                "Frozen frame"
            } else if stale {
                "Stale frame"
            } else {
                "Live preview"
            });
            ui.label(format!("{} × {} pixels", frame.width, frame.height));
            ui.label(format!(
                "Panel match {:.3}",
                if frozen {
                    self.detection_cache
                        .as_ref()
                        .map_or(0., |cache| cache.score)
                } else {
                    preview.panel_score
                }
            ));
            if self.editor.dirty() {
                ui.label("Unsaved changes");
            }
        });
        if !frozen {
            ui.label("Live preview sends no input. Take a Snapshot before editing.");
        }
        self.update_frame_texture(ui.ctx(), &frame);
        let (canvas, response) = ui.allocate_exact_size(
            ui.available_size().max(egui::vec2(1., 1.)),
            egui::Sense::click_and_drag(),
        );
        #[cfg(test)]
        {
            self.canvas_rect = Some(canvas);
        }
        let painter = ui.painter_at(canvas);
        painter.rect_filled(
            canvas,
            0.,
            if ui.visuals().dark_mode {
                egui::Color32::from_rgb(13, 19, 27)
            } else {
                egui::Color32::from_rgb(224, 229, 236)
            },
        );
        self.viewport.fit(
            [frame.width, frame.height],
            [f64::from(canvas.width()), f64::from(canvas.height())],
        );
        let pointer = response
            .interact_pointer_pos()
            .or_else(|| ui.input(|i| i.pointer.hover_pos()));
        let local = pointer.map(|p| {
            [
                f64::from(p.x - canvas.left()),
                f64::from(p.y - canvas.top()),
            ]
        });
        if response.hovered() && ui.ctx().current_pass_index() == 0 {
            let wheel = ui.input(|i| {
                i.events
                    .iter()
                    .filter_map(|event| match event {
                        egui::Event::MouseWheel { unit, delta, .. } => Some(
                            delta.y
                                * match unit {
                                    egui::MouseWheelUnit::Point => 1.,
                                    egui::MouseWheelUnit::Line => 16.,
                                    egui::MouseWheelUnit::Page => canvas.height(),
                                },
                        ),
                        _ => None,
                    })
                    .sum::<f32>()
            });
            if wheel != 0. {
                self.viewport.zoom_at(
                    local.unwrap_or([0., 0.]),
                    1.25_f64.powf(f64::from(wheel) / 50.),
                );
            }
        }
        if response.dragged_by(egui::PointerButton::Middle) {
            let delta = ui.input(|i| i.pointer.delta());
            self.viewport.offset[0] += f64::from(delta.x);
            self.viewport.offset[1] += f64::from(delta.y);
        }
        let image_start = self.viewport.to_canvas([0., 0.]);
        let image_end = self
            .viewport
            .to_canvas([f64::from(frame.width), f64::from(frame.height)]);
        let image_rect = egui::Rect::from_min_max(
            canvas.min + egui::vec2(image_start[0] as f32, image_start[1] as f32),
            canvas.min + egui::vec2(image_end[0] as f32, image_end[1] as f32),
        );
        if let Some(texture) = &self.texture {
            painter.image(
                texture.id(),
                image_rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                egui::Color32::WHITE,
            );
        }
        let screen_rect = |r: Region, viewport: &Viewport| {
            let a = viewport.to_canvas([
                r.left * f64::from(frame.width),
                r.top * f64::from(frame.height),
            ]);
            let b = viewport.to_canvas([
                r.right * f64::from(frame.width),
                r.bottom * f64::from(frame.height),
            ]);
            egui::Rect::from_min_max(
                canvas.min + egui::vec2(a[0] as f32, a[1] as f32),
                canvas.min + egui::vec2(b[0] as f32, b[1] as f32),
            )
        };
        let mut areas = vec![];
        for mode in [
            EditMode::World,
            EditMode::Panel,
            EditMode::Template,
            EditMode::BuffArea,
        ] {
            if let Some(r) = self.editor.region(mode) {
                areas.push((mode, r));
            }
        }
        for (i, r) in self.editor.profile.masks.iter().copied().enumerate() {
            areas.push((EditMode::Mask(Some(i)), r));
        }
        if matches!(
            self.mode,
            EditMode::BuffIcon { .. }
                | EditMode::SafetyReference(_)
                | EditMode::VisualClick(_)
                | EditMode::PeriodicClick(_)
        ) && let Some(r) = self.editor.region(self.mode)
        {
            areas.push((self.mode, r));
        }
        let mut measurement_requests = Vec::new();
        for (mode, r) in areas {
            let active = self.editing && mode == self.mode;
            let color = if active {
                egui::Color32::from_rgb(93, 161, 255)
            } else if self.hover_mode == Some(mode) {
                egui::Color32::YELLOW
            } else if matches!(mode, EditMode::Mask(_)) {
                egui::Color32::from_rgb(240, 104, 100)
            } else {
                egui::Color32::from_rgb(100, 205, 180)
            };
            let rect = screen_rect(r, &self.viewport);
            painter.rect_stroke(
                rect,
                0.,
                egui::Stroke::new(if active { 2. } else { 1. }, color),
                egui::StrokeKind::Inside,
            );
            let name = match mode {
                EditMode::SafetyReference(_) => "Visual reference",
                EditMode::VisualClick(_) | EditMode::PeriodicClick(_) => "Click target",
                EditMode::World => "Search area",
                EditMode::Panel => "Target panel",
                EditMode::Template => "Reference crop",
                EditMode::BuffArea => "Buff detection area",
                EditMode::Mask(_) => "Excluded area",
                EditMode::BuffIcon { inactive: true, .. } => "Inactive icon",
                EditMode::BuffIcon { .. } => "Active icon",
                _ => "Color",
            };
            if mode == self.hover_mode.unwrap_or(self.mode)
                && rect.width() > 90.
                && rect.height() > 24.
            {
                painter.text(
                    rect.min + egui::vec2(7., 5.),
                    egui::Align2::LEFT_TOP,
                    name,
                    egui::FontId::proportional(13.),
                    color,
                );
            }
            if active && frozen {
                for p in handles(rect) {
                    painter.rect_filled(
                        egui::Rect::from_center_size(p, egui::vec2(7., 7.)),
                        1.,
                        color,
                    );
                }
            }
            if self.show_sizes {
                let [x1, y1, x2, y2] = r.pixels(frame.width, frame.height);
                measurement_requests.push((
                    rect,
                    format!("{} × {} px", x2 - x1 + 1, y2 - y1 + 1),
                    color,
                ));
            }
        }
        if self.show_detections {
            let candidates = if frozen {
                self.detection_cache
                    .as_ref()
                    .map_or_else(Vec::new, |cache| cache.candidates.clone())
            } else {
                preview.candidates.clone()
            };
            for (i, c) in candidates.iter().enumerate() {
                let r = Region {
                    left: f64::from(c.x) / f64::from(frame.width),
                    top: f64::from(c.y) / f64::from(frame.height),
                    right: f64::from(c.x + c.width) / f64::from(frame.width),
                    bottom: f64::from(c.y + c.height) / f64::from(frame.height),
                };
                let rect = screen_rect(r, &self.viewport);
                painter.rect_stroke(
                    rect,
                    0.,
                    egui::Stroke::new(1., egui::Color32::GREEN),
                    egui::StrokeKind::Inside,
                );
                painter.text(
                    rect.min,
                    egui::Align2::LEFT_TOP,
                    (i + 1).to_string(),
                    egui::FontId::monospace(12.),
                    egui::Color32::GREEN,
                );
                if self.show_sizes {
                    measurement_requests.push((
                        rect,
                        format!("{} × {} px", c.width, c.height),
                        egui::Color32::GREEN,
                    ));
                }
            }
        }
        if response.dragged_by(egui::PointerButton::Middle) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if let Some(gesture) = self.gesture {
            ui.ctx().set_cursor_icon(gesture_cursor(gesture));
        } else if response.hovered()
            && self.editing
            && let Some(start) = local.and_then(|p| self.viewport.to_image(p, false))
        {
            let cursor = if !frozen {
                egui::CursorIcon::NotAllowed
            } else if self.mode == EditMode::Color {
                egui::CursorIcon::Crosshair
            } else {
                gesture_cursor(gesture_at(
                    start,
                    pointer.unwrap(),
                    self.editor
                        .region(self.mode)
                        .map(|r| (r, screen_rect(r, &self.viewport))),
                ))
            };
            ui.ctx().set_cursor_icon(cursor);
        }
        if self.editing && frozen && self.binding.is_none() {
            if self.mode == EditMode::Color
                && response.clicked()
                && let Some(point) = local.and_then(|p| self.viewport.to_image(p, false))
            {
                self.runtime.stop();
                if let Err(e) = self.editor.color(point) {
                    self.error = Some(e);
                }
            }
            if self.mode != EditMode::Color
                && response.drag_started_by(egui::PointerButton::Primary)
            {
                self.runtime.stop();
                if let Some(local) = local {
                    let delta = response.drag_delta();
                    let start_local =
                        [local[0] - f64::from(delta.x), local[1] - f64::from(delta.y)];
                    if let Some(start) = self.viewport.to_image(start_local, false) {
                        let position =
                            canvas.min + egui::vec2(start_local[0] as f32, start_local[1] as f32);
                        self.gesture = Some(gesture_at(
                            start,
                            position,
                            self.editor
                                .region(self.mode)
                                .map(|r| (r, screen_rect(r, &self.viewport))),
                        ));
                    }
                }
            }
            if let Some(gesture) = self.gesture
                && let Some(point) = local.and_then(|p| self.viewport.to_image(p, true))
            {
                if let Some(region) = gesture.region(point, [frame.width, frame.height]) {
                    painter.rect_stroke(
                        screen_rect(region, &self.viewport),
                        0.,
                        egui::Stroke::new(2., egui::Color32::WHITE),
                        egui::StrokeKind::Inside,
                    );
                    if self.show_sizes {
                        let [x1, y1, x2, y2] = region.pixels(frame.width, frame.height);
                        measurement_requests.push((
                            screen_rect(region, &self.viewport),
                            format!("{} × {} px", x2 - x1 + 1, y2 - y1 + 1),
                            egui::Color32::WHITE,
                        ));
                    }
                    if response.drag_stopped_by(egui::PointerButton::Primary) {
                        if let Err(e) = self.editor.set_region(self.mode, region) {
                            self.error = Some(e);
                        } else if self.mode == EditMode::Mask(None) {
                            self.mode = EditMode::Mask(Some(self.editor.profile.masks.len() - 1));
                        }
                        self.gesture = None;
                    }
                } else if response.drag_stopped_by(egui::PointerButton::Primary) {
                    self.gesture = None;
                }
            }
        }
        // Paint tags last so other rectangles and resize handles cannot draw
        // through their text when a panel and its crop overlap.
        let mut measurements = Vec::new();
        for (rect, text, color) in measurement_requests {
            measurement(&painter, canvas, rect, text, color, &mut measurements);
        }
    }
}
fn measurement(
    painter: &egui::Painter,
    canvas: egui::Rect,
    area: egui::Rect,
    text: String,
    color: egui::Color32,
    occupied: &mut Vec<egui::Rect>,
) {
    if !canvas.intersects(area) {
        return;
    }
    let galley = painter.layout_no_wrap(text, egui::FontId::monospace(12.), color);
    let size = galley.size() + egui::vec2(8., 4.);
    if size.x > canvas.width() || size.y > canvas.height() {
        return;
    }
    let x = area.left().clamp(canvas.left(), canvas.right() - size.x);
    let y = (area.top() - size.y - 3.).clamp(canvas.top(), canvas.bottom() - size.y);
    let mut label = egui::Rect::from_min_size(egui::pos2(x, y), size);
    // Shared top edges are common for a panel and its crop. Shift the tag
    // along that edge rather than painting two measurements over each other.
    for _ in 0..occupied.len() + 1 {
        let Some(other) = occupied
            .iter()
            .find(|other| other.expand(2.).intersects(label))
        else {
            break;
        };
        if other.right() + 3. + size.x <= canvas.right() {
            label = egui::Rect::from_min_size(egui::pos2(other.right() + 3., label.top()), size);
        } else if other.bottom() + 3. + size.y <= canvas.bottom() {
            label = egui::Rect::from_min_size(egui::pos2(x, other.bottom() + 3.), size);
        } else {
            return;
        }
    }
    if occupied
        .iter()
        .any(|other| other.expand(2.).intersects(label))
    {
        return;
    }
    painter.rect_filled(
        label,
        3.,
        egui::Color32::from_rgba_unmultiplied(12, 18, 24, 225),
    );
    painter.galley(label.min + egui::vec2(4., 2.), galley, color);
    occupied.push(label);
}
fn handles(r: egui::Rect) -> [egui::Pos2; 8] {
    [
        r.left_top(),
        r.center_top(),
        r.right_top(),
        r.right_center(),
        r.right_bottom(),
        r.center_bottom(),
        r.left_bottom(),
        r.left_center(),
    ]
}

fn tool_name(mode: EditMode) -> &'static str {
    match mode {
        EditMode::SafetyReference(_) => "Visual reference",
        EditMode::VisualClick(_) | EditMode::PeriodicClick(_) => "Click target",
        EditMode::World => "Search area",
        EditMode::Color => "Metin color - click a pixel",
        EditMode::Panel => "Target panel",
        EditMode::Template => "Reference crop",
        EditMode::BuffArea => "Buff detection area",
        EditMode::BuffIcon { inactive: true, .. } => "Inactive icon",
        EditMode::BuffIcon { .. } => "Active icon",
        EditMode::Mask(None) => "Draw excluded area",
        EditMode::Mask(Some(_)) => "Excluded area",
    }
}
fn gesture_at(
    start: [u32; 2],
    position: egui::Pos2,
    original: Option<(Region, egui::Rect)>,
) -> Gesture {
    if let Some((original, rect)) = original {
        if let Some(index) = handles(rect)
            .iter()
            .position(|p| p.distance(position) <= 9.)
        {
            return Gesture::Resize {
                start,
                original,
                handle: [
                    [-1, -1],
                    [0, -1],
                    [1, -1],
                    [1, 0],
                    [1, 1],
                    [0, 1],
                    [-1, 1],
                    [-1, 0],
                ][index],
            };
        }
        if rect.contains(position) {
            return Gesture::Move { start, original };
        }
    }
    Gesture::Create { start }
}
fn gesture_cursor(gesture: Gesture) -> egui::CursorIcon {
    match gesture {
        Gesture::Create { .. } => egui::CursorIcon::Crosshair,
        Gesture::Move { .. } => egui::CursorIcon::Move,
        Gesture::Resize { handle: [0, _], .. } => egui::CursorIcon::ResizeVertical,
        Gesture::Resize { handle: [_, 0], .. } => egui::CursorIcon::ResizeHorizontal,
        Gesture::Resize { handle, .. } if handle[0] == handle[1] => egui::CursorIcon::ResizeNwSe,
        Gesture::Resize { .. } => egui::CursorIcon::ResizeNeSw,
    }
}
