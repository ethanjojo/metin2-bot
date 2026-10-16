use super::*;
impl MetinApp {
    pub(super) fn keybind_button(&mut self, ui: &mut egui::Ui, binding: Binding, value: &str) {
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
    pub(super) fn icon_thumbnail(&mut self, ui: &mut egui::Ui, icon: &metin_core::BuffIcon) {
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
}
