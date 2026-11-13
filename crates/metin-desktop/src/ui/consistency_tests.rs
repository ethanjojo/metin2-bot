use super::*;

#[test]
fn settings_and_safety_use_aligned_plain_headings_and_one_navigation_separator() {
    let (mut app, ctx, _dir) = setup();
    for (settings, safety, title, first_group) in [
        (
            Some(SettingsPage::Combat),
            None,
            "Combat & timing",
            "Tempo preset",
        ),
        (
            Some(SettingsPage::Appearance),
            None,
            "Appearance & performance",
            "Interface",
        ),
        (
            Some(SettingsPage::Diagnostics),
            None,
            "Diagnostics",
            "Platform & build",
        ),
        (
            None,
            Some(SafetyPage::Recovery),
            "Recovery",
            "Stuck recovery",
        ),
        (
            None,
            Some(SafetyPage::Notifications),
            "Notifications",
            "Local notifications",
        ),
    ] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(460., 620.),
                )),
                ..Default::default()
            },
            |ui| {
                if let Some(page) = settings {
                    app.settings_page = page;
                    app.settings_view(ui);
                } else {
                    app.safety_page = safety.unwrap();
                    app.safety_view(ui);
                }
            },
        );
        output.textures_delta.clear();
        let regions = text_regions(&output);
        // The tab has the same text; the page heading uses the larger font.
        let heading = regions
            .iter()
            .filter(|(text, _)| text == title)
            .max_by(|a, b| a.1.height().total_cmp(&b.1.height()))
            .unwrap()
            .1;
        let first = regions
            .iter()
            .find(|(text, _)| text == first_group)
            .unwrap()
            .1;
        assert!(
            (heading.left() - first.left()).abs() < 1.,
            "{title} artwork displaced its heading: {heading:?} vs {first:?}"
        );
        for clipped in &output.shapes {
            if let egui::Shape::LineSegment { points, .. } = clipped.shape {
                let y = points[0].y;
                // A form control's bevel is not a full-width page separator.
                if points[0].x.min(points[1].x) <= heading.left() + 1.
                    && (points[1].x - points[0].x).abs() > 100.
                    && (points[1].y - y).abs() < 0.1
                {
                    assert!(
                        y <= heading.top() || y >= first.bottom(),
                        "extra separator between {title} and first group at {y}"
                    );
                }
            }
        }
    }
    app.runtime.shutdown().unwrap();
}

#[test]
fn buff_list_uses_buff_terminology_for_its_actions_and_empty_state() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.buffs.skills.clear();
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(460., 620.),
            )),
            ..Default::default()
        },
        |ui| app.buffs_view(ui),
    );
    output.textures_delta.clear();
    let regions = text_regions(&output);
    assert!(regions.iter().any(|(text, _)| text == "Add buff"));
    assert!(regions.iter().any(|(text, _)| text == "No buffs yet"));
    assert!(
        !regions
            .iter()
            .any(|(text, _)| text.contains("Add skill") || text.contains("No skills"))
    );
    app.runtime.shutdown().unwrap();
}
