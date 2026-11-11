use super::*;
use crate::ui_controls::numeric;
use widgets::{Icon, button};
impl MetinApp {
    pub(super) fn periodic_view(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("periodic-scroll").show(ui,|ui| {
            if self.selected_periodic.is_none() {
                widgets::icon_heading(ui,widgets::Icon::Clock,"Periodic actions","Named keys or clicks run between targets. Timers count active time and pause with the bot.");
                if button(ui,Icon::Plus,"Add action",self.editor.profile.periodic_actions.len()<16,"Create a disabled periodic action.").clicked(){let action=metin_core::PeriodicAction::new("New action");self.selected_periodic=Some(action.id);self.editor.profile.periodic_actions.push(action);self.runtime.stop();self.editor.record();}
                let mut remove=None;
                for action in &self.editor.profile.periodic_actions {
                    ui.horizontal_top(|ui| {
                        let width=(ui.available_width()-38.).max(80.);
                        ui.allocate_ui_with_layout(egui::vec2(width,44.),egui::Layout::top_down(egui::Align::Min),|ui| {
                            if widgets::catalog_row(ui,Icon::Clock,&action.name,&format!("{} · every {}",if action.enabled{"On"}else{"Off"},metin_core::duration(action.interval_seconds))).clicked(){self.selected_periodic=Some(action.id);self.binding=None;}
                        });
                        if button(ui,Icon::Trash,"",true,&format!("Delete {}. Undo restores it.",action.name)).clicked(){remove=Some(action.id);}
                    });
                }
                if let Some(id)=remove {self.runtime.stop();self.editor.profile.periodic_actions.retain(|action|action.id!=id);self.editor.record();}
                if self.editor.profile.periodic_actions.is_empty(){ui.small("No periodic actions yet. Add an action to begin.");}
                return;
            }
            let Some(index)=self.selected_periodic.and_then(|id|self.editor.profile.periodic_actions.iter().position(|action|action.id==id))else{self.selected_periodic=None;return;};
            if button(ui,Icon::Back,"Back to Periodic",true,"Return to the action list; edits are retained.").clicked(){self.selected_periodic=None;self.binding=None;return;}
            let id=self.editor.profile.periodic_actions[index].id;
            let title=self.editor.profile.periodic_actions[index].name.clone();
            widgets::icon_heading(ui,widgets::Icon::Clock,&title,"Configure the input before enabling. Periodic actions wait for a safe state between targets.");
            let mut changed=widgets::field(ui,"Action name",|ui|ui.add(egui::TextEdit::singleline(&mut self.editor.profile.periodic_actions[index].name).char_limit(60).desired_width(f32::INFINITY)).changed());
            let mut input=self.editor.profile.periodic_actions[index].input.clone();
            changed|=self.action_input_editor(ui,&mut input,Binding::Periodic(id),EditMode::PeriodicClick(id));
            let ready=input.validate(true,self.editor.profile.frame_size).is_ok();
            let action=&mut self.editor.profile.periodic_actions[index];
            action.input=input;
            if !ready {action.enabled=false;}
            changed|=ui.add_enabled(ready,egui::Checkbox::new(&mut action.enabled,"Enable this action")).changed();
            if !ready {ui.small("Record a shortcut or select a click area to enable.");}
            ui.add_enabled_ui(action.enabled,|ui| {
                changed|=numeric(ui,"Repeat interval (s)","interval_seconds",&mut action.interval_seconds);
                widgets::field(ui,"First use after Start",|ui| {
                    changed|=widgets::segmented_choice(ui,("periodic-first",id),&mut action.first_run,&[
                        (metin_core::FirstRun::Immediate,"Immediately","Use immediately when a safe state is available after Start."),
                        (metin_core::FirstRun::AfterInterval,"After interval","Wait for the first configured interval after Start."),
                    ]);
                });
                changed|=numeric(ui,"Wait after use (s)","wait_seconds",&mut action.wait_seconds);
                crate::ui_controls::help(ui,"The wait is a delay after input; it does not verify that an item or game action succeeded.");
            });
            if changed {self.runtime.stop();self.editor.record();}
            self.action_validation(ui);
        });
    }
    pub(super) fn puller_view(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("puller-scroll").show(ui,|ui| {
            widgets::icon_heading(ui,catalog::Artwork::Puller,"Auto-puller","Use a hotbar item during confirmed combat. A key press does not prove enemies were attracted. Missed uses are never replayed.");
            let mut changed=ui.checkbox(&mut self.editor.profile.puller.enabled,"Automatic puller").changed();
            let enabled=self.editor.profile.puller.enabled;
            ui.add_enabled_ui(enabled,|ui| {
                let key=self.editor.profile.puller.timing.shortcut.clone();
                widgets::field(ui,"Puller shortcut",|ui|self.keybind_button(ui,Binding::Puller,&key));
                let config=&mut self.editor.profile.puller;
                widgets::field(ui,"When to use",|ui| {
                    changed|=widgets::segmented_choice(ui,"puller-mode",&mut config.mode,&[
                        (metin_core::PullerMode::CombatStart,"Combat start","Use once at the start of confirmed combat."),
                        (metin_core::PullerMode::CombatPeriodic,"Repeat","Repeat during confirmed combat using the configured interval."),
                    ]);
                });
                changed|=crate::ui_controls::numeric_u32(ui,"Presses per sequence","puller_presses",&mut config.presses);
                if config.mode==metin_core::PullerMode::CombatPeriodic {changed|=numeric(ui,"Repeat interval (s)","puller_interval_seconds",&mut config.timing.interval_seconds);}
                changed|=numeric(ui,"Shortcut hold (s)","key_hold_seconds",&mut config.timing.hold_seconds);
                let repeat=config.mode==metin_core::PullerMode::CombatPeriodic;
                if config.presses>1 || repeat {
                    let summary=match (config.presses>1,repeat) {
                        (true,true)=>format!("Gap: {} s · after: {} s",config.press_interval_seconds,config.timing.wait_seconds),
                        (true,false)=>format!("Press gap: {} s",config.press_interval_seconds),
                        _=>format!("After sequence: {} s",config.timing.wait_seconds),
                    };
                    widgets::collapsing_summary(ui,"Advanced timing","puller-advanced-timing",summary,|ui| {
                        if config.presses>1 {changed|=numeric(ui,"Wait between presses (s)","puller_press_gap",&mut config.press_interval_seconds);}
                        if repeat {changed|=numeric(ui,"Wait after sequence (s)","puller_wait_seconds",&mut config.timing.wait_seconds);}
                    });
                }
            });
            if changed{self.runtime.stop();self.editor.record();if !enabled{self.binding=None;}}
            self.action_validation(ui);
        });
    }
    pub(super) fn out_of_target_view(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("out-of-target-scroll").show(ui,|ui| {
            widgets::icon_heading(ui,catalog::Artwork::AutoAttack,"Out-of-target attack","Timed key holds do not detect monsters or prove a kill. New panels/candidates, Pause/F8 and focus/geometry failures release the key.");
            let mut changed=false;
            let key=self.editor.profile.out_of_target.shortcut.clone();
            widgets::field(ui,"Attack shortcut",|ui|self.keybind_button(ui,Binding::OutOfTarget,&key));
            let config=&mut self.editor.profile.out_of_target;
            changed|=ui.checkbox(&mut config.after_combat,"Attack after a target ends").on_hover_text("Only after confirmed panel disappearance, before loot and settling. Never after an unconfirmed click.").changed();
            ui.add_enabled_ui(config.after_combat,|ui|{changed|=numeric(ui,"Attack duration (s)","attack_hold_seconds",&mut config.after_combat_seconds);});
            changed|=ui.checkbox(&mut config.during_search,"Attack when searching takes too long").changed();
            ui.add_enabled_ui(config.during_search,|ui| {
                changed|=numeric(ui,"After searching for (s)","idle_search_seconds",&mut config.search_after_seconds);
                changed|=numeric(ui,"Search attack duration (s)","attack_hold_seconds",&mut config.search_attack_seconds);
                widgets::collapsing_summary(ui,"Advanced search limits","search-advanced-limits",format!("Up to {} search attacks",config.max_search_attacks),|ui| {
                    changed|=crate::ui_controls::numeric_u32(ui,"Maximum search attacks","search_attack_attempts",&mut config.max_search_attacks);
                    crate::ui_controls::help(ui,"Existing search, rotation and recovery limits still apply. An exhausted limit pauses automation; the next selection requires a fresh frame.");
                });
            });
            if changed{self.runtime.stop();self.editor.record();}
            self.action_validation(ui);
        });
    }
    fn action_validation(&self, ui: &mut egui::Ui) {
        if let Err(error) = metin_core::validate_scheduled(&self.editor.profile) {
            ui.colored_label(widgets::RED, error.to_string());
        }
    }
}
