use super::*;

impl MetinApp {
    pub(in crate::ui) fn recognition_check_dialog(&mut self, ctx: &egui::Context) {
        self.poll_recognition_check(ctx);
        if !self.recognition_check.is_open() {
            return;
        }
        let title = match &self.recognition_check.state {
            DialogState::Invalidated { title } => title.clone(),
            _ => self
                .recognition_check
                .context()
                .expect("open check context")
                .title
                .clone(),
        };
        let subject = self.recognition_check.context().map(|check| check.subject);
        let retry_available = subject.is_some()
            && self.recognition_check_available()
            && !self.recognition_check.is_pending();
        let mut close = false;
        let mut retry = false;
        let mut edit = None;
        let width = (ctx.content_rect().width() - 36.).clamp(180., 720.);
        let frame = egui::Frame::new()
            .fill(ctx.global_style().visuals.window_fill)
            .stroke(egui::Stroke::new(1., widgets::BRONZE))
            .inner_margin(10.);
        let modal = egui::Modal::new(egui::Id::unique("recognition-check-dialog")).frame(frame).show(ctx, |ui| {
            ui.set_width(width);
            let top = ui.cursor().top();
            ui.add(egui::Label::new(egui::RichText::new(format!("Check recognition · {title}")).strong()).wrap());
            ui.separator();
            let header_height = ui.cursor().top() - top;
            let body_height = (ctx.content_rect().height() - header_height - 106.).clamp(32., 600.);
            select::menu_scroll(ui, "recognition-check-body", body_height, |ui| {
                if self.recognition_check.context().is_some_and(|check| check.unsaved) {
                    ui.colored_label(widgets::AMBER, "Using unsaved settings");
                    ui.add_space(4.);
                }
                match &self.recognition_check.state {
                    DialogState::Pending(_) => {
                        ui.horizontal_wrapped(|ui| { ui.spinner(); ui.strong("Waiting for a fresh image…"); });
                        ui.add(egui::Label::new("Capturing the selected game window. Keep it visible and responding.").wrap());
                        ctx.request_repaint_after(Duration::from_millis(50));
                    }
                    DialogState::Ready(report) => {
                        report.evidence.render(ui);
                        ui.small(format!("Captured {:.1} s ago", report.captured_at.elapsed().as_secs_f64()));
                        ui.add(egui::Label::new(egui::RichText::new("One frame only. Live confirmation and Active hold remain separate.").small()).wrap());
                        ui.separator();
                        render_images(ui, &report.images);
                        widgets::collapsing_summary(ui, "Technical details", "recognition-check-technical", "Source", |ui| {
                            ui.label(format!("Source client image: {} × {} px", report.source_size[0], report.source_size[1]));
                            if let SnapshotEvidence::Hp { bounds: [left, top, right, bottom], .. } = report.evidence {
                                ui.add(egui::Label::new(format!("Best match bounds: ({left}, {top})–({right}, {bottom}) px")).wrap());
                                ui.label(format!("Match size: {} × {} px", right - left + 1, bottom - top + 1));
                            }
                        });
                        if let RecognitionSubject::Buff(index) = report.context.subject {
                            ui.separator();
                            ui.strong("Separate live evidence");
                            self.buff_recognition_evidence(ui, index);
                        }
                        ctx.request_repaint_after(Duration::from_millis(250));
                    }
                    DialogState::Failure(failure) => {
                        ui.colored_label(widgets::AMBER, "Unable to check recognition");
                        ui.add(egui::Label::new(&failure.message).wrap());
                        ui.add(egui::Label::new("No comparison result is available. Resolve the capture or reference issue, then check again.").wrap());
                    }
                    DialogState::Invalidated { .. } => {
                        ui.colored_label(widgets::AMBER, "Result no longer applies");
                        ui.add(egui::Label::new("The profile, settings, window or running state changed. Close this dialog and check the current subject again.").wrap());
                    }
                    DialogState::Closed => unreachable!(),
                }
                if let Some(subject) = subject {
                    ui.separator();
                    match subject {
                        RecognitionSubject::HpPanel => {
                            if widgets::plain_button(ui, "Edit reference").clicked() { edit = Some(EditMode::Template); }
                        }
                        RecognitionSubject::Buff(index) => {
                            ui.horizontal_wrapped(|ui| {
                                if widgets::plain_button(ui, "Edit Active reference").clicked() { edit = Some(EditMode::BuffIcon { index, inactive: false }); }
                                if widgets::plain_button(ui, "Edit Inactive reference").clicked() { edit = Some(EditMode::BuffIcon { index, inactive: true }); }
                            });
                        }
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                retry = widgets::button(ui, widgets::Icon::Refresh, "Check again", retry_available,
                    "Compare another fresh image using the same current settings. This sends no game input.").clicked();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close = widgets::plain_button(ui, "Close").clicked();
                });
            });
        });
        // egui checks whether a nested popup was open before consuming Escape.
        close |= modal.should_close();
        if close || edit.is_some() {
            self.close_recognition_check();
        }
        if let Some(mode) = edit {
            self.edit_area(mode);
        } else if retry
            && !close
            && let Some(subject) = subject
        {
            self.begin_recognition_check(subject);
        }
    }
}
impl SnapshotEvidence {
    fn render(&self, ui: &mut egui::Ui) {
        match self {
            Self::Hp {
                score,
                threshold,
                matched,
                ..
            } => {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(if *matched {
                            "HP reference matches this image"
                        } else {
                            "No match · HP score is below the threshold"
                        })
                        .strong()
                        .color(if *matched {
                            widgets::GREEN
                        } else {
                            widgets::AMBER
                        }),
                    )
                    .wrap(),
                );
                ui.label(format!("Match score {score:.3} · required {threshold:.3}"));
            }
            Self::Buff {
                active_score,
                inactive_score,
                presence,
                reason,
            } => {
                let (label, color) = match presence {
                    metin_core::Presence::Active => ("Snapshot appearance: Active", widgets::GREEN),
                    metin_core::Presence::Absent => {
                        ("Snapshot appearance: Inactive", widgets::GREEN)
                    }
                    metin_core::Presence::Unknown => {
                        ("Snapshot appearance: Unknown", widgets::AMBER)
                    }
                };
                ui.add(egui::Label::new(egui::RichText::new(label).strong().color(color)).wrap());
                ui.add(egui::Label::new(reason).wrap());
                for (label, score) in [
                    ("Active score", active_score),
                    ("Inactive score", inactive_score),
                ] {
                    ui.label(match score {
                        Some(score) => format!("{label} {score:.3}"),
                        None => format!("{label}: unavailable"),
                    });
                }
                ui.label(format!(
                    "Required score {:.3}",
                    metin_vision::BUFF_MATCH_THRESHOLD
                ));
            }
        }
        ui.add(
            egui::Label::new(
                egui::RichText::new("Similarity scores are not probabilities.").small(),
            )
            .wrap(),
        );
    }
}
fn render_images(ui: &mut egui::Ui, images: &[ReportImage]) {
    let columns = if ui.available_width() >= 600. {
        3
    } else if ui.available_width() >= 340. {
        2
    } else {
        1
    };
    for chunk in images.chunks(columns) {
        if columns == 1 {
            render_image(ui, &chunk[0]);
        } else {
            ui.columns(columns, |columns| {
                for (column, image) in columns.iter_mut().zip(chunk) {
                    render_image(column, image);
                }
            });
        }
        ui.add_space(8.);
    }
}
fn render_image(ui: &mut egui::Ui, image: &ReportImage) {
    ui.strong(image.role.title());
    let size = image.texture.size();
    ui.small(format!("{} × {} px", size[0], size[1]));
    let scale = (ui.available_width() / size[0] as f32).min(150. / size[1] as f32);
    let fitted = egui::vec2(size[0] as f32 * scale, size[1] as f32 * scale);
    ui.add(egui::Image::new(&image.texture).fit_to_exact_size(fitted));
}
