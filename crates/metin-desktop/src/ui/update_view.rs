use super::*;
use crate::updates::{UpdateChannel, UpdateState, VerifiedInstaller};
use std::{ffi::OsString, path::Path};

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
    InstallerStarted { _guard: VerifiedInstaller },
    ReleaseFailed(String),
}

fn installer_arguments(root: &Path) -> Result<Vec<OsString>, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("The profile directory is unavailable: {e}"))?;
    let mut data = OsString::from("/COMPANIONDATADIR=");
    data.push(root);
    Ok(vec![OsString::from("/RESTARTCOMPANION=1"), data])
}

impl MetinApp {
    pub(super) fn update_exit_pending(&self) -> bool {
        matches!(
            self.update_handoff,
            Some(UpdateHandoff::InstallerStarted { .. })
        )
    }
    pub(super) fn sync_updates(&mut self, ctx: &egui::Context) {
        if self.updates.poll() {
            ctx.request_repaint();
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
        match state {
            UpdateState::Idle | UpdateState::Cancelled => {
                ui.weak(if matches!(state, UpdateState::Cancelled) {
                    "Update cancelled. Nothing was installed."
                } else {
                    "Check when you are ready. Nothing downloads automatically."
                });
                if widgets::plain_button(ui, "Check for updates").clicked() {
                    let _ = self.updates.check(self.settings.update_channel);
                }
            }
            UpdateState::Checking => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Checking GitHub…");
                    if widgets::plain_button(ui, "Cancel").clicked() {
                        self.updates.cancel();
                    }
                });
            }
            UpdateState::UpToDate => {
                widgets::badge(ui, "No compatible newer release", widgets::GREEN);
                ui.weak("Only verified Windows Companion installers in the selected channel are offered.");
                if widgets::plain_button(ui, "Check again").clicked() {
                    let _ = self.updates.check(self.settings.update_channel);
                }
            }
            UpdateState::Failed(error) => {
                ui.colored_label(widgets::RED, error.to_string());
                ui.weak("Your application and profiles have not been changed.");
                if widgets::plain_button(ui, "Retry check").clicked() {
                    let _ = self.updates.check(self.settings.update_channel);
                }
            }
            UpdateState::Available(candidate) | UpdateState::Ready(candidate) => {
                let ready = matches!(self.updates.state(), UpdateState::Ready(_));
                widgets::badge(
                    ui,
                    &format!("Companion {}", candidate.version()),
                    widgets::GREEN,
                );
                ui.small(format!("Windows x64 · {}", file_size(candidate.size())));
                if ready {
                    ui.label("Download verified. Ready to install.");
                    let allowed = !self.update_fixture
                        && !self.editor.is_recalibrating()
                        && self.pending_overlay.is_none();
                    if widgets::button(ui,widgets::Icon::Save,"Install and restart",allowed,"Pause and release input, resolve unsaved edits, then open the verified installer. Driver consent remains in Setup.").clicked() {
                        let ctx=ui.ctx().clone();
                        if let Err(error)=self.install_update_with(&ctx,|path,args| {
                            let mut command=std::process::Command::new(path);
                            command.args(args);
                            #[cfg(windows)] {
                                use std::os::windows::process::CommandExt;
                                command.creation_flags(0x08000000); // CREATE_NO_WINDOW
                            }
                            command.spawn().map(|_|()).map_err(|e|format!("The installer could not start: {e}"))
                        }) { self.error=Some(error); }
                    }
                    if self.update_fixture {
                        ui.small("Preview only · installation disabled.");
                    } else if !allowed {
                        ui.small("Finish or cancel the selection/calibration review first.");
                    }
                    if widgets::plain_button(ui, "Discard download").clicked() {
                        self.updates.cancel();
                    }
                } else if widgets::plain_button(ui, "Download update").clicked() {
                    let _ = self.updates.download();
                }
                ui.add_space(6.);
                ui.strong("Release notes");
                let notes = candidate.release_notes();
                egui::ScrollArea::vertical()
                    .id_salt("update-notes")
                    .max_height(100.)
                    .show(ui, |ui| {
                        ui.label(if notes.trim().is_empty() {
                            "No release notes were provided."
                        } else {
                            notes
                        });
                    });
                ui.hyperlink_to("View release on GitHub", candidate.release_url());
            }
            UpdateState::Downloading {
                candidate,
                downloaded,
                total,
            } => {
                ui.label(format!("Downloading Companion {}", candidate.version()));
                let fraction = if total > 0 {
                    downloaded as f32 / total as f32
                } else {
                    0.
                };
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 12.),
                    egui::Sense::hover(),
                );
                widgets::progress_track(ui, rect, Some(fraction));
                ui.small(format!("{} / {}", file_size(downloaded), file_size(total)));
                if widgets::plain_button(ui, "Cancel download").clicked() {
                    self.updates.cancel();
                }
            }
        }
        if primary_first {
            ui.separator();
            ui.small("Update preferences");
            self.update_preferences(ui, busy);
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

    /// Execution authority is a retained verified handle, never a caller-supplied path.
    pub(super) fn install_update_with(
        &mut self,
        ctx: &egui::Context,
        launch: impl FnOnce(&Path, &[OsString]) -> Result<(), String>,
    ) -> Result<(), String> {
        self.install_update_after_decision(ctx, |app| app.discard_or_save(), launch)
    }
    pub(super) fn install_update_after_decision(
        &mut self,
        ctx: &egui::Context,
        decide: impl FnOnce(&mut Self) -> bool,
        launch: impl FnOnce(&Path, &[OsString]) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.update_fixture || !matches!(self.updates.state(), UpdateState::Ready(_)) {
            return Err("No verified installer is ready.".into());
        }
        if self.editor.is_recalibrating() || self.pending_overlay.is_some() {
            return Err(
                "Finish or cancel the selection/calibration review before updating.".into(),
            );
        }
        self.runtime.stop();
        if !decide(self) {
            return Err("Update cancelled; your unsaved edits are still open.".into());
        }
        let args = installer_arguments(&self.root)?;
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
        if let Err(error) = launch(guard.installer_path(), &args) {
            // The runtime was safely stopped before execution. A failed launcher
            // must leave an operable paused application rather than a dead worker.
            let notify = ctx.clone();
            self.runtime = RuntimeHandle::spawn_with_notify(move || notify.request_repaint());
            self.snapshot = self.runtime.latest()?;
            self.target = None;
            self.open_window_chooser();
            return Err(error);
        }
        self.update_handoff = Some(UpdateHandoff::InstallerStarted { _guard: guard });
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        Ok(())
    }

    #[cfg(feature = "ui-smoke")]
    pub fn prepare_updates_fixture(&mut self, state: &str) -> Result<(), String> {
        use crate::updates::UpdateFixture;
        let fixture = match state {
            "idle" => UpdateFixture::Idle,
            "available" => UpdateFixture::Available,
            "downloading" => UpdateFixture::Downloading,
            "ready" => UpdateFixture::Ready,
            "failed" => UpdateFixture::Failed,
            _ => return Err("Unknown update fixture.".into()),
        };
        self.updates =
            crate::updates::UpdateService::fixture(fixture).map_err(|e| e.to_string())?;
        self.update_fixture = true;
        self.window_chooser.open = false;
        self.tab = Tab::Settings;
        self.settings_page = SettingsPage::Updates;
        Ok(())
    }
}
