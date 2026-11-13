#[test]
fn recognition_owns_metin_configuration_instead_of_a_separate_settings_tab() {
    let (mut app, ctx, _dir) = setup();
    app.tab = Tab::Calibration;
    app.editor.profile.color = Some(metin_core::ColorConfig::default());
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(460., 620.));
    for _ in 0..3 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(460., 620.));
    }
    assert!(
        text_regions(&output)
            .iter()
            .any(|(text, _)| text.contains("Metin detection")),
        "Recognition must expose the preset selector and editor next to the sampled color"
    );
    app.tab = Tab::Settings;
    output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(460., 620.));
    assert!(
        !text_regions(&output)
            .iter()
            .any(|(text, _)| text == "Detection"),
        "The obsolete Detection subtab must not duplicate Recognition settings"
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn picking_a_different_color_detaches_preset_without_losing_shape_limits() {
    let (mut app, _ctx, _dir) = setup();
    let mut color = metin_core::ColorConfig::default();
    let sampled = metin_vision::pick_color(app.editor.frozen.as_ref().unwrap(), [0, 0]).unwrap();
    color.hsv = [(sampled[0] + 40) % 180, sampled[1], sampled[2]];
    color.min_width = 37;
    app.editor.profile.color = Some(color);
    app.editor.profile.color_preset_id = Some(uuid::Uuid::new_v4());
    app.editor.color([0, 0]).unwrap();
    assert!(app.editor.profile.color_preset_id.is_none());
    assert_eq!(app.editor.profile.color.as_ref().unwrap().min_width, 37);
    app.runtime.shutdown().unwrap();
}

#[test]
fn clearing_color_clears_preset_provenance_and_undo_restores_both() {
    let (mut app, _ctx, _dir) = setup();
    app.editor.profile.color = Some(Default::default());
    app.editor.profile.color_preset_id = Some(uuid::Uuid::new_v4());
    app.editor.record();
    let before = app.editor.profile.clone();
    app.editor.delete(EditMode::Color).unwrap();
    assert!(app.editor.profile.color.is_none());
    assert!(app.editor.profile.color_preset_id.is_none());
    app.editor.undo();
    assert_eq!(app.editor.profile, before);
    app.runtime.shutdown().unwrap();
}

#[test]
fn metin_preset_cancel_and_capture_leave_main_profile_and_history_untouched() {
    let (mut app, ctx, _dir) = setup();
    let before = app.editor.profile.clone();
    let change = app.editor.change_id();
    app.open_metin_preset(true);
    app.metin_presets.draft.as_mut().unwrap().color.min_width = 49;
    let frame = app.editor.frozen.clone().unwrap();
    let capture = app
        .editor
        .begin_capture(EditMode::Color, frame.clone())
        .unwrap();
    app.commit_preset_color(capture, None, [0, 0]).unwrap();
    assert_eq!(
        app.metin_presets.draft.as_ref().unwrap().color.hsv,
        metin_vision::pick_color(&frame, [0, 0]).unwrap()
    );
    assert_eq!(
        app.metin_presets.draft.as_ref().unwrap().color.min_width,
        49
    );
    assert_eq!(app.editor.profile, before);
    assert_eq!(app.editor.change_id(), change);
    draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(460., 620.));
    let mut events = vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }];
    draw_at_size(&mut app, &ctx, &mut events, egui::vec2(460., 620.));
    assert!(app.metin_presets.draft.is_none());
    assert_eq!(app.editor.profile, before);
    assert_eq!(app.editor.change_id(), change);
    assert!(app.store.metin_presets().unwrap().is_empty());
    app.runtime.shutdown().unwrap();
}

#[test]
fn named_metin_presets_copy_all_values_and_do_not_rewrite_other_profiles() {
    let (mut app, _ctx, _dir) = setup();
    let mut other = app.editor.profile.clone();
    other.id = uuid::Uuid::new_v4();
    other.name = "Other profile".into();
    app.store
        .save(&other, app.editor.template.as_ref())
        .unwrap();
    app.open_metin_preset(true);
    let mut draft = app.metin_presets.draft.take().unwrap();
    draft.name = "Red stones".into();
    draft.color.min_width = 49;
    draft.color.saturation_tolerance = 71;
    app.apply_metin_preset(&draft).unwrap();
    let original_id = app.editor.profile.color_preset_id.unwrap();
    assert_eq!(app.editor.profile.color, Some(draft.color.clone()));
    assert!(app.editor.dirty());
    assert_eq!(app.store.load(other.id).unwrap().0, other);
    assert_eq!(app.store.metin_presets().unwrap()[0].color, draft.color);
    assert_eq!(
        app.metin_presets.active(&app.editor.profile).unwrap().id,
        original_id
    );
    app.open_metin_preset(false);
    let mut draft = app.metin_presets.draft.take().unwrap();
    draft.color.min_width = 50;
    app.apply_metin_preset(&draft).unwrap();
    assert_eq!(app.editor.profile.color_preset_id, Some(original_id));
    assert_eq!(app.store.load(other.id).unwrap().0, other);
    app.runtime.shutdown().unwrap();
}

#[test]
fn invalid_catalog_write_and_stale_preset_draft_never_apply_main_values() {
    let (mut app, _ctx, _dir) = setup();
    let before = app.editor.profile.clone();
    app.open_metin_preset(true);
    let mut draft = app.metin_presets.draft.take().unwrap();
    draft.name = " ".into();
    draft.color.min_width = 49;
    assert!(app.apply_metin_preset(&draft).is_err());
    assert_eq!(app.editor.profile, before);
    assert!(app.store.metin_presets().unwrap().is_empty());
    draft.name = "Valid".into();
    app.target = Some(TargetToken {
        handle: 100,
        pid: 101,
        created: 1,
    });
    assert!(app.apply_metin_preset(&draft).is_err());
    assert_eq!(app.editor.profile, before);
    assert!(app.store.metin_presets().unwrap().is_empty());
    app.target = None;
    app.editor.profile.name = "Changed main profile".into();
    app.editor.record();
    assert!(app.apply_metin_preset(&draft).is_err());
    assert!(app.store.metin_presets().unwrap().is_empty());
    app.runtime.shutdown().unwrap();
}

#[test]
fn metin_editor_blocks_start_save_switch_and_detached_provenance_resolves_custom() {
    let (mut app, _ctx, _dir) = setup();
    app.editor.profile.color = Some(Default::default());
    app.editor.profile.color_preset_id = Some(uuid::Uuid::new_v4());
    assert!(app.metin_presets.active(&app.editor.profile).is_none());
    app.open_metin_preset(false);
    assert!(matches!(
        app.metin_presets.draft.as_ref().unwrap().kind,
        metin_presets::DraftKind::Custom
    ));
    assert!(
        app.start_profile()
            .unwrap_err()
            .contains("current selection or edit")
    );
    assert!(app.save().is_err());
    assert!(!app.discard_or_save());
    let mut draft = app.metin_presets.draft.take().unwrap();
    draft.color.min_width = 49;
    app.apply_metin_preset(&draft).unwrap();
    assert!(app.editor.profile.color_preset_id.is_none());
    app.runtime.shutdown().unwrap();
}

#[test]
fn metin_preset_editor_keeps_fixed_footer_inside_small_scaled_viewport() {
    let (mut app, ctx, _dir) = setup();
    ctx.set_zoom_factor(1.5);
    app.open_metin_preset(true);
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(400., 620.));
    for _ in 0..4 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(400., 620.));
    }
    let labels = text_regions(&output);
    for label in ["Cancel", "Save & apply"] {
        let rect = labels.iter().find(|(text, _)| text == label).unwrap().1;
        assert!(
            ctx.content_rect().contains(rect.min) && ctx.content_rect().contains(rect.max),
            "Fixed footer {label} must stay visible: {rect:?}"
        );
    }
    let (_, _, thumb, _, _) = ctx.data(|data| data.get_temp::<(egui::Rect, egui::Rect, egui::Rect, f32, egui::Id)>(egui::Id::unique("select-scroll-geometry"))).expect("A compact preset editor must visibly indicate its scrollable fields");
    assert!(ctx.content_rect().contains(thumb.min) && ctx.content_rect().contains(thumb.max));
    app.runtime.shutdown().unwrap();
}

#[test]
fn invalid_metin_preset_save_shows_error_next_to_fixed_actions_at_150_percent() {
    let (mut app, ctx, _dir) = setup();
    ctx.set_zoom_factor(1.5);
    app.open_metin_preset(true);
    let size = egui::vec2(400., 480.);
    let before = app.editor.profile.clone();
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], size);
    for _ in 0..4 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], size);
    }
    let pos = text_regions(&output)
        .iter()
        .find(|(text, _)| text == "Save & apply")
        .unwrap()
        .1
        .center();
    for pressed in [true, false] {
        output = draw_at_size(
            &mut app,
            &ctx,
            &mut vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
            size,
        );
    }
    for _ in 0..3 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], size);
    }
    let error = app
        .metin_presets
        .draft
        .as_ref()
        .unwrap()
        .error
        .as_ref()
        .unwrap();
    let rect = text_regions(&output)
        .into_iter()
        .find(|(text, _)| text == error)
        .unwrap()
        .1;
    assert!(
        ctx.content_rect().contains(rect.min) && ctx.content_rect().contains(rect.max),
        "Failed save must stay visible above the fixed footer: {rect:?}"
    );
    assert_eq!(app.editor.profile, before);
    assert!(app.store.metin_presets().unwrap().is_empty());
    app.runtime.shutdown().unwrap();
}

#[cfg(feature = "ui-smoke")]
#[test]
fn repeated_preset_screenshot_fixtures_reuse_the_same_demo_catalog_entry() {
    let (mut app, ctx, _dir) = setup();
    for fixture in [
        QolFixture::MetinPreset,
        QolFixture::MetinPresetNew,
        QolFixture::MetinPreset,
    ] {
        app.metin_presets.draft = None;
        app.prepare_qol_fixture(&ctx, Some(fixture), None).unwrap();
        assert!(app.metin_presets.draft.is_some());
        assert_eq!(app.store.metin_presets().unwrap().len(), 1);
    }
    app.runtime.shutdown().unwrap();
}
