use super::*;
use crate::ui_controls::{numeric, numeric_u32};
use widgets::section;

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
        if widgets::subtabs(
            ui,
            &mut self.actions_page,
            &[
                (
                    ActionsPage::Buffs,
                    "Buffs",
                    "Skill order, timers and icon recognition.",
                ),
                (
                    ActionsPage::Loot,
                    "Loot",
                    "Pickup after a confirmed target end.",
                ),
                (
                    ActionsPage::Mount,
                    "Mount",
                    "Desired behavior and current declaration.",
                ),
                (
                    ActionsPage::VisualRules,
                    "Visual rules",
                    "Recognize a fixed image and notify, pause or perform an action.",
                ),
                (
                    ActionsPage::Periodic,
                    "Periodic actions",
                    "Named key or click actions performed between targets.",
                ),
                (
                    ActionsPage::OutOfTarget,
                    "Out-of-target attack",
                    "Timed attacks after combat or during a long search.",
                ),
                (
                    ActionsPage::Puller,
                    "Auto-puller",
                    "A sequence at confirmed combat start or repeated during combat.",
                ),
            ],
        ) {
            self.binding = None;
        }
        match self.actions_page {
            ActionsPage::Buffs => self.buffs_view(ui),
            ActionsPage::VisualRules => {
                egui::ScrollArea::vertical()
                    .id_salt("visual-rules-scroll")
                    .show(ui, |ui| {
                        widgets::heading(
                            ui,
                            "Visual rules",
                            "Recognize an event and choose a reaction. Changes apply on Save.",
                        );
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
                        widgets::heading(
                            ui,
                            "Mount",
                            "Confirm the current state before using automatic buffs.",
                        );
                        section(ui, "Mount behavior", |ui| self.mount_view(ui));
                        ui.small(
                            "Mount changes are part of Automatic buffs. Other edits apply on Save.",
                        );
                    });
            }
        }
    }
    fn loot_view(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("loot-scroll").show(ui, |ui| {
            widgets::heading(ui, "Loot", "Collect nearby items between targets. Changes apply on Save.");
            let mut changed = false;
            section(ui, "After a target ends", |ui| {
                changed |= ui.checkbox(&mut self.editor.profile.loot.enabled, "Automatic loot")
                    .on_hover_text("Starts only after a previously confirmed target panel disappears for the required number of frames. Never loots on startup or after an unconfirmed click.").changed();
                let enabled = self.editor.profile.loot.enabled;
                ui.add_enabled_ui(enabled, |ui| {
                    ui.label("Pickup shortcut");
                    let value = self.editor.profile.loot.shortcut.clone();
                    self.keybind_button(ui, Binding::Loot, &value);
                    let loot = &mut self.editor.profile.loot;
                    changed |= numeric_u32(ui, "Pickup presses", "loot_presses", &mut loot.presses);
                    changed |= numeric(ui, "Delay before pickup (s)", "loot_delay_seconds", &mut loot.delay_seconds);
                    changed |= numeric(ui, "Wait between presses (s)", "loot_interval_seconds", &mut loot.interval_seconds);
                    changed |= numeric(ui, "Pickup key hold (s)", "key_hold_seconds", &mut loot.hold_seconds);
                });
                ui.small("Uses the selected game's pickup key. Item names and inventory contents are not detected.");
            });
            if let Err(error) = self.editor.profile.loot.validate(&self.editor.profile) {
                ui.colored_label(widgets::RED, error.to_string());
            }
            section(ui, "Execution order", |ui| {
                ui.label("Target end, optional out-of-target attack, loot, settling, a due break, buffs and the next target.");
                ui.small("Pause, F8, focus loss or a new target panel interrupts pickup and releases held keys. Pickup is never resumed automatically after a pause.");
            });
            if changed { self.runtime.stop(); self.editor.record(); }
        });
    }
}
