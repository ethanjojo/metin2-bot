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

impl MetinApp {
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
            let width = ui.available_width() - 4. * 26. - 4. * ui.spacing().item_spacing.x;
            let response = widgets::combo_trigger(ui, egui::Id::unique("profile-menu"), &label, width);
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
            .width(response.rect.width());
        #[cfg(feature = "ui-smoke")]
        let popup = if self.profile_menu_fixture {
            popup.open(true)
        } else {
            popup
        };
        let open = popup.show(|ui| {
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
            select::menu_scroll(ui, "profile-options", 190., |ui| {
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
            ui.horizontal(|ui| {
                if widgets::button(
                    ui,
                    Icon::Plus,
                    "New profile",
                    true,
                    "Create a new profile; unsaved changes are checked first.",
                )
                .clicked()
                    && self.discard_or_save()
                {
                    self.replace_profile(Profile::create("New profile"), None);
                    self.navigate(Tab::Calibration);
                    ui.close();
                }
            });
        });
        if open.is_none() {
            self.profile_rename = None;
        }
    }
}
