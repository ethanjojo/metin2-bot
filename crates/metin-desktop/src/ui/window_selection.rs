use super::*;
use metin_platform::TargetInfo;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
#[path = "../window_catalog.rs"]
mod catalog;

struct Thumbnail {
    texture: Option<egui::TextureHandle>,
    message: Option<String>,
}
impl Thumbnail {
    fn captured(context: &egui::Context, name: String, frame: &metin_platform::Frame) -> Self {
        let rgb = frame
            .bgr
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|pixel| [pixel[2], pixel[1], pixel[0]])
            .collect::<Vec<_>>();
        let texture = context.load_texture(
            name,
            egui::ColorImage::from_rgb([frame.width as usize, frame.height as usize], &rgb),
            egui::TextureOptions::LINEAR,
        );
        Self {
            texture: Some(texture),
            message: None,
        }
    }
}
pub(super) struct WindowChooser {
    pub(super) open: bool,
    resize_pending: bool,
    worker: Option<catalog::PreviewWorker>,
    previews: HashMap<TargetToken, Thumbnail>,
    requested: Vec<TargetToken>,
    generation: Option<u64>,
    requested_at: Instant,
    worker_error: Option<String>,
}
impl Default for WindowChooser {
    fn default() -> Self {
        Self {
            open: true,
            resize_pending: true,
            worker: None,
            previews: HashMap::new(),
            requested: Vec::new(),
            generation: None,
            requested_at: Instant::now(),
            worker_error: None,
        }
    }
}
impl WindowChooser {
    fn suspend(&mut self) {
        if let Some(worker) = &self.worker {
            worker.cancel();
        }
        self.requested.clear();
        self.generation = None;
    }
    fn close(&mut self, context: &egui::Context, size: egui::Vec2) {
        self.open = false;
        self.suspend();
        context.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(
            super::placement::expanded_minimum(context),
        ));
        context.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
    }
    fn poll(&mut self, context: &egui::Context, targets: &[TargetInfo]) {
        self.previews
            .retain(|token, _| targets.iter().any(|target| target.token == *token));
        let Some(worker) = &self.worker else { return };
        for _ in 0..catalog::MAX_VISIBLE_PREVIEWS {
            let Some(result) = worker.poll() else { break };
            if self.generation != Some(result.generation)
                || !targets.iter().any(|target| target.token == result.target)
            {
                continue;
            }
            let preview = match result.image {
                Ok(frame) => Thumbnail::captured(
                    context,
                    format!(
                        "window-{}-{}-{}",
                        result.target.handle, result.target.pid, result.target.created
                    ),
                    &frame,
                ),
                Err(message) => Thumbnail {
                    texture: None,
                    message: Some(message),
                },
            };
            self.previews.insert(result.target, preview);
        }
    }
    fn request_visible(&mut self, context: &egui::Context, mut targets: Vec<TargetToken>) {
        targets.truncate(catalog::MAX_VISIBLE_PREVIEWS);
        if targets.is_empty() {
            self.suspend();
            return;
        }
        if targets == self.requested && self.requested_at.elapsed() < Duration::from_secs(5) {
            return;
        }
        if self.worker.is_none() && self.worker_error.is_none() {
            match catalog::PreviewWorker::new(context.clone()) {
                Ok(worker) => self.worker = Some(worker),
                Err(error) => self.worker_error = Some(error),
            }
        }
        if let Some(worker) = &self.worker {
            worker.cancel();
            if let Ok(generation) = worker.request(targets.clone()) {
                self.generation = Some(generation);
                self.requested_at = Instant::now();
                self.requested = targets;
                if self.previews.len() > 64 {
                    self.previews
                        .retain(|token, _| self.requested.contains(token));
                }
            }
        }
    }
}

#[cfg(any(test, feature = "ui-smoke"))]
pub(super) struct WindowFixture {
    pub(super) snapshot: Arc<RuntimeSnapshot>,
}
#[cfg(feature = "ui-smoke")]
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum WindowPickerFixture {
    All,
    Filtered,
    Empty,
    Ambiguous,
    Missing,
    Selected,
}
#[cfg(feature = "ui-smoke")]
impl MetinApp {
    pub fn set_window_picker_fixture(&mut self, case: WindowPickerFixture) {
        let mut targets=vec![
            TargetInfo{token:TargetToken{handle:100,pid:1201,created:1},title:"Forest · Warrior".into(),process_name:"server-a.exe".into()},
            TargetInfo{token:TargetToken{handle:200,pid:1202,created:2},title:"Dungeon · Sura".into(),process_name:"server-b.exe".into()},
            TargetInfo{token:TargetToken{handle:300,pid:1203,created:3},title:"A very long application title demonstrating bounded window tiles and full hover help".into(),process_name:"other-client.exe".into()},
        ];
        if matches!(case, WindowPickerFixture::Ambiguous) {
            targets[1].title = targets[0].title.clone();
            targets[1].process_name = targets[0].process_name.clone();
        }
        self.window_search = match case {
            WindowPickerFixture::Filtered => "SERVER-B".into(),
            WindowPickerFixture::Empty => "no-such-window".into(),
            _ => String::new(),
        };
        self.target = matches!(case, WindowPickerFixture::Selected).then_some(targets[0].token);
        self.window_chooser.open = !matches!(case, WindowPickerFixture::Selected);
        self.window_chooser.resize_pending = self.window_chooser.open;
        self.target_list_revision = 0;
        self.window_fixture = Some(WindowFixture {
            snapshot: Arc::new(RuntimeSnapshot {
                targets,
                targets_revision: 1,
                ..Default::default()
            }),
        });
    }
}

pub(super) fn matches_search(window: &TargetInfo, search: &str) -> bool {
    let search = search.trim().to_lowercase();
    search.is_empty()
        || window.title.to_lowercase().contains(&search)
        || window.process_name.to_lowercase().contains(&search)
        || window.token.pid.to_string() == search
}
impl MetinApp {
    pub(super) fn select_window(&mut self, target: TargetToken) -> Result<(), String> {
        if !self
            .snapshot
            .targets
            .iter()
            .any(|window| window.token == target)
        {
            return Err("This window has closed. Choose another available window.".into());
        }
        self.runtime.stop();
        self.cancel_overlay_capture();
        self.history_open = false;
        self.target = Some(target);
        self.configure()?;
        self.window_chooser.open = false;
        self.window_chooser.suspend();
        Ok(())
    }
    pub(super) fn open_window_chooser(&mut self) {
        self.metin_presets.draft = None;
        self.remember_latest_panel_position();
        self.cancel_size_review();
        self.size_change = None;
        self.runtime.stop();
        self.cancel_overlay_capture();
        if let Some(overlay) = &self.overlay {
            let _ = overlay.set_control_panel(None);
        }
        self.panel_link = None;
        self.collapsed = false;
        self.window_chooser.open = true;
        self.window_chooser.resize_pending = true;
        self.window_chooser.suspend();
        self.window_search.clear();
        self.send(Command::RefreshTargets);
    }
    pub(super) fn reconcile_windows(&mut self) {
        if self.snapshot.targets_revision == 0
            || self.snapshot.targets_revision == self.target_list_revision
        {
            return;
        }
        self.target_list_revision = self.snapshot.targets_revision;
        if self.target.is_some_and(|target| {
            !self
                .snapshot
                .targets
                .iter()
                .any(|window| window.token == target)
        }) {
            self.target = None;
            self.send(Command::Detach);
            self.open_window_chooser();
        }
    }
    pub(super) fn window_chooser_view(&mut self, ui: &mut egui::Ui) {
        if self.window_chooser.resize_pending {
            self.window_chooser.resize_pending = false;
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::MinInnerSize(
                    super::placement::expanded_minimum(ui.ctx()),
                ));
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(780., 600.)));
        }
        ui.spacing_mut().item_spacing.y = 10.;
        let header = ui.horizontal(|ui| {
            let width = (ui.available_width() - 32.).max(1.);
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, 32.), egui::Sense::drag());
            widgets::icon(
                ui.painter(),
                rect.left_center() + egui::vec2(12., 0.),
                widgets::Icon::Target,
                widgets::BRONZE,
            );
            ui.painter().text(
                rect.left_center() + egui::vec2(32., 0.),
                egui::Align2::LEFT_CENTER,
                "Choose a window",
                egui::FontId::proportional(20.),
                ui.visuals().text_color(),
            );
            if response.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if skin::native_control(ui, "close_button", "Close Companion").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        ui.painter().hline(
            header.response.rect.x_range(),
            header.response.rect.bottom() + 3.,
            egui::Stroke::new(1., widgets::BRONZE),
        );
        ui.weak("Choose the game window Companion should follow.");
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.target.is_some()
                    && widgets::plain_button(ui, "Back to Companion")
                        .on_hover_text("Keep the current connection. Automation remains paused.")
                        .clicked()
                {
                    self.window_chooser.close(ui.ctx(), self.expanded_size);
                }
                ui.add_sized(
                    egui::vec2(ui.available_width(), 28.),
                    egui::TextEdit::singleline(&mut self.window_search)
                        .hint_text("Search window title or application…"),
                )
                .on_hover_text("Filter by title, application filename or exact process ID.");
            });
        });
        if self.window_refresh_at.elapsed() >= Duration::from_secs(1) {
            self.window_refresh_at = Instant::now();
            self.send(Command::RefreshTargets);
        }
        let targets = self.snapshot.targets.clone();
        self.window_chooser.poll(ui.ctx(), &targets);
        #[cfg(feature = "ui-smoke")]
        if self.window_fixture.is_some() && self.window_chooser.previews.is_empty() {
            for (index, target) in targets.iter().enumerate() {
                let frame = self
                    .editor
                    .frozen
                    .as_ref()
                    .map(catalog::thumbnail)
                    .unwrap_or_else(|| {
                        let pixels = (0..320 * 180)
                            .flat_map(|pixel| {
                                let x = pixel % 320;
                                let y = pixel / 320;
                                [
                                    ((x / 3 + index * 30) % 180 + 40) as u8,
                                    ((y / 2 + index * 20) % 140 + 40) as u8,
                                    ((x / 6 + y / 4) % 110 + 40) as u8,
                                ]
                            })
                            .collect();
                        metin_platform::Frame::new(320, 180, pixels)
                            .expect("Synthetic preview dimensions")
                    });
                self.window_chooser.previews.insert(
                    target.token,
                    Thumbnail::captured(ui.ctx(), format!("synthetic-window-{index}"), &frame),
                );
            }
        }
        let filtered = targets
            .iter()
            .filter(|window| matches_search(window, &self.window_search))
            .collect::<Vec<_>>();
        let mut chosen = None;
        let mut visible = Vec::new();
        let grid_height = (ui.available_height() - 30.).max(100.);
        egui::ScrollArea::vertical()
            .id_salt("window-grid")
            .auto_shrink([false, false])
            .max_height(grid_height)
            .show(ui, |ui| {
                if filtered.is_empty() {
                    ui.add_space(32.);
                    ui.vertical_centered(|ui| {
                        widgets::icon(
                            ui.painter(),
                            ui.cursor().min + egui::vec2(ui.available_width() / 2., 12.),
                            widgets::Icon::Camera,
                            widgets::BRONZE,
                        );
                        ui.add_space(34.);
                        if self.snapshot.targets_revision == 0 {
                            ui.spinner();
                            ui.strong("Looking for open windows…");
                        } else if targets.is_empty() {
                            ui.strong("No windows available");
                            ui.weak("Open the game. It will appear here automatically.");
                        } else {
                            ui.strong("No matching windows");
                            if widgets::plain_button(ui, "Clear search").clicked() {
                                self.window_search.clear();
                            }
                        }
                    });
                }
                let columns = ((ui.available_width() + 10.) / 230.).floor().max(2.) as usize;
                let tile_width = ((ui.available_width() - (columns - 1) as f32 * 10.)
                    / columns as f32)
                    .max(100.);
                for row in filtered.chunks(columns) {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.;
                        for window in row {
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(tile_width, tile_width * 0.54 + 52.),
                                egui::Sense::hover(),
                            );
                            if ui.is_rect_visible(rect) {
                                visible.push(window.token);
                            } else {
                                continue;
                            }
                            let selected = self.target == Some(window.token);
                            let fill = if response.hovered() {
                                ui.visuals().widgets.hovered.bg_fill
                            } else {
                                ui.visuals().faint_bg_color
                            };
                            ui.painter().rect_filled(rect, 4., fill);
                            ui.painter().rect_stroke(
                                rect,
                                4.,
                                egui::Stroke::new(
                                    if selected { 2. } else { 1. },
                                    if selected {
                                        widgets::BRONZE
                                    } else {
                                        ui.visuals().widgets.noninteractive.bg_stroke.color
                                    },
                                ),
                                egui::StrokeKind::Inside,
                            );
                            let image_rect = egui::Rect::from_min_size(
                                rect.min + egui::vec2(6., 6.),
                                egui::vec2(tile_width - 12., tile_width * 0.54 - 8.),
                            );
                            ui.painter()
                                .rect_filled(image_rect, 2., egui::Color32::from_gray(12));
                            let preview = self.window_chooser.previews.get(&window.token);
                            if let Some(texture) =
                                preview.and_then(|preview| preview.texture.as_ref())
                            {
                                let size = texture.size_vec2();
                                let scale =
                                    (image_rect.width() / size.x).min(image_rect.height() / size.y);
                                ui.painter().image(
                                    texture.id(),
                                    egui::Rect::from_center_size(image_rect.center(), size * scale),
                                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                                    egui::Color32::WHITE,
                                );
                            } else {
                                widgets::icon(
                                    ui.painter(),
                                    image_rect.center() - egui::vec2(0., 10.),
                                    widgets::Icon::Camera,
                                    egui::Color32::from_gray(150),
                                );
                                let label = if preview
                                    .and_then(|preview| preview.message.as_ref())
                                    .is_some()
                                {
                                    "Preview unavailable"
                                } else {
                                    "Loading preview…"
                                };
                                ui.painter().text(
                                    image_rect.center() + egui::vec2(0., 15.),
                                    egui::Align2::CENTER_CENTER,
                                    label,
                                    egui::FontId::proportional(11.),
                                    egui::Color32::from_gray(190),
                                );
                            }
                            let title_rect = egui::Rect::from_min_size(
                                egui::pos2(rect.left() + 8., image_rect.bottom() + 5.),
                                egui::vec2(rect.width() - 16., 40.),
                            );
                            let mut labels = ui.new_child(
                                egui::UiBuilder::new()
                                    .max_rect(title_rect)
                                    .layout(egui::Layout::top_down(egui::Align::Min)),
                            );
                            labels.add(
                                egui::Label::new(egui::RichText::new(&window.title).strong())
                                    .selectable(false)
                                    .truncate(),
                            );
                            labels.add(
                                egui::Label::new(
                                    egui::RichText::new(format!(
                                        "{} · {}",
                                        window.process_name, window.token.pid
                                    ))
                                    .small()
                                    .weak(),
                                )
                                .selectable(false)
                                .truncate(),
                            );
                            let mut hint = format!(
                                "{}\n{} · PID {}\nClick to connect.",
                                window.title, window.process_name, window.token.pid
                            );
                            if let Some(message) =
                                preview.and_then(|preview| preview.message.as_ref())
                            {
                                hint.push_str(&format!("\n{message}"));
                            }
                            // Register the tile after its labels, so label text selection
                            // cannot consume a click intended for the whole window tile.
                            if ui
                                .interact(
                                    rect,
                                    egui::Id::unique(("window-tile", window.token)),
                                    egui::Sense::click(),
                                )
                                .on_hover_text(hint)
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                chosen = Some(window.token);
                            }
                        }
                    });
                }
            });
        let fixture = {
            #[cfg(any(test, feature = "ui-smoke"))]
            {
                self.window_fixture.is_some()
            }
            #[cfg(not(any(test, feature = "ui-smoke")))]
            {
                false
            }
        };
        if !fixture {
            self.window_chooser.request_visible(ui.ctx(), visible);
        }
        ui.horizontal(|ui| {
            if let Some(error) = self
                .error
                .as_ref()
                .or(self.window_chooser.worker_error.as_ref())
            {
                ui.add(
                    egui::Label::new(egui::RichText::new(error).small().color(widgets::RED))
                        .truncate(),
                )
                .on_hover_text(error);
            } else {
                ui.small(format!("{} windows", filtered.len()));
            }
        });
        if let Some(target) = chosen {
            match self.select_window(target) {
                Ok(()) => self.window_chooser.close(ui.ctx(), self.expanded_size),
                Err(error) => self.error = Some(error),
            }
        }
        ui.ctx().request_repaint_after(Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn render(
        app: &mut MetinApp,
        context: &egui::Context,
        size: egui::Vec2,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| app.window_chooser_view(ui));
            },
        );
        output.textures_delta.clear();
        output
    }
    fn fixture() -> (MetinApp, egui::Context, tempfile::TempDir) {
        let directory = tempfile::tempdir().unwrap();
        let context = egui::Context::default();
        let mut app = MetinApp::from_context(&context, directory.path().to_owned()).unwrap();
        app.snapshot = Arc::new(RuntimeSnapshot {
            targets: vec![
                TargetInfo {
                    token: TargetToken {
                        handle: 100,
                        pid: 101,
                        created: 1,
                    },
                    title: "Forest".into(),
                    process_name: "client-a.exe".into(),
                },
                TargetInfo {
                    token: TargetToken {
                        handle: 200,
                        pid: 202,
                        created: 2,
                    },
                    title: "Dungeon".into(),
                    process_name: "client-b.exe".into(),
                },
            ],
            targets_revision: 1,
            ..Default::default()
        });
        app.window_fixture = Some(WindowFixture {
            snapshot: app.snapshot.clone(),
        });
        (app, context, directory)
    }
    fn label_position(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
        output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.size() / 2.)
            }
            _ => None,
        })
    }
    #[test]
    fn actual_grid_tile_label_click_connects_at_small_and_large_sizes_in_metin_theme() {
        for size in [egui::vec2(400., 480.), egui::vec2(780., 600.)] {
            {
                let (mut app, context, _directory) = fixture();
                widgets::theme(&context);
                let target = app.snapshot.targets[0].token;
                let profile = app.editor.profile.clone();
                assert!(app.window_chooser.open);
                render(&mut app, &context, size, vec![]);
                let output = render(&mut app, &context, size, vec![]);
                let point = label_position(&output, "Forest").expect("Tile title remains visible");
                assert!(point.x < size.x && point.y < size.y);
                let second =
                    label_position(&output, "Dungeon").expect("Second grid tile remains visible");
                assert!(
                    second.x < size.x && second.y < size.y,
                    "Both grid columns fit the chooser"
                );
                let mut tiles = output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Rect(shape)
                            if shape.rect.height() > 100.
                                && shape.rect.height() < 210.
                                && shape.stroke.width > 0.
                                && shape.rect.top() > 100. =>
                        {
                            Some(shape.rect)
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                tiles.sort_by(|a, b| a.left().total_cmp(&b.left()));
                assert!(
                    tiles.len() >= 2,
                    "Two separate tile borders must be painted: {tiles:?}"
                );
                assert!(
                    tiles[1].left() - tiles[0].right() >= 9.5,
                    "Tile borders must have a ten-pixel gap: {tiles:?}"
                );
                let title = label_position(&output, "Forest").unwrap();
                let process = label_position(&output, "client-a.exe · 101")
                    .expect("Process metadata stays readable");
                assert!(
                    process.y - title.y >= 12.,
                    "Metadata must be on its own row"
                );
                assert!(tiles[0].contains(title) && tiles[0].contains(process));
                render(
                    &mut app,
                    &context,
                    size,
                    vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
                render(
                    &mut app,
                    &context,
                    size,
                    vec![egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                );
                assert_eq!(app.target, Some(target));
                assert!(!app.window_chooser.open);
                assert_eq!(
                    app.editor.profile, profile,
                    "Connecting is independent of profile settings"
                );
                app.runtime.shutdown().unwrap();
            }
        }
    }
    #[test]
    fn filtering_grid_preserves_the_session_target_and_displays_only_matching_tiles() {
        let (mut app, context, _directory) = fixture();
        app.target = Some(app.snapshot.targets[0].token);
        app.window_search = "CLIENT-B".into();
        let target = app.target;
        render(&mut app, &context, egui::vec2(400., 480.), vec![]);
        let output = render(&mut app, &context, egui::vec2(400., 480.), vec![]);
        assert!(label_position(&output, "Dungeon").is_some());
        assert!(label_position(&output, "Forest").is_none());
        assert_eq!(app.target, target);
        app.runtime.shutdown().unwrap();
    }
}
