use super::*;

impl MetinApp {
    /// eframe calls logic even when the native window is minimized or hidden.
    pub(super) fn background_tick(&mut self, ctx: &egui::Context) {
        #[cfg(feature = "ui-smoke")]
        if self.icon_sheet || self.update_helper_fixture.is_some() {
            return;
        }
        self.sync(ctx);
        self.sync_updates(ctx);
        self.sync_game_overlay(ctx);
        self.sync_preview_demand();
        self.flush_panel_position(false);
    }
}

#[cfg(feature = "ui-smoke")]
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum QolFixture {
    Changes,
    Import,
    Recognition,
    Paused,
    MetinPreset,
    MetinPresetNew,
    MetinPresetCustom,
}

#[cfg(feature = "ui-smoke")]
impl MetinApp {
    pub fn prepare_qol_fixture(
        &mut self,
        ctx: &egui::Context,
        fixture: Option<QolFixture>,
        zoom: Option<f32>,
    ) -> Result<(), String> {
        if let Some(zoom) = zoom {
            if !zoom.is_finite() || !(0.75..=1.5).contains(&zoom) {
                return Err("Screenshot zoom must be between 0.75 and 1.5.".into());
            }
            self.settings.ui_scale = zoom;
            ctx.set_zoom_factor(zoom);
        }
        match fixture {
            Some(
                QolFixture::MetinPreset
                | QolFixture::MetinPresetNew
                | QolFixture::MetinPresetCustom,
            ) => {
                if !matches!(fixture, Some(QolFixture::MetinPresetCustom)) {
                    let existing = self
                        .metin_presets
                        .catalog
                        .iter()
                        .find(|preset| preset.name == "Red stones")
                        .map(|preset| preset.id);
                    let preset = self
                        .store
                        .save_metin_preset(
                            existing,
                            "Red stones",
                            &self.editor.profile.color.clone().unwrap_or_default(),
                        )
                        .map_err(|error| error.to_string())?;
                    self.editor.profile.color = Some(preset.color.clone());
                    self.editor.profile.color_preset_id = Some(preset.id);
                    self.editor.record();
                    self.metin_presets.catalog = self
                        .store
                        .metin_presets()
                        .map_err(|error| error.to_string())?;
                }
                self.open_metin_preset(matches!(fixture, Some(QolFixture::MetinPresetNew)));
            }
            Some(QolFixture::Changes) => {
                self.editor.profile.loot.presses += 1;
                self.editor.profile.timing.search_timeout += 1.;
                if let Some(skill) = self.editor.profile.buffs.skills.first_mut() {
                    skill.interval_seconds = Some(skill.interval_seconds.unwrap_or(120.) + 60.);
                }
                self.editor.record();
                ctx.data_mut(|data| {
                    data.insert_temp(egui::Id::unique("qol-changes-fixture"), true)
                });
            }
            Some(QolFixture::Import) => self.prepare_profile_import_fixture(ctx)?,
            Some(QolFixture::Recognition) => self.prepare_recognition_check_fixture(ctx)?,
            Some(QolFixture::Paused) => {
                let mut snapshot = (*self.snapshot).clone();
                snapshot.running = false;
                snapshot.arming = false;
                snapshot.activity = metin_core::ActivityKind::Paused;
                snapshot.state = "Paused".into();
                snapshot.reason="No fresh frame was received from the selected window before the capture deadline.".into();
                snapshot.pause_info = Some(crate::runtime::PauseInfo {
                    cause: crate::runtime::PauseCause::StaleFrame,
                    details: snapshot.reason.clone(),
                });
                self.snapshot = Arc::new(snapshot);
                if let Some(fixture) = &mut self.window_fixture {
                    fixture.snapshot = self.snapshot.clone();
                }
            }
            None => {}
        }
        Ok(())
    }
}

impl MetinApp {
    pub(super) fn actions_breadcrumbs(&mut self, ui: &mut egui::Ui) -> bool {
        let section = match self.actions_page {
            ActionsPage::Buffs => "Buffs",
            ActionsPage::VisualRules => "Visual rules",
            ActionsPage::Loot => "Loot",
            ActionsPage::Mount => "Mount",
            ActionsPage::Periodic => "Periodic actions",
            ActionsPage::Puller => "Auto-puller",
            ActionsPage::OutOfTarget => "Out-of-target attack",
        };
        let detail = match self.actions_page {
            ActionsPage::Buffs if self.buff_detail => self
                .editor
                .profile
                .buffs
                .skills
                .get(self.selected_skill)
                .map(|skill| skill.name.clone())
                .or_else(|| Some("Buff".into())),
            ActionsPage::Periodic => self
                .selected_periodic
                .and_then(|id| {
                    self.editor
                        .profile
                        .periodic_actions
                        .iter()
                        .find(|action| action.id == id)
                })
                .map(|action| action.name.clone()),
            ActionsPage::VisualRules if self.visual_detail => self
                .editor
                .profile
                .safety
                .rules
                .get(self.selected_safety)
                .map(|rule| rule.name.clone()),
            _ => None,
        };
        let mut back_catalog = false;
        let mut back_list = false;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.;
            back_catalog = widgets::plain_button(ui, "Actions")
                .on_hover_text("Back to all actions; keep profile edits.")
                .clicked();
            ui.weak("›");
            if detail.is_some() {
                back_list = widgets::plain_button(ui, section)
                    .on_hover_text("Back to this action list; keep profile edits.")
                    .clicked();
            } else {
                ui.strong(section);
            }
            if let Some(name) = &detail {
                ui.weak("›");
                ui.add(egui::Label::new(name).truncate())
                    .on_hover_text(name);
            }
        });
        if back_catalog || back_list {
            self.buff_detail = false;
            self.selected_periodic = None;
            self.visual_detail = false;
            self.binding = None;
            if back_catalog {
                self.actions_detail = false;
            }
            return true;
        }
        false
    }
    pub(super) fn start_profile(&mut self) -> Result<(), String> {
        self.start_profile_in_generation(self.runtime.stop_generation())
    }
    fn start_profile_in_generation(&mut self, start_generation: u64) -> Result<(), String> {
        if self.window_chooser.open || self.startup_update_open || self.update_handoff_waiting() {
            return Err("Finish choosing the game window or the current update first.".into());
        }
        if self.pending_overlay.is_some()
            || self.metin_presets.draft.is_some()
            || self.binding.is_some()
            || self.profile_rename.is_some()
            || self.recognition_check.is_pending()
        {
            return Err("Finish or cancel the current selection or edit before starting.".into());
        }
        if let Some(blocker) = self.start_blocker() {
            return Err(blocker.message);
        }
        let generation = if self.has_unsaved_profile() {
            self.save_in_generation(Some(start_generation))?
        } else if self.snapshot.applied_revision != Some(self.revision) {
            self.configure_in_generation(Some(start_generation))?
        } else {
            Some(start_generation)
        };
        let generation =
            generation.ok_or("Profile saved; Start cancelled because Pause was requested.")?;
        let key = self.key().ok_or("Choose a game window before starting.")?;
        self.runtime.arm_in_generation(key, generation)
    }

    pub(super) fn consume_start_request(&mut self) {
        let Some(request) = self.snapshot.start_request else {
            return;
        };
        if request.id <= self.last_start_request {
            return;
        }
        self.last_start_request = request.id;
        let observed_revision = if self.snapshot.applied_revision == Some(self.revision) {
            self.revision
        } else {
            self.snapshot.revision
        };
        if !request.matches(
            self.key(),
            observed_revision,
            self.runtime.stop_generation(),
        ) {
            return;
        }
        if let Err(error) = self.start_profile_in_generation(request.generation) {
            self.error = Some(error);
        }
    }
}
