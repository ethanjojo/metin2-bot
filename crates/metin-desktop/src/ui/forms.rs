use super::*;
use crate::ui_controls::{numeric, numeric_u32};
impl MetinApp {
    fn choose_mode(&mut self, ui: &mut egui::Ui, mode: EditMode, label: &str) {
        let ready = if mode == EditMode::Color {
            self.editor.profile.color.is_some()
        } else {
            self.editor.region(mode).is_some()
        };
        let response = ui.selectable_label(
            self.mode == mode,
            format!("{} · {label}", if ready { "Ready" } else { "Set" }),
        );
        if response.hovered() {
            self.hover_mode = Some(mode);
        }
        if response.clicked() {
            self.runtime.stop();
            self.live = false;
            self.mode = mode;
            self.gesture = None;
        }
    }
    pub(super) fn calibration_form(&mut self, ui: &mut egui::Ui) {
        ui.label("Profile name");
        if ui
            .text_edit_singleline(&mut self.editor.profile.name)
            .changed()
        {
            self.editor.record();
        }
        ui.add_space(8.);
        ui.strong("Game window");
        let old = self.target;
        let mut target = self.target;
        egui::ComboBox::from_id_salt("game-window")
            .width(ui.available_width() - 8.)
            .selected_text(
                self.snapshot
                    .targets
                    .iter()
                    .find(|w| Some(w.token) == target)
                    .map(|w| w.title.as_str())
                    .unwrap_or("Select a game window"),
            )
            .show_ui(ui, |ui| {
                for window in &self.snapshot.targets {
                    ui.selectable_value(
                        &mut target,
                        Some(window.token),
                        format!("{} · {}", window.title, window.process_name),
                    );
                }
            });
        if target != old {
            self.runtime.stop();
            self.target = target;
            if let Err(error) = self.configure() {
                self.error = Some(error);
            }
            self.snapshot_requested = None;
        }
        ui.horizontal(|ui| {
            if ui.button("Refresh windows").clicked() {
                self.send(Command::RefreshTargets);
            }
            if ui
                .add_enabled(self.target.is_some(), egui::Button::new("Snapshot"))
                .clicked()
            {
                self.runtime.stop();
                self.snapshot_requested = Some(std::time::Instant::now());
            }
        });
        ui.label("Take a Snapshot with a target panel visible, then mark the areas below.");
        ui.separator();
        self.choose_mode(ui, EditMode::World, "Search area");
        self.choose_mode(ui, EditMode::Color, "Metin color");
        self.choose_mode(ui, EditMode::Panel, "Target panel");
        self.choose_mode(ui, EditMode::Template, "Fixed crop");
        ui.separator();
        ui.strong("Excluded areas");
        for i in 0..self.editor.profile.masks.len() {
            self.choose_mode(ui, EditMode::Mask(Some(i)), &format!("Mask {}", i + 1));
        }
        if ui.button("New excluded area").clicked() {
            self.runtime.stop();
            self.live = false;
            self.mode = EditMode::Mask(None);
            self.gesture = None;
        }
        ui.add_space(8.);
        if ui.button("Delete selected area").clicked() {
            self.runtime.stop();
            if let Err(e) = self.editor.delete(self.mode) {
                self.error = Some(e);
            }
        }
        if self.editor.profile.ready() {
            ui.label("Calibration ready");
        } else {
            ui.label("Complete the search area, color, panel and fixed crop before starting.");
        }
    }
    pub(super) fn settings_form(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        ui.strong("Window filter");
        changed |= ui
            .text_edit_singleline(&mut self.editor.profile.process_name)
            .changed();
        ui.label("A process-name hint; the selected window remains the target.");
        if let Some(c) = &mut self.editor.profile.color {
            ui.separator();
            ui.strong("Color detection");
            for (label, name, value) in [
                ("Hue tolerance", "hue_tolerance", &mut c.hue_tolerance),
                (
                    "Saturation tolerance",
                    "saturation_tolerance",
                    &mut c.saturation_tolerance,
                ),
                ("Value tolerance", "value_tolerance", &mut c.value_tolerance),
                ("Minimum area", "min_area", &mut c.min_area),
                ("Minimum width", "min_width", &mut c.min_width),
                ("Minimum height", "min_height", &mut c.min_height),
                ("Stable frames", "stable_frames", &mut c.stable_frames),
            ] {
                changed |= numeric_u32(ui, label, name, value);
            }
            changed |= numeric(
                ui,
                "Maximum area ratio",
                "max_area_ratio",
                &mut c.max_area_ratio,
            );
        }
        ui.separator();
        ui.strong("Panel confirmation");
        changed |= numeric(
            ui,
            "Match threshold",
            "threshold",
            &mut self.editor.profile.panel.threshold,
        );
        changed |= numeric_u32(
            ui,
            "Stable frames",
            "stable_frames",
            &mut self.editor.profile.panel.stable_frames,
        );
        ui.separator();
        ui.strong("Timing · seconds");
        let t = &mut self.editor.profile.timing;
        for (label, name, value) in [
            (
                "Capture interval",
                "sample_interval",
                &mut t.sample_interval,
            ),
            (
                "Cursor settling",
                "click_hover_seconds",
                &mut t.click_hover_seconds,
            ),
            (
                "Mouse hold",
                "click_hold_seconds",
                &mut t.click_hold_seconds,
            ),
            (
                "Settle after target",
                "settle_seconds",
                &mut t.settle_seconds,
            ),
            ("Search timeout", "search_timeout", &mut t.search_timeout),
            ("Acquire timeout", "acquire_timeout", &mut t.acquire_timeout),
            ("Attack timeout", "attack_timeout", &mut t.attack_timeout),
            ("Camera rotation", "rotate_seconds", &mut t.rotate_seconds),
            (
                "Failed point cooldown",
                "failed_cooldown",
                &mut t.failed_cooldown,
            ),
        ] {
            changed |= numeric(ui, label, name, value);
        }
        changed |= numeric_u32(
            ui,
            "Maximum rotations / retries",
            "max_rotations",
            &mut t.max_rotations,
        );
        if changed {
            self.runtime.stop();
            self.editor.record();
        }
        ui.separator();
        ui.strong("Platform support");
        if self.snapshot.capabilities.automation_ready() {
            ui.label("Capture and guarded input available. Game compatibility still depends on the client and calibration.");
        } else {
            ui.label(if self.snapshot.capabilities.reason.is_empty() {
                "Select a game window to check capture and input."
            } else {
                &self.snapshot.capabilities.reason
            });
        }
    }
    fn keybind_button(&mut self, ui: &mut egui::Ui, binding: Binding, value: &str) {
        let active = self
            .binding
            .is_some_and(|(recording, _)| recording == binding);
        let response = ui.button(if active {
            "Press a key…"
        } else if value.is_empty() {
            "Record shortcut"
        } else {
            value
        });
        if response.clicked() || response.gained_focus() {
            self.runtime.stop();
            response.request_focus();
            self.binding = Some((binding, response.id));
        }
        response.on_hover_text("Ctrl, Shift or Alt with a letter, number or F1–F12. F7/F8 are reserved. Escape cancels.");
    }
    pub(super) fn capture_binding(&mut self, ctx: &egui::Context) {
        let Some((binding, id)) = self.binding else {
            return;
        };
        if !ctx.input(|i| i.focused) || ctx.memory(|m| m.focused()) != Some(id) {
            self.binding = None;
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            if let egui::Event::Key {
                key,
                physical_key,
                pressed: true,
                modifiers,
                ..
            } = event
            {
                if key == egui::Key::Escape || key == egui::Key::Tab {
                    self.binding = None;
                    return;
                }
                if key == egui::Key::Delete || key == egui::Key::Backspace {
                    if let Binding::Skill(index) = binding {
                        if let Some(skill) = self.editor.profile.buffs.skills.get_mut(index) {
                            skill.shortcut.clear();
                            self.editor.record();
                        }
                        self.binding = None;
                    }
                    return;
                }
                if let Some(key) = crate::ui_controls::egui_key(physical_key.unwrap_or(key)) {
                    match crate::ui_controls::capture_shortcut(
                        key,
                        modifiers.ctrl,
                        modifiers.shift,
                        modifiers.alt,
                    ) {
                        Ok(shortcut) => {
                            match binding {
                                Binding::Mount => {
                                    self.editor.profile.buffs.mount_shortcut = shortcut
                                }
                                Binding::Skill(i) => {
                                    if let Some(skill) = self.editor.profile.buffs.skills.get_mut(i)
                                    {
                                        skill.shortcut = shortcut;
                                    }
                                }
                            }
                            self.editor.record();
                            self.binding = None;
                        }
                        Err(error) => self.error = Some(error),
                    }
                    return;
                }
            }
        }
    }
    fn icon_thumbnail(&mut self, ui: &mut egui::Ui, icon: &metin_core::BuffIcon) {
        if !self.icon_textures.contains_key(&icon.png)
            && let Ok(bytes) = icon.png_bytes()
            && let Ok(frame) = metin_vision::decode_png(&bytes)
        {
            let pixels = frame
                .bgr
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|p| [p[2], p[1], p[0]])
                .collect::<Vec<_>>();
            let texture = ui.ctx().load_texture(
                "buff-icon",
                egui::ColorImage::from_rgb([frame.width as usize, frame.height as usize], &pixels),
                egui::TextureOptions::NEAREST,
            );
            if self.icon_textures.len() > 32 {
                self.icon_textures.clear();
            }
            self.icon_textures.insert(icon.png.clone(), texture);
        }
        if let Some(texture) = self.icon_textures.get(&icon.png) {
            ui.image((texture.id(), egui::vec2(36., 36.)));
        }
    }
    pub(super) fn buffs_form(&mut self, ui: &mut egui::Ui) {
        let mut changed = ui
            .checkbox(&mut self.editor.profile.buffs.enabled, "Automatic buffs")
            .changed();
        changed |= ui
            .checkbox(
                &mut self.editor.profile.buffs.fight_mounted,
                "Fight mounted",
            )
            .changed();
        ui.separator();
        ui.strong("Current mount state");
        let mut mounted = self.settings.last_mounted;
        ui.horizontal(|ui| {
            ui.selectable_value(&mut mounted, false, "Off · on foot");
            ui.selectable_value(&mut mounted, true, "On · mounted");
        });
        if self.snapshot.key == self.key() && self.snapshot.mounted.is_none() {
            ui.colored_label(
                egui::Color32::YELLOW,
                "Unknown: check the game and explicitly set On or Off.",
            );
        }
        let declare = mounted != self.settings.last_mounted
            || (self.snapshot.mounted.is_none()
                && ui.button("Confirm selected mount state").clicked());
        if declare {
            self.runtime.stop();
            self.settings.last_mounted = mounted;
            if let Err(e) = self.settings.save(&self.root) {
                self.error = Some(e.to_string());
            }
            if let Some(key) = self.key() {
                self.revision += 1;
                self.send(Command::DeclareMount {
                    key,
                    mounted,
                    revision: self.revision,
                });
            }
        }
        ui.collapsing("Mount settings", |ui| {
            let value = self.editor.profile.buffs.mount_shortcut.clone();
            self.keybind_button(ui, Binding::Mount, &value);
            changed |= numeric(
                ui,
                "Wait after mount toggle",
                "mount_wait_seconds",
                &mut self.editor.profile.buffs.mount_wait_seconds,
            );
            changed |= numeric(
                ui,
                "Key hold duration",
                "key_hold_seconds",
                &mut self.editor.profile.buffs.key_hold_seconds,
            );
        });
        ui.separator();
        if ui.button("Edit active buff area").clicked() {
            self.runtime.stop();
            self.mode = EditMode::BuffArea;
            self.live = false;
        }
        let mut remove = None;
        let mut reorder = None;
        for index in 0..self.editor.profile.buffs.skills.len() {
            ui.push_id(index, |ui| {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.strong(format!("Skill {}", index + 1));
                    if ui.add_enabled(index > 0, egui::Button::new("Up")).clicked() {
                        reorder = Some((index, index - 1));
                    }
                    if ui
                        .add_enabled(
                            index + 1 < self.editor.profile.buffs.skills.len(),
                            egui::Button::new("Down"),
                        )
                        .clicked()
                    {
                        reorder = Some((index, index + 1));
                    }
                    if ui.button("Remove").clicked() {
                        remove = Some(index);
                    }
                });
                changed |= ui
                    .text_edit_singleline(&mut self.editor.profile.buffs.skills[index].name)
                    .changed();
                let value = self.editor.profile.buffs.skills[index].shortcut.clone();
                self.keybind_button(ui, Binding::Skill(index), &value);
                let skill = &mut self.editor.profile.buffs.skills[index];
                let old_mode = skill.mode;
                egui::ComboBox::from_id_salt("mode")
                    .selected_text(if skill.mode == metin_core::BuffMode::Refresh {
                        "Refresh periodically"
                    } else {
                        "Activate when inactive"
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut skill.mode,
                            metin_core::BuffMode::Refresh,
                            "Refresh periodically",
                        );
                        ui.selectable_value(
                            &mut skill.mode,
                            metin_core::BuffMode::Inactive,
                            "Activate when inactive",
                        );
                    });
                if old_mode != skill.mode {
                    skill.interval_seconds = if skill.mode == metin_core::BuffMode::Refresh {
                        Some(120.)
                    } else {
                        None
                    };
                    changed = true;
                }
                if let Some(interval) = &mut skill.interval_seconds {
                    ui.horizontal(|ui| {
                        let factor = if self.units[index] { 60. } else { 1. };
                        let mut shown = *interval / factor;
                        if ui
                            .add(
                                egui::DragValue::new(&mut shown)
                                    .range(10. / factor..=86400. / factor)
                                    .speed(1.),
                            )
                            .changed()
                        {
                            *interval = shown * factor;
                            changed = true;
                        }
                        egui::ComboBox::from_id_salt("units")
                            .selected_text(if self.units[index] { "min" } else { "s" })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.units[index], false, "s");
                                ui.selectable_value(&mut self.units[index], true, "min");
                            });
                    });
                    if !self.editor.dirty() && self.snapshot.key == self.key() {
                        if let Some(Some(left)) = self.snapshot.timers.get(index) {
                            ui.label(format!("Next use  {}", metin_core::duration(*left)));
                        }
                        if ui.button("Reset timer").clicked()
                            && let Some(key) = self.key()
                        {
                            self.send(Command::ResetSkillTimer { key, index });
                        }
                    }
                }
                changed |= numeric(
                    ui,
                    "Wait after skill",
                    "wait_seconds",
                    &mut self.editor.profile.buffs.skills[index].wait_seconds,
                );
                for inactive in [false, true] {
                    let mode = EditMode::BuffIcon { index, inactive };
                    let icon = if inactive {
                        self.editor.profile.buffs.skills[index]
                            .inactive_icon
                            .clone()
                    } else {
                        self.editor.profile.buffs.skills[index].icon.clone()
                    };
                    ui.horizontal(|ui| {
                        if let Some(icon) = &icon {
                            self.icon_thumbnail(ui, icon);
                        }
                        if ui
                            .button(if inactive {
                                "Select inactive icon"
                            } else {
                                "Select active icon"
                            })
                            .clicked()
                        {
                            self.runtime.stop();
                            self.mode = mode;
                            self.live = false;
                        }
                        if icon.is_some() && ui.button("Clear").clicked() {
                            self.runtime.stop();
                            let _ = self.editor.delete(mode);
                        }
                    });
                    let other = EditMode::BuffIcon {
                        index,
                        inactive: !inactive,
                    };
                    if let Some(region) = self.editor.region(other)
                        && ui
                            .button(if inactive {
                                "Reuse active area"
                            } else {
                                "Reuse inactive area"
                            })
                            .clicked()
                    {
                        self.runtime.stop();
                        if let Err(e) = self.editor.set_region(mode, region) {
                            self.error = Some(e);
                        }
                    }
                }
            });
        }
        if let Some(index) = remove {
            self.editor.profile.buffs.skills.remove(index);
            self.binding = None;
            changed = true;
        }
        if let Some((a, b)) = reorder {
            self.editor.profile.buffs.skills.swap(a, b);
            self.units.swap(a, b);
            self.binding = None;
            changed = true;
        }
        if ui
            .add_enabled(
                self.editor.profile.buffs.skills.len() < 8,
                egui::Button::new("+ Add skill"),
            )
            .clicked()
        {
            let mut skill = metin_core::BuffSkill::default();
            for n in [1, 2, 3, 4, 5, 6, 9, 10, 11, 12] {
                let key = format!("F{n}");
                if !self
                    .editor
                    .profile
                    .buffs
                    .skills
                    .iter()
                    .any(|s| s.shortcut == key)
                {
                    skill.shortcut = key;
                    break;
                }
            }
            self.editor.profile.buffs.skills.push(skill);
            changed = true;
        }
        if changed {
            self.runtime.stop();
            self.editor.record();
        }
        ui.separator();
        ui.label("Save or Start applies skill changes. Mount On/Off is applied immediately. Buffs are used between targets; uncertain toggle icons are skipped.");
    }
}
