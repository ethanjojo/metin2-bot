use super::*;
use crate::ui_controls::numeric;
use widgets::{Icon, button};
impl MetinApp {
    #[cfg(feature = "ui-smoke")]
    pub fn prepare_buff_detail_fixture(&mut self) -> Result<(), String> {
        let index = self
            .editor
            .profile
            .buffs
            .skills
            .iter()
            .position(|skill| {
                skill.mode == metin_core::BuffMode::Inactive
                    && skill.icon.is_some()
                    && skill.inactive_icon.is_some()
            })
            .ok_or(
                "Buff detail fixture requires an inactive skill with both captured appearances.",
            )?;
        self.navigate(Tab::Actions);
        self.actions_page = ActionsPage::Buffs;
        self.actions_detail = true;
        self.buff_detail = true;
        self.selected_skill = index;
        Ok(())
    }
    pub(super) fn buffs_view(&mut self, ui: &mut egui::Ui) {
        if self.buff_detail && !self.editor.profile.buffs.skills.is_empty() {
            egui::ScrollArea::vertical()
                .id_salt("buff-detail")
                .show(ui, |ui| {
                    if button(
                        ui,
                        Icon::Back,
                        "Back to Buffs",
                        true,
                        "Return to the buff order without discarding edits.",
                    )
                    .clicked()
                    {
                        self.buff_detail = false;
                        self.binding = None;
                        return;
                    }
                    self.selected_skill = self
                        .selected_skill
                        .min(self.editor.profile.buffs.skills.len() - 1);
                    self.skill_detail(ui, self.selected_skill);
                });
            return;
        }
        widgets::icon_heading(
            ui,
            catalog::Artwork::Skills,
            "Buffs",
            "Buffs run between targets in this order. F7/F8 remain reserved for Start and Pause.",
        );
        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut self.editor.profile.buffs.enabled, "Automatic buffs")
                .changed()
            {
                self.runtime.stop();
                self.editor.record();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if button(
                    ui,
                    Icon::Plus,
                    "Add buff",
                    self.editor.profile.buffs.skills.len() < 8,
                    "Add a buff. Up to eight buffs are supported.",
                )
                .clicked()
                {
                    self.add_skill();
                    self.buff_detail = true;
                }
            });
        });
        egui::ScrollArea::vertical()
            .id_salt("skill-list")
            .show(ui, |ui| {
                let mut reorder = None;
                let mut remove = None;
                let list_width = ui.available_width();
                for (i, skill) in self.editor.profile.buffs.skills.iter().enumerate() {
                    let name = if skill.name.is_empty() {
                        format!("Buff {}", i + 1)
                    } else {
                        skill.name.clone()
                    };
                    let (timing, fraction) = self.skill_timing(i);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.;
                        ui.allocate_ui_with_layout(
                            egui::vec2((list_width - 70.).max(70.), 44.),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                let glyph = skill_icon::glyph(ui, skill, &mut self.icon_textures);
                                let response = widgets::catalog_row(
                                    ui,
                                    glyph,
                                    &format!("{}. {name}", i + 1),
                                    &format!("{} · {timing}", skill.shortcut),
                                );
                                if skill.mode == metin_core::BuffMode::Refresh {
                                    let strip = egui::Rect::from_min_max(
                                        egui::pos2(
                                            response.rect.left() + 36.,
                                            response.rect.bottom() - 4.,
                                        ),
                                        egui::pos2(
                                            response.rect.right() - 26.,
                                            response.rect.bottom() - 1.,
                                        ),
                                    );
                                    widgets::progress_track(ui, strip, fraction);
                                }
                                if response.clicked() {
                                    self.selected_skill = i;
                                    self.buff_detail = true;
                                    self.binding = None;
                                }
                            },
                        );
                        ui.allocate_ui_with_layout(
                            egui::vec2(26., 36.),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.add_space(4.);
                                ui.set_width(26.);
                                ui.spacing_mut().item_spacing.y = 2.;
                                if widgets::compact_icon_button(
                                    ui,
                                    Icon::Up,
                                    i > 0,
                                    &format!("Move {name} up in the sequence"),
                                )
                                .clicked()
                                {
                                    reorder = Some((i, i - 1));
                                }
                                if widgets::compact_icon_button(
                                    ui,
                                    Icon::Down,
                                    i + 1 < self.editor.profile.buffs.skills.len(),
                                    &format!("Move {name} down in the sequence"),
                                )
                                .clicked()
                                {
                                    reorder = Some((i, i + 1));
                                }
                            },
                        );
                        ui.vertical(|ui| {
                            ui.add_space(4.);
                            if widgets::sized_button(
                                ui,
                                Icon::Trash,
                                "",
                                egui::vec2(26., 32.),
                                true,
                                false,
                                &format!("Delete {name}. Undo restores the buff."),
                            )
                            .clicked()
                            {
                                remove = Some(i);
                            }
                        });
                    });
                }
                if let Some((a, b)) = reorder {
                    self.reorder_skill(a, b);
                }
                if let Some(index) = remove {
                    self.remove_skill(index);
                }
                if self.editor.profile.buffs.skills.is_empty() {
                    widgets::section(ui, "No buffs yet", |ui| {
                        ui.label("Add a buff, record its shortcut and choose when to use it.");
                    });
                }
            });
    }
    pub(super) fn skill_timing(&self, index: usize) -> (String, Option<f32>) {
        let Some(skill) = self.editor.profile.buffs.skills.get(index) else {
            return ("Unknown".into(), None);
        };
        if !self.editor.profile.buffs.enabled {
            return ("Off".into(), None);
        }
        if self.key().is_none() {
            return ("Connect to see state".into(), None);
        }
        if self.editor.dirty()
            || self.snapshot.key != self.key()
            || self.snapshot.revision != self.revision
        {
            return ("Save to apply".into(), None);
        }
        if skill.mode == metin_core::BuffMode::Inactive {
            let presence = self
                .snapshot
                .captured_at
                .filter(|time| time.elapsed() < std::time::Duration::from_millis(500))
                .and_then(|_| self.snapshot.buff_presence.get(index).copied())
                .unwrap_or(metin_core::Presence::Unknown);
            return (
                match presence {
                    metin_core::Presence::Active => "Active",
                    metin_core::Presence::Absent => "Inactive",
                    metin_core::Presence::Unknown => "Unknown",
                }
                .into(),
                None,
            );
        }
        let Some(left) = self.snapshot.timers.get(index).copied().flatten() else {
            return ("On Start".into(), None);
        };
        let fraction = skill
            .interval_seconds
            .filter(|total| total.is_finite() && *total > 0.)
            .map(|total| (left / total).clamp(0., 1.) as f32);
        let text = if left <= 0. {
            "Due on next opportunity".into()
        } else {
            format!(
                "{}{}",
                metin_core::duration(left.ceil()),
                if self.snapshot.running {
                    ""
                } else {
                    " · paused"
                }
            )
        };
        (text, fraction)
    }
    pub(super) fn mount_view(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        widgets::icon_heading(
            ui,
            catalog::Artwork::Mount,
            "Mount",
            "Desired behavior. Confirm the actual state with the horse icon in the bottom bar.",
        );
        changed|=ui.checkbox(&mut self.editor.profile.buffs.fight_mounted,"Fight mounted").on_hover_text("Desired behavior after buffing; separate from declaring the current state. Mount changes run as part of Automatic buffs.").changed();
        let key = self.editor.profile.buffs.mount_shortcut.clone();
        widgets::field(ui, "Mount shortcut", |ui| {
            self.keybind_button(ui, Binding::Mount, &key)
        });
        changed |= numeric(
            ui,
            "Key hold duration (s)",
            "key_hold_seconds",
            &mut self.editor.profile.buffs.key_hold_seconds,
        );
        widgets::collapsing_summary(
            ui,
            "Advanced timing",
            "mount-advanced-timing",
            format!(
                "After mount: {} s",
                self.editor.profile.buffs.mount_wait_seconds
            ),
            |ui| {
                changed |= numeric(
                    ui,
                    "Wait after mount toggle (s)",
                    "mount_wait_seconds",
                    &mut self.editor.profile.buffs.mount_wait_seconds,
                );
                crate::ui_controls::help(
                    ui,
                    "Mount declaration applies to this connection and pauses automation. Other settings apply on Save. Uncertain toggle icons are skipped.",
                );
            },
        );
        if changed {
            self.runtime.stop();
            self.editor.record();
        }
    }
    fn skill_detail(&mut self, ui: &mut egui::Ui, index: usize) {
        let name = self.editor.profile.buffs.skills[index].name.clone();
        let glyph = skill_icon::glyph(
            ui,
            &self.editor.profile.buffs.skills[index],
            &mut self.icon_textures,
        );
        widgets::icon_heading(
            ui,
            glyph,
            if name.is_empty() { "Buff" } else { &name },
            "Record a shortcut, then choose a timer or an inactive-icon check. Captures remain unsaved edits until Save.",
        );
        let mut changed = skill_icon::picker(
            ui,
            &mut self.editor.profile.buffs.skills[index].display_icon,
        );
        changed |= widgets::field(ui, "Buff name", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.editor.profile.buffs.skills[index].name)
                    .desired_width(f32::INFINITY),
            )
            .changed()
        });
        let value = self.editor.profile.buffs.skills[index].shortcut.clone();
        widgets::field(ui, "Keyboard shortcut", |ui| {
            self.keybind_button(ui, Binding::Skill(index), &value)
        });
        let skill = &mut self.editor.profile.buffs.skills[index];
        let old_mode = skill.mode;
        widgets::field(ui, "When to use", |ui| {
            widgets::segmented_choice(
                ui,
                ("skill-mode", index),
                &mut skill.mode,
                &[
                    (
                        metin_core::BuffMode::Refresh,
                        "Timed",
                        "Refresh periodically using the configured interval.",
                    ),
                    (
                        metin_core::BuffMode::Inactive,
                        "Icon-based",
                        "Activate when inactive, using captured buff icons.",
                    ),
                ],
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
            widgets::field(ui, "Refresh every", |ui| {
                ui.allocate_ui_with_layout(egui::vec2(82.,28.),egui::Layout::left_to_right(egui::Align::Center),|ui| {
                    widgets::segmented_choice(ui,("skill-units",index),&mut self.units[index],&[
                        (false,"s","Display this interval in seconds; the saved duration is unchanged."),
                        (true,"min","Display this interval in minutes; the saved duration is unchanged."),
                    ]);
                });
                let factor = if self.units[index] { 60. } else { 1. };
                let mut value = *interval / factor;
                if ui
                    .add(
                        egui::DragValue::new(&mut value)
                            .range(10. / factor..=86400. / factor)
                            .speed(1.),
                    )
                    .changed()
                {
                    *interval = value * factor;
                    changed = true;
                }
            });
            let applied =
                !self.editor.dirty() && self.snapshot.key == self.key() && self.key().is_some();
            let (timing, fraction) = self.skill_timing(index);
            ui.label(&timing);
            let (strip, _) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 3.), egui::Sense::hover());
            widgets::progress_track(ui, strip, fraction);
            ui.horizontal(|ui| {
                if button(
                    ui,
                    Icon::Refresh,
                    "Reset timer",
                    applied,
                    "Reset this applied timer; Save edits first.",
                )
                .clicked()
                    && let Some(key) = self.key()
                {
                    self.send(Command::ResetSkillTimer { key, index });
                }
            });
        }
        if self.editor.profile.buffs.skills[index].mode == metin_core::BuffMode::Inactive {
            if numeric(
                ui,
                "Keep Active for (s)",
                "buff_active_hold_seconds",
                &mut self.editor.profile.buffs.skills[index].active_hold_seconds,
            ) {
                self.runtime.stop();
                self.editor.record();
            }
            widgets::compact_heading(
                ui,
                &format!("Icon recognition · {}", self.skill_timing(index).0),
                None,
            );
            ui.columns(2,|columns| {
                for (column,inactive) in columns.iter_mut().zip([false,true]) {
                    let icon=if inactive{self.editor.profile.buffs.skills[index].inactive_icon.clone()}else{self.editor.profile.buffs.skills[index].icon.clone()};
                    let mode=EditMode::BuffIcon{index,inactive};
                    widgets::panel(column,|ui| {
                        ui.small(if inactive{"Inactive icon (optional)"}else{"Active icon"});
                        if let Some(icon)=&icon {self.icon_thumbnail(ui,icon);}else{let (rect,_)=ui.allocate_exact_size(egui::vec2(36.,36.),egui::Sense::hover());skin::sprite(ui,"public/slot_base",rect);widgets::icon(ui.painter(),rect.center(),Icon::Camera,ui.visuals().weak_text_color());}
                        ui.horizontal(|ui| {
                            if button(ui,Icon::Camera,"Capture",true,"Select a distinctive 5–128 px icon over a fresh game image. Active and inactive appearances must differ.").clicked(){self.edit_area(mode);}
                            if widgets::small_icon_button(ui,Icon::Trash,icon.is_some(),"Clear this icon. Undo restores it.").clicked(){self.delete_area(mode);}
                        });
                    });
                }
            });
            widgets::field(ui, "Detection area", |ui| {
                if widgets::plain_button(ui,if self.editor.profile.buffs.icon_region.is_some(){"Edit area"}else{"Select area"}).on_hover_text("Icon capture derives this area automatically. Expand it to include every possible buff position.").clicked(){self.edit_area(EditMode::BuffArea);}
            });
        }
        widgets::collapsing_summary(
            ui,
            "Advanced timing",
            ("skill-advanced-timing", index),
            format!(
                "After buff: {} s",
                self.editor.profile.buffs.skills[index].wait_seconds
            ),
            |ui| {
                changed |= numeric(
                    ui,
                    "Wait after buff (s)",
                    "wait_seconds",
                    &mut self.editor.profile.buffs.skills[index].wait_seconds,
                );
            },
        );
        if changed {
            self.runtime.stop();
            self.editor.record();
        }
    }
}
