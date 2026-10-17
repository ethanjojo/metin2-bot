mod actions;
mod buffs;
mod calibration;
mod forms;
mod mount;
mod periodic;
mod preview;
mod run;
mod safety;
mod settings;
mod shell;
mod state;
mod widgets;
mod window_selection;
use crate::{
    editor::{EditMode, Editor, Gesture, Viewport},
    runtime::{Command, RuntimeHandle, RuntimeSnapshot, SessionKey},
    storage::{ProfileStore, UISettings},
};
pub use actions::ActionsPage;
use eframe::egui;
use metin_core::Profile;
use metin_platform::TargetToken;
pub use safety::SafetyPage;
pub use settings::SettingsPage;
use state::SetupAction;
use std::{path::PathBuf, sync::Arc};
#[cfg(feature = "ui-smoke")]
pub use window_selection::WindowPickerFixture;
#[cfg(feature = "ui-smoke")]
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum BuffCaptureFixture {
    Area,
    Active,
    Inactive,
}
#[cfg(feature = "ui-smoke")]
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum RunFixture {
    Attack,
    Looting,
    Break,
    Stopped,
    History,
    Uncalibrated,
    PreviewOff,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ui-smoke", derive(clap::ValueEnum))]
pub enum Tab {
    Run,
    Calibration,
    Actions,
    Safety,
    Settings,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Binding {
    Mount,
    Loot,
    Skill(usize),
    Periodic(uuid::Uuid),
    Visual(uuid::Uuid),
    Puller,
    OutOfTarget,
}
pub struct MetinApp {
    #[cfg(target_os = "macos")]
    native_dock: bool,
    root: PathBuf,
    store: ProfileStore,
    settings: UISettings,
    #[cfg(windows)]
    native_window: Option<Arc<winit::window::Window>>,
    profiles: Vec<Profile>,
    editor: Editor,
    runtime: RuntimeHandle,
    snapshot: Arc<RuntimeSnapshot>,
    target: Option<TargetToken>,
    window_search: String,
    target_list_revision: u64,
    #[cfg(any(test, feature = "ui-smoke"))]
    window_fixture: Option<window_selection::WindowFixture>,
    revision: u64,
    preview_demand: Option<(SessionKey, bool, u64)>,
    tab: Tab,
    mode: EditMode,
    editing: bool,
    selected_skill: usize,
    actions_page: ActionsPage,
    settings_page: settings::SettingsPage,
    selected_periodic: Option<uuid::Uuid>,
    selected_safety: usize,
    safety_page: SafetyPage,
    return_to_safety: bool,
    preview_snapshot: Option<Arc<RuntimeSnapshot>>,
    preview_updated: std::time::Instant,
    snapshot_score: Option<(uuid::Uuid, f64)>,
    recent_log: Option<String>,
    warnings_only: bool,
    history_open: bool,
    run_live: bool,
    run_mount_editing: bool,
    return_to_buffs: bool,
    show_detections: bool,
    show_sizes: bool,
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
    click_preview: Option<(
        metin_platform::Frame,
        metin_core::Region,
        egui::TextureHandle,
    )>,
    #[cfg(feature = "ui-smoke")]
    smoke: Option<(PathBuf, Option<std::time::Instant>)>,
    #[cfg(feature = "ui-smoke")]
    smoke_requested: bool,
    #[cfg(feature = "ui-smoke")]
    smoke_frames: u8,
    #[cfg(test)]
    canvas_rect: Option<egui::Rect>,
}
impl MetinApp {
    pub fn new(cc: &eframe::CreationContext<'_>, root: PathBuf) -> Result<Self, String> {
        let app = Self::from_context(&cc.egui_ctx, root)?;
        #[cfg(target_os = "macos")]
        let app = {
            let mut app = app;
            app.native_dock = true;
            app.apply_theme(&cc.egui_ctx, app.settings.theme == "dark")?;
            app
        };
        #[cfg(windows)]
        let app = {
            let mut app = app;
            app.native_window = cc.winit_window().cloned();
            app.sync_taskbar_icon(&crate::app_icon(app.settings.theme == "dark")?)?;
            app
        };
        Ok(app)
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
        let notify = context.clone();
        let runtime = RuntimeHandle::spawn_with_notify(move || notify.request_repaint());
        let snapshot = runtime.latest()?;
        runtime.send(Command::RefreshTargets)?;
        let app = Self {
            #[cfg(target_os = "macos")]
            native_dock: false,
            root,
            store,
            settings,
            #[cfg(windows)]
            native_window: None,
            profiles,
            editor: Editor::new(profile, template),
            runtime,
            snapshot,
            target: None,
            window_search: String::new(),
            target_list_revision: 0,
            #[cfg(any(test, feature = "ui-smoke"))]
            window_fixture: None,
            revision: 0,
            preview_demand: None,
            tab: Tab::Run,
            mode: EditMode::World,
            editing: false,
            selected_skill: 0,
            actions_page: ActionsPage::Buffs,
            settings_page: settings::SettingsPage::Detection,
            selected_periodic: None,
            selected_safety: 0,
            safety_page: SafetyPage::Schedule,
            return_to_safety: false,
            preview_snapshot: None,
            preview_updated: std::time::Instant::now(),
            snapshot_score: None,
            recent_log: None,
            warnings_only: false,
            history_open: false,
            run_live: true,
            run_mount_editing: false,
            return_to_buffs: false,
            show_detections: true,
            show_sizes: false,
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
            click_preview: None,
            #[cfg(feature = "ui-smoke")]
            smoke: None,
            #[cfg(feature = "ui-smoke")]
            smoke_requested: false,
            #[cfg(feature = "ui-smoke")]
            smoke_frames: 0,
            #[cfg(test)]
            canvas_rect: None,
        };
        app.apply_theme(context, app.settings.theme == "dark")?;
        context.set_zoom_factor(app.settings.ui_scale);
        Ok(app)
    }
    fn apply_theme(&self, context: &egui::Context, dark: bool) -> Result<(), String> {
        let icon = crate::app_icon(dark)?;
        #[cfg(windows)]
        self.sync_taskbar_icon(&icon)?;
        #[cfg(target_os = "macos")]
        if self.native_dock {
            crate::dock_icon::set(dark)?;
        }
        widgets::theme(context, dark);
        context.send_viewport_cmd(egui::ViewportCommand::SetTheme(if dark {
            egui::SystemTheme::Dark
        } else {
            egui::SystemTheme::Light
        }));
        context.send_viewport_cmd(egui::ViewportCommand::Icon(Some(Arc::new(icon))));
        Ok(())
    }
    #[cfg(windows)]
    fn sync_taskbar_icon(&self, icon: &egui::IconData) -> Result<(), String> {
        use winit::platform::windows::WindowExtWindows;
        if let Some(window) = &self.native_window {
            let icon = winit::window::Icon::from_rgba(icon.rgba.clone(), icon.width, icon.height)
                .map_err(|e| e.to_string())?;
            window.set_taskbar_icon(Some(icon));
        }
        Ok(())
    }
    fn toggle_theme(&mut self, context: &egui::Context) {
        let dark = self.settings.theme != "dark";
        if let Err(error) = self.apply_theme(context, dark) {
            self.error = Some(error);
            return;
        }
        self.settings.theme = if dark { "dark" } else { "light" }.into();
        if let Err(error) = self.settings.save(&self.root) {
            self.error = Some(error.to_string());
        }
    }
    pub fn load_fixture(&mut self, path: &std::path::Path) -> Result<(), String> {
        let image = metin_vision::load_image(path).map_err(|e| e.to_string())?;
        self.editor.freeze(image)?;
        self.live = false;
        self.editor.mark_saved(self.editor.profile.clone());
        Ok(())
    }
    #[cfg(feature = "ui-smoke")]
    pub fn prepare_capture_fixture(
        &mut self,
        capture: Option<BuffCaptureFixture>,
        sizes: bool,
    ) -> Result<(), String> {
        self.show_sizes = sizes;
        if let Some(capture) = capture {
            let index = self.selected_skill;
            if self.editor.profile.buffs.skills.get(index).is_none() {
                return Err("Buff capture fixture needs a skill.".into());
            }
            self.edit_area(match capture {
                BuffCaptureFixture::Area => EditMode::BuffArea,
                BuffCaptureFixture::Active => EditMode::BuffIcon {
                    index,
                    inactive: false,
                },
                BuffCaptureFixture::Inactive => EditMode::BuffIcon {
                    index,
                    inactive: true,
                },
            });
        }
        Ok(())
    }
    #[cfg(feature = "ui-smoke")]
    pub fn request_screenshot(&mut self, path: PathBuf, tab: Option<Tab>) {
        if let Some(tab) = tab {
            self.tab = tab;
        }
        self.smoke = Some((path, None));
        self.smoke_frames = 0;
    }
    #[cfg(feature = "ui-smoke")]
    pub fn prepare_actions_fixture(&mut self, page: Option<ActionsPage>, run: Option<RunFixture>) {
        if let Some(page) = page {
            self.actions_page = page;
        }
        let Some(run) = run else {
            return;
        };
        self.set_window_picker_fixture(WindowPickerFixture::Selected);
        if matches!(run, RunFixture::Uncalibrated) {
            self.editor = Editor::new(Profile::create("Uncalibrated profile"), None);
            self.editor.mark_saved(self.editor.profile.clone());
            let key = self.key();
            let fixture = self.window_fixture.as_mut().expect("fixture catalog");
            let mut snapshot = (*fixture.snapshot).clone();
            snapshot.key = key;
            snapshot.revision = self.revision;
            snapshot.reason = "Complete calibration before starting.".into();
            fixture.snapshot = Arc::new(snapshot);
            return;
        }
        self.run_live = !matches!(run, RunFixture::PreviewOff);
        let mut profile = self.editor.profile.clone();
        profile.buffs.enabled = false;
        profile.periodic_actions.clear();
        profile.puller.enabled = false;
        profile.out_of_target.after_combat = matches!(run, RunFixture::Attack);
        profile.out_of_target.during_search = false;
        profile.panel.stable_frames = 2;
        profile.loot.enabled = true;
        profile.safety.stop_enabled = true;
        profile.safety.breaks_enabled = true;
        let mut c = metin_core::Controller::new(profile);
        c.buffs.mounted = Some(false);
        c.start(0.).expect("valid demonstration profile");
        let observation = |panel| metin_core::Observation {
            ready: true,
            panel_present: panel,
            ..Default::default()
        };
        let now = match run {
            RunFixture::Attack => {
                c.step(0., &observation(true));
                c.step(0.25, &observation(true));
                c.step(0.5, &observation(false));
                let action = c.step(0.75, &observation(false)).expect("attack due");
                c.ack(0.75, action.id, true)
                    .expect("attack held in fixture");
                0.75
            }
            RunFixture::Looting | RunFixture::History | RunFixture::PreviewOff => {
                c.step(0., &observation(true));
                c.step(0.25, &observation(true));
                c.step(0.5, &observation(false));
                c.step(0.75, &observation(false));
                if matches!(run, RunFixture::History) {
                    let action = c.step(1.25, &observation(false)).expect("pickup due");
                    c.ack(1.4, action.id, true).expect("pickup acknowledged");
                    c.pause(2., "Paused (F8).");
                    self.history_open = true;
                    2.
                } else {
                    0.75
                }
            }
            RunFixture::Break => {
                c.step(2700., &observation(false));
                c.step(2700.25, &observation(false));
                2700.25
            }
            RunFixture::Stopped => {
                c.tick(14400.);
                14400.
            }
            RunFixture::Uncalibrated => unreachable!("handled before controller construction"),
        };
        self.editor.mark_saved(c.profile.clone());
        let fixture = self.window_fixture.as_mut().expect("fixture catalog");
        let mut snapshot = (*fixture.snapshot).clone();
        snapshot.key = self.target.map(|target| SessionKey {
            profile_id: self.editor.profile.id,
            target,
        });
        snapshot.revision = self.revision;
        snapshot.mounted = c.buffs.mounted;
        snapshot.frame = self.editor.frozen.clone();
        snapshot.captured_at = Some(std::time::Instant::now());
        snapshot.state = c.state.clone();
        snapshot.reason = c.reason.clone();
        snapshot.running = c.running;
        snapshot.schedule = c.schedule(now);
        snapshot.action_timers = c.action_timers();
        snapshot.attack_remaining = if c.phase == metin_core::Phase::OutOfTarget {
            c.remaining(now)
        } else {
            None
        };
        snapshot.statistics = c.session.snapshot();
        snapshot.events = c.events.into_iter().collect();
        fixture.snapshot = Arc::new(snapshot);
    }
    #[cfg(feature = "ui-smoke")]
    pub fn prepare_safety_fixture(&mut self, page: SafetyPage) {
        self.safety_page = page;
    }
    #[cfg(feature = "ui-smoke")]
    pub fn prepare_safety_capture_fixture(&mut self) -> Result<(), String> {
        if self.editor.profile.safety.rules.is_empty() {
            return Err("Safety capture fixture requires a rule.".into());
        }
        self.selected_safety = 0;
        self.edit_area(EditMode::SafetyReference(0));
        Ok(())
    }
    #[cfg(feature = "ui-smoke")]
    pub fn prepare_settings_fixture(&mut self, page: SettingsPage) {
        self.settings_page = page;
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
        self.preview_demand = None;
        self.revision += 1;
        self.runtime.send(Command::Connect {
            key: SessionKey {
                profile_id: self.editor.profile.id,
                target,
            },
            revision: self.revision,
        })?;
        if self.editor.profile.validate().is_err() {
            return Ok(());
        }
        self.runtime.send(Command::Configure {
            profile: Box::new(self.editor.profile.clone()),
            template: self.editor.template.clone(),
            target,
            revision: self.revision,
        })
    }
    fn save(&mut self) -> Result<(), String> {
        let profile = self.profile_for_save();
        let saved = self
            .store
            .save(&profile, self.editor.template.as_ref())
            .map_err(|e| e.to_string())?;
        if let Some(warning) = saved.cleanup_warning {
            tracing::warn!(profile_id = %saved.profile.id, %warning, "Saved profile asset cleanup failed");
            self.error = Some(warning);
        }
        let saved = saved.profile;
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
                    self.navigate(Tab::Actions);
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
        let snapshot = self.runtime.latest();
        #[cfg(any(test, feature = "ui-smoke"))]
        let snapshot = self
            .window_fixture
            .as_ref()
            .map(|f| f.snapshot.clone())
            .map_or(snapshot, Ok);
        if let Ok(snapshot) = snapshot {
            if snapshot.running && !self.snapshot.running && snapshot.key == self.key() {
                self.live = true;
                self.snapshot_requested = None;
            }
            self.snapshot = snapshot;
        }
        self.reconcile_windows();
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
    fn declared_mount(&self) -> Option<bool> {
        if self.key().is_some()
            && self.snapshot.key == self.key()
            && self.snapshot.revision == self.revision
        {
            self.snapshot.mounted
        } else {
            None
        }
    }
    fn sync_preview_demand(&mut self) {
        let Some(key) = self.key() else {
            self.preview_demand = None;
            return;
        };
        let enabled =
            (self.tab == Tab::Run && self.run_live) || (self.tab == Tab::Calibration && self.live);
        // A stop retires queued commands. Reassert presentation demand in the
        // new generation even when the visible tab itself did not change.
        let demand = (key, enabled, self.runtime.stop_generation());
        if self.preview_demand != Some(demand) {
            match self.runtime.send(Command::SetPreview { key, enabled }) {
                Ok(()) => self.preview_demand = Some(demand),
                Err(error) => self.error = Some(error),
            }
        }
    }
    fn presentation_snapshot(&mut self) -> Arc<RuntimeSnapshot> {
        let interval = std::time::Duration::from_millis(
            if self.settings.lite_mode {
                self.settings.preview_refresh_ms.max(1000)
            } else {
                self.settings.preview_refresh_ms
            }
            .into(),
        );
        if self
            .preview_snapshot
            .as_ref()
            .is_none_or(|old| old.key != self.snapshot.key)
            || self.preview_updated.elapsed() >= interval
        {
            self.preview_snapshot = Some(self.snapshot.clone());
            self.preview_updated = std::time::Instant::now();
        }
        self.preview_snapshot
            .as_ref()
            .expect("preview initialized")
            .clone()
    }
}

impl MetinApp {
    pub fn render(&mut self, ui: &mut egui::Ui) {
        #[cfg(feature = "ui-smoke")]
        if let Some((_, started)) = &mut self.smoke {
            // Startup shader work can precede the first rendered frame.
            // Allow actual UI/popup layout and fade-in time before capture.
            self.smoke_frames = self.smoke_frames.saturating_add(1);
            if self.smoke_frames >= 3 {
                started.get_or_insert_with(std::time::Instant::now);
            }
        }
        #[cfg(feature = "ui-smoke")]
        if let Some((path, Some(started))) = &self.smoke {
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
            Tab::Actions => self.actions_view(ui),
            Tab::Safety => self.safety_view(ui),
            Tab::Settings => self.settings_view(ui),
        });
        let history_was_open = self.history_open;
        self.run_history_dialog(ui.ctx());
        self.sync_preview_demand();
        if !history_was_open && !self.history_open && !binding_was_active && self.binding.is_none()
        {
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
                if self.gesture.is_none() {
                    self.editing = false;
                }
                self.gesture = None;
            }
            if input.1 && self.tab == Tab::Calibration && !ui.ctx().egui_wants_keyboard_input() {
                self.delete_selected();
            }
            if input.2 && input.3 && !ui.ctx().egui_wants_keyboard_input() {
                if input.5 {
                    self.redo_profile();
                } else {
                    self.undo_profile();
                }
            }
            if input.2 && input.4 && !ui.ctx().egui_wants_keyboard_input() {
                self.redo_profile();
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
