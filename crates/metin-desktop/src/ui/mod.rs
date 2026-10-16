mod buffs;
mod calibration;
mod forms;
mod preview;
mod run;
mod settings;
mod shell;
mod state;
mod widgets;
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
    Run,
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
    calibration_controls: bool,
    selected_skill: usize,
    return_to_buffs: bool,
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
    smoke: Option<(PathBuf, std::time::Instant)>,
    #[cfg(feature = "ui-smoke")]
    smoke_requested: bool,
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
        widgets::theme(context, settings.theme == "dark");
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
            tab: Tab::Run,
            mode: EditMode::World,
            calibration_controls: true,
            selected_skill: 0,
            return_to_buffs: false,
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
            #[cfg(feature = "ui-smoke")]
            smoke_requested: false,
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
        self.smoke = Some((path, std::time::Instant::now()));
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
        let profile = self.profile_for_save();
        let saved = self
            .store
            .save(&profile, self.editor.template.as_ref())
            .map_err(|e| e.to_string())?;
        self.store.select(saved.id).map_err(|e| e.to_string())?;
        let kept = self.retained_skill_indices();
        let retained_units = kept.iter().map(|i| self.units[*i]).collect::<Vec<_>>();
        self.units.fill(false);
        for (i, unit) in retained_units.into_iter().enumerate() {
            self.units[i] = unit;
        }
        self.selected_skill = kept
            .iter()
            .position(|i| *i >= self.selected_skill)
            .unwrap_or(kept.len().saturating_sub(1));
        self.binding = None;
        self.gesture = None;
        if let EditMode::BuffIcon { index, inactive } = self.mode {
            if let Some(index) = kept.iter().position(|i| *i == index) {
                self.mode = EditMode::BuffIcon { index, inactive };
            } else {
                self.mode = EditMode::World;
                if self.return_to_buffs {
                    self.navigate(Tab::Buffs);
                }
                self.return_to_buffs = false;
            }
        }
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
            .set_title("Unsaved profile changes")
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
        match self.store.load(id) {
            Ok((profile, template)) => {
                self.replace_profile(profile, template);
                if let Err(e) = self.store.select(id) {
                    self.error = Some(e.to_string());
                }
            }
            Err(e) => {
                self.runtime.stop();
                self.error = Some(e.to_string());
            }
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
}

impl MetinApp {
    pub fn render(&mut self, ui: &mut egui::Ui) {
        #[cfg(feature = "ui-smoke")]
        if let Some((path, started)) = &self.smoke {
            if !self.smoke_requested && started.elapsed() >= std::time::Duration::from_millis(700) {
                let path = path.clone();
                self.smoke_requested = true;
                tracing::info!(?path, "UI screenshot requested");
                let context = ui.ctx().clone();
                ui.ctx().request_screenshot(move |image| {
                    tracing::info!(
                        width = image.size[0],
                        height = image.size[1],
                        "UI screenshot received"
                    );
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
            if self.smoke.is_some() {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(30));
            }
        }
        self.sync(ui.ctx());
        let binding_was_active = self.binding.is_some();
        self.capture_binding(ui.ctx());
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        egui::Panel::top("navigation").show(ui, |ui| self.navigation(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        self.hover_mode = None;
        #[cfg(test)]
        {
            self.canvas_rect = None;
        }
        egui::CentralPanel::default().show(ui, |ui| match self.tab {
            Tab::Run => self.run_view(ui),
            Tab::Calibration => self.calibration_view(ui),
            Tab::Buffs => self.buffs_view(ui),
            Tab::Settings => self.settings_view(ui),
        });
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
                && self.tab == Tab::Calibration
                && !ui.ctx().egui_wants_keyboard_input()
                && let Err(e) = self.editor.delete(self.mode)
            {
                self.error = Some(e);
            }
            if input.2 && input.3 && !ui.ctx().egui_wants_keyboard_input() {
                if input.5 {
                    self.editor.redo();
                } else {
                    self.editor.undo();
                }
            }
            if input.2 && input.4 && !ui.ctx().egui_wants_keyboard_input() {
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
