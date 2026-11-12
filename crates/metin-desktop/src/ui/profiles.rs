use super::*;
use widgets::Icon;

pub(super) struct RenameDraft {
    id: uuid::Uuid,
    name: String,
    focus: bool,
    error: Option<String>,
}
struct RowFocus {
    choice: egui::Id,
    pencil: egui::Id,
    rect: egui::Rect,
}

#[derive(Clone)]
pub(super) struct ImportPreview {
    pub(super) package: Arc<crate::storage::ProfilePackage>,
    pub(super) error: Option<String>,
}

pub(super) fn import_preview_id() -> egui::Id {
    egui::Id::unique("profile-import-preview")
}

impl MetinApp {
    #[cfg(feature = "ui-smoke")]
    pub fn prepare_profile_import_fixture(
        &mut self,
        context: &egui::Context,
    ) -> Result<(), String> {
        let package = self
            .store
            .export_profile(self.editor.profile.id)
            .map_err(|e| e.to_string())?;
        context.data_mut(|d| {
            d.insert_temp(
                import_preview_id(),
                ImportPreview {
                    package: Arc::new(package),
                    error: None,
                },
            )
        });
        self.profile_menu_fixture = true;
        self.profile_open_requested = true;
        Ok(())
    }

    fn transfer_profile_source(&mut self) -> Result<Option<uuid::Uuid>, String> {
        if self.pending_overlay.is_some() || self.editor.is_recalibrating() {
            return Err("Finish or cancel the current capture or calibration review first.".into());
        }
        if !self.discard_or_save() {
            return Ok(None);
        }
        let id = self.editor.profile.id;
        self.store
            .load(id)
            .map_err(|_| "Save this profile before duplicating or exporting it.".to_owned())?;
        Ok(Some(id))
    }

    fn duplicate_selected_profile(&mut self) -> Result<bool, String> {
        let Some(id) = self.transfer_profile_source()? else {
            return Ok(false);
        };
        self.runtime.stop();
        let duplicate = self
            .store
            .duplicate_profile(id)
            .map_err(|e| e.to_string())?;
        self.activate_imported_profile(duplicate)?;
        self.show_feedback("Profile duplicated", false);
        Ok(true)
    }

    fn activate_imported_profile(&mut self, profile: Profile) -> Result<(), String> {
        let id = profile.id;
        let (profile, template) = self.store.load(id).map_err(|e| e.to_string())?;
        self.store.select(id).map_err(|e| e.to_string())?;
        self.profiles = self.store.list().map_err(|e| e.to_string())?;
        self.replace_profile(profile, template);
        Ok(())
    }

    fn confirm_profile_import(
        &mut self,
        package: &crate::storage::ProfilePackage,
    ) -> Result<bool, String> {
        if self.pending_overlay.is_some() || self.editor.is_recalibrating() {
            return Err("Finish or cancel the current capture or calibration review first.".into());
        }
        if !self.discard_or_save() {
            return Ok(false);
        }
        self.runtime.stop();
        let imported = self
            .store
            .import_profile(package)
            .map_err(|e| e.to_string())?;
        self.activate_imported_profile(imported)?;
        self.show_feedback("Profile imported", false);
        Ok(true)
    }

    fn profile_import_preview(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(mut preview) = ui
            .ctx()
            .data(|d| d.get_temp::<ImportPreview>(import_preview_id()))
        else {
            return false;
        };
        widgets::heading(
            ui,
            "Import profile",
            "Review this package before creating a separate profile.",
        );
        let summary_height = (ui.available_height() - 100.).clamp(55., 145.);
        egui::ScrollArea::vertical().id_salt("profile-import-summary").max_height(summary_height).show(ui,|ui| {
            ui.add(egui::Label::new(egui::RichText::new(preview.package.profile_name()).strong()).wrap());
            ui.label(format!("{} recognition images",preview.package.image_count()));
            ui.label(preview.package.frame_size().map_or_else(||"No calibration size saved".to_owned(),|[w,h]|format!("Calibrated size: {w} × {h} px")));
            ui.small("Existing profiles are kept. Session state, mount confirmation and timers are not imported. Review recognition for the selected game window before Start.");
            if let Some(error)=&preview.error {ui.colored_label(widgets::RED,error);}
        });
        ui.separator();
        let mut confirm = false;
        let mut cancel = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        ui.horizontal_wrapped(|ui| {
            confirm=widgets::button(ui,Icon::Check,"Import profile",self.pending_overlay.is_none()&&!self.editor.is_recalibrating(),"Create a new profile from this exact package; existing profiles are never overwritten.").clicked();
            cancel|=widgets::button(ui,Icon::Cross,"Cancel",true,"Cancel profile import without changing profiles.").clicked();
        });
        if cancel {
            ui.ctx()
                .data_mut(|d| d.remove::<ImportPreview>(import_preview_id()));
        } else if confirm {
            match self.confirm_profile_import(&preview.package) {
                Ok(true) => {
                    ui.ctx()
                        .data_mut(|d| d.remove::<ImportPreview>(import_preview_id()));
                    ui.close();
                }
                Ok(false) => {}
                Err(error) => {
                    preview.error = Some(error);
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(import_preview_id(), preview));
                }
            }
        }
        true
    }

    fn profile_transfer_actions(&mut self, ui: &mut egui::Ui) {
        let allowed = self.pending_overlay.is_none()
            && !self.editor.is_recalibrating()
            && self.profile_rename.is_none();
        ui.horizontal_wrapped(|ui| {
            if widgets::button(ui,Icon::Plus,"New profile",allowed,"Create a new profile; unsaved changes are checked first.").clicked() && self.discard_or_save() {
                self.replace_profile(Profile::create("New profile"),None);
                self.navigate(Tab::Calibration);
                ui.close();
            }
            ui.add_enabled_ui(allowed,|ui| {
                if widgets::plain_button(ui,"Duplicate").on_hover_text("Duplicate the selected saved profile and its images. Unsaved edits are checked first; the copy starts with no session state.").clicked() {
                    match self.duplicate_selected_profile(){Err(error)=>self.error=Some(error),Ok(true)=>ui.close(),Ok(false)=>{}}
                }
                if widgets::plain_button(ui,"Export…").on_hover_text("Save the selected profile and recognition images in one portable Companion profile package.").clicked() {
                    let result=(||->Result<(),String>{
                        let Some(id)=self.transfer_profile_source()? else {return Ok(());};
                        let package=self.store.export_profile(id).map_err(|e|e.to_string())?;
                        let name=package.profile_name().chars().filter(|c|!c.is_control()&&!"/\\:*?\"<>|".contains(*c)).take(60).collect::<String>();
                        if let Some(path)=rfd::FileDialog::new().set_title("Export profile").set_file_name(format!("Companion-{name}.companion-profile")).add_filter("Companion profile",&["companion-profile"]).save_file(){
                            let resolved=crate::storage::resolve_new_path(&path).map_err(|e|e.to_string())?;
                            let data_root=self.root.canonicalize().map_err(|e|e.to_string())?;
                            if resolved.starts_with(data_root){return Err("Choose an export location outside Companion's application data folder.".into());}
                            package.write(&path).map_err(|e|e.to_string())?;
                            self.show_feedback("Profile exported",false);
                        }
                        Ok(())
                    })();
                    if let Err(error)=result {self.error=Some(error);}
                }
                if widgets::plain_button(ui,"Import…").on_hover_text("Choose a Companion profile package and review it before creating a new profile.").clicked() && let Some(path)=rfd::FileDialog::new().set_title("Import profile").add_filter("Companion profile",&["companion-profile"]).pick_file() {
                        match crate::storage::ProfilePackage::read(&path) {
                            Ok(package)=>{ui.ctx().data_mut(|d|d.insert_temp(import_preview_id(),ImportPreview{package:Arc::new(package),error:None}));self.profile_open_requested=true;}
                            Err(error)=>self.error=Some(error.to_string()),
                        }
                }
            });
        });
    }

    pub(super) fn begin_profile_rename(&mut self, id: uuid::Uuid) {
        let name = if id == self.editor.profile.id {
            Some(self.editor.profile.name.clone())
        } else {
            self.profiles
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.name.clone())
        };
        self.profile_rename = name.map(|name| RenameDraft {
            id,
            name,
            focus: true,
            error: None,
        });
    }

    fn commit_profile_rename(&mut self) -> Result<(), String> {
        if self.pending_overlay.is_some() || self.editor.is_recalibrating() {
            return Err("Finish or cancel the current capture or calibration review first.".into());
        }
        let draft = self
            .profile_rename
            .as_ref()
            .ok_or("No profile name is being edited.")?;
        let id = draft.id;
        let name = draft.name.trim().to_owned();
        // Read the persisted profile, never the active editor's unrelated draft values.
        let mut profile =
            if id == self.editor.profile.id && !self.profiles.iter().any(|p| p.id == id) {
                self.editor.saved_profile().clone()
            } else {
                self.store.load(id).map_err(|e| e.to_string())?.0
            };
        profile.name = name;
        let saved = self.store.save(&profile, None).map_err(|e| e.to_string())?;
        if id == self.editor.profile.id {
            self.editor.apply_saved_name(&saved.profile.name);
            self.store.select(id).map_err(|e| e.to_string())?;
        }
        self.profiles = self.store.list().map_err(|e| e.to_string())?;
        self.profile_rename = None;
        self.profile_search.clear();
        self.show_feedback("Profile renamed", false);
        Ok(())
    }

    fn profile_menu_row(
        &mut self,
        ui: &mut egui::Ui,
        id: uuid::Uuid,
        name: &str,
    ) -> Option<RowFocus> {
        let editing = self
            .profile_rename
            .as_ref()
            .is_some_and(|edit| edit.id == id);
        let allowed = self.pending_overlay.is_none() && !self.editor.is_recalibrating();
        let mut confirm = false;
        let mut cancel = false;
        let mut focus = None;
        ui.push_id(id, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.;
                let controls = if editing { 2. } else { 1. };
                let width = (ui.available_width() - controls * 31.).max(40.);
                if editing {
                    let draft = self.profile_rename.as_mut().expect("editing row");
                    let input = ui.add_sized(
                        egui::vec2(width, 28.),
                        egui::TextEdit::singleline(&mut draft.name)
                            .id(egui::Id::unique(("profile-name", id)))
                            .char_limit(80)
                            .hint_text("Profile name"),
                    );
                    if draft.focus {
                        input.request_focus();
                        input.scroll_to_me(Some(egui::Align::Center));
                        draft.focus = false;
                    }
                    if input.changed() {
                        draft.error = None;
                    }
                    let enter = (input.has_focus() || input.lost_focus())
                        && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                    let escape =
                        ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
                    let valid = !draft.name.trim().is_empty();
                    confirm = (widgets::small_icon_button(
                        ui,
                        Icon::Check,
                        allowed && valid,
                        "Save profile name (Enter)",
                    )
                    .on_disabled_hover_text(if valid {
                        "Finish the current capture or calibration review first."
                    } else {
                        "Enter a profile name before saving (up to 80 characters)."
                    })
                    .clicked()
                        || enter)
                        && allowed
                        && valid;
                    cancel = widgets::small_icon_button(
                        ui,
                        Icon::Cross,
                        true,
                        "Cancel profile rename (Escape)",
                    )
                    .clicked()
                        || escape;
                } else {
                    let label = if name.trim().is_empty() {
                        "Unnamed profile"
                    } else {
                        name
                    };
                    let choice = select::sized_option_row(
                        ui,
                        id == self.editor.profile.id,
                        label,
                        egui::vec2(width, 28.),
                    );
                    if choice.clicked() {
                        self.switch_profile(id);
                        ui.close();
                    }
                    let pencil = widgets::small_icon_button(
                        ui,
                        Icon::Edit,
                        allowed,
                        &format!("Rename profile: {label}"),
                    )
                    .on_disabled_hover_text(
                        "Finish the current capture or calibration review first.",
                    );
                    focus = Some(RowFocus {
                        choice: choice.id,
                        pencil: pencil.id,
                        rect: choice.rect,
                    });
                    #[cfg(test)]
                    ui.ctx().data_mut(|d| {
                        d.insert_temp(egui::Id::unique(("rename-profile", id)), pencil.rect)
                    });
                    if pencil.clicked() {
                        self.begin_profile_rename(id);
                    }
                }
            })
        });
        if cancel {
            self.profile_rename = None;
        } else if confirm
            && let Err(error) = self.commit_profile_rename()
            && let Some(draft) = &mut self.profile_rename
        {
            draft.error = Some(error);
        }
        if let Some(error) = self
            .profile_rename
            .as_ref()
            .filter(|edit| edit.id == id)
            .and_then(|edit| edit.error.as_ref())
        {
            ui.label(egui::RichText::new(error).small().color(widgets::RED));
        }
        focus
    }
    pub(super) fn profile_select(&mut self, ui: &mut egui::Ui) {
        let dirty = self.has_unsaved_profile();
        let label = format!(
            "{}{}",
            self.editor.profile.name,
            if dirty { "  *" } else { "" }
        );
        let response = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.;
            let change_width = self.changes_width(ui);
            let width = ui.available_width() - 4. * 26. - 4. * ui.spacing().item_spacing.x - change_width;
            let response = widgets::combo_trigger(ui, egui::Id::unique("profile-menu"), &label, width);
            self.changes_badge(ui);
            let allowed = self.pending_overlay.is_none();
            if widgets::small_icon_button(
                ui, Icon::Undo, self.editor.can_undo() && allowed, "Undo profile edit (Ctrl+Z)",
            ).clicked() {
                self.undo_profile();
            }
            if widgets::small_icon_button(
                ui, Icon::Redo, self.editor.can_redo() && allowed, "Redo profile edit (Ctrl+Y)",
            ).clicked() {
                self.redo_profile();
            }
            let can_commit = dirty && allowed && !self.editor.is_recalibrating();
            if widgets::small_icon_button(
                ui, Icon::Revert, can_commit,
                "Revert all unsaved changes to the saved profile. The connected game window stays selected.",
            ).clicked() {
                self.revert_profile();
            }
            if widgets::small_icon_button(
                ui, Icon::Save, can_commit, "Save profile changes. Applying changes pauses automation.",
            ).clicked() && let Err(error) = self.save() {
                self.error = Some(error);
            }
            response
        }).inner;
        #[cfg(test)]
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::unique("profile-trigger"), response.rect));
        if response.clicked() {
            self.profile_search.clear();
            self.profile_rename = None;
            ui.ctx()
                .data_mut(|d| d.remove::<ImportPreview>(import_preview_id()));
        }
        if self.profile_open_requested {
            egui::Popup::open_id(ui.ctx(), egui::Popup::default_response_id(&response));
            self.profile_open_requested = false;
        }
        // This is an editable chooser, not a flat menu: menu arrow navigation
        // would visit each pencil as a separate vertical item.
        let popup = egui::Popup::from_toggle_button_response(&response)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .layout(egui::Layout::top_down_justified(egui::Align::Min))
            .frame(skin::popup_frame(ui))
            .width(
                response
                    .rect
                    .width()
                    .max(300.)
                    .min(ui.ctx().content_rect().width() - 24.),
            );
        #[cfg(feature = "ui-smoke")]
        let popup = if self.profile_menu_fixture {
            popup.open(true)
        } else {
            popup
        };
        let open = popup.show(|ui| {
            if self.profile_import_preview(ui) {
                return;
            }
            let direction = if self.profile_rename.is_none() {
                ui.input_mut(|i| {
                    if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                        1
                    } else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                        -1
                    } else {
                        0
                    }
                })
            } else {
                0
            };
            if direction != 0 {
                ui.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
            }
            let focused = ui.memory(|m| m.focused());
            ui.add(
                egui::TextEdit::singleline(&mut self.profile_search)
                    .hint_text("Search profiles…")
                    .desired_width(f32::INFINITY),
            );
            let mut options = self
                .profiles
                .iter()
                .map(|p| {
                    (
                        p.id,
                        if p.id == self.editor.profile.id {
                            self.editor.profile.name.clone()
                        } else {
                            p.name.clone()
                        },
                    )
                })
                .collect::<Vec<_>>();
            if !options.iter().any(|(id, _)| *id == self.editor.profile.id) {
                options.push((self.editor.profile.id, self.editor.profile.name.clone()));
            }
            let profiles = options
                .iter()
                .filter(|(id, name)| {
                    self.profile_rename
                        .as_ref()
                        .is_some_and(|edit| edit.id == *id)
                        || self.profile_search.trim().is_empty()
                        || name
                            .to_lowercase()
                            .contains(&self.profile_search.trim().to_lowercase())
                })
                .cloned()
                .collect::<Vec<_>>();
            let options_height = (ui.available_height() - 100.).clamp(40., 190.);
            select::menu_scroll(ui, "profile-options", options_height, |ui| {
                let mut rows = Vec::new();
                if profiles.is_empty() {
                    ui.weak("No matching profiles.");
                }
                for (id, name) in profiles {
                    if let Some(row) = self.profile_menu_row(ui, id, &name) {
                        rows.push(row);
                    }
                }
                if direction != 0 && !rows.is_empty() {
                    let current = rows
                        .iter()
                        .position(|row| focused == Some(row.choice) || focused == Some(row.pencil));
                    let next = match (current, direction) {
                        (Some(index), 1) => (index + 1) % rows.len(),
                        (Some(index), _) => (index + rows.len() - 1) % rows.len(),
                        (None, 1) => 0,
                        (None, _) => rows.len() - 1,
                    };
                    ui.memory_mut(|m| m.request_focus(rows[next].choice));
                    ui.scroll_to_rect(rows[next].rect, Some(egui::Align::Center));
                    ui.ctx().request_repaint();
                }
            });
            ui.separator();
            self.profile_transfer_actions(ui);
        });
        if open.is_none() {
            self.profile_rename = None;
            ui.ctx()
                .data_mut(|d| d.remove::<ImportPreview>(import_preview_id()));
        }
    }
}
