use super::*;

/// Upload an already validated BGR reference; callers retain/cache the handle.
pub(super) fn frame_texture(
    context: &egui::Context,
    name: impl Into<String>,
    frame: &metin_platform::Frame,
    options: egui::TextureOptions,
) -> egui::TextureHandle {
    let rgb = frame
        .bgr
        .as_chunks::<3>()
        .0
        .iter()
        .flat_map(|pixel| [pixel[2], pixel[1], pixel[0]])
        .collect::<Vec<_>>();
    context.load_texture(
        name,
        egui::ColorImage::from_rgb([frame.width as usize, frame.height as usize], &rgb),
        options,
    )
}

impl MetinApp {
    pub(super) fn action_input_editor(
        &mut self,
        ui: &mut egui::Ui,
        input: &mut metin_core::ActionInput,
        binding: Binding,
        mode: EditMode,
    ) -> bool {
        let mut changed = false;
        let mut click = matches!(input, metin_core::ActionInput::Click { .. });
        widgets::field(ui, "Input", |ui| {
            changed |= ui.radio_value(&mut click, true, "Click").changed();
            changed |= ui.radio_value(&mut click, false, "Key").changed();
        });
        if changed {
            self.binding = None;
            *input = if click {
                metin_core::ActionInput::Click { region: None }
            } else {
                metin_core::ActionInput::default()
            };
        }
        match input {
            metin_core::ActionInput::Key {
                shortcut,
                hold_seconds,
            } => {
                widgets::field(ui, "Keyboard shortcut", |ui| {
                    self.keybind_button(ui, binding, shortcut)
                });
                changed |= crate::ui_controls::numeric(
                    ui,
                    "Key hold (s)",
                    "key_hold_seconds",
                    hold_seconds,
                );
            }
            metin_core::ActionInput::Click { region } => {
                ui.horizontal(|ui| {
                    if widgets::button(ui,widgets::Icon::Target,"Select click target",true,"Select a small area inside the intended button over a fresh game image. Input uses its source-pixel center.").clicked(){self.edit_area(mode);}
                    if widgets::small_icon_button(ui,widgets::Icon::Trash,region.is_some(),"Clear this click target. Undo restores it.").clicked(){*region=None;changed=true;}
                });
                self.click_target_preview(ui, *region);
            }
        }
        changed
    }
    pub(super) fn click_target_preview(
        &mut self,
        ui: &mut egui::Ui,
        region: Option<metin_core::Region>,
    ) {
        let Some(region) = region else {
            ui.small("Click target missing. Select a target to continue.");
            return;
        };
        if let Some([width, height]) = self.editor.profile.frame_size {
            ui.small(format!(
                "Click center: {:.0}, {:.0} px · target {:.0} × {:.0} px",
                (region.left + region.right) * 0.5 * f64::from(width),
                (region.top + region.bottom) * 0.5 * f64::from(height),
                (region.right - region.left) * f64::from(width),
                (region.bottom - region.top) * f64::from(height)
            ));
        }
        if let Some(frame) = self.editor.frozen.as_ref() {
            let cached = self
                .click_preview
                .as_ref()
                .is_some_and(|(source, area, _)| {
                    Arc::ptr_eq(&source.bgr, &frame.bgr) && *area == region
                });
            if !cached {
                self.click_preview = None;
            }
            if !cached && let Ok(crop) = metin_vision::crop(frame, region) {
                let rgb = crop
                    .bgr
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .flat_map(|pixel| [pixel[2], pixel[1], pixel[0]])
                    .collect::<Vec<_>>();
                let texture = ui.ctx().load_texture(
                    "click-target-preview",
                    egui::ColorImage::from_rgb([crop.width as usize, crop.height as usize], &rgb),
                    egui::TextureOptions::NEAREST,
                );
                self.click_preview = Some((frame.clone(), region, texture));
            }
            if let Some((_, _, texture)) = &self.click_preview {
                let size = texture.size_vec2();
                ui.horizontal(|ui| {
                    ui.image((
                        texture.id(),
                        size * (96. / size.x).min(64. / size.y).min(1.),
                    ));
                    ui.small("Current snapshot preview");
                });
            }
        }
    }
    pub(super) fn keybind_button(&mut self, ui: &mut egui::Ui, binding: Binding, value: &str) {
        let active = self
            .binding
            .is_some_and(|(recording, _)| recording == binding);
        let response = widgets::plain_button(
            ui,
            if active {
                "Press a key…"
            } else if value.is_empty() {
                "Record shortcut"
            } else {
                value
            },
        );
        if response.clicked() || response.gained_focus() {
            self.runtime.stop();
            response.request_focus();
            self.binding = Some((binding, response.id));
        }
        response.on_hover_text("Ctrl, Shift or Alt with Space, a letter, number or F1–F12. F7/F8 are reserved. Escape cancels.");
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
                if let Some(captured_key) =
                    crate::ui_controls::egui_key(physical_key.unwrap_or(key))
                {
                    match crate::ui_controls::capture_shortcut(
                        captured_key,
                        modifiers.ctrl,
                        modifiers.shift,
                        modifiers.alt,
                    ) {
                        Ok(shortcut) => {
                            match binding {
                                Binding::OutOfTarget => {
                                    self.editor.profile.out_of_target.shortcut = shortcut
                                }
                                Binding::Puller => {
                                    self.editor.profile.puller.timing.shortcut = shortcut
                                }
                                Binding::Periodic(id) => {
                                    if let Some(action) = self
                                        .editor
                                        .profile
                                        .periodic_actions
                                        .iter_mut()
                                        .find(|a| a.id == id)
                                        && let metin_core::ActionInput::Key {
                                            shortcut: key, ..
                                        } = &mut action.input
                                    {
                                        *key = shortcut;
                                    }
                                }
                                Binding::Visual(id) => {
                                    if let Some(rule) = self
                                        .editor
                                        .profile
                                        .safety
                                        .rules
                                        .iter_mut()
                                        .find(|r| r.id == id)
                                        && let metin_core::ActionInput::Key {
                                            shortcut: key, ..
                                        } = &mut rule.execution.input
                                    {
                                        *key = shortcut;
                                    }
                                }
                                Binding::Loot => self.editor.profile.loot.shortcut = shortcut,
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
                            // Space also activates a focused button; consume the recorded key
                            // before rendering so it cannot re-arm this recorder.
                            ctx.input_mut(|input| {
                                input.consume_key(modifiers, key);
                            });
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
    pub(super) fn icon_thumbnail(&mut self, ui: &mut egui::Ui, icon: &metin_core::BuffIcon) {
        if let Some(texture) = icon_texture(ui, icon, &mut self.icon_textures) {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(36., 36.), egui::Sense::hover());
            super::skin::sprite(ui, "public/slot_base", rect);
            ui.painter().image(
                texture.id(),
                rect.shrink(2.),
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                egui::Color32::WHITE,
            );
        }
    }
}

pub(super) fn icon_texture(
    ui: &egui::Ui,
    icon: &metin_core::BuffIcon,
    textures: &mut std::collections::HashMap<String, egui::TextureHandle>,
) -> Option<egui::TextureHandle> {
    if !textures.contains_key(&icon.png)
        && let Ok(bytes) = icon.png_bytes()
        && let Ok(frame) = metin_vision::decode_png(&bytes)
    {
        let texture = frame_texture(ui.ctx(), "buff-icon", &frame, egui::TextureOptions::NEAREST);
        if textures.len() > 32 {
            textures.clear();
        }
        textures.insert(icon.png.clone(), texture);
    }
    textures.get(&icon.png).cloned()
}
