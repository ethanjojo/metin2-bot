#[test]
fn profile_transfer_duplicate_from_actual_menu_selects_independent_profile() {
    let (mut app, ctx, _dir) = setup();
    app.save().unwrap();
    let original = app.editor.profile.clone();
    let size = egui::vec2(460., 620.);
    compact_settle(&mut app, &ctx, size);
    let trigger = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::unique("profile-trigger")))
        .unwrap();
    compact_click_at(&mut app, &ctx, size, trigger.center());
    compact_settle(&mut app, &ctx, size);
    let duplicate = compact_control_rect(&ctx, "Duplicate");
    compact_click_at(&mut app, &ctx, size, duplicate.center());
    assert_ne!(app.editor.profile.id, original.id);
    assert!(app.editor.profile.name.ends_with(" (copy)"));
    assert_eq!(app.store.selected().unwrap(), Some(app.editor.profile.id));
    assert_eq!(app.store.load(original.id).unwrap().0, original);
    assert!(!app.editor.dirty());
    app.runtime.shutdown().unwrap();
}

#[test]
fn profile_transfer_preview_cancel_does_not_touch_store_or_current_draft() {
    let (mut app, ctx, _dir) = setup();
    app.save().unwrap();
    let package = app.store.export_profile(app.editor.profile.id).unwrap();
    app.editor.profile.timing.search_timeout += 1.;
    app.editor.record();
    let before = app.editor.profile.clone();
    let profiles = app.store.list().unwrap();
    ctx.data_mut(|d| {
        d.insert_temp(
            profiles::import_preview_id(),
            profiles::ImportPreview {
                package: Arc::new(package),
                error: None,
            },
        )
    });
    app.profile_open_requested = true;
    let size = egui::vec2(460., 620.);
    compact_settle(&mut app, &ctx, size);
    let cancel = compact_control_rect(&ctx, "Cancel profile import without changing profiles.");
    compact_click_at(&mut app, &ctx, size, cancel.center());
    assert_eq!(app.editor.profile, before);
    assert!(app.editor.dirty());
    assert_eq!(app.store.list().unwrap(), profiles);
    assert!(
        ctx.data(|d| d.get_temp::<profiles::ImportPreview>(profiles::import_preview_id()))
            .is_none()
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn profile_transfer_preview_confirmation_imports_exact_owned_contents() {
    let (mut app, ctx, dir) = setup();
    app.save().unwrap();
    let original = app.editor.profile.clone();
    let file = dir.path().join("preview.companion-profile");
    app.store
        .export_profile(original.id)
        .unwrap()
        .write(&file)
        .unwrap();
    let package = crate::storage::ProfilePackage::read(&file).unwrap();
    std::fs::write(&file, b"changed after preview").unwrap();
    ctx.data_mut(|d| {
        d.insert_temp(
            profiles::import_preview_id(),
            profiles::ImportPreview {
                package: Arc::new(package),
                error: None,
            },
        )
    });
    app.profile_open_requested = true;
    let size = egui::vec2(460., 620.);
    compact_settle(&mut app, &ctx, size);
    let confirm = compact_control_rect(
        &ctx,
        "Create a new profile from this exact package; existing profiles are never overwritten.",
    );
    compact_click_at(&mut app, &ctx, size, confirm.center());
    assert_ne!(app.editor.profile.id, original.id);
    assert_eq!(app.editor.profile.name, original.name);
    assert_eq!(app.store.list().unwrap().len(), 2);
    assert_eq!(app.store.load(original.id).unwrap().0, original);
    assert!(!app.editor.dirty());
    assert!(!app.snapshot.running);
    app.runtime.shutdown().unwrap();
}

#[test]
fn profile_transfer_preview_keeps_confirm_cancel_inside_compact_popup() {
    for (width, scale) in [(400., 1.), (460., 1.5)] {
        let (mut app, ctx, _dir) = setup();
        app.save().unwrap();
        app.settings.ui_scale = scale;
        ctx.set_zoom_factor(scale);
        let package = app.store.export_profile(app.editor.profile.id).unwrap();
        ctx.data_mut(|d| {
            d.insert_temp(
                profiles::import_preview_id(),
                profiles::ImportPreview {
                    package: Arc::new(package),
                    error: Some("Long imported package information ".repeat(12)),
                },
            )
        });
        app.profile_open_requested = true;
        let size = egui::vec2(width, 620.);
        compact_settle(&mut app, &ctx, size);
        for hint in [
            "Create a new profile from this exact package; existing profiles are never overwritten.",
            "Cancel profile import without changing profiles.",
        ] {
            let rect = compact_control_rect(&ctx, hint);
            let clip = ctx
                .data(|d| d.get_temp::<egui::Rect>(egui::Id::unique(("control-clip", hint))))
                .unwrap();
            assert!(
                clip.contains(rect.center()),
                "{hint} must remain visible at {width}px / {scale}x"
            );
        }
        app.runtime.shutdown().unwrap();
    }
}
