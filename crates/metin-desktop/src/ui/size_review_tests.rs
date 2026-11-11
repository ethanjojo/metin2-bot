#[test]
fn size_review_ui_blocks_save_and_start_and_escape_restores_the_original_profile() {
    let (mut app, ctx, _dir) = setup();
    let target = window_catalog()[0].token;
    app.target = Some(target);
    let original = app.editor.profile.clone();
    let frame = metin_platform::Frame::new(800, 585, vec![40; 800 * 585 * 3]).unwrap();
    let geometry = metin_platform::TargetState {
        target,
        left: 0,
        top: 0,
        width: 800,
        height: 585,
        visible: true,
        foreground: true,
    };
    app.snapshot = Arc::new(RuntimeSnapshot {
        key: app.key(),
        revision: app.revision,
        geometry: Some(geometry),
        captured_at: Some(std::time::Instant::now()),
        frame: Some(frame.clone()),
        calibration_review: true,
        ..Default::default()
    });
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot: app.snapshot.clone(),
    });
    app.begin_size_review_frame(frame, EditMode::World).unwrap();
    assert!(app.editor.is_recalibrating());
    assert!(app.save().unwrap_err().contains("review"));
    assert!(app.start_blocker().unwrap().message.contains("review"));
    app.review_context.as_mut().unwrap().initial_mode = None;
    app.navigate(Tab::Calibration);
    let size = egui::vec2(400., 500.);
    compact_settle(&mut app, &ctx, size);
    let output = draw_at_size(
        &mut app,
        &ctx,
        &mut vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
        size,
    );
    drop(output);
    assert!(!app.editor.is_recalibrating());
    assert_eq!(app.editor.profile, original);
    assert!(app.review_context.is_none());
    app.runtime.shutdown().unwrap();
}

#[test]
fn size_review_ui_rejects_changed_owner_or_geometry_and_does_not_apply_a_draft() {
    for lose_owner in [true, false] {
        let (mut app, _ctx, _dir) = setup();
        app.target = Some(window_catalog()[0].token);
        let original = app.editor.profile.clone();
        let target = app.target.unwrap();
        let frame = metin_platform::Frame::new(800, 585, vec![40; 800 * 585 * 3]).unwrap();
        let geometry = metin_platform::TargetState {
            target,
            left: 0,
            top: 0,
            width: 800,
            height: 585,
            visible: true,
            foreground: true,
        };
        app.snapshot = Arc::new(RuntimeSnapshot {
            key: app.key(),
            revision: app.revision,
            geometry: Some(geometry),
            captured_at: Some(std::time::Instant::now()),
            frame: Some(frame.clone()),
            calibration_review: true,
            ..Default::default()
        });
        app.begin_size_review_frame(frame, EditMode::World).unwrap();
        if lose_owner {
            app.target = None;
        } else {
            // Same session/revision, but the client was resized again while the
            // private draft was open. Never accept or persist the first remap.
            let mut changed = (*app.snapshot).clone();
            changed.geometry.as_mut().unwrap().height = 584;
            app.snapshot = Arc::new(changed);
        }
        assert!(!app.size_review_can_confirm());
        app.sync_size_review();
        assert!(!app.editor.is_recalibrating());
        assert_eq!(app.editor.profile, original);
        assert!(app.review_context.is_none());
        assert!(
            app.error
                .as_deref()
                .unwrap()
                .contains("previous calibration was restored")
        );
        app.runtime.shutdown().unwrap();
    }
}

#[test]
fn size_review_rows_fit_the_minimum_width_with_all_edit_and_remove_controls() {
    let (mut app, ctx, _dir) = setup();
    let old = app.editor.profile.frame_size.unwrap();
    app.editor.profile.region = Some(metin_core::Region {
        left: 0.1,
        top: 0.1,
        right: 0.8,
        bottom: 0.8,
    });
    app.editor.profile.masks = vec![
        metin_core::Region {
            left: 0.1,
            top: 0.9,
            right: 0.3,
            bottom: 0.99
        };
        3
    ];
    app.editor
        .begin_size_review(
            metin_platform::Frame::new(
                old[0],
                old[1] - 15,
                vec![40; (old[0] * (old[1] - 15) * 3) as usize],
            )
            .unwrap(),
        )
        .unwrap();
    let mut snapshot = (*app.snapshot).clone();
    snapshot.calibration_review = true;
    app.snapshot = Arc::new(snapshot);
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400., 500.),
            )),
            ..Default::default()
        },
        |ui| app.size_review_view(ui),
    );
    output.textures_delta.clear();
    let registry = ctx
        .data(|d| {
            d.get_temp::<(u64, Vec<(String, egui::Rect)>)>(egui::Id::unique(
                "skin-control-registry",
            ))
        })
        .unwrap()
        .1;
    assert!(registry.iter().any(|(hint, _)| hint == "Edit"));
    for (hint, rect) in registry {
        assert!(
            rect.left() >= 0. && rect.right() <= 400.,
            "Review control exceeds viewport: {hint}: {rect:?}"
        );
    }
    app.runtime.shutdown().unwrap();
}

#[test]
fn review_end_is_retained_when_the_command_queue_is_full_then_delivered_once() {
    let key = SessionKey {
        profile_id: uuid::Uuid::new_v4(),
        target: window_catalog()[0].token,
    };
    let wanted = size_review::ReviewEnd { key, revision: 17 };
    let mut pending = Some(wanted);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    assert!(tx.try_send(Command::RefreshTargets).is_ok());
    assert!(!size_review::deliver_review_end(&mut pending, |cmd| tx
        .try_send(cmd)
        .map_err(|_| "Full".into())));
    assert_eq!(pending, Some(wanted));
    assert!(matches!(rx.try_recv(), Ok(Command::RefreshTargets)));
    assert!(size_review::deliver_review_end(&mut pending, |cmd| tx
        .try_send(cmd)
        .map_err(|_| "Full".into())));
    assert!(pending.is_none());
    assert!(
        matches!(rx.try_recv(),Ok(Command::CalibrationReview{key:k,revision:17,active:false}) if k==key)
    );
    assert!(size_review::deliver_review_end(&mut pending, |_| panic!(
        "No duplicate End after delivery"
    )));
}
