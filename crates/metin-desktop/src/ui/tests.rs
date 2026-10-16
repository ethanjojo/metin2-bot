use super::*;
#[test]
fn runtime_mount_is_persisted_only_for_current_acknowledged_revision() {
    let (mut app, _ctx, dir) = setup();
    let target = TargetToken {
        handle: 1,
        pid: 2,
        created: 3,
    };
    app.target = Some(target);
    app.revision = 4;
    app.snapshot = Arc::new(RuntimeSnapshot {
        key: app.key(),
        revision: 4,
        mounted: Some(true),
        ..Default::default()
    });
    app.remember_mount();
    assert!(UISettings::load(dir.path()).unwrap().last_mounted);
    app.revision = 5;
    app.settings.last_mounted = false;
    app.remember_mount();
    assert!(!app.settings.last_mounted);
    app.target = None;
    app.remember_mount();
    assert!(!app.settings.last_mounted);
}
fn draw(app: &mut MetinApp, ctx: &egui::Context, mut events: Vec<egui::Event>) {
    draw_at_size(app, ctx, &mut events, egui::vec2(1280., 820.));
}
fn draw_at_size(
    app: &mut MetinApp,
    ctx: &egui::Context,
    events: &mut Vec<egui::Event>,
    size: egui::Vec2,
) -> egui::FullOutput {
    let modifiers = events
        .iter()
        .rev()
        .find_map(|event| match event {
            egui::Event::Key { modifiers, .. } => Some(*modifiers),
            _ => None,
        })
        .unwrap_or_default();
    events.insert(0, egui::Event::ModifiersChanged(modifiers));
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events: std::mem::take(events),
            ..Default::default()
        },
        |ui| app.render(ui),
    );
    assert!(!output.shapes.is_empty());
    output.textures_delta.clear();
    output
}
fn setup() -> (MetinApp, egui::Context, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let ctx = egui::Context::default();
    let mut app = MetinApp::from_context(&ctx, dir.path().to_owned()).unwrap();
    app.load_fixture(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/parity/frames/search.png")
            .as_path(),
    )
    .unwrap();
    app.navigate(Tab::Calibration);
    (app, ctx, dir)
}
#[test]
fn migrated_template_pixels_and_start_explanation_share_one_readiness_model() {
    let (mut app, _ctx, _dir) = setup();
    let p = Profile::from_json(include_str!(
        "../../../../tests/parity/profiles/calibrated.json"
    ))
    .unwrap();
    app.editor.profile = p;
    app.editor.template = Some(
        metin_vision::decode_png(include_bytes!(
            "../../../../tests/parity/frames/template.png"
        ))
        .unwrap(),
    );
    app.editor.profile.template_region = None;
    assert!(app.calibration_ready(EditMode::Template));
    assert!(app.start_blocker().is_some());
    app.navigate(Tab::Run);
    assert_eq!(app.tab, Tab::Run);
    app.runtime.shutdown().unwrap();
}
#[test]
fn skill_reorder_and_removal_keep_selection_units_and_crop_identity_consistent() {
    let (mut app, _ctx, _dir) = setup();
    for key in ["F1", "F2", "F3"] {
        app.editor.profile.buffs.skills.push(metin_core::BuffSkill {
            shortcut: key.into(),
            ..Default::default()
        });
    }
    app.selected_skill = 1;
    app.units[1] = true;
    app.mode = EditMode::BuffIcon {
        index: 1,
        inactive: false,
    };
    app.reorder_skill(1, 2);
    assert_eq!(app.selected_skill, 2);
    assert_eq!(app.editor.profile.buffs.skills[2].shortcut, "F2");
    assert!(app.units[2]);
    assert!(!matches!(app.mode, EditMode::BuffIcon { .. }));
    app.remove_skill(0);
    assert_eq!(app.selected_skill, 1);
    assert_eq!(app.editor.profile.buffs.skills[1].shortcut, "F2");
    assert!(app.units[1]);
    app.runtime.shutdown().unwrap();
}
#[test]
fn actual_egui_drag_commits_and_keyboard_undo_redo_restores_region() {
    let (mut app, ctx, _dir) = setup();
    app.mode = EditMode::World;
    // Panels measure their text on initial frames; use the settled, rendered
    // canvas when choosing physical pointer positions for this interaction.
    draw(&mut app, &ctx, vec![]);
    draw(&mut app, &ctx, vec![]);
    draw(&mut app, &ctx, vec![]);
    let canvas = app.canvas_rect.unwrap();
    let start = app.viewport.to_canvas([80., 120.]);
    let end = app.viewport.to_canvas([600., 480.]);
    let start = canvas.min + egui::vec2(start[0] as f32, start[1] as f32);
    let end = canvas.min + egui::vec2(end[0] as f32, end[1] as f32);
    let m = egui::Modifiers::default();
    draw(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: m,
            },
        ],
    );
    draw(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(app.editor.profile.region.is_none());
    draw(
        &mut app,
        &ctx,
        vec![egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: m,
        }],
    );
    let region = app
        .editor
        .profile
        .region
        .expect("drag must commit a world region");
    assert!((region.left - 0.1).abs() < 0.003);
    assert!(
        (region.right - 0.75).abs() < 0.003,
        "actual region {region:?}, viewport {:?}",
        app.viewport
    );
    let control = egui::Modifiers {
        ctrl: true,
        ..Default::default()
    };
    draw(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Z,
            physical_key: Some(egui::Key::Z),
            pressed: true,
            repeat: false,
            modifiers: control,
        }],
    );
    assert!(app.editor.profile.region.is_none());
    draw(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Y,
            physical_key: Some(egui::Key::Y),
            pressed: true,
            repeat: false,
            modifiers: control,
        }],
    );
    assert_eq!(app.editor.profile.region, Some(region));
    app.runtime.shutdown().unwrap();
}
#[test]
fn all_tabs_and_both_themes_render_with_complete_buff_controls() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.buffs.skills.push(Default::default());
    for dark in [false, true] {
        ctx.set_visuals(if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        for tab in [Tab::Run, Tab::Calibration, Tab::Buffs, Tab::Settings] {
            app.tab = tab;
            draw(&mut app, &ctx, vec![]);
        }
    }
    app.runtime.shutdown().unwrap();
}

#[test]
fn minimum_window_and_simulated_hidpi_render_all_tabs_with_eight_skills() {
    let (mut app, ctx, _dir) = setup();
    for shortcut in ["F1", "F2", "F3", "F4", "F5", "F6", "F9", "F10"] {
        let skill = metin_core::BuffSkill {
            shortcut: shortcut.into(),
            name: "Dlouhý název dovednosti".into(),
            ..Default::default()
        };
        app.editor.profile.buffs.skills.push(skill);
    }
    for scale in [1., 1.25, 1.5, 2.] {
        ctx.set_pixels_per_point(scale);
        for dark in [false, true] {
            ctx.set_visuals(if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
            for tab in [Tab::Run, Tab::Calibration, Tab::Buffs, Tab::Settings] {
                app.tab = tab;
                draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(800., 560.));
            }
        }
    }
    app.runtime.shutdown().unwrap();
}

#[test]
fn escape_cancels_pointer_gesture_without_a_history_commit() {
    let (mut app, ctx, _dir) = setup();
    app.mode = EditMode::World;
    for _ in 0..3 {
        draw(&mut app, &ctx, vec![]);
    }
    let canvas = app.canvas_rect.unwrap();
    let start = canvas.min + egui::vec2(70., 120.);
    let end = canvas.min + egui::vec2(300., 300.);
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    draw(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    draw(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(app.gesture.is_some());
    let before = app.editor.profile.clone();
    draw(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: Some(egui::Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    draw(&mut app, &ctx, vec![button(end, false)]);
    assert!(app.gesture.is_none());
    assert_eq!(app.editor.profile, before);
    assert!(!app.editor.can_undo());
    app.runtime.shutdown().unwrap();
}

fn text_regions(output: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    fn visit(shape: &egui::Shape, regions: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(text) => regions.push((
                text.galley.job.text.clone(),
                egui::Rect::from_min_size(text.pos, text.galley.size()),
            )),
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    visit(shape, regions);
                }
            }
            _ => {}
        }
    }
    let mut regions = vec![];
    for shape in &output.shapes {
        let mut painted = vec![];
        visit(&shape.shape, &mut painted);
        regions.extend(
            painted
                .into_iter()
                .filter(|(_, rect)| shape.clip_rect.contains(rect.center())),
        );
    }
    regions
}
fn click_label(app: &mut MetinApp, ctx: &egui::Context, label: &str) {
    let mut output = draw_at_size(app, ctx, &mut vec![], egui::vec2(1280., 820.));
    for _ in 0..2 {
        output = draw_at_size(app, ctx, &mut vec![], egui::vec2(1280., 820.));
    }
    let pos = text_regions(&output)
        .into_iter()
        .find(|(text, _)| text == label)
        .unwrap_or_else(|| panic!("Missing visible control: {label}"))
        .1
        .center();
    for pressed in [true, false] {
        draw(
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
        );
    }
}
#[test]
fn pointer_navigation_preserves_dirty_profile_and_only_calibration_has_editable_canvas() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.name = "Edited profile".into();
    app.editor.record();
    for (label, tab) in [
        ("Buffs", Tab::Buffs),
        ("Run", Tab::Run),
        ("Settings", Tab::Settings),
        ("Calibration", Tab::Calibration),
    ] {
        click_label(&mut app, &ctx, label);
        assert!(app.tab == tab, "Navigation failed: {label}");
        assert!(app.editor.dirty());
        assert_eq!(app.editor.profile.name, "Edited profile");
        assert_eq!(app.canvas_rect.is_some(), tab == Tab::Calibration);
    }
    app.runtime.shutdown().unwrap();
}
#[test]
fn skill_buttons_move_the_selected_skill_and_keep_detail_identity() {
    let (mut app, ctx, _dir) = setup();
    for key in ["F1", "F2"] {
        app.editor.profile.buffs.skills.push(metin_core::BuffSkill {
            shortcut: key.into(),
            ..Default::default()
        });
    }
    app.navigate(Tab::Buffs);
    click_label(&mut app, &ctx, "Move down");
    assert_eq!(app.selected_skill, 1);
    assert_eq!(app.editor.profile.buffs.skills[1].shortcut, "F1");
    click_label(&mut app, &ctx, "Remove");
    assert_eq!(app.editor.profile.buffs.skills.len(), 1);
    assert_eq!(app.editor.profile.buffs.skills[0].shortcut, "F2");
    app.runtime.shutdown().unwrap();
}
#[test]
fn settings_rendering_does_not_modify_profile_and_vector_buttons_have_hover_help() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile = Profile::from_json(include_str!(
        "../../../../tests/parity/profiles/calibrated.json"
    ))
    .unwrap();
    let before = app.editor.profile.clone();
    app.navigate(Tab::Settings);
    for _ in 0..4 {
        draw(&mut app, &ctx, vec![]);
    }
    assert_eq!(app.editor.profile, before);
    let hover_ctx = egui::Context::default();
    let mut hovered = false;
    let mut tooltip_visible = false;
    for frame in 0..4 {
        let mut output = hover_ctx.run_ui(
            egui::RawInput {
                time: Some(f64::from(frame)),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 560.),
                )),
                events: if frame == 0 {
                    vec![egui::Event::PointerMoved(egui::pos2(20., 20.))]
                } else {
                    vec![]
                },
                ..Default::default()
            },
            |ui| {
                let r = widgets::button(
                    ui,
                    widgets::Icon::Up,
                    "Move up",
                    true,
                    "Move this skill earlier.",
                );
                hovered = r.hovered();
            },
        );
        tooltip_visible |= text_regions(&output)
            .iter()
            .any(|(text, _)| text == "Move this skill earlier.");
        output.textures_delta.clear();
    }
    assert!(hovered, "Icon buttons must expose a real pointer hit area");
    assert!(
        tooltip_visible,
        "Hovering a vector button must render its explanatory tooltip"
    );
    app.runtime.shutdown().unwrap();
}
#[test]
fn typing_and_text_undo_do_not_undo_the_profile_or_remove_a_skill() {
    let (mut app, ctx, _dir) = setup();
    app.add_skill();
    app.editor.profile.buffs.skills[0].name = "Aura".into();
    app.editor.record();
    app.navigate(Tab::Buffs);
    click_label(&mut app, &ctx, "Aura");
    draw(&mut app, &ctx, vec![egui::Event::Text("x".into())]);
    assert_ne!(app.editor.profile.buffs.skills[0].name, "Aura");
    draw(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Z,
            physical_key: Some(egui::Key::Z),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                ..Default::default()
            },
        }],
    );
    assert_eq!(app.editor.profile.buffs.skills.len(), 1);
    assert_eq!(app.editor.profile.buffs.skills[0].shortcut, "F1");
    app.runtime.shutdown().unwrap();
}
#[cfg(feature = "ui-smoke")]
#[test]
fn screenshot_request_survives_a_discarded_layout_pass() {
    let (mut app, ctx, dir) = setup();
    app.smoke = Some((
        dir.path().join("test.png"),
        std::time::Instant::now() - std::time::Duration::from_secs(1),
    ));
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280., 820.),
            )),
            ..Default::default()
        },
        |ui| {
            if ui.ctx().current_pass_index() == 0 {
                ui.ctx()
                    .request_discard("Exercise multi-pass screenshot capture");
            }
            app.render(ui);
        },
    );
    let requests = output
        .viewport_output
        .values()
        .flat_map(|v| &v.commands)
        .filter(|cmd| matches!(cmd, egui::ViewportCommand::Screenshot(_)))
        .count();
    output.textures_delta.clear();
    assert_eq!(
        requests, 1,
        "The final frame must retain exactly one screenshot request"
    );
    assert!(
        app.smoke.is_some(),
        "Keep requesting redraws while the GPU screenshot callback is pending"
    );
    app.runtime.shutdown().unwrap();
}
#[test]
fn save_readiness_normalizes_blank_rows_and_new_profiles_can_be_saved() {
    let (mut app, _ctx, _dir) = setup();
    assert!(app.has_unsaved_profile());
    app.editor.profile.buffs.skills.push(metin_core::BuffSkill {
        shortcut: String::new(),
        ..Default::default()
    });
    assert!(app.profile_for_save().buffs.skills.is_empty());
    app.runtime.shutdown().unwrap();
}
#[test]
fn start_readiness_routes_to_window_name_and_buff_fixes_before_allowing_start() {
    let (mut app, ctx, _dir) = setup();
    let capabilities = metin_platform::CapabilityReport {
        capture: true,
        identity: true,
        foreground: true,
        obstruction: true,
        manual_input: true,
        input: true,
        emergency_stop: true,
        reason: String::new(),
    };
    app.snapshot = Arc::new(RuntimeSnapshot {
        capabilities,
        ..Default::default()
    });
    app.editor.profile = Profile::from_json(include_str!(
        "../../../../tests/parity/profiles/calibrated.json"
    ))
    .unwrap();
    app.editor.template = Some(
        metin_vision::decode_png(include_bytes!(
            "../../../../tests/parity/frames/template.png"
        ))
        .unwrap(),
    );
    assert!(app.start_blocker().unwrap().message.contains("game window"));
    app.target = Some(TargetToken {
        handle: 1,
        pid: 2,
        created: 3,
    });
    assert!(app.start_blocker().is_none());
    let mut running = (*app.snapshot).clone();
    running.running = true;
    running.state = "Attacking".into();
    app.snapshot = Arc::new(running);
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280., 820.),
            )),
            ..Default::default()
        },
        |ui| app.run_view(ui),
    );
    let ready_to_start = text_regions(&output)
        .iter()
        .any(|(text, _)| text == "Ready to start");
    output.textures_delta.clear();
    assert!(
        !ready_to_start,
        "A running session must not tell the user to start again"
    );
    app.editor.profile.name.clear();
    assert!(app.start_blocker().unwrap().tab == Tab::Calibration);
    app.editor.profile.name = "Valid".into();
    app.editor.profile.buffs.enabled = true;
    app.editor.profile.buffs.skills.push(metin_core::BuffSkill {
        mode: metin_core::BuffMode::Inactive,
        interval_seconds: None,
        ..Default::default()
    });
    assert!(app.start_blocker().unwrap().tab == Tab::Buffs);
    app.runtime.shutdown().unwrap();
}
#[test]
fn small_calibration_canvas_and_instructions_stay_inside_the_window() {
    let (mut app, ctx, _dir) = setup();
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(800., 560.));
    for _ in 0..3 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(800., 560.));
    }
    assert!(
        app.canvas_rect.unwrap().right() <= 800.,
        "Canvas extends outside small window: {:?}",
        app.canvas_rect
    );
    for (text, rect) in text_regions(&output) {
        if text.starts_with("Drag to draw or move;") {
            assert!(
                rect.right() <= 800.,
                "Instructions extend outside window: {rect:?}"
            );
        }
    }
    app.runtime.shutdown().unwrap();
}
#[test]
fn save_compaction_preserves_selected_skill_crop_mode_and_return_route() {
    let (mut app, _ctx, _dir) = setup();
    for key in ["", "F1", "F2"] {
        app.editor.profile.buffs.skills.push(metin_core::BuffSkill {
            shortcut: key.into(),
            ..Default::default()
        });
    }
    app.selected_skill = 1;
    app.units[1] = true;
    app.edit_area(EditMode::BuffIcon {
        index: 1,
        inactive: true,
    });
    app.save().unwrap();
    assert_eq!(app.selected_skill, 0);
    assert_eq!(app.editor.profile.buffs.skills[0].shortcut, "F1");
    assert!(app.units[0]);
    assert!(
        app.mode
            == EditMode::BuffIcon {
                index: 0,
                inactive: true
            }
    );
    assert!(app.return_to_buffs);
    app.runtime.shutdown().unwrap();
}
#[test]
fn switching_profiles_retires_the_previous_skill_crop_mode() {
    let (mut app, _ctx, _dir) = setup();
    let other = app.store.save(&Profile::create("Other"), None).unwrap();
    app.editor.profile.buffs.skills.push(Default::default());
    app.save().unwrap();
    app.edit_area(EditMode::BuffIcon {
        index: 0,
        inactive: false,
    });
    app.switch_profile(other.id);
    assert!(app.mode == EditMode::World);
    assert!(!app.return_to_buffs);
    assert!(app.editor.profile.buffs.skills.is_empty());
    app.runtime.shutdown().unwrap();
}
#[test]
fn process_hint_matches_filenames_without_case_or_substring_surprises() {
    assert!(state::matches_process(
        "VerzumClient_X64.EXE",
        "verzumclient_x64.exe"
    ));
    assert!(!state::matches_process(
        "other-verzumclient_x64.exe",
        "verzumclient_x64.exe"
    ));
    assert!(!state::matches_process(
        "notepad.exe",
        "verzumclient_x64.exe"
    ));
}
#[test]
fn long_profile_names_keep_pause_and_navigation_visible_in_small_window() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.name =
        "Very long calibration profile name for a warrior in the forest area".into();
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(800., 560.));
    for _ in 0..3 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(800., 560.));
    }
    for label in ["Pause · F8", "Run", "Calibration", "Buffs", "Settings"] {
        let (_, rect) = text_regions(&output)
            .into_iter()
            .find(|(text, _)| text == label)
            .unwrap_or_else(|| panic!("Missing control: {label}"));
        assert!(
            rect.right() <= 800. && rect.bottom() <= 560.,
            "{label} lies outside the small window: {rect:?}"
        );
    }
    app.runtime.shutdown().unwrap();
}
#[test]
fn scrolling_buff_detail_reaches_icon_capture_and_returns_to_the_same_skill() {
    let (mut app, ctx, _dir) = setup();
    app.add_skill();
    app.navigate(Tab::Buffs);
    for _ in 0..3 {
        draw(&mut app, &ctx, vec![]);
    }
    draw(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(egui::pos2(600., 600.)),
            egui::Event::MouseWheel {
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Unknown,
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., -1800.),
                modifiers: Default::default(),
            },
        ],
    );
    for _ in 0..20 {
        draw(&mut app, &ctx, vec![]);
    }
    click_label(&mut app, &ctx, "Capture");
    assert!(app.tab == Tab::Calibration);
    assert!(app.return_to_buffs);
    assert!(matches!(app.mode, EditMode::BuffIcon { index: 0, .. }));
    click_label(&mut app, &ctx, "Back to buffs");
    assert!(app.tab == Tab::Buffs);
    assert_eq!(app.selected_skill, 0);
    app.runtime.shutdown().unwrap();
}
#[test]
fn wide_run_overview_keeps_window_connection_visible_without_scrolling() {
    let (mut app, ctx, _dir) = setup();
    app.navigate(Tab::Run);
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    for _ in 0..3 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    }
    let connection = text_regions(&output)
        .into_iter()
        .find(|(text, _)| text == "Game window");
    assert!(
        connection.is_some_and(|(_, rect)| rect.bottom() < 740.),
        "Window connection should be visible in a wide Run overview"
    );
    app.runtime.shutdown().unwrap();
}
