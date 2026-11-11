// Exercise the actual popup, key events and persisted profiles.
fn profile_menu_begin_rename(app: &mut MetinApp, ctx: &egui::Context, id: uuid::Uuid) {
    let size = egui::vec2(460., 620.);
    compact_settle(app, ctx, size);
    if !egui::Popup::is_any_open(ctx) {
        let trigger = ctx
            .data(|d| d.get_temp::<egui::Rect>(egui::Id::unique("profile-trigger")))
            .unwrap();
        compact_click_at(app, ctx, size, trigger.center());
    }
    compact_settle(app, ctx, size);
    let pencil = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::unique(("rename-profile", id))))
        .expect("Each profile needs its own pencil action");
    compact_click_at(app, ctx, size, pencil.center());
    compact_settle(app, ctx, size);
}

fn profile_menu_type_name(app: &mut MetinApp, ctx: &egui::Context, name: &str) {
    let modifiers = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    draw_at_size(
        app,
        ctx,
        &mut vec![
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            },
            egui::Event::Text(name.into()),
        ],
        egui::vec2(460., 620.),
    );
}

fn profile_menu_key(app: &mut MetinApp, ctx: &egui::Context, key: egui::Key) {
    for pressed in [true, false] {
        draw_at_size(
            app,
            ctx,
            &mut vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Default::default(),
            }],
            egui::vec2(460., 620.),
        );
    }
    compact_settle(app, ctx, egui::vec2(460., 620.));
}

#[test]
fn profile_menu_rename_another_profile_saves_only_its_name_without_switching() {
    let (mut app, ctx, _dir) = setup();
    app.save().unwrap();
    let active = app.editor.profile.clone();
    let other = Profile::create("Other profile");
    app.store.save(&other, None).unwrap();
    app.profiles = app.store.list().unwrap();
    app.editor.profile.timing.search_timeout += 1.;
    app.editor.record();
    let draft = app.editor.profile.clone();
    profile_menu_begin_rename(&mut app, &ctx, other.id);
    profile_menu_type_name(&mut app, &ctx, "  Renamed profile  ");
    let check = compact_control_rect(&ctx, "Save profile name (Enter)");
    compact_click_at(&mut app, &ctx, egui::vec2(460., 620.), check.center());
    let mut renamed = other;
    renamed.name = "Renamed profile".into();
    assert_eq!(app.store.load(renamed.id).unwrap().0, renamed);
    assert_eq!(app.store.load(active.id).unwrap().0, active);
    assert_eq!(app.store.selected().unwrap(), Some(active.id));
    assert_eq!(app.editor.profile, draft);
    assert!(app.editor.dirty());
    app.runtime.shutdown().unwrap();
}

#[test]
fn profile_menu_rename_cancel_and_enter_preserve_unsaved_settings_and_saved_assets() {
    let (mut app, ctx, _dir) = setup();
    app.save().unwrap();
    let original = app.editor.profile.clone();
    app.editor.profile.timing.search_timeout += 2.;
    app.editor.record();
    let draft = app.editor.profile.clone();
    profile_menu_begin_rename(&mut app, &ctx, original.id);
    profile_menu_type_name(&mut app, &ctx, "Discarded name");
    let cancel = compact_control_rect(&ctx, "Cancel profile rename (Escape)");
    compact_click_at(&mut app, &ctx, egui::vec2(460., 620.), cancel.center());
    assert_eq!(app.editor.profile, draft);
    assert_eq!(app.store.load(original.id).unwrap().0, original);
    profile_menu_begin_rename(&mut app, &ctx, original.id);
    profile_menu_type_name(&mut app, &ctx, "Confirmed name");
    profile_menu_key(&mut app, &ctx, egui::Key::Enter);
    let mut saved = original;
    saved.name = "Confirmed name".into();
    assert_eq!(app.store.load(saved.id).unwrap().0, saved);
    assert_eq!(app.editor.profile.timing, draft.timing);
    assert!(app.editor.dirty());
    app.editor.undo();
    assert_eq!(app.editor.profile.name, saved.name);
    app.editor.revert();
    assert_eq!(app.editor.profile, saved);
    app.runtime.shutdown().unwrap();
}

#[test]
fn profile_menu_rename_empty_name_is_disabled_and_escape_discards_only_the_row() {
    let (mut app, ctx, _dir) = setup();
    app.save().unwrap();
    let saved = app.editor.profile.clone();
    profile_menu_begin_rename(&mut app, &ctx, saved.id);
    profile_menu_type_name(&mut app, &ctx, "   ");
    let check = compact_control_rect(&ctx, "Save profile name (Enter)");
    compact_click_at(&mut app, &ctx, egui::vec2(460., 620.), check.center());
    assert_eq!(app.store.load(saved.id).unwrap().0, saved);
    profile_menu_key(&mut app, &ctx, egui::Key::Escape);
    assert_eq!(app.editor.profile, saved);
    assert!(
        egui::Popup::is_any_open(&ctx),
        "Escape cancels the rename, leaving profile navigation open"
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn profile_menu_rename_failure_keeps_the_input_and_can_be_retried() {
    let (mut app, ctx, dir) = setup();
    app.save().unwrap();
    let id = app.editor.profile.id;
    profile_menu_begin_rename(&mut app, &ctx, id);
    profile_menu_type_name(&mut app, &ctx, "Retry name");
    let path = dir
        .path()
        .join("profiles")
        .join(id.to_string())
        .join("profile.json");
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"invalid").unwrap();
    let check = compact_control_rect(&ctx, "Save profile name (Enter)");
    compact_click_at(&mut app, &ctx, egui::vec2(460., 620.), check.center());
    assert!(app.profile_rename.is_some());
    let output = compact_settle(&mut app, &ctx, egui::vec2(460., 620.));
    assert!(
        text_regions(&output)
            .iter()
            .any(|(text, _)| text == "Retry name")
    );
    std::fs::write(&path, original).unwrap();
    compact_click_at(&mut app, &ctx, egui::vec2(460., 620.), check.center());
    assert_eq!(app.store.load(id).unwrap().0.name, "Retry name");
    assert!(app.profile_rename.is_none());
    app.runtime.shutdown().unwrap();
}

#[test]
fn profile_menu_rename_new_profile_keeps_its_settings_in_the_draft() {
    let (mut app, ctx, _dir) = setup();
    app.save().unwrap();
    let baseline = Profile::create("New profile");
    app.replace_profile(baseline.clone(), None);
    app.editor.profile.timing.search_timeout += 3.;
    app.editor.record();
    profile_menu_begin_rename(&mut app, &ctx, baseline.id);
    profile_menu_type_name(&mut app, &ctx, "Named draft");
    profile_menu_key(&mut app, &ctx, egui::Key::Enter);
    let persisted = app.store.load(baseline.id).unwrap().0;
    assert_eq!(persisted.name, "Named draft");
    assert_eq!(persisted.timing, baseline.timing);
    assert_eq!(app.store.selected().unwrap(), Some(baseline.id));
    assert!(app.editor.dirty());
    assert_ne!(app.editor.profile.timing, persisted.timing);
    app.runtime.shutdown().unwrap();
}
