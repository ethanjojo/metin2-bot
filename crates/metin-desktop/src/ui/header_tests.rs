// Included from tests.rs: exercise the real shell, painter output and pointer events.
const PROFILE_HEADER_HINTS: [&str; 4] = [
    "Undo profile edit (Ctrl+Z)",
    "Redo profile edit (Ctrl+Y)",
    "Revert all unsaved changes to the saved profile. The connected game window stays selected.",
    "Save profile changes. Applying changes pauses automation.",
];

fn header_profile_geometry(ctx: &egui::Context) -> [egui::Rect; 5] {
    let trigger = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::unique("profile-trigger")))
        .unwrap();
    let controls = PROFILE_HEADER_HINTS.map(|hint| compact_control_rect(ctx, hint));
    [trigger, controls[0], controls[1], controls[2], controls[3]]
}

#[test]
fn header_profile_actions_stay_visible_aligned_and_fixed_across_clean_and_dirty_states() {
    let (mut app, ctx, _dir) = setup();
    app.save().unwrap();
    let size = egui::vec2(400., 500.);
    compact_settle(&mut app, &ctx, size);
    let clean = header_profile_geometry(&ctx);
    let header = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::unique("shell-header")))
        .unwrap();
    let gap = clean[0].top() - (header.top() + 4. + 23.);
    assert!((6. ..=8.).contains(&gap), "Profile row must follow the visible title strip without a blank band: {gap}");
    for rect in clean {
        assert_eq!(rect.height(), 28.);
        assert_eq!(rect.top(), clean[0].top());
        assert!(rect.left() >= 12. && rect.right() <= size.x - 12.);
    }
    for pair in clean.windows(2) {
        assert!(pair[0].right() < pair[1].left());
    }
    app.editor.profile.name =
        "An exceptionally long profile name that must truncate without moving actions".into();
    app.editor.record();
    compact_settle(&mut app, &ctx, size);
    assert_eq!(header_profile_geometry(&ctx), clean);
    compact_click_at(&mut app, &ctx, size, clean[1].center());
    assert!(!app.editor.dirty());
    assert!(!egui::Popup::is_any_open(&ctx));
    compact_settle(&mut app, &ctx, size);
    assert_eq!(header_profile_geometry(&ctx), clean);
    compact_click_at(&mut app, &ctx, size, clean[2].center());
    assert!(app.editor.dirty());
    compact_settle(&mut app, &ctx, size);
    compact_click_at(&mut app, &ctx, size, clean[4].center());
    assert!(!app.editor.dirty());
    assert_eq!(
        app.store.load(app.editor.profile.id).unwrap().0,
        app.editor.profile
    );
    compact_settle(&mut app, &ctx, size);
    assert_eq!(header_profile_geometry(&ctx), clean);
    app.runtime.shutdown().unwrap();
}

#[test]
fn header_glyph_controls_keep_pointer_keyboard_focus_and_collapse_actions() {
    let (mut app, ctx, _dir) = setup();
    let size = egui::vec2(400., 500.);
    compact_settle(&mut app, &ctx, size);
    let before = app.pinned;
    let pin = compact_control_rect(
        &ctx,
        if before {
            "Unpin the panel from the selected game window"
        } else {
            "Pin the panel to the selected game window"
        },
    );
    compact_click_at(&mut app, &ctx, size, pin.center());
    assert_eq!(app.pinned, !before);
    // Pointer activation follows egui's normal focus behavior. Tab into the
    // title controls before verifying keyboard activation and stable focus.
    for _ in 0..2 {
        for pressed in [true, false] {
            draw_at_size(
                &mut app,
                &ctx,
                &mut vec![egui::Event::Key {
                    key: egui::Key::Tab,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Default::default(),
                }],
                size,
            );
        }
    }
    for pressed in [true, false] {
        draw_at_size(
            &mut app,
            &ctx,
            &mut vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Default::default(),
            }],
            size,
        );
    }
    assert_eq!(
        app.pinned, before,
        "Pin must keep focus when its tooltip/state changes"
    );
    for pressed in [true, false] {
        draw_at_size(
            &mut app,
            &ctx,
            &mut vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Default::default(),
            }],
            size,
        );
    }
    assert_eq!(
        app.pinned, !before,
        "Changing pin state must retain keyboard focus"
    );
    compact_settle(&mut app, &ctx, size);
    let collapse = compact_control_rect(&ctx, "Collapse panel");
    compact_click_at(&mut app, &ctx, size, collapse.center());
    assert!(app.collapsed);
    let compact = egui::vec2(400., 42.);
    let collapsed = compact_settle(&mut app, &ctx, compact);
    assert_eq!(eframe::App::clear_color(&app, &egui::Visuals::dark()), [0.; 4]);
    assert!(!collapsed.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Rect(rect) if rect.fill.a() > 0 && rect.rect.width() > 100.
    )), "Collapsed mode must paint only the source title artwork, without an opaque window backing");
    let expand = compact_control_rect(&ctx, "Expand panel");
    compact_click_at(&mut app, &ctx, compact, expand.center());
    assert!(!app.collapsed);
    app.runtime.shutdown().unwrap();
}

fn header_toolbar_frame(app: &mut MetinApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400., 100.),
            )),
            events,
            ..Default::default()
        },
        |ui| app.toolbar(ui),
    );
    output.textures_delta.clear();
}

fn header_toolbar_click(app: &mut MetinApp, ctx: &egui::Context, point: egui::Pos2) {
    for pressed in [true, false] {
        header_toolbar_frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}

#[test]
fn header_profile_save_and_revert_cannot_modify_a_size_review_but_undo_redo_edit_the_draft() {
    let (mut app, ctx, _dir) = setup();
    app.save().unwrap();
    let saved = app.editor.profile.clone();
    let [width, height] = saved.frame_size.unwrap();
    let frame = metin_platform::Frame::new(
        width + 10,
        height + 10,
        vec![0; ((width + 10) * (height + 10) * 3) as usize],
    )
    .unwrap();
    app.editor.begin_size_review(frame).unwrap();
    let initial_draft = app.editor.profile.clone();
    app.editor.profile.name = "Unconfirmed review draft".into();
    app.editor.record();
    let edited_draft = app.editor.profile.clone();
    for _ in 0..4 {
        header_toolbar_frame(&mut app, &ctx, vec![]);
    }
    let rects = header_profile_geometry(&ctx);
    for index in [3, 4] {
        header_toolbar_click(&mut app, &ctx, rects[index].center());
        assert!(app.editor.is_recalibrating());
        assert_eq!(
            app.editor.profile, edited_draft,
            "Revert/Save must be disabled during review"
        );
        assert_eq!(app.store.load(saved.id).unwrap().0, saved);
    }
    header_toolbar_click(&mut app, &ctx, rects[1].center());
    assert_eq!(app.editor.profile, initial_draft);
    assert!(app.editor.is_recalibrating());
    header_toolbar_click(&mut app, &ctx, rects[2].center());
    assert_eq!(app.editor.profile, edited_draft);
    assert!(app.editor.is_recalibrating());
    app.editor.cancel_size_review();
    app.runtime.shutdown().unwrap();
}

// Find an actual atlas quad by source UVs, rather than trusting a test-only layout receipt.
fn header_atlas_quads(output: &egui::FullOutput, name: &str) -> Vec<(egui::Rect, egui::Color32)> {
    let manifest: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../assets/metin-ui/manifest.json")).unwrap();
    let source = manifest["regions"][name]["rect"].as_array().unwrap();
    let values = source
        .iter()
        .map(|v| v.as_f64().unwrap() as f32)
        .collect::<Vec<_>>();
    let size = egui::vec2(
        manifest["size"][0].as_f64().unwrap() as f32,
        manifest["size"][1].as_f64().unwrap() as f32,
    );
    let expected = egui::Rect::from_min_size(
        (egui::vec2(values[0], values[1]) / size).to_pos2(),
        egui::vec2(values[2], values[3]) / size,
    );
    fn visit(shape: &egui::Shape, uv: egui::Rect, result: &mut Vec<(egui::Rect, egui::Color32)>) {
        match shape {
            egui::Shape::Mesh(mesh) if mesh.vertices.len() == 4 => {
                let source = egui::Rect::from_points(
                    &mesh.vertices.iter().map(|v| v.uv).collect::<Vec<_>>(),
                );
                if (source.min - uv.min).length() < 0.00001
                    && (source.max - uv.max).length() < 0.00001
                {
                    result.push((mesh.calc_bounds(), mesh.vertices[0].color));
                }
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    visit(shape, uv, result);
                }
            }
            _ => {}
        }
    }
    let mut result = vec![];
    for shape in &output.shapes {
        visit(&shape.shape, expected, &mut result);
    }
    result
}

fn header_atlas_quad(output: &egui::FullOutput, name: &str) -> egui::Rect {
    let quads = header_atlas_quads(output, name);
    assert_eq!(
        quads.len(),
        1,
        "Expected exactly one painted atlas quad {name}"
    );
    quads[0].0
}

#[test]
fn header_title_and_native_controls_use_logical_title_center_and_source_dimensions() {
    let (mut app, ctx, _dir) = setup();
    for pixels_per_point in [1., 1.5, 2.] {
        ctx.set_pixels_per_point(pixels_per_point);
        for (size, collapsed) in [
            (egui::vec2(400., 500.), false),
            (egui::vec2(460., 620.), false),
            (egui::vec2(400., 42.), true),
            (egui::vec2(320., 42.), true),
        ] {
            app.collapsed = collapsed;
            let output = compact_settle(&mut app, &ctx, size);
            let header = ctx
                .data(|d| d.get_temp::<egui::Rect>(egui::Id::unique("shell-header")))
                .unwrap();
            let center_y = header.top() + 4. + 23. / 2.;
            let title = text_regions(&output)
                .into_iter()
                .find(|(text, _)| text == crate::PRODUCT_NAME)
                .unwrap()
                .1;
            let ink = output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == crate::PRODUCT_NAME =>
                    Some(text.galley.mesh_bounds.translate(text.pos.to_vec2())),
                _ => None,
            }).unwrap();
            assert!(
                (ink.center().y - center_y).abs() <= 0.75,
                "Painted letters must be centered in the 23 px strip: {ink:?}, center {center_y}"
            );
            assert!(ink.left() >= header.left() + 22., "Title must clear the source ornament: {ink:?}");
            let controls = [
                "Change the connected game window. Automation pauses before the window chooser opens.",
                if app.pinned { "Unpin the panel from the selected game window" } else { "Pin the panel to the selected game window" },
                if collapsed { "Expand panel" } else { "Collapse panel" },
                "Close Companion",
            ].map(|hint| compact_control_rect(&ctx, hint));
            for rect in controls {
                assert_eq!(rect.size(), egui::vec2(26., 26.));
                assert!((rect.center().y - center_y).abs() <= 0.75);
                assert!(rect.left() > header.left() + 4. && rect.right() < header.right() - 4.);
                assert!(rect.top() >= 0. && rect.bottom() <= size.y);
            }
            for pair in controls.windows(2) {
                assert!(pair[0].right() <= pair[1].left());
            }
            assert!(title.right() < controls[0].left());
            let artwork = header_atlas_quad(&output, "public/minimize_button_01");
            assert_eq!(artwork.size(), egui::vec2(15., 15.));
            assert!((artwork.center().y - center_y).abs() <= 0.75);
            assert!(controls[2].contains_rect(artwork));
            let cap = header_atlas_quad(&output, "pattern/titlebar_right");
            assert_eq!(cap.size(), egui::vec2(32., 32.));
            assert!((controls[3].center().x - (cap.right() - 11.)).abs() < 0.01);
            for source in [
                "public/close_button_01",
                "public/close_button_02",
                "public/close_button_03",
            ] {
                assert!(
                    header_atlas_quads(&output, source).is_empty(),
                    "The title cap already contains X; a public close sprite duplicates it"
                );
            }
        }
    }
    app.runtime.shutdown().unwrap();
}

#[test]
fn header_integrated_close_has_hover_pressed_focus_and_keyboard_activation() {
    let (mut app, ctx, _dir) = setup();
    let size = egui::vec2(400., 500.);
    let output = compact_settle(&mut app, &ctx, size);
    let close = compact_control_rect(&ctx, "Close Companion");
    let normal = header_atlas_quads(&output, "pattern/titlebar_right");
    assert_eq!(normal.len(), 1);
    draw_at_size(
        &mut app,
        &ctx,
        &mut vec![egui::Event::PointerMoved(close.center())],
        size,
    );
    let hover = draw_at_size(&mut app, &ctx, &mut vec![], size);
    let hover = header_atlas_quads(&hover, "pattern/titlebar_right");
    assert_eq!(hover.len(), 1);
    assert_ne!(
        hover[0].1, normal[0].1,
        "Hover must highlight the integrated source close"
    );
    let output = draw_at_size(
        &mut app,
        &ctx,
        &mut vec![egui::Event::PointerButton {
            pos: close.center(),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
        size,
    );
    let pressed = header_atlas_quads(&output, "pattern/titlebar_right");
    assert_eq!(pressed.len(), 1);
    assert_ne!(pressed[0].1, hover[0].1);
    draw_at_size(
        &mut app,
        &ctx,
        &mut vec![
            egui::Event::PointerButton {
                pos: close.center(),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
            egui::Event::PointerGone,
        ],
        size,
    );
    for _ in 0..4 {
        for pressed in [true, false] {
            draw_at_size(
                &mut app,
                &ctx,
                &mut vec![egui::Event::Key {
                    key: egui::Key::Tab,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Default::default(),
                }],
                size,
            );
        }
    }
    let output = draw_at_size(&mut app, &ctx, &mut vec![], size);
    assert!(
        output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Rect(rect) if rect.rect == close && rect.stroke.color == widgets::BRONZE
        )),
        "The fourth title control keeps a visible keyboard focus outline"
    );
    let output = draw_at_size(
        &mut app,
        &ctx,
        &mut vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
        size,
    );
    assert!(output.viewport_output.values().any(|viewport| {
        viewport
            .commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Close))
    }));
    app.runtime.shutdown().unwrap();
}
