#[test]
fn display_reset_restores_and_persists_target_position_and_size_without_profile_edits() {
    let (mut app, ctx, dir) = setup();
    let target = TargetToken {
        handle: 900,
        pid: 901,
        created: 902,
    };
    let key = crate::placement::WindowPlacementKey {
        process: "client.exe".into(),
        title: "Window".into(),
        instance: 0,
    };
    app.placement_key = Some((target, key.clone()));
    app.settings.panel_positions = vec![crate::placement::SavedPanelPosition {
        key,
        offset: crate::placement::PanelOffset {
            right: 120,
            top: 180,
        },
        expanded_size: Some(crate::placement::PanelSize {
            width: 700,
            height: 800,
        }),
    }];
    app.expanded_size = egui::vec2(700., 800.);
    app.settings_page = SettingsPage::Appearance;
    let profile = app.editor.profile.clone();
    let mut render = |events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(460., 620.),
                )),
                events,
                ..Default::default()
            },
            |ui| app.settings_view(ui),
        );
        output.textures_delta.clear();
        output
    };
    let mut output = render(vec![]);
    for _ in 0..3 {
        output = render(vec![]);
    }
    let position = text_regions(&output)
        .iter()
        .find(|(text, _)| text == "Reset position and size")
        .expect("Display must expose placement reset")
        .1
        .center();
    for pressed in [true, false] {
        render(vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            },
        ]);
    }
    drop(render);
    let saved = crate::storage::UISettings::load(dir.path()).unwrap();
    assert_eq!(
        saved.panel_positions[0].offset,
        crate::placement::PanelOffset::default()
    );
    assert_eq!(
        saved.panel_positions[0].expanded_size,
        Some(crate::placement::PanelSize {
            width: 460,
            height: 620
        })
    );
    assert_eq!(app.expanded_size, egui::vec2(460., 620.));
    assert_eq!(app.editor.profile, profile);
    app.runtime.shutdown().unwrap();
}

#[test]
fn returning_to_actions_retains_the_open_detail_and_unsaved_draft() {
    let (mut app, _ctx, _dir) = setup();
    let action = metin_core::PeriodicAction::new("Use potion");
    app.selected_periodic = Some(action.id);
    app.editor.profile.periodic_actions.push(action);
    app.editor.profile.buffs.enabled = true;
    app.editor.record();
    let draft = app.editor.profile.clone();
    app.tab = Tab::Actions;
    app.actions_page = ActionsPage::Periodic;
    app.actions_detail = true;
    app.navigate(Tab::Run);
    app.navigate(Tab::Actions);
    assert_eq!(app.actions_page, ActionsPage::Periodic);
    assert!(app.actions_detail);
    assert_eq!(app.editor.profile, draft);
    assert_eq!(app.selected_periodic, Some(draft.periodic_actions[0].id));
    app.runtime.shutdown().unwrap();
}

#[test]
fn invalid_dirty_f7_request_is_consumed_once_without_saving_and_stale_request_is_ignored() {
    let (mut app, _ctx, _dir) = setup();
    let target = TargetToken {
        handle: 900,
        pid: 901,
        created: 902,
    };
    app.target = Some(target);
    app.editor.profile.timing.search_timeout += 1.;
    app.editor.record();
    let draft = app.editor.profile.clone();
    let request = crate::runtime::StartRequest {
        id: 900,
        key: SessionKey {
            profile_id: draft.id,
            target,
        },
        revision: app.revision,
        generation: app.runtime.stop_generation(),
        requested_at: std::time::Instant::now(),
    };
    app.snapshot = Arc::new(RuntimeSnapshot {
        start_request: Some(request),
        ..Default::default()
    });
    app.consume_start_request();
    assert!(app.error.is_some());
    assert!(app.editor.dirty());
    assert_eq!(app.editor.profile, draft);
    assert!(app.store.list().unwrap().is_empty());
    app.error = None;
    app.consume_start_request();
    assert!(
        app.error.is_none(),
        "A handled request cannot start again after a dialog closes"
    );
    let mut stale = request;
    stale.id += 1;
    stale.generation = stale.generation.wrapping_sub(1);
    app.snapshot = Arc::new(RuntimeSnapshot {
        start_request: Some(stale),
        ..Default::default()
    });
    app.consume_start_request();
    assert!(app.error.is_none());
    app.runtime.shutdown().unwrap();
}

#[test]
fn pause_during_save_keeps_saved_data_but_never_restamps_the_cancelled_start() {
    let (mut app, _ctx, _dir) = setup();
    app.target = Some(TargetToken {
        handle: 100,
        pid: 201,
        created: 1,
    });
    app.editor.profile.timing.search_timeout += 1.;
    app.editor.record();
    let start_generation = app.runtime.stop_generation();
    app.runtime.stop(); // F8 received after the start intent, before apply/Arm.
    assert_eq!(
        app.save_in_generation(Some(start_generation)).unwrap(),
        None
    );
    assert!(!app.editor.dirty());
    assert_eq!(
        app.store.load(app.editor.profile.id).unwrap().0,
        app.editor.profile
    );
    assert!(app.runtime.stop_generation() > start_generation);
    assert!(!app.runtime.latest().unwrap().running);
    app.runtime.shutdown().unwrap();
}

#[test]
fn f7_requests_are_processed_in_the_hidden_window_logic_pass_without_painting() {
    let (mut app, ctx, _dir) = setup();
    let target = TargetToken {
        handle: 100,
        pid: 201,
        created: 1,
    };
    app.target = Some(target);
    let key = SessionKey {
        profile_id: app.editor.profile.id,
        target,
    };
    let request = crate::runtime::StartRequest {
        id: 901,
        key,
        revision: app.revision,
        generation: app.runtime.stop_generation(),
        requested_at: std::time::Instant::now(),
    };
    app.snapshot = Arc::new(RuntimeSnapshot {
        key: Some(key),
        revision: app.revision,
        start_request: Some(request),
        targets: window_catalog(),
        targets_revision: 1,
        ..Default::default()
    });
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot: app.snapshot.clone(),
    });
    let _output = ctx.run_logic(&egui::RawInput::default(), |ctx| app.background_tick(ctx));
    assert_eq!(app.last_start_request, 901);
    assert!(
        app.error.is_some(),
        "The invalid draft is explained without starting or saving"
    );
    assert!(app.store.list().unwrap().is_empty());
    app.runtime.shutdown().unwrap();
}

#[test]
fn profile_change_popup_and_percentage_scale_fit_a_small_scaled_window() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.timing.search_timeout += 1.;
    app.editor.record();
    ctx.set_zoom_factor(1.5);
    let size = egui::vec2(400., 500.);
    for _ in 0..3 {
        draw_at_size(&mut app, &ctx, &mut vec![], size);
    }
    let geometry = header_profile_geometry(&ctx);
    for rect in geometry {
        assert!(
            rect.right() <= ctx.content_rect().right(),
            "Header control must fit at150%: {rect:?}"
        );
    }
    app.navigate(Tab::Settings);
    app.settings_page = SettingsPage::Appearance;
    for _ in 0..3 {
        draw_at_size(&mut app, &ctx, &mut vec![], size);
    }
    let output = draw_at_size(&mut app, &ctx, &mut vec![], size);
    assert!(
        text_regions(&output)
            .iter()
            .any(|(text, _)| text.contains('%'))
    );
    for (text, rect) in text_regions(&output) {
        assert!(
            rect.right() <= ctx.content_rect().right() + 1.,
            "{text} overflows scaled viewport: {rect:?}"
        );
    }
    app.runtime.shutdown().unwrap();
}

#[test]
fn changed_fields_open_the_correct_secondary_page_and_primary_tabs_do_not_overlap() {
    let (mut app, ctx, _dir) = setup();
    let before = app.editor.profile.clone();
    app.editor.profile.timing.search_timeout += 1.;
    let rows = changes::profile_changes(&before, &app.editor.profile, [None, None], &[]);
    app.settings_page = SettingsPage::Diagnostics;
    app.open_profile_change(&rows[0]);
    assert_eq!(app.settings_page, SettingsPage::Combat);
    let before = app.editor.profile.clone();
    app.editor.profile.safety.sound = true;
    let rows = changes::profile_changes(&before, &app.editor.profile, [None, None], &[]);
    app.open_profile_change(&rows[0]);
    assert_eq!(app.safety_page, SafetyPage::Notifications);
    app.editor.profile.color = Some(metin_core::ColorConfig::default());
    let before = app.editor.profile.clone();
    app.editor.profile.color.as_mut().unwrap().stable_frames += 1;
    let rows = changes::profile_changes(&before, &app.editor.profile, [None, None], &[]);
    app.open_profile_change(&rows[0]);
    assert_eq!(app.tab, Tab::Calibration);
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400. / 1.5, 80.),
            )),
            ..Default::default()
        },
        |ui| app.navigation(ui),
    );
    let mut labels = text_regions(&output);
    output.textures_delta.clear();
    labels.sort_by(|a, b| a.1.left().total_cmp(&b.1.left()));
    for pair in labels.windows(2) {
        assert!(
            pair[0].1.right() <= pair[1].1.left(),
            "Primary tab labels overlap at400px/150%"
        );
    }
    app.runtime.shutdown().unwrap();
}
