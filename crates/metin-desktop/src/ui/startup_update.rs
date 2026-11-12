use super::*;
use crate::updates::UpdateState;

impl MetinApp {
    #[cfg(feature = "ui-smoke")]
    pub fn prepare_update_helper_fixture(&mut self, phase: &str) {
        self.update_fixture = true;
        self.update_helper_fixture = Some(phase.to_owned());
    }
    pub(super) fn dismiss_startup_update(&mut self) {
        self.startup_update_open = false;
        self.update_install_requested = None;
        self.updates.cancel();
        self.update_error = None;
    }

    pub(super) fn startup_update_view(&mut self, ui: &mut egui::Ui) {
        if skin::simple_title(ui, crate::PRODUCT_NAME) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(16, 12))
            .show(ui, |ui| {
                let checking = matches!(self.updates.state(), UpdateState::Checking);
                let footer_height = if ui.available_width() < 260. {
                    72.
                } else {
                    40.
                };
                let body_height = (ui.available_height() - footer_height).max(1.);
                egui::ScrollArea::vertical()
                    .id_salt("startup-update-body")
                    .auto_shrink([false, false])
                    .max_height(body_height)
                    .show(ui, |ui| {
                        ui.add_space((body_height * 0.06).min(30.));
                        ui.heading(if checking {
                            "Checking for updates"
                        } else {
                            "Update Companion"
                        });
                        ui.add_space(8.);
                        ui.label(format!("Installed version: {}", crate::VERSION));
                        ui.small("Your profiles and settings will be kept.");
                        ui.add_space(14.);
                        self.update_status(ui, true);
                    });
                ui.add_space(12.);
                ui.horizontal_wrapped(|ui| {
                    self.update_primary_action(ui, true);
                    let label = if matches!(self.updates.state(), UpdateState::Downloading { .. }) {
                        "Cancel download"
                    } else if checking {
                        "Continue"
                    } else {
                        "Skip for now"
                    };
                    if ui
                        .add_enabled_ui(!self.update_handoff_waiting(), |ui| {
                            widgets::plain_button(ui, label)
                        })
                        .inner
                        .on_hover_text(
                            "Continue to the game window chooser. Nothing will be installed.",
                        )
                        .clicked()
                    {
                        self.dismiss_startup_update();
                    }
                });
            });
    }
}
