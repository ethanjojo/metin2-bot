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
    pub(super) fn preview(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.show_detections, "Detections");
            ui.checkbox(&mut self.live, "Live preview");
            if ui.button("Fit").clicked() {
                self.viewport.reset();
            }
            if ui.button("−").clicked() {
                self.viewport.zoom_at([0., 0.], 1. / 1.25);
            }
            if ui.button("+").clicked() {
                self.viewport.zoom_at([0., 0.], 1.25);
            }
            ui.label(format!("{:.0}%", self.viewport.zoom * 100.));
            if ui
                .add_enabled(self.editor.can_undo(), egui::Button::new("Undo"))
                .clicked()
            {
                self.runtime.stop();
                self.editor.undo();
            }
            if ui
                .add_enabled(self.editor.can_redo(), egui::Button::new("Redo"))
                .clicked()
            {
                self.runtime.stop();
                self.editor.redo();
            }
        });
        let current = if self.snapshot.key == self.key() {
            self.snapshot.frame.clone()
        } else {
            None
        };
        let frozen = !self.live && self.editor.frozen.is_some();
        let frame = if frozen {
            self.editor.frozen.clone()
        } else {
            current
        };
        let Some(frame) = frame else {
            ui.add_space(32.);
            ui.heading("Calibrate the game view");
            ui.label("Select a game window and take a Snapshot with its target panel visible.");
            ui.label("Then select the search area, Metin color, target panel and fixed crop.");
            return;
        };
        let stale = !frozen
            && self.snapshot.captured_at.is_none_or(|time| {
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
                    self.snapshot.panel_score
                }
            ));
            if self.editor.dirty() {
                ui.label("Unsaved changes");
            }
        });
        if !frozen {
            ui.label("Live preview sends no input. Take a Snapshot before editing.");
        }
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
                self.texture = Some(ui.ctx().load_texture(
                    "game-frame",
                    image,
                    egui::TextureOptions::LINEAR,
                ));
            }
            self.texture_source = Some(frame.clone());
        }
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
        if response.hovered() {
            let wheel = ui.input(|i| i.smooth_scroll_delta.y);
            if wheel != 0. {
                self.viewport.zoom_at(
                    local.unwrap_or([0., 0.]),
                    if wheel > 0. { 1.1 } else { 1. / 1.1 },
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
        if matches!(self.mode, EditMode::BuffIcon { .. })
            && let Some(r) = self.editor.region(self.mode)
        {
            areas.push((self.mode, r));
        }
        for (mode, r) in areas {
            let active = mode == self.mode;
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
            if active && frozen {
                for p in handles(rect) {
                    painter.rect_filled(
                        egui::Rect::from_center_size(p, egui::vec2(7., 7.)),
                        1.,
                        color,
                    );
                }
            }
        }
        if self.show_detections {
            let candidates = if frozen {
                self.detection_cache
                    .as_ref()
                    .map_or_else(Vec::new, |cache| cache.candidates.clone())
            } else {
                self.snapshot.candidates.clone()
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
            }
        }
        if frozen && self.binding.is_none() {
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
                        self.gesture =
                            Some(if let Some(original) = self.editor.region(self.mode) {
                                let rect = screen_rect(original, &self.viewport);
                                let handle = handles(rect)
                                    .iter()
                                    .position(|p| p.distance(position) <= 9.);
                                if let Some(i) = handle {
                                    Gesture::Resize {
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
                                        ][i],
                                    }
                                } else if rect.contains(position) {
                                    Gesture::Move { start, original }
                                } else {
                                    Gesture::Create { start }
                                }
                            } else {
                                Gesture::Create { start }
                            });
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
    }
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
