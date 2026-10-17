use super::*;
use crate::ui_controls::numeric;
use widgets::{Icon, button, section};
impl MetinApp {
    pub(super) fn buffs_view(&mut self, ui: &mut egui::Ui) {
        widgets::heading(
            ui,
            "Buffs",
            "Skills are used between targets, in the order shown. Changes apply when saved.",
        );
        if ui
            .checkbox(&mut self.editor.profile.buffs.enabled, "Automatic buffs")
            .on_hover_text("Enable the skill sequence between targets. Changes apply on Save.")
            .changed()
        {
            self.runtime.stop();
            self.editor.record();
        }
        ui.separator();
        let height = ui.available_height();
        let list_width = if ui.available_width() < 950. {
            210.
        } else {
            260.
        };
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(list_width,height),egui::Layout::top_down(egui::Align::Min),|ui| {
                ui.set_max_width(list_width);
                egui::ScrollArea::vertical().id_salt("skill-list").show(ui,|ui| {
                    ui.strong(format!("Skill order  {}/8",self.editor.profile.buffs.skills.len()));
                    ui.small("Select a skill to edit its details.");
                    let mut reorder=None;
                    let mut remove=None;
                    for (i,skill) in self.editor.profile.buffs.skills.iter().enumerate() {
                        let name=if skill.name.is_empty(){format!("Skill {}",i+1)}else{skill.name.clone()};
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x=4.;
                            if ui.add_sized([list_width-70.,32.],egui::Button::new(format!("{}. {}",i+1,name)).truncate().selected(self.selected_skill==i)).on_hover_text(format!("{} - {}",name,skill.shortcut)).clicked() { self.selected_skill=i; self.binding=None; }
                            ui.allocate_ui_with_layout(egui::vec2(26.,32.),egui::Layout::top_down(egui::Align::Min),|ui| {
                                ui.set_width(26.);
                                ui.spacing_mut().item_spacing.y=2.;
                                if widgets::compact_icon_button(ui,Icon::Up,i>0,&format!("Move {name} up in the sequence")).clicked() { reorder=Some((i,i-1)); }
                                if widgets::compact_icon_button(ui,Icon::Down,i+1<self.editor.profile.buffs.skills.len(),&format!("Move {name} down in the sequence")).clicked() { reorder=Some((i,i+1)); }
                            });
                            if button(ui,Icon::Trash,"",true,&format!("Delete {name}. Undo restores the skill.")).clicked() { remove=Some(i); }
                        });
                        ui.small(format!("{} - {}",skill.shortcut,if skill.mode==metin_core::BuffMode::Refresh{"Periodic"}else{"When inactive"}));
                        ui.add_space(5.);
                    }
                    if let Some((a,b))=reorder {self.reorder_skill(a,b);}
                    if let Some(index)=remove {self.remove_skill(index);}
                    if button(ui,Icon::Plus,"Add skill",self.editor.profile.buffs.skills.len()<8,"Add a skill with the next available shortcut. Up to eight skills are supported.").clicked(){self.add_skill();}
                    ui.separator();
                    ui.small("F7 and F8 are reserved for Start and Pause.");
                });
            });
            ui.separator();
            ui.allocate_ui_with_layout(ui.available_size(),egui::Layout::top_down(egui::Align::Min),|ui| {
                egui::ScrollArea::vertical().id_salt("buff-detail").show(ui,|ui| {
                    if self.editor.profile.buffs.skills.is_empty() {
                        section(ui,"No skills yet",|ui| {ui.label("Add a skill, record its shortcut, then choose when it should be used.");if button(ui,Icon::Plus,"Add first skill",true,"Create the first skill in this profile.").clicked(){self.add_skill();}});
                    } else {
                        self.selected_skill=self.selected_skill.min(self.editor.profile.buffs.skills.len()-1);
                        self.skill_detail(ui,self.selected_skill);
                    }
                });
            });
        });
    }
    pub(super) fn mount_view(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        self.mount_state_control(ui);
        changed |= ui.checkbox(&mut self.editor.profile.buffs.fight_mounted, "Fight mounted")
            .on_hover_text("Desired behavior: remount after buffing before continuing combat. This is separate from confirming the current mount state.").changed();
        ui.separator();
        {
            ui.small("Declared state applies only to this connection and pauses automation. Other buff edits apply on Save. Uncertain toggle icons are skipped.");
            ui.label("Mount shortcut");
            let value = self.editor.profile.buffs.mount_shortcut.clone();
            self.keybind_button(ui, Binding::Mount, &value);
            changed |= numeric(
                ui,
                "Wait after mount toggle (s)",
                "mount_wait_seconds",
                &mut self.editor.profile.buffs.mount_wait_seconds,
            );
            changed |= numeric(
                ui,
                "Key hold duration (s)",
                "key_hold_seconds",
                &mut self.editor.profile.buffs.key_hold_seconds,
            );
        }
        if changed {
            self.runtime.stop();
            self.editor.record();
        }
    }
    fn skill_detail(&mut self, ui: &mut egui::Ui, index: usize) {
        section(ui, &format!("Skill {} details", index + 1), |ui| {
            let mut changed = false;
            ui.label("Skill name");
            changed|=ui.add(egui::TextEdit::singleline(&mut self.editor.profile.buffs.skills[index].name).hint_text("e.g. Aura of the Sword").desired_width(f32::INFINITY)).on_hover_text("Optional display name. The shortcut determines which skill is sent to the game.").changed();
            ui.label("Keyboard shortcut");
            let value = self.editor.profile.buffs.skills[index].shortcut.clone();
            self.keybind_button(ui, Binding::Skill(index), &value);
            ui.small("Click to record. Escape cancels; Backspace clears. F7/F8 are reserved.");
            let skill = &mut self.editor.profile.buffs.skills[index];
            let old_mode = skill.mode;
            ui.label("When to use");
            egui::ComboBox::from_id_salt(("skill-mode",index)).selected_text(if skill.mode==metin_core::BuffMode::Refresh{"Refresh periodically"}else{"Activate when inactive"}).show_ui(ui,|ui| {
                ui.selectable_value(&mut skill.mode,metin_core::BuffMode::Refresh,"Refresh periodically");
                ui.selectable_value(&mut skill.mode,metin_core::BuffMode::Inactive,"Activate when inactive");
            }).response.on_hover_text("Periodic skills use a timer. Toggle skills require recognition of their active icon.");
            if old_mode != skill.mode {
                skill.interval_seconds = if skill.mode == metin_core::BuffMode::Refresh {
                    Some(120.)
                } else {
                    None
                };
                changed = true;
            }
            if let Some(interval) = &mut skill.interval_seconds {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Refresh every");let factor=if self.units[index]{60.}else{1.};let mut value=*interval/factor;
                    if ui.add(egui::DragValue::new(&mut value).range(10./factor..=86400./factor).speed(1.)).on_hover_text("Timer interval: 10 seconds to 24 hours. Changing units preserves the duration.").changed(){*interval=value*factor;changed=true;}
                    egui::ComboBox::from_id_salt(("skill-units",index)).width(90.).selected_text(if self.units[index]{"minutes"}else{"seconds"}).show_ui(ui,|ui| {ui.selectable_value(&mut self.units[index],false,"seconds");ui.selectable_value(&mut self.units[index],true,"minutes");});
                });
                let applied =
                    !self.editor.dirty() && self.snapshot.key == self.key() && self.key().is_some();
                if applied && let Some(Some(left)) = self.snapshot.timers.get(index) {
                    ui.label(format!("Next use in {}", metin_core::duration(*left)));
                }
                if button(ui,Icon::Refresh,"Reset timer",applied,"Reset this applied skill timer without changing its saved interval. Save edits first.").clicked() && let Some(key)=self.key(){self.send(Command::ResetSkillTimer{key,index});}
            }
            changed |= numeric(
                ui,
                "Wait after skill (s)",
                "wait_seconds",
                &mut self.editor.profile.buffs.skills[index].wait_seconds,
            );
            if changed {
                self.runtime.stop();
                self.editor.record();
            }
        });
        if index >= self.editor.profile.buffs.skills.len() || self.selected_skill != index {
            return;
        }
        if self.editor.profile.buffs.skills[index].mode != metin_core::BuffMode::Inactive {
            return;
        }
        section(ui, "Icon recognition", |ui| {
            ui.label("Capture icons from a frozen snapshot. Active and inactive crops may have different sizes.");
            if button(
                ui,
                Icon::Target,
                if self.editor.profile.buffs.icon_region.is_some() {
                    "Edit buff detection area"
                } else {
                    "Set buff detection area"
                },
                true,
                "Select the full area where active buff icons appear.",
            )
            .clicked()
            {
                self.edit_area(EditMode::BuffArea);
            }
            for inactive in [false, true] {
                ui.separator();
                ui.strong(if inactive {
                    "Inactive icon (optional)"
                } else {
                    "Active icon"
                });
                let mode = EditMode::BuffIcon { index, inactive };
                let icon = if inactive {
                    self.editor.profile.buffs.skills[index]
                        .inactive_icon
                        .clone()
                } else {
                    self.editor.profile.buffs.skills[index].icon.clone()
                };
                ui.horizontal_wrapped(|ui| {
                    if let Some(icon) = &icon {
                        self.icon_thumbnail(ui, icon);
                    } else {
                        ui.small("No icon captured");
                    }
                    if button(
                        ui,
                        Icon::Camera,
                        "Capture",
                        true,
                        "Open the shared image editor and drag around this icon.",
                    )
                    .clicked()
                    {
                        self.edit_area(mode);
                    }
                    if widgets::small_icon_button(
                        ui,
                        Icon::Trash,
                        icon.is_some(),
                        "Clear this icon crop. Undo restores it.",
                    )
                    .clicked()
                    {
                        self.delete_area(mode);
                    }
                });
                let other = EditMode::BuffIcon {
                    index,
                    inactive: !inactive,
                };
                if let Some(region)=self.editor.region(other) && ui.button(if inactive{"Capture inactive at active area"}else{"Capture active at inactive area"}).on_hover_text("Reuse the other crop's coordinates, capturing current frozen pixels. This does not copy its image.").clicked(){self.runtime.stop();if let Err(e)=self.editor.set_region(mode,region){self.error=Some(e);}}
            }
        });
    }
}
