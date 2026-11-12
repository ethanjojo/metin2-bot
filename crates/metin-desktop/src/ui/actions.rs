use super::catalog::Artwork;
use super::*;
use crate::ui_controls::{numeric, numeric_u32};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ui-smoke", derive(clap::ValueEnum))]
pub enum ActionsPage {
    Buffs,
    VisualRules,
    Loot,
    Mount,
    Periodic,
    Puller,
    OutOfTarget,
}

impl MetinApp {
    pub(super) fn actions_view(&mut self, ui: &mut egui::Ui) {
        if !self.actions_detail {
            egui::ScrollArea::vertical()
                .id_salt("action-catalog")
                .show(ui, |ui| {
                    widgets::heading(ui, "Actions", "Choose a behavior. Changes apply on Save.");
                    for summary in action_summary::summaries(&self.editor.profile) {
                        let (kind, label) = match summary.page {
                            ActionsPage::Buffs => (Artwork::Skills.into(), "Buffs"),
                            ActionsPage::Loot => (Artwork::Loot.into(), "Loot"),
                            ActionsPage::Mount => (Artwork::Mount.into(), "Mount"),
                            ActionsPage::VisualRules => (Artwork::Camera.into(), "Visual rules"),
                            ActionsPage::Periodic => (
                                widgets::Glyph::Vector(widgets::Icon::Clock),
                                "Periodic actions",
                            ),
                            ActionsPage::OutOfTarget => {
                                (Artwork::AutoAttack.into(), "Out-of-target attack")
                            }
                            ActionsPage::Puller => (Artwork::Puller.into(), "Auto-puller"),
                        };
                        let status_color = match summary.state {
                            action_summary::ActionState::On => widgets::GREEN,
                            action_summary::ActionState::Off => ui.visuals().weak_text_color(),
                            action_summary::ActionState::SetupNeeded => widgets::AMBER,
                        };
                        let response = widgets::catalog_row_status(
                            ui,
                            kind,
                            label,
                            &summary.text(),
                            summary.state.label(),
                            status_color,
                        );
                        if response.clicked() {
                            self.actions_page = summary.page;
                            self.actions_detail = true;
                            self.buff_detail = false;
                            self.binding = None;
                        }
                    }
                });
            return;
        }
        if self.actions_breadcrumbs(ui) {
            return;
        }
        ui.add_space(6.);
        match self.actions_page {
            ActionsPage::Buffs => self.buffs_view(ui),
            ActionsPage::VisualRules => {
                egui::ScrollArea::vertical()
                    .id_salt("visual-rules-scroll")
                    .show(ui, |ui| {
                        self.safety_rules(ui);
                    });
            }
            ActionsPage::Loot => self.loot_view(ui),
            ActionsPage::Periodic => self.periodic_view(ui),
            ActionsPage::Puller => self.puller_view(ui),
            ActionsPage::OutOfTarget => self.out_of_target_view(ui),
            ActionsPage::Mount => {
                egui::ScrollArea::vertical()
                    .id_salt("mount-scroll")
                    .show(ui, |ui| {
                        self.mount_view(ui);
                        ui.small(
                            "Mount changes are part of Automatic buffs. Other edits apply on Save.",
                        );
                    });
            }
        }
    }
    fn loot_view(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("loot-scroll").show(ui,|ui| {
            widgets::icon_heading(ui,Artwork::Loot,"Loot","Collect nearby items after a confirmed target ends. This sends the pickup key; it does not inspect inventory or item names.");
            let mut changed=ui.checkbox(&mut self.editor.profile.loot.enabled,"Automatic loot").changed();
            ui.add_enabled_ui(self.editor.profile.loot.enabled,|ui| {
                let key=self.editor.profile.loot.shortcut.clone();
                widgets::field(ui,"Pickup shortcut",|ui|self.keybind_button(ui,Binding::Loot,&key));
                changed|=numeric_u32(ui,"Pickup presses","loot_presses",&mut self.editor.profile.loot.presses);
                egui::CollapsingHeader::new("Advanced timing").show(ui,|ui| {
                    let loot=&mut self.editor.profile.loot;
                    changed|=numeric(ui,"Delay before pickup (s)","loot_delay_seconds",&mut loot.delay_seconds);
                    changed|=numeric(ui,"Wait between presses (s)","loot_interval_seconds",&mut loot.interval_seconds);
                    changed|=numeric(ui,"Pickup key hold (s)","key_hold_seconds",&mut loot.hold_seconds);
                    crate::ui_controls::help(ui,"Order: target end, optional timed attack, loot, settling, a due break, buffs and next target. Pause/F8 or focus loss interrupts pickup and releases input.");
                });
            });
            if let Err(error)=self.editor.profile.loot.validate(&self.editor.profile){ui.colored_label(widgets::RED,error.to_string());}
            if changed{self.runtime.stop();self.editor.record();}
        });
    }
}
