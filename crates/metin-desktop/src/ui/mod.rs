mod forms;
mod preview;
use crate::{
    editor::{EditMode, Editor, Gesture, Viewport},
    runtime::{Command, RuntimeHandle, RuntimeSnapshot, SessionKey},
    storage::{ProfileStore, UISettings},
};
use eframe::egui;
use metin_core::Profile;
use metin_platform::TargetToken;
use std::{path::PathBuf, sync::Arc};

#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ui-smoke", derive(clap::ValueEnum, Debug))]
pub enum Tab {
    Calibration,
    Buffs,
    Settings,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Binding {
    Mount,
    Skill(usize),
}
pub struct MetinApp {
    root: PathBuf,
    store: ProfileStore,
    settings: UISettings,
    profiles: Vec<Profile>,
    editor: Editor,
    runtime: RuntimeHandle,
    snapshot: Arc<RuntimeSnapshot>,
    target: Option<TargetToken>,
    revision: u64,
    tab: Tab,
    mode: EditMode,
    sidebar: bool,
    show_detections: bool,
    live: bool,
    snapshot_requested: Option<std::time::Instant>,
    viewport: Viewport,
    texture: Option<egui::TextureHandle>,
    texture_source: Option<metin_platform::Frame>,
    gesture: Option<Gesture>,
    binding: Option<(Binding, egui::Id)>,
    error: Option<String>,
    units: Vec<bool>,
    hover_mode: Option<EditMode>,
    icon_textures: std::collections::HashMap<String, egui::TextureHandle>,
    detection_cache: Option<preview::DetectionCache>,
    #[cfg(feature = "ui-smoke")]
    smoke: Option<(PathBuf, u32)>,
    #[cfg(test)]
    canvas_rect: Option<egui::Rect>,
}
impl MetinApp {
    pub fn new(cc: &eframe::CreationContext<'_>, root: PathBuf) -> Result<Self, String> {
        Self::from_context(&cc.egui_ctx, root)
    }
    pub fn from_context(context: &egui::Context, root: PathBuf) -> Result<Self, String> {
        crate::storage::validate_data_directory(&root).map_err(|e| e.to_string())?;
        let mut store = ProfileStore::open(&root.join("profiles")).map_err(|e| e.to_string())?;
        store
            .migrate_legacy(&root.join("config.json"))
            .map_err(|e| e.to_string())?;
        let profiles = store.list().map_err(|e| e.to_string())?;
        let selected = store
            .selected()
            .map_err(|e| e.to_string())?
            .or_else(|| profiles.first().map(|p| p.id));
        let (profile, template) = if let Some(id) = selected {
            store.load(id).map_err(|e| e.to_string())?
        } else {
            (Profile::create("New profile"), None)
        };
        let settings = UISettings::load(&root).map_err(|e| e.to_string())?;
        context.set_visuals(if settings.theme == "dark" {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        context.send_viewport_cmd(egui::ViewportCommand::SetTheme(
            if settings.theme == "dark" {
                egui::SystemTheme::Dark
            } else {
                egui::SystemTheme::Light
            },
        ));
        let notify = context.clone();
        let runtime = RuntimeHandle::spawn_with_notify(move || notify.request_repaint());
        let snapshot = runtime.latest()?;
        runtime.send(Command::RefreshTargets)?;
        Ok(Self {
            root,
            store,
            settings,
            profiles,
            editor: Editor::new(profile, template),
            runtime,
            snapshot,
            target: None,
            revision: 0,
            tab: Tab::Calibration,
            mode: EditMode::World,
            sidebar: true,
            show_detections: true,
            live: false,
            snapshot_requested: None,
            viewport: Viewport::default(),
            texture: None,
            texture_source: None,
            gesture: None,
            binding: None,
            error: None,
            units: vec![false; 8],
            hover_mode: None,
            icon_textures: Default::default(),
            detection_cache: None,
            #[cfg(feature = "ui-smoke")]
            smoke: None,
            #[cfg(test)]
            canvas_rect: None,
        })
    }
    pub fn load_fixture(&mut self, path: &std::path::Path) -> Result<(), String> {
        let image = metin_vision::load_image(path).map_err(|e| e.to_string())?;
        self.editor.freeze(image)?;
        self.live = false;
        self.editor.mark_saved(self.editor.profile.clone());
        Ok(())
    }
    #[cfg(feature = "ui-smoke")]
    pub fn request_screenshot(&mut self, path: PathBuf, tab: Option<Tab>) {
        if let Some(tab) = tab {
            self.tab = tab;
        }
        self.smoke = Some((path, 0));
    }
    fn key(&self) -> Option<SessionKey> {
        self.target.map(|target| SessionKey {
            profile_id: self.editor.profile.id,
            target,
        })
    }
    fn send(&mut self, command: Command) {
        if let Err(e) = self.runtime.send(command) {
            self.error = Some(e);
        }
    }
    fn configure(&mut self) -> Result<(), String> {
        let Some(target) = self.target else {
            return Ok(());
        };
        self.runtime.stop();
        self.revision += 1;
        self.runtime.send(Command::Configure {
            profile: Box::new(self.editor.profile.clone()),
            template: self.editor.template.clone(),
            target,
            mounted: self.settings.last_mounted,
            revision: self.revision,
        })
    }
    fn save(&mut self) -> Result<(), String> {
        let mut profile = self.editor.profile.clone();
        profile
            .buffs
            .skills
            .retain(|s| !s.shortcut.trim().is_empty());
        let saved = self
            .store
            .save(&profile, self.editor.template.as_ref())
            .map_err(|e| e.to_string())?;
        self.store.select(saved.id).map_err(|e| e.to_string())?;
        self.editor.mark_saved(saved);
        self.profiles = self.store.list().map_err(|e| e.to_string())?;
        self.configure()
    }
    fn discard_or_save(&mut self) -> bool {
        if !self.editor.dirty() {
            return true;
        }
        self.runtime.stop();
        let choice = rfd::MessageDialog::new()
            .set_title("Unsaved calibration")
            .set_description("Save changes to the current profile before continuing?")
            .set_buttons(rfd::MessageButtons::YesNoCancel)
            .show();
        match choice {
            rfd::MessageDialogResult::Yes => {
                if let Err(error) = self.save() {
                    self.error = Some(error);
                    false
                } else {
                    true
                }
            }
            rfd::MessageDialogResult::No => true,
            _ => false,
        }
    }
    fn switch_profile(&mut self, id: uuid::Uuid) {
        if id == self.editor.profile.id || !self.discard_or_save() {
            return;
        }
        self.runtime.stop();
        self.send(Command::Detach);
        match self.store.load(id) {
            Ok((profile, template)) => {
                self.editor = Editor::new(profile, template);
                self.target = None;
                self.live = false;
                self.texture_source = None;
                self.gesture = None;
                if let Err(e) = self.store.select(id) {
                    self.error = Some(e.to_string());
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    fn sync(&mut self, context: &egui::Context) {
        if let Ok(snapshot) = self.runtime.latest() {
            if snapshot.running && !self.snapshot.running && snapshot.key == self.key() {
                self.live = true;
                self.snapshot_requested = None;
            }
            self.snapshot = snapshot;
        }
        self.remember_mount();
        if self.snapshot_requested.is_some_and(|requested| {
            self.snapshot
                .captured_at
                .is_some_and(|captured| captured >= requested)
        }) && self.snapshot.key == self.key()
            && let Some(frame) = &self.snapshot.frame
        {
            if let Err(error) = self.editor.freeze(frame.clone()) {
                self.error = Some(error);
            }
            self.snapshot_requested = None;
            self.live = false;
            self.viewport.reset();
        }
        if let Some(requested) = self.snapshot_requested {
            if requested.elapsed() >= std::time::Duration::from_secs(3) {
                self.snapshot_requested = None;
                self.error =
                    Some("No fresh frame arrived. Check the selected window and try again.".into());
            } else {
                context.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
    }
    fn remember_mount(&mut self) {
        if self.snapshot.key.is_some()
            && self.snapshot.key == self.key()
            && self.snapshot.revision == self.revision
            && let Some(mounted) = self.snapshot.mounted
            && mounted != self.settings.last_mounted
        {
            self.settings.last_mounted = mounted;
            if let Err(error) = self.settings.save(&self.root) {
                self.error = Some(error.to_string());
            }
        }
    }
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.heading("MetinBot");
            ui.separator();
            if ui
                .button(if self.sidebar {
                    "Hide sidebar"
                } else {
                    "Show sidebar"
                })
                .clicked()
            {
                self.sidebar = !self.sidebar;
            }
            let mut selected = self.editor.profile.id;
            egui::ComboBox::from_id_salt("profile")
                .selected_text(&self.editor.profile.name)
                .show_ui(ui, |ui| {
                    for p in &self.profiles {
                        ui.selectable_value(&mut selected, p.id, &p.name);
                    }
                });
            if selected != self.editor.profile.id {
                self.switch_profile(selected);
            }
            if ui.button("New profile").clicked() && self.discard_or_save() {
                self.runtime.stop();
                self.send(Command::Detach);
                self.editor = Editor::new(Profile::create("New profile"), None);
                self.target = None;
                self.live = false;
                self.texture_source = None;
            }
            if ui
                .button(if self.editor.dirty() {
                    "Save *"
                } else {
                    "Save"
                })
                .clicked()
                && let Err(error) = self.save()
            {
                self.error = Some(error);
            }
            if ui
                .add_enabled(
                    self.target.is_some() && self.editor.profile.ready(),
                    egui::Button::new("Start · F7"),
                )
                .clicked()
            {
                match self.save() {
                    Ok(()) => {
                        if let Some(key) = self.key() {
                            self.send(Command::Arm { key });
                        }
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            if ui.button("Pause · F8").clicked() {
                self.runtime.stop();
            }
            if ui
                .button(if self.settings.theme == "dark" {
                    "Light theme"
                } else {
                    "Dark theme"
                })
                .clicked()
            {
                self.settings.theme = if self.settings.theme == "dark" {
                    "light"
                } else {
                    "dark"
                }
                .into();
                ui.ctx().set_visuals(if self.settings.theme == "dark" {
                    egui::Visuals::dark()
                } else {
                    egui::Visuals::light()
                });
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::SetTheme(
                    if self.settings.theme == "dark" {
                        egui::SystemTheme::Dark
                    } else {
                        egui::SystemTheme::Light
                    },
                ));
                if let Err(e) = self.settings.save(&self.root) {
                    self.error = Some(e.to_string());
                }
            }
        });
    }
    fn overview(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.strong(&self.snapshot.state);
            ui.label(&self.snapshot.reason)
                .on_hover_text(&self.snapshot.reason);
        });
        ui.horizontal_wrapped(|ui| {
            let s = &self.snapshot.statistics;
            ui.label(format!(
                "Active time  {}",
                metin_core::duration(s.active_seconds)
            ));
            ui.separator();
            ui.label(format!("Targets ended  {}", s.completed))
                .on_hover_text("Confirmed panel disappearances; not independent proof of a kill.");
            ui.separator();
            ui.label(format!("Clicks  {}", s.clicks));
            ui.separator();
            ui.label(format!("Unconfirmed  {}", s.failed));
            if ui.button("Reset statistics").clicked()
                && let Some(key) = self.key()
            {
                self.send(Command::ResetStatistics { key });
            }
        });
    }
}
impl MetinApp {
    pub fn render(&mut self, ui: &mut egui::Ui) {
        #[cfg(feature = "ui-smoke")]
        if let Some((path, count)) = &mut self.smoke {
            *count += 1;
            if *count == 4 {
                let path = path.clone();
                let context = ui.ctx().clone();
                ui.ctx().request_screenshot(move |image| {
                    let bytes = image
                        .pixels
                        .iter()
                        .flat_map(|p| [p.b(), p.g(), p.r()])
                        .collect();
                    match metin_platform::Frame::new(
                        image.size[0] as u32,
                        image.size[1] as u32,
                        bytes,
                    )
                    .map_err(|e| e.to_string())
                    .and_then(|frame| metin_vision::encode_png(&frame).map_err(|e| e.to_string()))
                    .and_then(|png| {
                        crate::storage::atomic_write(&path, &png).map_err(|e| e.to_string())
                    }) {
                        Ok(()) => {}
                        Err(error) => tracing::error!(%error,"UI screenshot failed"),
                    }
                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                });
            }
            if *count < 5 {
                ui.ctx().request_repaint();
            }
        }
        self.sync(ui.ctx());
        let binding_was_active = self.binding.is_some();
        self.capture_binding(ui.ctx());
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("overview").show(ui, |ui| {
            self.overview(ui);
            if let Some(error) = self.error.clone() {
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(200, 70, 60), error);
                    if ui.button("Dismiss").clicked() {
                        self.error = None;
                    }
                });
            }
        });
        self.hover_mode = None;
        if self.sidebar {
            egui::Panel::left("controls")
                .resizable(true)
                .default_size(310.)
                .min_size(230.)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.tab, Tab::Calibration, "Calibration");
                        ui.selectable_value(&mut self.tab, Tab::Buffs, "Buffs");
                        ui.selectable_value(&mut self.tab, Tab::Settings, "Settings");
                    });
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| match self.tab {
                        Tab::Calibration => self.calibration_form(ui),
                        Tab::Buffs => self.buffs_form(ui),
                        Tab::Settings => self.settings_form(ui),
                    });
                });
        }
        egui::CentralPanel::default().show(ui, |ui| self.preview(ui));
        if !binding_was_active && self.binding.is_none() {
            let input = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::Escape),
                    i.key_pressed(egui::Key::Delete),
                    i.modifiers.command || i.modifiers.ctrl,
                    i.key_pressed(egui::Key::Z),
                    i.key_pressed(egui::Key::Y),
                    i.modifiers.shift,
                )
            });
            if input.0 {
                self.gesture = None;
            }
            if input.1
                && !ui.ctx().egui_wants_keyboard_input()
                && let Err(e) = self.editor.delete(self.mode)
            {
                self.error = Some(e);
            }
            if input.2 && input.3 {
                if input.5 {
                    self.editor.redo();
                } else {
                    self.editor.undo();
                }
            }
            if input.2 && input.4 {
                self.editor.redo();
            }
        }
        if ui.input(|i| i.viewport().close_requested()) {
            self.runtime.stop();
            if !self.discard_or_save() {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
    }
}
impl eframe::App for MetinApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render(ui);
    }
    fn on_exit(&mut self) {
        if let Err(error) = self.runtime.shutdown() {
            tracing::error!(%error,"Shutdown failed");
        }
    }
}
#[cfg(test)]
mod tests;
