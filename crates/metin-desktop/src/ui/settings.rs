use super::*;
use crate::ui_controls::{numeric, numeric_u32};
use widgets::{Icon, button, section};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ui-smoke", derive(clap::ValueEnum))]
pub enum SettingsPage {
    Detection,
    Combat,
    Appearance,
    Diagnostics,
    Updates,
}
impl MetinApp {
    pub(super) fn settings_view(&mut self, ui: &mut egui::Ui) {
        if widgets::subtabs(
            ui,
            &mut self.settings_page,
            &[
                (
                    SettingsPage::Detection,
                    "Detection",
                    "Recognition settings for the selected profile.",
                ),
                (
                    SettingsPage::Combat,
                    "Timing",
                    "Timing presets, delays and input guards.",
                ),
                (
                    SettingsPage::Appearance,
                    "Display",
                    "Application-wide scale and preview settings.",
                ),
                (
                    SettingsPage::Diagnostics,
                    "Diagnostics",
                    "Build information, logs and diagnostic export.",
                ),
                (
                    SettingsPage::Updates,
                    "Updates",
                    "Checked downloads and confirmed installation.",
                ),
            ],
        ) {
            self.binding = None;
        }
        egui::ScrollArea::vertical().id_salt(("settings-scroll",self.settings_page as u8)).show(ui,|ui| {
            let mut changed=false;
            match self.settings_page {
                SettingsPage::Detection=>{widgets::heading(ui,"Detection","Recognition settings for the selected profile.");changed|=self.recognition_settings(ui);}
                SettingsPage::Combat=>{widgets::heading(ui,"Combat & timing","Concrete timing values; detection and input guards remain independent.");changed|=self.timing_preset(ui);changed|=self.timing_settings(ui);}
                SettingsPage::Appearance=>self.appearance_settings(ui),
                SettingsPage::Diagnostics=>self.diagnostics_view(ui),
                SettingsPage::Updates=>self.updates_view(ui),
            }
            if changed {self.runtime.stop();self.editor.record();}
        });
    }
    fn timing_preset(&mut self, ui: &mut egui::Ui) -> bool {
        let current = metin_core::TimingPreset::infer(&self.editor.profile.timing);
        let mut chosen = current;
        widgets::field(ui, "Tempo preset", |ui| {
            widgets::ComboBox::from_id_salt("tempo-preset")
                .width(ui.available_width())
                .selected_text(current.map_or("Custom", |p| p.label()))
                .show_ui(ui, |ui| {
                    for p in [
                        metin_core::TimingPreset::Careful,
                        metin_core::TimingPreset::Normal,
                        metin_core::TimingPreset::Responsive,
                    ] {
                        select::option(ui, &mut chosen, Some(p), p.label());
                    }
                    ui.add_enabled(false, egui::Label::new("Custom — edit values below"));
                });
        });
        crate::ui_controls::help(
            ui,
            "A preset replaces all timing values below. Manual edits become Custom; detection thresholds and safeguards are preserved.",
        );
        if chosen != current
            && let Some(p) = chosen
        {
            self.editor.profile.timing = p.timing();
            true
        } else {
            false
        }
    }
    fn appearance_settings(&mut self, ui: &mut egui::Ui) {
        widgets::heading(
            ui,
            "Appearance & performance",
            "Application-wide presentation preferences. Saved immediately.",
        );
        let mut changed = false;
        widgets::first_section(ui, "Interface", |ui| {
            ui.label("UI scale");
            changed|=ui.add(egui::Slider::new(&mut self.settings.ui_scale,0.75..=1.5).step_by(0.05).show_value(true)).on_hover_text("Scales text and controls. The window can be resized; scroll areas keep settings accessible.").changed();
        });
        section(ui, "Overlay rendering", |ui| {
            changed|=ui.checkbox(&mut self.settings.lite_mode,"Lite Mode").on_hover_text("Draws overlay changes at most once per second. Freshness checks remain active; detection sampling, scheduling and input safety are unchanged.").changed();
            ui.horizontal_wrapped(|ui| {
                ui.label("Refresh interval (ms)");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut self.settings.preview_refresh_ms)
                            .range(100..=5000)
                            .speed(50),
                    )
                    .changed();
            });
        });
        if changed {
            ui.ctx().set_zoom_factor(self.settings.ui_scale);
            if let Err(error) = self.settings.save(&self.root) {
                self.error = Some(error.to_string());
            }
        }
    }
    fn diagnostics_view(&mut self, ui: &mut egui::Ui) {
        widgets::heading(
            ui,
            "Diagnostics",
            "Build information, platform support and local diagnostic export.",
        );
        widgets::first_section(ui, "Platform & build", |ui| {
            ui.label(format!("{} {}", crate::PRODUCT_NAME, crate::VERSION));
            egui::CollapsingHeader::new("Advanced build details").show(ui, |ui| {
                ui.monospace(
                    crate::build_info()["commit_sha"]
                        .as_str()
                        .unwrap_or("unknown"),
                );
                ui.small(
                    "Windows x64 automation uses the HIDMaestro driver installed by Windows setup.",
                );
            });
            if self.snapshot.capabilities.automation_ready() {
                widgets::badge(ui, "Capture and guarded input available", widgets::GREEN);
            } else {
                widgets::badge(ui, "Automation unavailable", widgets::AMBER);
                ui.label(if self.snapshot.capabilities.reason.is_empty() {
                    "Checking platform capabilities..."
                } else {
                    &self.snapshot.capabilities.reason
                });
            }

            if button(
                ui,
                Icon::Target,
                "Open recognition",
                true,
                "Select the game window and inspect capture.",
            )
            .clicked()
            {
                self.navigate(Tab::Calibration);
            }
        });
        section(ui, "Reports & notifications", |ui| {
            ui.label(&self.snapshot.notification_status);
            if button(ui,Icon::Save,"Export diagnostics",true,"Save build details, platform capabilities, session statistics and visible event messages. No images, profiles or raw log files are included.").clicked()
                && let Some(path)=rfd::FileDialog::new().set_title("Export diagnostics").set_file_name("companion-diagnostics.json").add_filter("JSON",&["json"]).save_file() {
                    let c=&self.snapshot.capabilities;
                    let report=serde_json::json!({"schema_version":1,"build":crate::build_info(),"platform":{"capture":c.capture,"input":c.input,"foreground":c.foreground,"emergency_stop":c.emergency_stop,"reason":c.reason},"statistics":self.snapshot.statistics,"state":self.snapshot.state,"reason":self.snapshot.reason,"events":self.snapshot.events});
                    match serde_json::to_vec_pretty(&report).map_err(|e|e.to_string()).and_then(|bytes|crate::storage::atomic_write(&path,&bytes).map_err(|e|e.to_string())) {Ok(())=>{},Err(error)=>self.error=Some(error)}
                }
            ui.label(format!(
                "Log file: {}",
                self.root.join("logs/run.log").display()
            ));
            if button(ui,Icon::Refresh,"Read recent log",true,"Display up to 64 KiB from this application's log. The log is not included in diagnostic exports.").clicked() {
                self.recent_log=Some(read_recent_log(&self.root).unwrap_or_else(|e|format!("Log unavailable: {e}")));
            }
        });
        if let Some(log) = &self.recent_log {
            egui::ScrollArea::vertical()
                .id_salt("recent-log")
                .max_height(250.)
                .min_scrolled_height(140.)
                .show(ui, |ui| {
                    ui.monospace(log);
                });
        }
    }
    fn recognition_settings(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        widgets::first_section(ui, "Target recognition", |ui| {
            if let Some(c) = &mut self.editor.profile.color {
                changed |= numeric_u32(
                    ui,
                    "Stable detections (frames)",
                    "stable_frames",
                    &mut c.stable_frames,
                );

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
                ui.label("Pick a Metin color in Recognition before adjusting its tolerances.");
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

fn read_recent_log(root: &std::path::Path) -> Result<String, String> {
    use std::io::{Read, Seek, SeekFrom};
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let path = root
        .join("logs/run.log")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !path.starts_with(&root) {
        return Err("Log points outside the data folder.".into());
    }
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let length = file.metadata().map_err(|e| e.to_string())?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(65536)))
        .map_err(|e| e.to_string())?;
    let mut bytes = vec![];
    file.take(65536)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
