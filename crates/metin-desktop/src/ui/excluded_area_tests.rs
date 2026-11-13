fn exclusion_begin_rename(app: &mut MetinApp, ctx: &egui::Context, label: &str) {
    app.navigate(Tab::Calibration);
    compact_settle(app, ctx, egui::vec2(460., 620.));
    let pencil = compact_control_rect(ctx, &format!("Rename excluded area: {label}"));
    compact_click_at(app, ctx, egui::vec2(460., 620.), pencil.center());
    compact_settle(app, ctx, egui::vec2(460., 620.));
}

#[test]
fn excluded_area_inline_rename_enter_escape_and_undo_keep_geometry() {
    let (mut app, ctx, _dir) = setup();
    let region = metin_core::Region {
        left: 0.1,
        top: 0.1,
        right: 0.2,
        bottom: 0.2,
    };
    app.editor.set_region(EditMode::Mask(None), region).unwrap();
    let original = app.editor.profile.masks.clone();
    exclusion_begin_rename(&mut app, &ctx, "Excluded area 1");
    profile_menu_type_name(&mut app, &ctx, "Discarded");
    profile_menu_key(&mut app, &ctx, egui::Key::Escape);
    assert_eq!(app.editor.profile.masks, original);
    exclusion_begin_rename(&mut app, &ctx, "Excluded area 1");
    profile_menu_type_name(&mut app, &ctx, "Minimap");
    profile_menu_key(&mut app, &ctx, egui::Key::Enter);
    assert_eq!(app.editor.profile.masks[0].name, "Minimap");
    assert_eq!(app.editor.profile.masks[0].region, region);
    app.undo_profile();
    assert_eq!(app.editor.profile.masks, original);
    app.runtime.shutdown().unwrap();
}

#[test]
fn named_exclusion_review_labels_and_temporary_highlight_never_change_geometry() {
    let (mut app, ctx, _dir) = setup();
    let region = metin_core::Region {
        left: 0.1,
        top: 0.1,
        right: 0.2,
        bottom: 0.2,
    };
    app.editor.set_region(EditMode::Mask(None), region).unwrap();
    app.editor.rename_excluded_area(0, "Chat").unwrap();
    assert!(
        app.editor
            .review_regions()
            .iter()
            .any(|area| area.label == "Chat" && area.region == region)
    );
    app.navigate(Tab::Calibration);
    // Highlight remains a display-only temporary scoped to the current profile.
    let before = app.editor.profile.clone();
    app.show_excluded_area(&ctx, app.editor.profile.masks[0].clone());
    assert_eq!(
        app.highlighted_exclusion(&ctx),
        Some((region, "Chat".into()))
    );
    app.navigate(Tab::Run);
    assert!(app.highlighted_exclusion(&ctx).is_none());
    app.navigate(Tab::Calibration);
    compact_settle(&mut app, &ctx, egui::vec2(400., 620.));
    assert_eq!(app.editor.profile, before);
    assert_eq!(
        app.highlighted_exclusion(&ctx),
        Some((region, "Chat".into()))
    );
    app.editor.rename_excluded_area(0, "Changed").unwrap();
    assert!(
        app.highlighted_exclusion(&ctx).is_none(),
        "Do not display obsolete annotation evidence after an edit"
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn long_exclusion_names_keep_row_actions_inside_a_small_panel() {
    let (mut app, ctx, _dir) = setup();
    let region = metin_core::Region {
        left: 0.1,
        top: 0.1,
        right: 0.2,
        bottom: 0.2,
    };
    app.editor.set_region(EditMode::Mask(None), region).unwrap();
    app.editor.rename_excluded_area(0, &"M".repeat(80)).unwrap();
    app.navigate(Tab::Calibration);
    for scale in [1., 1.5] {
        ctx.set_zoom_factor(scale);
        compact_settle(&mut app, &ctx, egui::vec2(400., 620.));
        let actions = control_registry(&ctx).into_iter().filter(|(hint, _)| {
            hint.starts_with("Rename excluded area:")
                || hint.starts_with("Show ")
                || hint.starts_with("Move or resize ")
                || hint.starts_with("Delete this excluded area.")
        });
        let mut count = 0;
        for (hint, rect) in actions {
            count += 1;
            assert!(
                rect.left() >= 0. && rect.right() <= 400.,
                "{hint}: {rect:?}"
            );
        }
        assert_eq!(count, 4);
    }
    app.runtime.shutdown().unwrap();
}

#[test]
fn excluded_area_geometry_is_a_distinct_compact_action_without_duplicate_edit_label() {
    let (mut app, ctx, _dir) = setup();
    app.editor
        .set_region(
            EditMode::Mask(None),
            metin_core::Region {
                left: 0.1,
                top: 0.1,
                right: 0.2,
                bottom: 0.2,
            },
        )
        .unwrap();
    app.navigate(Tab::Calibration);
    ctx.set_zoom_factor(1.5);
    let output = compact_settle(&mut app, &ctx, egui::vec2(400., 620.));
    let pencil = compact_control_rect(&ctx, "Rename excluded area: Excluded area 1");
    let geometry = compact_control_rect(
        &ctx,
        "Move or resize Excluded area 1 over a frozen game image",
    );
    assert!(
        !text_regions(&output)
            .iter()
            .any(|(text, rect)| text == "Edit"
                && (rect.center().y - geometry.center().y).abs() < 10.),
        "The excluded-area row must not contain another Edit label beside its rename pencil"
    );
    assert_eq!(pencil.size(), geometry.size());
    assert!((pencil.top() - geometry.top()).abs() < 0.1);
    app.runtime.shutdown().unwrap();
}
