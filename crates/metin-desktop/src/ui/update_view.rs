use super::*;
use crate::update_helper::PreparedUpdate;
use crate::updates::{UpdateChannel, UpdateState};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
};

fn file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1_048_576 {
        format!("{:.1} KB", bytes as f64 / 1024.)
    } else {
        format!("{:.1} MB", bytes as f64 / 1_048_576.)
    }
}

pub(super) enum UpdateHandoff {
    Preparing(Receiver<Result<PreparedUpdate, String>>),
    Waiting(PreparedUpdate),
    InstallerStarted { _guard: PreparedUpdate },
    ReleaseFailed(String),
}

impl MetinApp {
    pub(super) fn update_exit_pending(&self) -> bool {
        matches!(
            self.update_handoff,
            Some(UpdateHandoff::InstallerStarted { .. })
        )
    }
    pub(super) fn sync_updates(&mut self, ctx: &egui::Context) {
        self.poll_update_handoff(ctx);
        if self.updates.poll() {
            ctx.request_repaint();
        }
        if matches!(
            self.updates.state(),
            UpdateState::Idle
                | UpdateState::Checking
                | UpdateState::UpToDate
                | UpdateState::Cancelled
                | UpdateState::Failed(_)
        ) {
            self.update_install_requested = None;
        }
        let requested_ready = self.update_install_requested.as_ref().is_some_and(|intent| {
            matches!(self.updates.state(), UpdateState::Ready(candidate) if candidate.version() == intent.version() && candidate.digest_hex() == intent.digest_hex())
        });
        if requested_ready && !self.update_fixture {
            self.begin_update_install(ctx);
        }
        if self.startup_update_open
            && self.update_handoff.is_none()
            && self.update_error.is_none()
            && matches!(
                self.updates.state(),
                UpdateState::Idle | UpdateState::UpToDate | UpdateState::Cancelled
            )
        {
            self.startup_update_open = false;
        }
        if matches!(
            self.updates.state(),
            UpdateState::Checking | UpdateState::Downloading { .. }
        ) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    pub(super) fn updates_view(&mut self, ui: &mut egui::Ui) {
        widgets::heading(
            ui,
            "Updates",
            "Updates come from the fixed Companion GitHub repository. Downloads are checked against the published SHA256. Installation always needs your confirmation.",
        );
        ui.label(format!("Installed version: {}", crate::VERSION));
        let state = self.updates.state().clone();
        let busy = matches!(
            state,
            UpdateState::Checking | UpdateState::Downloading { .. }
        );
        let primary_first = matches!(
            state,
            UpdateState::Available(_) | UpdateState::Downloading { .. } | UpdateState::Ready(_)
        );
        if !primary_first {
            self.update_preferences(ui, busy);
        }
        ui.separator();
        self.update_status(ui, false);
        if primary_first {
            ui.separator();
            ui.small("Update preferences");
            self.update_preferences(ui, busy);
        }
    }

    pub(super) fn update_status(&mut self, ui: &mut egui::Ui, startup: bool) {
        if self.update_handoff_waiting() || self.update_exit_pending() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Preparing update…");
            });
            ui.small("Safely releasing input and opening the update window.");
            return;
        }
        if let Some(error) = &self.update_error {
            ui.colored_label(widgets::RED, error);
        }
        let state = self.updates.state().clone();
        let offer = matches!(state, UpdateState::Available(_) | UpdateState::Ready(_));
        match state {
            UpdateState::Idle | UpdateState::Cancelled => {
                ui.weak("Check when you are ready. Nothing downloads automatically.");
            }
            UpdateState::Checking => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Checking GitHub…");
                });
            }
            UpdateState::UpToDate => {
                widgets::badge(ui, "No compatible newer release", widgets::GREEN);
                ui.weak("Only verified Windows Companion installers in the selected channel are offered.");
            }
            UpdateState::Failed(error) => {
                ui.colored_label(widgets::RED, error.to_string());
                ui.weak("Your application and profiles have not been changed.");
            }
            UpdateState::Available(candidate) | UpdateState::Ready(candidate) => {
                widgets::badge(
                    ui,
                    &format!("Companion {}", candidate.version()),
                    widgets::GREEN,
                );
                ui.small(format!("Windows x64 · {}", file_size(candidate.size())));
                ui.horizontal_wrapped(|ui| {
                    ui.small("Includes input-driver and machine-wide certificate/service updates.");
                    crate::ui_controls::help(ui,"Selecting Install permits Setup to create or reuse the HIDMaestro signing certificate (including its private key in LocalMachine\\My), trust it in LocalMachine\\Root and TrustedPublisher, install or update the shared virtual keyboard/mouse driver, and set GameInputSvc to automatic startup. Uninstall retains these shared components unless you explicitly request removal. Profiles and settings are retained.");
                });
                if matches!(self.updates.state(), UpdateState::Ready(_)) {
                    ui.label("Download verified. Ready to install.");
                }
                if !startup {
                    self.update_primary_action(ui, false);
                    ui.add_space(6.);
                    ui.strong("Release notes");
                    egui::ScrollArea::vertical()
                        .id_salt("update-notes")
                        .max_height(100.)
                        .show(ui, |ui| {
                            ui.label(if candidate.release_notes().trim().is_empty() {
                                "No release notes were provided."
                            } else {
                                candidate.release_notes()
                            });
                        });
                    ui.hyperlink_to("View release on GitHub", candidate.release_url());
                }
            }
            UpdateState::Downloading {
                candidate,
                downloaded,
                total,
            } => {
                let verifying = downloaded >= total;
                ui.label(if verifying {
                    "Verifying update…".into()
                } else {
                    format!("Downloading Companion {}", candidate.version())
                });
                let fraction = if total > 0 {
                    downloaded as f32 / total as f32
                } else {
                    0.
                };
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 12.),
                    egui::Sense::hover(),
                );
                widgets::progress_track(ui, rect, (!verifying).then_some(fraction));
                ui.small(format!("{} / {}", file_size(downloaded), file_size(total)));
            }
        }
        if !startup && !offer {
            self.update_primary_action(ui, false);
        }
    }

    pub(super) fn update_primary_action(&mut self, ui: &mut egui::Ui, startup: bool) {
        if self.update_handoff_waiting() || self.update_exit_pending() {
            return;
        }
        match self.updates.state().clone() {
            UpdateState::Idle
            | UpdateState::Cancelled
            | UpdateState::UpToDate
            | UpdateState::Failed(_) => {
                let label = match self.updates.state() {
                    UpdateState::Failed(_) => "Retry check",
                    UpdateState::UpToDate => "Check again",
                    _ => "Check for updates",
                };
                if widgets::plain_button(ui, label).clicked() {
                    self.update_error = None;
                    self.update_install_requested = None;
                    let _ = self.updates.check(self.settings.update_channel);
                }
            }
            UpdateState::Checking | UpdateState::Downloading { .. } => {
                if !startup
                    && widgets::plain_button(
                        ui,
                        if matches!(self.updates.state(), UpdateState::Checking) {
                            "Cancel"
                        } else {
                            "Cancel download"
                        },
                    )
                    .clicked()
                {
                    self.update_install_requested = None;
                    self.updates.cancel();
                }
            }
            UpdateState::Available(candidate) => {
                if widgets::plain_button(
                    ui,
                    if startup {
                        "Install update"
                    } else {
                        "Download update"
                    },
                )
                .clicked()
                {
                    self.update_error = None;
                    self.update_install_requested = startup.then_some(candidate);
                    let _ = self.updates.download();
                }
            }
            UpdateState::Ready(_) => {
                let allowed = !self.update_fixture
                    && !self.editor.is_recalibrating()
                    && self.pending_overlay.is_none();
                if widgets::button(ui,widgets::Icon::Save,if startup {"Install update"} else {"Install and restart"},allowed,"Pause and release input, resolve unsaved edits, then update through the verified Companion update window.").clicked() { self.begin_update_install(ui.ctx()); }
                if !startup {
                    if self.update_fixture {
                        ui.small("Preview only · installation disabled.");
                    } else if !allowed {
                        ui.small("Finish or cancel the selection/calibration review first.");
                    }
                    if widgets::plain_button(ui, "Discard download").clicked() {
                        self.update_install_requested = None;
                        self.updates.cancel();
                    }
                }
            }
        }
    }

    fn update_preferences(&mut self, ui: &mut egui::Ui, busy: bool) {
        let original = self.settings.clone();
        let mut changed=ui.checkbox(&mut self.settings.check_updates_on_start,"Check automatically on startup").on_hover_text("Contacts GitHub once per launch. No profiles, captures or game data are sent. Downloads and installation remain manual.").changed();
        ui.add_enabled_ui(!busy, |ui| {
            changed |= widgets::field(ui, "Release channel", |ui| {
                widgets::segmented_choice(
                    ui,
                    "update-channel",
                    &mut self.settings.update_channel,
                    &[
                        (
                            UpdateChannel::Stable,
                            "Stable",
                            "Published stable releases only.",
                        ),
                        (
                            UpdateChannel::Preview,
                            "Preview",
                            "Include prereleases as well as newer stable releases.",
                        ),
                    ],
                )
            });
        });
        if changed {
            if let Err(error) = self.settings.save(&self.root) {
                self.settings = original;
                self.error = Some(error.to_string());
            } else if self.settings.update_channel != original.update_channel {
                self.updates.cancel();
            }
        }
    }

    fn begin_update_install(&mut self, ctx: &egui::Context) {
        self.update_install_requested = None;
        let bounds = ctx.input(|input| input.viewport().inner_rect);
        let zoom = ctx.zoom_factor();
        let result = self.install_update_with(ctx, move |guard, candidate, root| {
            let mut prepared = PreparedUpdate::prepare(guard, &candidate, &root)?;
            if let Some(bounds) = bounds {
                prepared.set_window_bounds(bounds, zoom)?;
            }
            prepared.spawn()?;
            Ok(prepared)
        });
        if let Err(error) = result {
            self.update_error = Some(error);
        }
    }

    pub(super) fn update_handoff_waiting(&self) -> bool {
        matches!(
            self.update_handoff,
            Some(UpdateHandoff::Preparing(_) | UpdateHandoff::Waiting(_))
        )
    }

    fn recover_update_runtime(
        &mut self,
        ctx: &egui::Context,
        error: String,
    ) -> Option<UpdateHandoff> {
        let notify = ctx.clone();
        self.runtime = RuntimeHandle::spawn_with_notify(move || notify.request_repaint());
        self.snapshot = match self.runtime.latest() {
            Ok(snapshot) => snapshot,
            Err(worker_error) => {
                return Some(UpdateHandoff::ReleaseFailed(worker_error));
            }
        };
        self.target = None;
        self.open_window_chooser();
        self.update_install_requested = None;
        self.update_error = Some(error);
        None
    }

    fn poll_update_handoff(&mut self, ctx: &egui::Context) {
        let Some(handoff) = self.update_handoff.take() else {
            return;
        };
        self.update_handoff = match handoff {
            UpdateHandoff::Preparing(receiver) => match receiver.try_recv() {
                Ok(Ok(helper)) => Some(UpdateHandoff::Waiting(helper)),
                Ok(Err(error)) => self.recover_update_runtime(ctx, error),
                Err(TryRecvError::Empty) => Some(UpdateHandoff::Preparing(receiver)),
                Err(TryRecvError::Disconnected) => self.recover_update_runtime(
                    ctx,
                    "The update preparation worker stopped. Nothing was installed.".into(),
                ),
            },
            UpdateHandoff::Waiting(mut helper) => match helper.poll() {
                Ok(true) => match helper.commit() {
                    Ok(()) => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        Some(UpdateHandoff::InstallerStarted { _guard: helper })
                    }
                    Err(error) => self.recover_update_runtime(ctx, error),
                },
                Ok(false) => Some(UpdateHandoff::Waiting(helper)),
                Err(error) => self.recover_update_runtime(ctx, error),
            },
            state => Some(state),
        };
        if self.update_handoff_waiting() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    /// Only a retained verified download can enter the asynchronous helper handoff.
    pub(super) fn install_update_with(
        &mut self,
        ctx: &egui::Context,
        prepare: impl FnOnce(
            crate::updates::VerifiedInstaller,
            crate::updates::UpdateCandidate,
            PathBuf,
        ) -> Result<PreparedUpdate, String>
        + Send
        + 'static,
    ) -> Result<(), String> {
        self.install_update_after_decision(ctx, |app| app.discard_or_save(), prepare)
    }

    pub(super) fn install_update_after_decision(
        &mut self,
        _ctx: &egui::Context,
        decide: impl FnOnce(&mut Self) -> bool,
        prepare: impl FnOnce(
            crate::updates::VerifiedInstaller,
            crate::updates::UpdateCandidate,
            PathBuf,
        ) -> Result<PreparedUpdate, String>
        + Send
        + 'static,
    ) -> Result<(), String> {
        if self.update_fixture || self.update_handoff.is_some() {
            return Err("No verified installer is ready.".into());
        }
        let crate::updates::UpdateState::Ready(candidate) = self.updates.state() else {
            return Err("No verified installer is ready.".into());
        };
        let candidate = candidate.clone();
        if self.editor.is_recalibrating() || self.pending_overlay.is_some() {
            return Err(
                "Finish or cancel the selection/calibration review before updating.".into(),
            );
        }
        self.runtime.stop();
        if !decide(self) {
            return Err("Update cancelled; your unsaved edits are still open.".into());
        }
        let root = self
            .root
            .canonicalize()
            .map_err(|e| format!("The profile directory is unavailable: {e}"))?;
        let guard = self
            .updates
            .take_verified_installer()
            .map_err(|e| e.to_string())?;
        if let Err(error) = self.runtime.shutdown() {
            let message = format!(
                "Input could not be released; no installer was launched. Restart Companion before starting again: {error}"
            );
            self.update_handoff = Some(UpdateHandoff::ReleaseFailed(message.clone()));
            return Err(message);
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        if let Err(error) = std::thread::Builder::new()
            .name("companion-update-handoff".into())
            .spawn(move || {
                let _ = sender.send(prepare(guard, candidate, root));
            })
        {
            self.update_handoff = self.recover_update_runtime(
                _ctx,
                format!("The update preparation worker could not start: {error}"),
            );
            return Err(
                "The update preparation worker could not start. Nothing was installed.".into(),
            );
        }
        self.update_handoff = Some(UpdateHandoff::Preparing(receiver));
        self.startup_update_open = true;
        Ok(())
    }

    #[cfg(feature = "ui-smoke")]
    pub fn prepare_updates_fixture(&mut self, state: &str) -> Result<(), String> {
        use crate::updates::UpdateFixture;
        let fixture = match state {
            "idle" => UpdateFixture::Idle,
            "checking" => UpdateFixture::Checking,
            "up-to-date" => UpdateFixture::UpToDate,
            "available" => UpdateFixture::Available,
            "downloading" => UpdateFixture::Downloading,
            "ready" => UpdateFixture::Ready,
            "failed" => UpdateFixture::Failed,
            _ => return Err("Unknown update fixture.".into()),
        };
        self.updates =
            crate::updates::UpdateService::fixture(fixture).map_err(|e| e.to_string())?;
        self.update_fixture = true;
        self.startup_update_open = false;
        self.window_chooser.open = false;
        self.tab = Tab::Settings;
        self.settings_page = SettingsPage::Updates;
        Ok(())
    }

    #[cfg(feature = "ui-smoke")]
    pub fn prepare_startup_update_fixture(&mut self) {
        self.startup_update_open = true;
        self.window_chooser.open = true;
    }
}
