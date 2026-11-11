fn qol_choice_frame(
    ctx: &egui::Context,
    value: &mut bool,
    enabled: bool,
    events: Vec<egui::Event>,
) -> [egui::Rect; 2] {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400., 180.),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            ui.set_width(180.);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    widgets::segmented_choice(
                        ui,
                        "qol-choice",
                        value,
                        &[
                            (false, "Timed", "Use a timer"),
                            (true, "Icon-based", "Recognize an icon"),
                        ],
                    );
                });
            });
        },
    );
    output.textures_delta.clear();
    ["Use a timer", "Recognize an icon"].map(|hint| compact_control_rect(ctx, hint))
}

#[test]
fn qol_segmented_choices_fit_keep_order_and_ignore_disabled_pointer_input() {
    let ctx = egui::Context::default();
    widgets::theme(&ctx);
    let mut value = false;
    let rects = qol_choice_frame(&ctx, &mut value, true, vec![]);
    assert!(rects[0].right() < rects[1].left());
    assert!(rects.iter().all(|r| r.left() >= 0. && r.right() <= 180.));
    for pressed in [true, false] {
        let pos = rects[1].center();
        qol_choice_frame(
            &ctx,
            &mut value,
            false,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    assert!(!value);
}

#[test]
fn qol_segmented_choices_support_pointer_selection_and_arrow_navigation() {
    let ctx = egui::Context::default();
    widgets::theme(&ctx);
    let mut value = false;
    let rects = qol_choice_frame(&ctx, &mut value, true, vec![]);
    for pressed in [true, false] {
        let pos = rects[1].center();
        qol_choice_frame(
            &ctx,
            &mut value,
            true,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    assert!(value);
    qol_choice_frame(
        &ctx,
        &mut value,
        true,
        vec![egui::Event::Key {
            key: egui::Key::ArrowLeft,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    assert!(!value);
}

#[test]
fn qol_feedback_undo_is_retired_by_an_intervening_edit() {
    let (mut app, _ctx, _dir) = setup();
    app.editor.profile.region = Some(metin_core::Region {
        left: 0.1,
        top: 0.1,
        right: 0.8,
        bottom: 0.8,
    });
    app.editor.record();
    let original = app.editor.profile.clone();
    app.delete_area(EditMode::World);
    assert!(app.feedback_current());
    assert!(app.undo_feedback());
    assert_eq!(app.editor.profile, original);
    app.delete_area(EditMode::World);
    app.editor.profile.name = "Another edit".into();
    app.editor.record();
    assert!(!app.feedback_current());
    assert!(!app.undo_feedback());
    assert_eq!(app.editor.profile.name, "Another edit");
    assert!(app.editor.profile.region.is_none());
    app.runtime.shutdown().unwrap();
}

#[test]
fn qol_feedback_reports_only_successfully_saved_changes() {
    let (mut app, _ctx, _dir) = setup();
    app.editor.profile.name.clear();
    app.editor.record();
    assert!(app.save().is_err());
    assert!(!app.feedback_current());
    app.editor.profile.name = "Saved profile".into();
    app.editor.record();
    app.save().unwrap();
    assert!(app.feedback_current());
    assert_eq!(app.feedback.as_ref().unwrap().message, "Saved");
    app.runtime.shutdown().unwrap();
}

#[test]
fn qol_noop_removal_cannot_offer_undo_for_an_unrelated_edit() {
    let (mut app, _ctx, _dir) = setup();
    app.editor.profile.name = "Keep this name".into();
    app.editor.record();
    let original = app.editor.profile.clone();
    app.delete_area(EditMode::Mask(None));
    assert!(!app.feedback_current());
    assert!(!app.undo_feedback());
    assert_eq!(app.editor.profile, original);
    app.runtime.shutdown().unwrap();
}

#[test]
fn qol_cancelled_calibration_draft_cannot_undo_an_original_edit() {
    let (mut app, _ctx, _dir) = setup();
    app.editor.profile.region = Some(metin_core::Region {
        left: 0.1,
        top: 0.1,
        right: 0.8,
        bottom: 0.8,
    });
    app.editor = Editor::new(app.editor.profile.clone(), None);
    app.editor.profile.name = "Original edit".into();
    app.editor.record();
    let original = app.editor.profile.clone();
    let original_change = app.editor.change_id();
    app.editor
        .begin_size_review(metin_platform::Frame::new(800, 585, vec![40; 800 * 585 * 3]).unwrap())
        .unwrap();
    app.delete_area(EditMode::World);
    assert_eq!(
        app.editor.change_id(),
        original_change,
        "Reproduce the two editor lifetimes sharing an operation number"
    );
    app.cancel_size_review();
    assert_eq!(app.editor.profile, original);
    assert!(!app.feedback_current());
    assert!(!app.undo_feedback());
    assert_eq!(app.editor.profile, original);
    app.runtime.shutdown().unwrap();
}
