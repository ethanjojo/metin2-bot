mod action_summary;
mod actions;
mod buffs;
mod calibration;
mod catalog;
mod feedback;
mod forms;
pub(crate) mod icons;
mod mount;
mod overlay;
mod periodic;
mod run;
mod safety;
mod select;
mod settings;
mod shell;
mod size_review;
mod skill_icon;
pub(crate) mod skin;
mod state;
mod update_view;
pub(crate) mod widgets;
mod window_selection;
use crate::{
    editor::{EditMode, Editor},
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
    profile_search: String,
    profile_rename: bool,
    profile_open_requested: bool,
    window_chooser: window_selection::WindowChooser,
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
    actions_detail: bool,
    buff_detail: bool,
    collapsed: bool,
    expanded_size: egui::Vec2,
    pinned: bool,
    overlay: Option<crate::game_overlay::GameOverlay>,
    pending_overlay: Option<overlay::PendingCapture>,
    size_change: Option<size_review::SizeChange>,
    review_context: Option<size_review::ReviewContext>,
    pending_review_end: Option<size_review::ReviewEnd>,
    overlay_request_id: u64,
    overlay_frame_id: u64,
    overlay_preview: Option<overlay::RecognitionPreview>,
    window_refresh_at: std::time::Instant,
    panel_link: Option<crate::game_overlay::ControlPanelLink>,
    settings_page: settings::SettingsPage,
    selected_periodic: Option<uuid::Uuid>,
    selected_safety: usize,
    visual_detail: bool,
    safety_page: SafetyPage,
    return_to_safety: bool,
    snapshot_score: Option<(uuid::Uuid, f64)>,
    recent_log: Option<String>,
    warnings_only: bool,
    history_open: bool,
    run_live: bool,
    return_to_buffs: bool,
    show_detections: bool,
    show_regions: bool,
    show_sizes: bool,
    reference_texture: Option<(metin_platform::Frame, egui::TextureHandle)>,
    binding: Option<(Binding, egui::Id)>,
    error: Option<String>,
    feedback: Option<feedback::Feedback>,
    updates: crate::updates::UpdateService,
    update_handoff: Option<update_view::UpdateHandoff>,
    update_fixture: bool,
    units: Vec<bool>,
    icon_textures: std::collections::HashMap<String, egui::TextureHandle>,
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
    #[cfg(feature = "ui-smoke")]
    overlay_controls_fixture: bool,
    #[cfg(feature = "ui-smoke")]
    icon_sheet: bool,
    #[cfg(feature = "ui-smoke")]
    profile_menu_fixture: bool,
}
impl MetinApp {
    pub fn new(cc: &eframe::CreationContext<'_>, root: PathBuf) -> Result<Self, String> {
        let app = Self::from_context(&cc.egui_ctx, root)?;
        #[cfg(windows)]
        let app = {
            let mut app = app;
            app.native_window = cc.winit_window().cloned();
            app.sync_taskbar_icon(&crate::app_icon()?)?;
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
        let pinned = settings.pin_to_game;
        let notify = context.clone();
        let runtime = RuntimeHandle::spawn_with_notify(move || notify.request_repaint());
        let snapshot = runtime.latest()?;
        runtime.send(Command::RefreshTargets)?;
        let mut app = Self {
            updates: crate::updates::UpdateService::new(crate::VERSION)
                .map_err(|e| e.to_string())?,
            update_handoff: None,
            update_fixture: false,
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
            profile_search: String::new(),
            profile_rename: false,
            profile_open_requested: false,
            window_chooser: window_selection::WindowChooser::default(),
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
            actions_detail: false,
            buff_detail: false,
            collapsed: false,
            expanded_size: egui::vec2(460., 620.),
            pinned,
            overlay: None,
            pending_overlay: None,
            size_change: None,
            review_context: None,
            pending_review_end: None,
            overlay_request_id: 0,
            overlay_frame_id: 0,
            overlay_preview: None,
            window_refresh_at: std::time::Instant::now(),
            panel_link: None,
            settings_page: settings::SettingsPage::Detection,
            selected_periodic: None,
            selected_safety: 0,
            visual_detail: false,
            safety_page: SafetyPage::Schedule,
            return_to_safety: false,
            snapshot_score: None,
            recent_log: None,
            warnings_only: false,
            history_open: false,
            run_live: true,
            return_to_buffs: false,
            show_detections: true,
            show_regions: true,
            show_sizes: false,
            reference_texture: None,
            binding: None,
            error: None,
            feedback: None,
            units: vec![false; 8],
            icon_textures: Default::default(),
            click_preview: None,
            #[cfg(feature = "ui-smoke")]
            smoke: None,
            #[cfg(feature = "ui-smoke")]
            smoke_requested: false,
            #[cfg(feature = "ui-smoke")]
            smoke_frames: 0,
            #[cfg(feature = "ui-smoke")]
            overlay_controls_fixture: false,
            #[cfg(feature = "ui-smoke")]
            icon_sheet: false,
            #[cfg(feature = "ui-smoke")]
            profile_menu_fixture: false,
        };
        app.apply_appearance(context)?;
        context.set_zoom_factor(app.settings.ui_scale);
        if app.settings.check_updates_on_start {
            let _ = app.updates.check(app.settings.update_channel);
        }
        Ok(app)
    }
    fn apply_appearance(&self, context: &egui::Context) -> Result<(), String> {
        let icon = crate::app_icon()?;
        #[cfg(windows)]
        self.sync_taskbar_icon(&icon)?;
        widgets::theme(context);
        context.send_viewport_cmd(egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark));
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
    pub fn load_fixture(&mut self, path: &std::path::Path) -> Result<(), String> {
        let image = metin_vision::load_image(path).map_err(|e| e.to_string())?;
        self.editor.freeze(image)?;
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
            self.actions_detail = true;
            self.buff_detail = false;
            self.visual_detail = page == ActionsPage::VisualRules;
            if page == ActionsPage::Periodic {
                self.selected_periodic = self
                    .editor
                    .profile
                    .periodic_actions
                    .first()
                    .map(|action| action.id);
            }
        }
        let Some(run) = run else {
            let key = self.key();
            if page == Some(ActionsPage::Buffs)
                && let Some(fixture) = self.window_fixture.as_mut()
            {
                // Explicitly synthetic timers/presence for real native UI screenshots.
                let mut snapshot = (*fixture.snapshot).clone();
                snapshot.key = key;
                snapshot.revision = self.revision;
                snapshot.frame = self.editor.frozen.clone();
                snapshot.captured_at = Some(std::time::Instant::now());
                snapshot.timers = self
                    .editor
                    .profile
                    .buffs
                    .skills
                    .iter()
                    .map(|skill| skill.interval_seconds.map(|interval| interval * 0.65))
                    .collect();
                snapshot.buff_presence = self
                    .editor
                    .profile
                    .buffs
                    .skills
                    .iter()
                    .map(|skill| {
                        if skill.mode == metin_core::BuffMode::Inactive {
                            metin_core::Presence::Active
                        } else {
                            metin_core::Presence::Unknown
                        }
                    })
                    .collect();
                fixture.snapshot = Arc::new(snapshot);
            }
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
        snapshot.activity = c.activity();
        snapshot.active_skill = c.active_skill(now);
        snapshot.state = c.state.clone();
        snapshot.reason = c.reason.clone();
        snapshot.running = c.running;
        snapshot.schedule = c.schedule(now);
        snapshot.action_timers = c.action_timers();
        snapshot.activity_progress = c.activity_progress(now);
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
    pub fn prepare_overlay_controls_fixture(&mut self, enabled: bool) {
        self.tab = Tab::Calibration;
        self.run_live = enabled;
        self.overlay_controls_fixture = true;
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
        if self.editor.is_recalibrating() {
            return Err(
                "Confirm or cancel the calibration review before applying settings.".into(),
            );
        }
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
        if self.editor.is_recalibrating() {
            return Err("Confirm or cancel the calibration review before saving.".into());
        }
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
        self.configure()?;
        self.show_feedback("Saved", false);
        Ok(())
    }
    fn discard_or_save(&mut self) -> bool {
        if self.editor.is_recalibrating() {
            self.error =
                Some("Confirm or cancel the calibration review before switching profiles.".into());
            return false;
        }
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
    fn sync(&mut self, _context: &egui::Context) {
        let snapshot = self.runtime.latest();
        #[cfg(any(test, feature = "ui-smoke"))]
        let snapshot = self
            .window_fixture
            .as_ref()
            .map(|f| f.snapshot.clone())
            .map_or(snapshot, Ok);
        if let Ok(snapshot) = snapshot {
            self.snapshot = snapshot;
        }
        #[cfg(feature = "ui-smoke")]
        if self.smoke.is_some() && self.window_fixture.is_some() && self.snapshot.frame.is_some() {
            // The explicit static demonstration scene remains available on each
            // screenshot frame. Production never rewrites source timestamps.
            let mut snapshot = (*self.snapshot).clone();
            snapshot.captured_at = Some(std::time::Instant::now());
            self.snapshot = Arc::new(snapshot);
        }
        self.reconcile_windows();
        self.sync_size_review();
        if self.pending_review_end.is_some() {
            _context.request_repaint_after(std::time::Duration::from_millis(40));
        }
        if !self.window_chooser.open
            && self.target.is_some()
            && self.window_refresh_at.elapsed() >= std::time::Duration::from_secs(2)
        {
            self.window_refresh_at = std::time::Instant::now();
            self.send(Command::RefreshTargets);
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
        let enabled = (self.run_live || self.editor.is_recalibrating())
            && !self.window_chooser.open
            && self.pending_overlay.is_none();
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
}

impl MetinApp {
    #[cfg(feature = "ui-smoke")]
    pub fn prepare_icon_sheet(&mut self) {
        self.icon_sheet = true;
    }
    #[cfg(feature = "ui-smoke")]
    pub fn prepare_profile_menu_fixture(&mut self) {
        self.profile_menu_fixture = true;
    }

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
        #[cfg(feature = "ui-smoke")]
        if self.icon_sheet {
            egui::CentralPanel::default().show(ui, icons::sheet);
            return;
        }
        skin::background(ui, self.collapsed);
        self.sync(ui.ctx());
        self.sync_updates(ui.ctx());
        if !self.window_chooser.open && !self.collapsed {
            self.expanded_size = ui.ctx().content_rect().size();
        }
        self.sync_game_overlay(ui.ctx());
        if self.window_chooser.open {
            egui::CentralPanel::default().show(ui, |ui| self.window_chooser_view(ui));
            self.window_frame(ui);
            self.sync_preview_demand();
            if ui.input(|i| i.viewport().close_requested()) {
                self.runtime.stop();
                if !self.update_exit_pending() && !self.discard_or_save() {
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::CancelClose);
                }
            }
            return;
        }
        let binding_was_active = self.binding.is_some();
        self.capture_binding(ui.ctx());
        egui::Panel::top("toolbar")
            .frame(egui::Frame::new().inner_margin(egui::Margin {
                left: 0,
                right: 0,
                top: 1,
                bottom: 0,
            }))
            .show(ui, |ui| self.toolbar(ui));
        if !self.collapsed {
            egui::Panel::top("navigation").show(ui, |ui| self.navigation(ui));
            egui::Panel::bottom("status")
                .frame(
                    egui::Frame::new()
                        .fill(ui.visuals().panel_fill)
                        .inner_margin(egui::Margin {
                            left: 10,
                            right: 10,
                            top: 4,
                            bottom: 10,
                        }),
                )
                .show(ui, |ui| self.footer(ui));
        }
        egui::CentralPanel::default().show(ui, |ui| {
            if !self.collapsed {
                match self.tab {
                    Tab::Run => self.run_view(ui),
                    Tab::Calibration => self.calibration_view(ui),
                    Tab::Actions => self.actions_view(ui),
                    Tab::Safety => self.safety_view(ui),
                    Tab::Settings => self.settings_view(ui),
                }
            }
        });
        self.window_frame(ui);
        if self.editor.is_recalibrating()
            && ui.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::F8))
        {
            self.cancel_size_review();
        }
        let history_was_open = self.history_open;
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
                self.cancel_overlay_capture();
                self.editing = false;
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
            self.cancel_size_review();
            self.runtime.stop();
            if !self.update_exit_pending() && !self.discard_or_save() {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
    }
}
impl eframe::App for MetinApp {
    #[cfg(feature = "ui-smoke")]
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        if self.smoke.is_some() {
            // Documentation captures must not inherit the desktop cursor's
            // hover/tooltips or incidental input. Real sessions are unaffected.
            raw.events.clear();
            raw.events.push(egui::Event::PointerGone);
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render(ui);
    }
    fn on_exit(&mut self) {
        self.updates.cancel();
        self.cancel_size_review();
        self.cancel_overlay_capture();
        self.overlay.take();
        if let Err(error) = self.runtime.shutdown() {
            tracing::error!(%error,"Shutdown failed");
        }
    }
}
#[cfg(test)]
mod tests;
