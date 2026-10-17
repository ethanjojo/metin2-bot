use super::*;
use crate::ui_controls::numeric;
use widgets::{Icon, button, section};
impl MetinApp {
    pub(super) fn periodic_view(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("periodic-scroll").show(ui,|ui| {
            widgets::heading(ui,"Periodic actions","Named key or click actions run between targets; the oldest due action goes first. Changes apply on Save.");
            if button(ui,Icon::Plus,"Add action",self.editor.profile.periodic_actions.len()<16,"Add a disabled action; record a shortcut and enable it when configured.").clicked() {
                let action=metin_core::PeriodicAction::new("New action");
                self.selected_periodic=Some(action.id);self.editor.profile.periodic_actions.push(action);
                self.runtime.stop();self.editor.record();
            }
            if self.editor.profile.periodic_actions.is_empty() {ui.label("No periodic actions yet. Use Add action to create one.");return;}
            let mut remove=None;
            for action in &self.editor.profile.periodic_actions {
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.selected_periodic==Some(action.id),format!("{}  ·  {}",action.name,if action.enabled {"On"} else {"Off"})).clicked() {self.selected_periodic=Some(action.id);self.binding=None;}
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {if button(ui,Icon::Trash,"",true,&format!("Delete {}. Undo restores it.",action.name)).clicked(){remove=Some(action.id);}});
                });
            }
            if let Some(id)=remove {
                self.runtime.stop();self.editor.profile.periodic_actions.retain(|a|a.id!=id);self.binding=None;
                self.selected_periodic=None;self.editor.record();
            }
            let Some(index)=self.selected_periodic.and_then(|id|self.editor.profile.periodic_actions.iter().position(|a|a.id==id)).or_else(||(!self.editor.profile.periodic_actions.is_empty()).then_some(0)) else{return;};
            let id=self.editor.profile.periodic_actions[index].id;self.selected_periodic=Some(id);
            section(ui,"Action details",|ui| {
                let mut changed=false;
                let a=&mut self.editor.profile.periodic_actions[index];
                let ready = a.input.validate(true, self.editor.profile.frame_size).is_ok();
                changed|=ui.add_enabled(ready, egui::Checkbox::new(&mut a.enabled,"Enable this action")).on_hover_text("Waits until there is no target, loot, break or buff sequence. Input is guarded by focus and window checks.").changed();
                ui.label("Action name");
                changed|=ui.add(egui::TextEdit::singleline(&mut a.name).hint_text("e.g. Use experience item").char_limit(60)).changed();
                ui.add_enabled_ui(a.enabled, |ui| {
                    ui.label("First use after Start");
                    ui.horizontal_wrapped(|ui| {
                    changed |= ui.radio_value(&mut a.first_run, metin_core::FirstRun::AfterInterval, "After the first interval").changed();
                    changed |= ui.radio_value(&mut a.first_run, metin_core::FirstRun::Immediate, "Immediately when available").changed();
                    });
                    changed |= numeric(ui, "Repeat interval (s)", "interval_seconds", &mut a.interval_seconds);
                    changed |= numeric(ui, "Wait after use (s)", "wait_seconds", &mut a.wait_seconds);
                });
                let mut input = a.input.clone();
                changed |= self.action_input_editor(ui, &mut input, Binding::Periodic(id), EditMode::PeriodicClick(id));
                if matches!(input, metin_core::ActionInput::Click { region: None }) { self.editor.profile.periodic_actions[index].enabled = false; }
                self.editor.profile.periodic_actions[index].input = input;
                ui.small("The wait is a delay after input; it does not verify that the item or game action succeeded.");
                if changed {self.runtime.stop();self.editor.record();}
            });
            self.action_validation(ui);
        });
    }
    pub(super) fn puller_view(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("puller-scroll").show(ui,|ui| {
            widgets::heading(ui,"Auto-puller","Use a hotbar item only during confirmed combat. Changes apply on Save.");
            section(ui,"Puller behavior",|ui| {
                let mut changed=ui.checkbox(&mut self.editor.profile.puller.enabled,"Automatic puller").on_hover_text("Sends your item shortcut. A successful key press does not prove enemies were attracted.").changed();
                let enabled=self.editor.profile.puller.enabled;
                ui.add_enabled_ui(enabled,|ui| {
                    ui.label("Puller shortcut");let key=self.editor.profile.puller.timing.shortcut.clone();self.keybind_button(ui,Binding::Puller,&key);
                    ui.label("When to use");
                    changed|=ui.radio_value(&mut self.editor.profile.puller.mode,metin_core::PullerMode::CombatStart,"At combat start").on_hover_text("One sequence after the target HP panel is confirmed. A brief flicker or Pause/Resume does not start another sequence for the same combat.").changed();
                    changed|=ui.radio_value(&mut self.editor.profile.puller.mode,metin_core::PullerMode::CombatPeriodic,"Repeat during combat").on_hover_text("Repeat sequences only while the target HP panel is confirmed. Never pulls during searching, loot, buffs or breaks; missed uses are not replayed.").changed();
                    changed|=crate::ui_controls::numeric_u32(ui,"Presses per sequence","puller_presses",&mut self.editor.profile.puller.presses);
                    ui.add_enabled_ui(self.editor.profile.puller.presses>1,|ui| {changed|=numeric(ui,"Wait between presses (s)","puller_press_gap",&mut self.editor.profile.puller.press_interval_seconds);});
                    if self.editor.profile.puller.mode==metin_core::PullerMode::CombatPeriodic {
                        changed|=numeric(ui,"Repeat interval (s)","puller_interval_seconds",&mut self.editor.profile.puller.timing.interval_seconds);
                        changed|=numeric(ui,"Wait after sequence (s)","puller_wait_seconds",&mut self.editor.profile.puller.timing.wait_seconds);
                    }
                    ui.collapsing("Advanced input",|ui|{changed|=numeric(ui,"Shortcut hold (s)","key_hold_seconds",&mut self.editor.profile.puller.timing.hold_seconds);});
                });
                if changed {self.runtime.stop();self.editor.record();if !enabled {self.binding=None;}}
            });
            self.action_validation(ui);
        });
    }
    pub(super) fn out_of_target_view(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("out-of-target-scroll").show(ui,|ui| {
            widgets::heading(ui,"Out-of-target attack","Hold an attack key for a limited time. Changes apply on Save.");
            ui.small("No enemy detection is assumed: elapsed attack time does not confirm that nearby enemies are dead.");
            let mut changed=false;
            section(ui,"Attack input",|ui| {
                ui.label("Attack shortcut");let key=self.editor.profile.out_of_target.shortcut.clone();self.keybind_button(ui,Binding::OutOfTarget,&key);
                ui.small("Space is supported. The key is released before another action, on Pause/F8, focus loss or an error.");
            });
            section(ui,"After combat",|ui| {
                let c=&mut self.editor.profile.out_of_target;
                changed|=ui.checkbox(&mut c.after_combat,"Attack after a target ends").on_hover_text("Starts only after confirmed target-panel disappearance. Holds the key, releases it, then continues with loot and settling. Never runs after an unconfirmed click.").changed();
                ui.add_enabled_ui(c.after_combat,|ui|{changed|=numeric(ui,"Attack duration (s)","attack_hold_seconds",&mut c.after_combat_seconds);});
            });
            section(ui,"During long searching",|ui| {
                let c=&mut self.editor.profile.out_of_target;
                changed|=ui.checkbox(&mut c.during_search,"Attack when searching takes too long").on_hover_text("Only while searching without a target. Does not run during loot, buffs, breaks, acquisition or recovery. Existing search/rotation limits still apply.").changed();
                ui.add_enabled_ui(c.during_search,|ui| {
                    changed|=numeric(ui,"After searching for (s)","idle_search_seconds",&mut c.search_after_seconds);
                    changed|=numeric(ui,"Search attack duration (s)","attack_hold_seconds",&mut c.search_attack_seconds);
                    changed|=crate::ui_controls::numeric_u32(ui,"Maximum search attacks","search_attack_attempts",&mut c.max_search_attacks);
                });
                ui.small("Continues searching after each attack. A new panel or candidate interrupts the hold; selection uses a fresh frame. An exhausted limit pauses the bot.");
            });
            if changed {self.runtime.stop();self.editor.record();}
            self.action_validation(ui);
        });
    }
    fn action_validation(&self, ui: &mut egui::Ui) {
        if let Err(error) = metin_core::validate_scheduled(&self.editor.profile) {
            ui.colored_label(widgets::RED, error.to_string());
        }
    }
}
