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
) {
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
    (app, ctx, dir)
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
        for tab in [Tab::Calibration, Tab::Buffs, Tab::Settings] {
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
            for tab in [Tab::Calibration, Tab::Buffs, Tab::Settings] {
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
