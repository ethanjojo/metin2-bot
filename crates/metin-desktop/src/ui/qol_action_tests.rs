// Included from tests.rs: exercise the real forms with the neutral, non-input fixture.
fn qol_action_frame(
    app: &mut MetinApp,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    mut render: impl FnMut(&mut MetinApp, &mut egui::Ui),
) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400., 900.),
            )),
            events,
            ..Default::default()
        },
        |ui| render(app, ui),
    );
    output.textures_delta.clear();
    output
}

fn qol_action_click(
    app: &mut MetinApp,
    ctx: &egui::Context,
    output: &egui::FullOutput,
    label: &str,
    mut render: impl FnMut(&mut MetinApp, &mut egui::Ui),
) {
    let pos = text_regions(output)
        .into_iter()
        .find(|(text, _)| text == label)
        .unwrap_or_else(|| panic!("Missing inline control: {label}"))
        .1
        .center();
    for pressed in [true, false] {
        qol_action_frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
            &mut render,
        );
    }
}

#[test]
fn qol_action_key_hold_is_editable_without_expanding_advanced_input() {
    let (mut app, ctx, _dir) = setup();
    let id = metin_core::PeriodicAction::new("Key").id;
    let mut input = metin_core::ActionInput::Key {
        shortcut: "F4".into(),
        hold_seconds: 0.37,
    };
    let output = qol_action_frame(&mut app, &ctx, vec![], |app, ui| {
        assert!(!app.action_input_editor(
            ui,
            &mut input,
            Binding::Periodic(id),
            EditMode::PeriodicClick(id),
        ));
    });
    assert!(
        text_regions(&output)
            .iter()
            .any(|(text, _)| text == "Key hold (s)")
    );
    assert_eq!(
        input,
        metin_core::ActionInput::Key {
            shortcut: "F4".into(),
            hold_seconds: 0.37
        }
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn qol_action_missing_click_target_and_captured_preview_are_inline() {
    let (mut app, ctx, _dir) = setup();
    let id = metin_core::PeriodicAction::new("Click").id;
    let mut input = metin_core::ActionInput::Click { region: None };
    let output = qol_action_frame(&mut app, &ctx, vec![], |app, ui| {
        app.action_input_editor(
            ui,
            &mut input,
            Binding::Periodic(id),
            EditMode::PeriodicClick(id),
        );
    });
    assert!(
        text_regions(&output)
            .iter()
            .any(|(text, _)| text.contains("Click target missing"))
    );
    input = metin_core::ActionInput::Click {
        region: Some(metin_core::Region {
            left: 0.2,
            top: 0.3,
            right: 0.4,
            bottom: 0.5,
        }),
    };
    let output = qol_action_frame(&mut app, &ctx, vec![], |app, ui| {
        app.action_input_editor(
            ui,
            &mut input,
            Binding::Periodic(id),
            EditMode::PeriodicClick(id),
        );
    });
    assert!(
        app.click_preview.is_some(),
        "Configured click targets need a visible crop without expanding Advanced"
    );
    assert!(
        text_regions(&output)
            .iter()
            .any(|(text, _)| text == "Current snapshot preview")
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn qol_action_periodic_first_use_choices_apply_directly_and_wait_is_inline() {
    let (mut app, ctx, _dir) = setup();
    let mut action = metin_core::PeriodicAction::new("Potion");
    action.enabled = true;
    action.interval_seconds = 123.;
    action.wait_seconds = 2.5;
    app.selected_periodic = Some(action.id);
    app.editor.profile.periodic_actions.push(action);
    let output = qol_action_frame(&mut app, &ctx, vec![], MetinApp::periodic_view);
    assert!(
        text_regions(&output)
            .iter()
            .any(|(text, _)| text == "Wait after use (s)")
    );
    qol_action_click(
        &mut app,
        &ctx,
        &output,
        "Immediately",
        MetinApp::periodic_view,
    );
    assert_eq!(
        app.editor.profile.periodic_actions[0].first_run,
        metin_core::FirstRun::Immediate
    );
    let output = qol_action_frame(&mut app, &ctx, vec![], MetinApp::periodic_view);
    qol_action_click(
        &mut app,
        &ctx,
        &output,
        "After interval",
        MetinApp::periodic_view,
    );
    let action = &app.editor.profile.periodic_actions[0];
    assert_eq!(action.first_run, metin_core::FirstRun::AfterInterval);
    assert_eq!(action.interval_seconds, 123.);
    assert_eq!(action.wait_seconds, 2.5);
    assert!(app.editor.dirty());
    app.runtime.shutdown().unwrap();
}

#[test]
fn qol_action_buff_choices_change_only_selected_skill_with_existing_timer_semantics() {
    let (mut app, ctx, _dir) = setup();
    app.add_skill();
    app.add_skill();
    app.buff_detail = true;
    let first = app.editor.profile.buffs.skills[0].clone();
    app.editor.profile.buffs.skills[1].interval_seconds = Some(180.);
    let output = qol_action_frame(&mut app, &ctx, vec![], MetinApp::buffs_view);
    let choices = text_regions(&output);
    let timed = choices
        .iter()
        .find(|(text, _)| text == "Timed")
        .expect("Timed is immediately visible")
        .1;
    let icon = choices
        .iter()
        .find(|(text, _)| text == "Icon-based")
        .expect("Icon-based is immediately visible")
        .1;
    assert!(timed.right() <= icon.left() && icon.right() <= 400.);
    qol_action_click(&mut app, &ctx, &output, "Icon-based", MetinApp::buffs_view);
    assert_eq!(
        app.editor.profile.buffs.skills[1].mode,
        metin_core::BuffMode::Inactive
    );
    assert_eq!(app.editor.profile.buffs.skills[1].interval_seconds, None);
    let output = qol_action_frame(&mut app, &ctx, vec![], MetinApp::buffs_view);
    qol_action_click(&mut app, &ctx, &output, "Timed", MetinApp::buffs_view);
    assert_eq!(
        app.editor.profile.buffs.skills[1].mode,
        metin_core::BuffMode::Refresh
    );
    assert_eq!(
        app.editor.profile.buffs.skills[1].interval_seconds,
        Some(120.)
    );
    assert_eq!(app.editor.profile.buffs.skills[0], first);
    app.runtime.shutdown().unwrap();
}

#[test]
fn qol_action_puller_choices_respect_disabled_state_and_preserve_timing() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.puller.timing.interval_seconds = 43.;
    app.editor.profile.puller.timing.hold_seconds = 0.35;
    app.editor.profile.puller.press_interval_seconds = 0.6;
    let before = app.editor.profile.puller.clone();
    let output = qol_action_frame(&mut app, &ctx, vec![], MetinApp::puller_view);
    qol_action_click(&mut app, &ctx, &output, "Repeat", MetinApp::puller_view);
    assert_eq!(app.editor.profile.puller, before);
    assert!(app.binding.is_none());
    app.editor.profile.puller.enabled = true;
    let output = qol_action_frame(&mut app, &ctx, vec![], MetinApp::puller_view);
    assert!(
        text_regions(&output)
            .iter()
            .any(|(text, _)| text == "Shortcut hold (s)")
    );
    qol_action_click(&mut app, &ctx, &output, "Repeat", MetinApp::puller_view);
    assert_eq!(
        app.editor.profile.puller.mode,
        metin_core::PullerMode::CombatPeriodic
    );
    let config = &app.editor.profile.puller;
    assert_eq!(config.timing.interval_seconds, 43.);
    assert_eq!(config.timing.hold_seconds, 0.35);
    assert_eq!(config.press_interval_seconds, 0.6);
    app.runtime.shutdown().unwrap();
}

#[test]
fn qol_action_mount_hold_is_inline_and_collapsed_timing_exposes_configured_wait() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.buffs.key_hold_seconds = 0.45;
    app.editor.profile.buffs.mount_wait_seconds = 3.25;
    let profile = app.editor.profile.clone();
    let output = qol_action_frame(&mut app, &ctx, vec![], MetinApp::mount_view);
    let texts = text_regions(&output);
    assert!(
        texts
            .iter()
            .any(|(text, _)| text == "Key hold duration (s)")
    );
    assert!(
        texts.iter().any(|(text, _)| text.contains("3.25 s")),
        "A collapsed timing group must still reveal its configured wait"
    );
    assert_eq!(app.editor.profile, profile);
    app.runtime.shutdown().unwrap();
}
