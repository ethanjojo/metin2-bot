use super::*;
use crate::ui_controls::{numeric, numeric_u32};
use widgets::{Icon, button, section};
impl MetinApp {
    pub(super) fn settings_view(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("settings-scroll").show(ui,|ui| {
            widgets::heading(ui,"Settings","Tune recognition and timing. Existing limits remain enforced when you save or start.");
            let mut changed=false;
            if ui.available_width()>=1000. {
                ui.columns(2,|cols| {changed|=self.recognition_settings(&mut cols[0]);changed|=self.timing_settings(&mut cols[1]);});
            } else {changed|=self.recognition_settings(ui);changed|=self.timing_settings(ui);}
            if changed {self.runtime.stop();self.editor.record();}
            section(ui,"Platform & appearance",|ui| {
                if self.snapshot.capabilities.automation_ready(){widgets::badge(ui,"Capture and guarded input available",widgets::GREEN);ui.label("Game compatibility still depends on the client and calibration.");}
                else {widgets::badge(ui,"Automation unavailable",widgets::AMBER);ui.label(if self.snapshot.capabilities.reason.is_empty(){"Checking platform capabilities..."}else{&self.snapshot.capabilities.reason});}
                ui.small("Profile editing and saved-image calibration remain available on unsupported platforms.");
                if button(ui,Icon::Target,"Open calibration",true,"Return to the selected window and calibration checklist.").clicked(){self.navigate(Tab::Calibration);}
                ui.small("Use the sun icon in the header to switch light and dark appearance.");
            });
        });
    }
    fn recognition_settings(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        section(ui, "Target recognition", |ui| {
            if let Some(c) = &mut self.editor.profile.color {
                changed |= numeric_u32(
                    ui,
                    "Stable detections (frames)",
                    "stable_frames",
                    &mut c.stable_frames,
                );
                ui.small("A candidate must remain stable before the bot selects it.");
                ui.collapsing("Advanced color & shape limits", |ui| {
                    for (label, name, value) in [
                        ("Hue tolerance", "hue_tolerance", &mut c.hue_tolerance),
                        (
                            "Saturation tolerance",
                            "saturation_tolerance",
                            &mut c.saturation_tolerance,
                        ),
                        (
                            "Brightness tolerance",
                            "value_tolerance",
                            &mut c.value_tolerance,
                        ),
                        ("Minimum area (px²)", "min_area", &mut c.min_area),
                        ("Minimum width (px)", "min_width", &mut c.min_width),
                        ("Minimum height (px)", "min_height", &mut c.min_height),
                    ] {
                        changed |= numeric_u32(ui, label, name, value);
                    }
                    changed |= numeric(
                        ui,
                        "Maximum area ratio",
                        "max_area_ratio",
                        &mut c.max_area_ratio,
                    );
                });
            } else {
                ui.label("Pick a Metin color in Calibration before adjusting its tolerances.");
            }
        });
        section(ui, "Target panel confirmation", |ui| {
            changed |= numeric(
                ui,
                "Match threshold",
                "threshold",
                &mut self.editor.profile.panel.threshold,
            );
            changed |= numeric_u32(
                ui,
                "Stable matches (frames)",
                "stable_frames",
                &mut self.editor.profile.panel.stable_frames,
            );
            ui.small("Higher thresholds require a closer match to the captured reference crop.");
        });
        changed
    }
    fn timing_settings(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        section(ui, "Timing", |ui| {
            let t = &mut self.editor.profile.timing;
            for (label, name, value) in [
                (
                    "Capture interval (s)",
                    "sample_interval",
                    &mut t.sample_interval,
                ),
                (
                    "After target ends (s)",
                    "settle_seconds",
                    &mut t.settle_seconds,
                ),
                (
                    "Search timeout (s)",
                    "search_timeout",
                    &mut t.search_timeout,
                ),
                (
                    "Confirmation timeout (s)",
                    "acquire_timeout",
                    &mut t.acquire_timeout,
                ),
                (
                    "Attack timeout (s)",
                    "attack_timeout",
                    &mut t.attack_timeout,
                ),
            ] {
                changed |= numeric(ui, label, name, value);
            }
            ui.collapsing("Advanced input & retry timing", |ui| {
                for (label, name, value) in [
                    (
                        "Cursor settling (s)",
                        "click_hover_seconds",
                        &mut t.click_hover_seconds,
                    ),
                    (
                        "Mouse hold (s)",
                        "click_hold_seconds",
                        &mut t.click_hold_seconds,
                    ),
                    (
                        "Camera rotation (s)",
                        "rotate_seconds",
                        &mut t.rotate_seconds,
                    ),
                    (
                        "Failed point cooldown (s)",
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
            });
        });
        changed
    }
}
