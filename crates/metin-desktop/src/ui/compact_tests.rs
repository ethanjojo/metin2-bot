// Included from tests.rs to reuse the real UI fixture and pointer helpers.
#[test]
fn full_history_wraps_long_messages_and_keeps_setup_links_inside_the_small_panel() {
    let (mut app, ctx, _dir) = setup();
    let message = "Long event message with a concrete reason and a useful next step. ".repeat(4);
    let snapshot = Arc::new(RuntimeSnapshot {
        events: vec![metin_core::RunEvent {
            activity: "A deliberately long activity heading".repeat(3),
            message: message.clone(),
            destination: metin_core::EventDestination::Loot,
            ..Default::default()
        }],
        ..Default::default()
    });
    app.window_fixture = Some(window_selection::WindowFixture { snapshot });
    app.tab = Tab::Run;
    app.history_open = true;
    for size in [egui::vec2(400., 500.), egui::vec2(460., 620.)] {
        let output = compact_settle(&mut app, &ctx, size);
        let texts = text_regions(&output);
        let (_, body) = texts.iter().find(|(text, _)| text == &message).unwrap();
        assert!(
            body.height() > 24. && body.left() >= 0. && body.right() < size.x,
            "Full messages must wrap within the panel: {body:?}"
        );
        let setup = compact_control_rect(&ctx, "Open setup");
        assert!(
            setup.left() >= 0. && setup.right() < size.x,
            "History action must remain visible: {setup:?}"
        );
    }
    app.runtime.shutdown().unwrap();
}

fn long_select_frame(
    ctx: &egui::Context,
    selected: &mut usize,
    events: Vec<egui::Event>,
) -> (egui::Rect, Option<(egui::Rect, egui::Rect)>) {
    let mut trigger = egui::Rect::NOTHING;
    let mut focused = None;
    let mut output = ctx.run_ui(
        egui::RawInput {
            time: Some(ctx.input(|i| i.time) + 0.1),
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400., 400.),
            )),
            ..Default::default()
        },
        |ui| {
            trigger = widgets::ComboBox::from_id_salt("long-select")
                .width(280.)
                .selected_text(format!("Choice {selected}"))
                .show_ui(ui, |ui| {
                    for i in 0..25 {
                        let response = select::option(ui, selected, i, format!("Choice {i}"));
                        if response.has_focus() {
                            focused = Some((response.rect, ui.clip_rect()));
                        }
                    }
                })
                .response
                .rect;
        },
    );
    output.textures_delta.clear();
    (trigger, focused)
}
fn select_click(ctx: &egui::Context, selected: &mut usize, point: egui::Pos2) {
    for pressed in [true, false] {
        long_select_frame(
            ctx,
            selected,
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
    for _ in 0..4 {
        long_select_frame(ctx, selected, vec![]);
    }
}
#[test]
fn metin_select_scrollbar_click_drag_and_keyboard_do_not_accept_an_option() {
    let ctx = egui::Context::default();
    widgets::theme(&ctx);
    let mut selected = 0;
    let (trigger, _) = long_select_frame(&ctx, &mut selected, vec![]);
    select_click(&ctx, &mut selected, trigger.center());
    let geometry = || {
        ctx.data(|d| {
            d.get_temp::<(egui::Rect, egui::Rect, egui::Rect, f32, egui::Id)>(egui::Id::unique(
                "select-scroll-geometry",
            ))
        })
        .unwrap()
    };
    let (_, down, thumb, offset, id) = geometry();
    select_click(&ctx, &mut selected, thumb.center());
    assert!(
        (geometry().3 - offset).abs() < 0.1,
        "A click on the thumb must not page down"
    );
    select_click(&ctx, &mut selected, down.center());
    let before_keyboard = geometry().3;
    assert!(before_keyboard > offset);
    ctx.memory_mut(|m| m.request_focus(id.with("down")));
    long_select_frame(&ctx, &mut selected, vec![]);
    for pressed in [true, false] {
        long_select_frame(
            &ctx,
            &mut selected,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
    }
    for _ in 0..4 {
        long_select_frame(&ctx, &mut selected, vec![]);
    }
    assert!(
        egui::Popup::is_any_open(&ctx),
        "Activating a scroll arrow must leave the select open"
    );
    assert!(geometry().3 > before_keyboard);
    let thumb = geometry().2;
    long_select_frame(
        &ctx,
        &mut selected,
        vec![
            egui::Event::PointerMoved(thumb.center()),
            egui::Event::PointerButton {
                pos: thumb.center(),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    long_select_frame(
        &ctx,
        &mut selected,
        vec![egui::Event::PointerMoved(
            thumb.center() + egui::vec2(0., 12.),
        )],
    );
    long_select_frame(
        &ctx,
        &mut selected,
        vec![egui::Event::PointerButton {
            pos: thumb.center() + egui::vec2(0., 12.),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    for _ in 0..4 {
        long_select_frame(&ctx, &mut selected, vec![]);
    }
    assert!(
        geometry().3 > before_keyboard + 28.,
        "Dragging the native thumb changes the canonical scroll offset"
    );
    assert_eq!(selected, 0);
}
#[test]
fn metin_select_keyboard_scrolls_a_long_list_to_the_focused_option() {
    let ctx = egui::Context::default();
    widgets::theme(&ctx);
    let mut selected = 0;
    let (trigger, _) = long_select_frame(&ctx, &mut selected, vec![]);
    select_click(&ctx, &mut selected, trigger.center());
    for _ in 0..17 {
        for pressed in [true, false] {
            long_select_frame(
                &ctx,
                &mut selected,
                vec![egui::Event::Key {
                    key: egui::Key::ArrowDown,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Default::default(),
                }],
            );
        }
    }
    let mut focused = None;
    for _ in 0..10 {
        focused = long_select_frame(&ctx, &mut selected, vec![]).1;
    }
    let (row, clip) = focused.expect("keyboard focuses a real row");
    assert!(
        row.top() >= clip.top() - 1. && row.bottom() <= clip.bottom() + 1.,
        "Focused option must be visible: {row:?}, clip {clip:?}"
    );
    long_select_frame(
        &ctx,
        &mut selected,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    assert!(selected > 10);
    assert!(!egui::Popup::is_any_open(&ctx));
}

#[test]
fn profile_menu_keyboard_keeps_a_long_list_visible_and_accepts_the_focused_profile() {
    let (mut app, ctx, _dir) = setup();
    let profiles = (0..25)
        .map(|i| Profile::create(format!("Profile {i:02}")))
        .collect::<Vec<_>>();
    for profile in &profiles {
        app.store.save(profile, None).unwrap();
    }
    app.profiles = profiles.clone();
    app.editor = Editor::new(profiles[0].clone(), None);
    let size = egui::vec2(460., 620.);
    compact_settle(&mut app, &ctx, size);
    let trigger: egui::Rect = ctx
        .data(|d| d.get_temp(egui::Id::unique("profile-trigger")))
        .unwrap();
    for pressed in [true, false] {
        draw_at_size(
            &mut app,
            &ctx,
            &mut vec![
                egui::Event::PointerMoved(trigger.center()),
                egui::Event::PointerButton {
                    pos: trigger.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
            size,
        );
    }
    compact_settle(&mut app, &ctx, size);
    let row = |name: &str| {
        ctx.data(|d| {
            d.get_temp::<(egui::Id, egui::Rect, egui::Rect)>(egui::Id::unique(("select-row", name)))
        })
        .unwrap()
    };
    let first_row = row("Profile 00").0;
    ctx.memory_mut(|m| m.request_focus(first_row));
    for _ in 0..16 {
        for pressed in [true, false] {
            draw_at_size(
                &mut app,
                &ctx,
                &mut vec![egui::Event::Key {
                    key: egui::Key::ArrowDown,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Default::default(),
                }],
                size,
            );
        }
    }
    for _ in 0..16 {
        draw_at_size(&mut app, &ctx, &mut vec![], size);
    }
    let (id, rect, clip) = row("Profile 16");
    assert_eq!(
        ctx.memory(|m| m.focused()),
        Some(id),
        "Actual row focus: {:?}",
        profiles
            .iter()
            .find(|p| Some(row(&p.name).0) == ctx.memory(|m| m.focused()))
            .map(|p| &p.name)
    );
    assert!(
        rect.top() >= clip.top() - 1. && rect.bottom() <= clip.bottom() + 1.,
        "Focused profile must scroll into view"
    );
    draw_at_size(
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
    assert_eq!(app.editor.profile.id, profiles[16].id);
    assert!(!egui::Popup::is_any_open(&ctx));
    app.runtime.shutdown().unwrap();
}

#[test]
fn active_buff_artwork_rejects_other_sessions_revisions_and_draft_order() {
    let (mut app, _ctx, _dir) = setup();
    app.editor.profile.buffs.skills = vec![metin_core::BuffSkill::default(); 2];
    app.editor.mark_saved(app.editor.profile.clone());
    app.target = Some(window_catalog()[0].token);
    app.snapshot = Arc::new(RuntimeSnapshot {
        key: app.key(),
        revision: app.revision,
        applied_revision: Some(app.revision),
        activity: metin_core::ActivityKind::Buffing,
        active_skill: Some(1),
        ..Default::default()
    });
    assert_eq!(app.active_skill_presentation(), Some(1));
    app.editor.profile.buffs.skills.swap(0, 1);
    app.editor.profile.buffs.skills[0].name = "Unsaved order".into();
    assert_eq!(app.active_skill_presentation(), None);
    app.editor.revert();
    app.revision += 1;
    assert_eq!(app.active_skill_presentation(), None);
    app.revision -= 1;
    app.editor.profile.id = uuid::Uuid::new_v4();
    app.editor.mark_saved(app.editor.profile.clone());
    assert_eq!(app.active_skill_presentation(), None);
    app.runtime.shutdown().unwrap();
}

#[test]
fn buff_icon_picker_changes_class_and_display_without_touching_recognition_or_mode() {
    let (mut app, ctx, _dir) = setup();
    app.add_skill();
    app.editor.profile.buffs.skills[0].display_icon = metin_core::SkillIconId::from_id(4);
    let before = app.editor.profile.buffs.skills[0].clone();
    app.navigate(Tab::Actions);
    app.actions_detail = true;
    app.buff_detail = true;
    let size = egui::vec2(460., 620.);
    compact_click(&mut app, &ctx, size, "Aura of the Sword");
    compact_click(&mut app, &ctx, size, "Sura");
    compact_settle(&mut app, &ctx, size);
    let rect: egui::Rect = ctx
        .data(|d| d.get_temp(egui::Id::unique(("skill-icon", 61u16))))
        .unwrap();
    for pressed in [true, false] {
        draw_at_size(
            &mut app,
            &ctx,
            &mut vec![
                egui::Event::PointerMoved(rect.center()),
                egui::Event::PointerButton {
                    pos: rect.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
            size,
        );
    }
    let mut expected = before.clone();
    expected.display_icon = metin_core::SkillIconId::from_id(61);
    assert_eq!(app.editor.profile.buffs.skills[0], expected);
    assert!(!egui::Popup::is_any_open(&ctx));
    compact_click(&mut app, &ctx, size, "Finger Strike");
    draw_at_size(
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
    assert_eq!(
        app.editor.profile.buffs.skills[0], expected,
        "Escape cancels selection without changing the previous icon"
    );
    compact_click(&mut app, &ctx, size, "Finger Strike");
    compact_click(&mut app, &ctx, size, "Automatic icon");
    expected.display_icon = None;
    assert_eq!(app.editor.profile.buffs.skills[0], expected);
    app.runtime.shutdown().unwrap();
}

fn compact_settle(app: &mut MetinApp, ctx: &egui::Context, size: egui::Vec2) -> egui::FullOutput {
    let mut output = draw_at_size(app, ctx, &mut vec![], size);
    for _ in 0..3 {
        output = draw_at_size(app, ctx, &mut vec![], size);
    }
    output
}

#[test]
fn metin_select_keeps_keyboard_menu_navigation_and_escape() {
    let ctx = egui::Context::default();
    widgets::theme(&ctx);
    let mut selected = 0;
    let mut trigger = egui::Rect::NOTHING;
    let mut draw = |events: Vec<egui::Event>| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400., 300.),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                trigger = widgets::ComboBox::from_id_salt("keyboard-menu")
                    .selected_text(if selected == 0 { "One" } else { "Two" })
                    .show_ui(ui, |ui| {
                        select::option(ui, &mut selected, 0, "One");
                        select::option(ui, &mut selected, 1, "Two");
                    })
                    .response
                    .rect;
            },
        );
        output.textures_delta.clear();
        trigger
    };
    let rect = draw(vec![]);
    for pressed in [true, false] {
        draw(vec![
            egui::Event::PointerMoved(rect.center()),
            egui::Event::PointerButton {
                pos: rect.center(),
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            },
        ]);
    }
    for key in [egui::Key::ArrowDown, egui::Key::ArrowDown, egui::Key::Enter] {
        draw(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }]);
        draw(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Default::default(),
        }]);
    }
    assert!(
        !egui::Popup::is_any_open(&ctx),
        "Enter closes an accepted menu"
    );
    for pressed in [true, false] {
        draw(vec![
            egui::Event::PointerMoved(rect.center()),
            egui::Event::PointerButton {
                pos: rect.center(),
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            },
        ]);
    }
    draw(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]);
    drop(draw);
    assert_eq!(
        selected, 1,
        "Arrow keys and Enter activate a real menu option"
    );
    assert!(!egui::Popup::is_any_open(&ctx));
}

#[test]
fn missing_profile_name_opens_the_canonical_rename_editor_without_losing_calibration() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.name.clear();
    let before = app.editor.profile.clone();
    app.open_setup(SetupAction::Calibration);
    let output = compact_settle(&mut app, &ctx, egui::vec2(460., 620.));
    assert!(app.profile_rename.is_some());
    assert!(
        text_regions(&output)
            .iter()
            .any(|(text, _)| text == "Profile name")
    );
    assert_eq!(app.editor.profile, before);
    app.runtime.shutdown().unwrap();
}

#[test]
fn compact_chrome_fills_header_and_keeps_footer_clear_of_window_frame() {
    let (mut app, ctx, _dir) = setup();
    {
        widgets::theme(&ctx);
        for size in [
            egui::vec2(400., 480.),
            egui::vec2(460., 620.),
            egui::vec2(800., 700.),
        ] {
            compact_settle(&mut app, &ctx, size);
            let header: egui::Rect = ctx
                .data(|data| data.get_temp(egui::Id::unique("shell-header")))
                .unwrap();
            assert!(
                header.left().abs() < 0.1 && (header.right() - size.x).abs() < 0.1,
                "Header must span the viewport: {header:?} / {size:?}"
            );
            let close = compact_control_rect(&ctx, "Close Companion");
            assert!(
                (size.x - close.right() - 8.).abs() < 1.,
                "Title controls align to the trailing inset: {close:?}"
            );
            let primary: egui::Rect = ctx
                .data(|data| data.get_temp(egui::Id::unique("shell-primary")))
                .unwrap();
            assert!(
                size.y - primary.bottom() >= 9. && primary.left() >= 9.,
                "Start must clear the outer frame: {primary:?} / {size:?}"
            );
            let footer: egui::Rect = ctx
                .data(|d| d.get_temp(egui::Id::unique("shell-footer")))
                .unwrap();
            assert!(
                primary.top() - footer.top() <= 6.,
                "Idle footer must not reserve an empty message row: {primary:?}, {footer:?}"
            );
            app.show_feedback("Saved", false);
            compact_settle(&mut app, &ctx, size);
            let with_notice: egui::Rect = ctx
                .data(|d| d.get_temp(egui::Id::unique("shell-primary")))
                .unwrap();
            assert_eq!(
                with_notice, primary,
                "Messages must not move Start or the mount control"
            );
            app.feedback = None;
        }
        app.collapsed = true;
        compact_settle(&mut app, &ctx, egui::vec2(320., 42.));
        let close = compact_control_rect(&ctx, "Close Companion");
        assert!(close.right() <= 312.1 && close.bottom() < 42.);
        app.collapsed = false;
    }
    app.runtime.shutdown().unwrap();
}

#[test]
fn compact_stat_values_and_labels_have_separate_vertical_space() {
    let (mut app, ctx, _dir) = setup();
    app.tab = Tab::Run;
    {
        widgets::theme(&ctx);
        let output = compact_settle(&mut app, &ctx, egui::vec2(460., 620.));
        let texts = text_regions(&output);
        for label in ["Active time", "Targets ended", "Clicks", "Unconfirmed"] {
            let (_, caption) = texts.iter().find(|(s, _)| s == label).unwrap();
            let (_, value) = texts
                .iter()
                .find(|(s, r)| {
                    (s == "0" || s == "00:00:00")
                        && (r.left() - caption.left()).abs() < 1.
                        && r.top() < caption.top()
                        && r.bottom() > caption.top() - 40.
                })
                .unwrap();
            assert!(
                value.bottom() <= caption.top(),
                "Stat value overlaps {label}: {value:?} / {caption:?}"
            );
        }
    }
    app.runtime.shutdown().unwrap();
}
fn compact_click(app: &mut MetinApp, ctx: &egui::Context, size: egui::Vec2, label: &str) {
    let mut output = compact_settle(app, ctx, size);
    if label == "History" && !text_regions(&output).iter().any(|(text, _)| text == label) {
        draw_at_size(
            app,
            ctx,
            &mut vec![
                egui::Event::PointerMoved(egui::pos2(size.x / 2., size.y / 2.)),
                egui::Event::MouseWheel {
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0., -1200.),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            size,
        );
        output = compact_settle(app, ctx, size);
    }
    let point = text_regions(&output)
        .into_iter()
        .find(|(text, _)| text == label)
        .unwrap_or_else(|| panic!("Missing compact control {label}"))
        .1
        .center();
    for pressed in [true, false] {
        draw_at_size(
            app,
            ctx,
            &mut vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
            size,
        );
    }
}
fn compact_control_rect(ctx: &egui::Context, hint: &str) -> egui::Rect {
    ctx.data(|data| data.get_temp(egui::Id::unique(("control", hint))))
        .unwrap_or_else(|| panic!("Missing accessible icon control: {hint}"))
}
fn compact_click_at(app: &mut MetinApp, ctx: &egui::Context, size: egui::Vec2, point: egui::Pos2) {
    for pressed in [true, false] {
        draw_at_size(
            app,
            ctx,
            &mut vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
            size,
        );
    }
}
#[test]
fn compact_footer_and_all_tabs_stay_visible_at_narrow_size() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.name =
        "An exceptionally long profile name that must not displace safety controls".into();
    {
        widgets::theme(&ctx);
        for size in [egui::vec2(400., 500.), egui::vec2(460., 620.)] {
            for tab in [Tab::Run, Tab::Actions, Tab::Safety, Tab::Settings] {
                app.navigate(tab);
                let output = compact_settle(&mut app, &ctx, size);
                let text = text_regions(&output);
                for label in [
                    "Run",
                    "Actions",
                    "Recognition",
                    "Safety",
                    "Settings",
                    "Save & Start",
                ] {
                    let rect = text
                        .iter()
                        .find(|(value, _)| value == label)
                        .unwrap_or_else(|| panic!("Missing {label}"))
                        .1;
                    assert!(
                        rect.left() >= 0.
                            && rect.top() >= 0.
                            && rect.right() <= size.x
                            && rect.bottom() <= size.y,
                        "{label} outside {size:?}: {rect:?}"
                    );
                }
            }
        }
    }
    app.runtime.shutdown().unwrap();
}
#[test]
fn compact_actions_list_and_skill_details_preserve_profile_through_back_navigation() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.buffs.skills.clear();
    app.editor.profile.buffs.skills.push(metin_core::BuffSkill {
        name: "Aura".into(),
        shortcut: "F1".into(),
        ..Default::default()
    });
    let before = app.editor.profile.clone();
    app.navigate(Tab::Actions);
    app.actions_detail = false;
    let size = egui::vec2(400., 500.);
    compact_click(&mut app, &ctx, size, "Buffs");
    assert!(app.actions_detail && !app.buff_detail);
    compact_click(&mut app, &ctx, size, "1. Aura");
    assert!(app.buff_detail);
    let output = compact_settle(&mut app, &ctx, size);
    assert!(text_regions(&output).iter().any(|(text, _)| text == "Aura"));
    compact_click(&mut app, &ctx, size, "Buffs");
    assert!(!app.buff_detail);
    // The back row is in the content panel; the tab carries the same label.
    let output = compact_settle(&mut app, &ctx, size);
    let point = text_regions(&output)
        .into_iter()
        .filter(|(text, _)| text == "Actions")
        .max_by(|a, b| a.1.top().total_cmp(&b.1.top()))
        .unwrap()
        .1
        .center();
    for pressed in [true, false] {
        draw_at_size(
            &mut app,
            &ctx,
            &mut vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
            size,
        );
    }
    assert!(!app.actions_detail);
    assert_eq!(app.editor.profile, before);
    app.runtime.shutdown().unwrap();
}
#[test]
fn compact_history_stays_inside_run_and_escape_returns_without_changing_profile() {
    let (mut app, ctx, _dir) = setup();
    app.navigate(Tab::Run);
    let before = app.editor.profile.clone();
    let size = egui::vec2(400., 500.);
    compact_click(&mut app, &ctx, size, "History");
    assert!(app.history_open);
    let output = compact_settle(&mut app, &ctx, size);
    let texts = text_regions(&output);
    for label in [
        "Back to Run",
        "Event history",
        "Warnings only",
        "Save & Start",
    ] {
        assert!(
            texts
                .iter()
                .any(|(text, rect)| text == label && rect.right() <= size.x),
            "Missing in-panel {label}"
        );
    }
    draw_at_size(
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
    assert!(!app.history_open);
    assert_eq!(app.tab, Tab::Run);
    assert_eq!(app.editor.profile, before);
    app.runtime.shutdown().unwrap();
}

#[test]
fn compact_all_detail_forms_scroll_without_horizontal_overflow_or_profile_mutation() {
    let (mut app, ctx, _dir) = setup();
    app.add_skill();
    app.editor
        .profile
        .periodic_actions
        .push(metin_core::PeriodicAction::new(
            "A deliberately long periodic action name",
        ));
    app.editor
        .profile
        .safety
        .rules
        .push(metin_core::SafetyRule::new(
            "A deliberately long visual event name",
        ));
    app.editor.profile.periodic_actions[0].enabled = true;
    app.editor.profile.safety.rules[0].reaction.pause = false;
    app.editor.profile.safety.rules[0].reaction.execute = true;
    app.editor.profile.puller.enabled = true;
    app.editor.profile.out_of_target.after_combat = true;
    app.editor.profile.out_of_target.during_search = true;
    app.editor.profile.safety.stop_enabled = true;
    app.editor.profile.safety.breaks_enabled = true;
    app.editor.profile.safety.clock_enabled = true;
    app.editor.profile.safety.recovery_enabled = true;
    let mut expanded = std::collections::HashSet::new();
    let before = app.editor.profile.clone();
    let size = egui::vec2(400., 500.);
    {
        widgets::theme(&ctx);
        let mut screens = Vec::new();
        for page in [
            ActionsPage::Buffs,
            ActionsPage::Loot,
            ActionsPage::Mount,
            ActionsPage::VisualRules,
            ActionsPage::Periodic,
            ActionsPage::Puller,
            ActionsPage::OutOfTarget,
        ] {
            screens.push((Tab::Actions, Some(page), None, None));
        }
        for page in [
            SettingsPage::Detection,
            SettingsPage::Combat,
            SettingsPage::Appearance,
            SettingsPage::Diagnostics,
        ] {
            screens.push((Tab::Settings, None, Some(page), None));
        }
        for page in [
            SafetyPage::Schedule,
            SafetyPage::Recovery,
            SafetyPage::Notifications,
        ] {
            screens.push((Tab::Safety, None, None, Some(page)));
        }
        for (tab, action, settings, safety) in screens {
            app.navigate(tab);
            if let Some(page) = action {
                app.actions_page = page;
                app.actions_detail = true;
                app.buff_detail = true;
                app.visual_detail = page == ActionsPage::VisualRules;
                if page == ActionsPage::Periodic {
                    app.selected_periodic = app
                        .editor
                        .profile
                        .periodic_actions
                        .first()
                        .map(|action| action.id);
                }
            }
            if let Some(page) = settings {
                app.settings_page = page;
            }
            if let Some(page) = safety {
                app.safety_page = page;
            }
            let mut output = compact_settle(&mut app, &ctx, size);
            for _ in 0..16 {
                if let Some((label, rect)) = text_regions(&output).into_iter().find(|(text, _)| {
                    text.starts_with("Advanced")
                        && !expanded.contains(&format!(
                            "{tab:?}/{action:?}/{settings:?}/{safety:?}/{text}"
                        ))
                }) {
                    expanded.insert(format!(
                        "{tab:?}/{action:?}/{settings:?}/{safety:?}/{label}"
                    ));
                    for pressed in [true, false] {
                        draw_at_size(
                            &mut app,
                            &ctx,
                            &mut vec![
                                egui::Event::PointerMoved(rect.center()),
                                egui::Event::PointerButton {
                                    pos: rect.center(),
                                    button: egui::PointerButton::Primary,
                                    pressed,
                                    modifiers: Default::default(),
                                },
                            ],
                            size,
                        );
                    }
                    output = compact_settle(&mut app, &ctx, size);
                }

                for (text, rect) in text_regions(&output) {
                    assert!(
                        rect.left() >= -0.5 && rect.right() <= size.x + 0.5,
                        "{tab:?}/{action:?}/{settings:?}/{safety:?}: {text:?} overflows horizontally {rect:?}"
                    );
                }
                output = draw_at_size(
                    &mut app,
                    &ctx,
                    &mut vec![
                        egui::Event::PointerMoved(egui::pos2(220., 300.)),
                        egui::Event::MouseWheel {
                            phase: egui::TouchPhase::Move,
                            source: egui::MouseWheelSource::Unknown,
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0., -180.),
                            modifiers: Default::default(),
                        },
                    ],
                    size,
                );
            }
        }
    }
    assert_eq!(app.editor.profile, before);
    app.runtime.shutdown().unwrap();
}

#[test]
fn compact_catalog_fits_all_seven_actions_at_default_size_and_summary_is_clickable() {
    let (mut app, ctx, _dir) = setup();
    app.navigate(Tab::Actions);
    app.actions_detail = false;
    let size = egui::vec2(460., 620.);
    let output = compact_settle(&mut app, &ctx, size);
    let texts = text_regions(&output);
    let footer = texts
        .iter()
        .find(|(text, _)| text == "Save & Start")
        .unwrap()
        .1
        .top();
    for label in [
        "Buffs",
        "Loot",
        "Mount",
        "Visual rules",
        "Periodic actions",
        "Out-of-target attack",
        "Auto-puller",
    ] {
        let rect = texts
            .iter()
            .find(|(text, _)| text == label)
            .unwrap_or_else(|| panic!("Catalog hides {label}"))
            .1;
        assert!(
            rect.bottom() < footer && rect.left() < 60.,
            "Catalog row {label} needs a visible left-aligned title: {rect:?}"
        );
    }
    let point = texts
        .iter()
        .find(|(text, _)| text.contains("F4 · Combat start"))
        .unwrap()
        .1
        .center();
    for pressed in [true, false] {
        draw_at_size(
            &mut app,
            &ctx,
            &mut vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
            size,
        );
    }
    assert!(app.actions_detail);
    assert_eq!(app.actions_page, ActionsPage::Puller);
    app.runtime.shutdown().unwrap();
}

#[test]
fn compact_running_overview_shows_schedule_and_recent_events_without_scrolling() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.name = "Main profile".into();
    app.editor.profile.buffs.enabled = false;
    app.editor.profile.loot.enabled = true;
    app.save().unwrap();
    app.target = Some(TargetToken {
        handle: 100,
        pid: 201,
        created: 1,
    });
    app.navigate(Tab::Run);
    let snapshot = Arc::new(RuntimeSnapshot {
        key: app.key(),
        revision: app.revision,
        applied_revision: Some(app.revision),
        running: true,
        state: "Looting".into(),
        activity_progress: Some(metin_core::ActivityProgress {
            label: "Next pickup".into(),
            elapsed_seconds: 0.5,
            remaining_seconds: Some(1.5),
            total_seconds: Some(2.),
        }),
        reason: "Target end confirmed, collecting nearby loot.".into(),
        mounted: Some(false),
        schedule: metin_core::ScheduleSnapshot {
            stop_remaining: Some(14400.),
            break_remaining: Some(2700.),
            ..Default::default()
        },
        events: (0..4)
            .map(|id| metin_core::RunEvent {
                id,
                at_seconds: id as f64,
                activity: "Loot".into(),
                message: format!("Recent fixture event {id}"),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    });
    app.window_fixture = Some(window_selection::WindowFixture { snapshot });
    let before = app.editor.profile.clone();
    {
        widgets::theme(&ctx);
        let output = compact_settle(&mut app, &ctx, egui::vec2(460., 620.));
        let texts = text_regions(&output);
        let footer = ctx
            .data(|data| data.get_temp::<egui::Rect>(egui::Id::unique("shell-primary")))
            .unwrap()
            .top()
            - 32.;
        for label in [
            "Looting",
            "Session",
            "Next actions",
            "Safety schedule",
            "Recent activity",
            "History",
            "Recent fixture event 3",
            "Recent fixture event 2",
            "Recent fixture event 1",
        ] {
            let rect = texts
                .iter()
                .find(|(text, _)| text == label)
                .unwrap_or_else(|| panic!("Default overview hides {label}: {texts:?}"))
                .1;
            assert!(
                rect.bottom() < footer,
                "{label} falls below fixed footer {rect:?}"
            );
        }
    }
    assert_eq!(app.editor.profile, before);
    app.runtime.shutdown().unwrap();
}

#[test]
fn compact_buff_detail_pairs_both_capture_cards_above_the_footer() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.buffs.skills = vec![metin_core::BuffSkill {
        name: "Aura".into(),
        shortcut: "F1".into(),
        mode: metin_core::BuffMode::Inactive,
        interval_seconds: None,
        ..Default::default()
    }];
    app.navigate(Tab::Actions);
    app.actions_page = ActionsPage::Buffs;
    app.actions_detail = true;
    app.buff_detail = true;
    let output = compact_settle(&mut app, &ctx, egui::vec2(440., 620.));
    let texts = text_regions(&output);
    let active = texts
        .iter()
        .find(|(text, _)| text == "Active icon")
        .unwrap()
        .1;
    let inactive = texts
        .iter()
        .find(|(text, _)| text == "Inactive icon (optional)")
        .unwrap()
        .1;
    assert!(
        (active.top() - inactive.top()).abs() < 1. && active.right() < inactive.left(),
        "Capture cards must form one pair: {active:?}, {inactive:?}"
    );
    let footer = texts
        .iter()
        .find(|(text, _)| text == "Save & Start")
        .unwrap()
        .1
        .top();
    let captures = texts
        .iter()
        .filter(|(text, _)| text == "Capture")
        .collect::<Vec<_>>();
    assert_eq!(captures.len(), 2);
    assert!(
        captures
            .iter()
            .all(|(_, rect)| rect.bottom() < footer && rect.right() <= 440.)
    );
    assert!(
        !texts
            .iter()
            .any(|(text, _)| text == "Automatic buffs" || text == "Skill 1 details")
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn compact_numeric_labels_start_at_the_same_left_edge() {
    let ctx = egui::Context::default();
    widgets::theme(&ctx);
    let mut frames = 3.;
    let mut threshold = 0.88;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(440., 180.),
            )),
            ..Default::default()
        },
        |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                crate::ui_controls::numeric(
                    ui,
                    "Stable detections (frames)",
                    "stable_frames",
                    &mut frames,
                );
                crate::ui_controls::numeric(ui, "Match threshold", "threshold", &mut threshold);
            });
        },
    );
    let text = text_regions(&output);
    output.textures_delta.clear();
    let left = |label: &str| {
        text.iter()
            .find(|(text, _)| text == label)
            .unwrap()
            .1
            .left()
    };
    assert!((left("Stable detections (frames)") - left("Match threshold")).abs() < 1.);
    assert!(
        left("Match threshold") < 20.,
        "Numeric labels belong at the panel edge, not centered in their allocated column."
    );
}

#[test]
fn compact_footer_keeps_controls_fixed_for_long_status_and_error_messages() {
    let (mut app, ctx, _dir) = setup();
    app.save().unwrap();
    app.navigate(Tab::Actions);
    let size = egui::vec2(400., 500.);
    let mut positions = None;
    for (state,reason,error) in [
        ("Stopped".into(),"Ready.".into(),None),
        ("Stopped".into(),"A long reason explaining which window or profile setting needs attention. ".repeat(8),None),
        ("An unusually long state name ".repeat(8),"Ready.".into(),None),
        ("Stopped".into(),"Ready.".into(),Some("Capture failed.".into())),
        ("Stopped".into(),"Ready.".into(),Some("A long capture error with recovery instructions that must remain available on hover. ".repeat(8))),
    ] {
        app.window_fixture=Some(window_selection::WindowFixture{snapshot:Arc::new(RuntimeSnapshot{state,reason,..Default::default()})});
        app.error=error;
        let output=compact_settle(&mut app,&ctx,size);
        let text=text_regions(&output);
        let controls=[text.iter().find(|(value,_)|value=="Start · F7").unwrap().1,compact_control_rect(&ctx,"Connect a game window first and finish any open capture. Mount confirmation needs no snapshot and sends no game input.")];
        if let Some(previous)=positions {assert_eq!(controls,previous,"Status/error length must not move the fixed controls.");}
        positions=Some(controls);
        assert!(controls.iter().all(|rect|rect.bottom()<=size.y&&rect.right()<=size.x));
        if app.error.is_some(){assert!(text.iter().any(|(value,_)|value=="Dismiss"));}
    }
    app.runtime.shutdown().unwrap();
}

#[test]
fn compact_profile_icon_controls_are_aligned_and_revert_undo_redo_use_real_clicks() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.name = "Saved profile".into();
    app.save().unwrap();
    let saved = app.editor.profile.clone();
    app.editor.profile.name = "Edited profile".into();
    app.editor.record();
    let size = egui::vec2(400., 500.);
    let output = compact_settle(&mut app, &ctx, size);
    assert!(
        !text_regions(&output)
            .iter()
            .any(|(text, _)| text == "Profile")
    );
    let hints = [
        "Save profile changes. Applying changes pauses automation.",
        "Revert all unsaved changes to the saved profile. The connected game window stays selected.",
        "Undo profile edit (Ctrl+Z)",
        "Redo profile edit (Ctrl+Y)",
    ];
    let trigger = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::unique("profile-trigger")))
        .unwrap();
    let rects = hints.map(|hint| compact_control_rect(&ctx, hint));
    assert!(rects[0].right() <= size.x && rects[1].right() <= size.x);
    assert!(
        rects
            .iter()
            .all(|rect| rect.top() == trigger.top() && rect.height() == 28.)
    );
    assert!(rects[2].right() <= rects[3].left());
    compact_click_at(&mut app, &ctx, size, rects[2].center());
    assert_eq!(app.editor.profile, saved);
    compact_settle(&mut app, &ctx, size);
    compact_click_at(
        &mut app,
        &ctx,
        size,
        compact_control_rect(&ctx, hints[3]).center(),
    );
    assert_eq!(app.editor.profile.name, "Edited profile");
    compact_settle(&mut app, &ctx, size);
    compact_click_at(
        &mut app,
        &ctx,
        size,
        compact_control_rect(&ctx, hints[1]).center(),
    );
    assert_eq!(app.editor.profile, saved);
    assert!(!app.editor.dirty());
    app.runtime.shutdown().unwrap();
}

#[test]
fn compact_mount_icons_confirm_without_capture_or_profile_edit_and_wait_for_ack() {
    let (mut app, ctx, dir) = setup();
    app.editor = Editor::new(Profile::create("No recognition"), None);
    app.save().unwrap();
    app.target = Some(TargetToken {
        handle: 100,
        pid: 201,
        created: 1,
    });
    app.tab = Tab::Settings;
    let before = app.editor.profile.clone();
    let size = egui::vec2(400., 500.);
    let hint =
        "Mount state unknown. Choose On foot or Mounted. Pauses automation; sends no game input.";
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot: Arc::new(RuntimeSnapshot {
            key: app.key(),
            revision: app.revision,
            ..Default::default()
        }),
    });
    let output = compact_settle(&mut app, &ctx, size);
    assert!(
        !text_regions(&output)
            .iter()
            .any(|(text, _)| text == "Mount: unknown")
    );
    let mount = compact_control_rect(&ctx, hint);
    assert!(mount.bottom() <= size.y && mount.top() > size.y - 55.);
    let revision = app.revision;
    let generation = app.runtime.stop_generation();
    compact_click_at(&mut app, &ctx, size, mount.center());
    assert_eq!(
        app.revision, revision,
        "Opening Unknown does not infer a mount state"
    );
    compact_click(&mut app, &ctx, size, "Mounted");
    assert_eq!(app.revision, revision + 1);
    assert!(app.runtime.stop_generation() > generation);
    assert_eq!(
        app.declared_mount(),
        None,
        "A command is not an acknowledgement"
    );
    assert_eq!(app.editor.profile, before);
    assert!(!app.editor.dirty());
    assert!(app.editor.frozen.is_none());
    assert!(!dir.path().join("ui-settings.json").exists());
    compact_settle(&mut app, &ctx, size);
    let waiting = compact_control_rect(
        &ctx,
        "Connect a game window first and finish any open capture. Mount confirmation needs no snapshot and sends no game input.",
    );
    let awaiting_revision = app.revision;
    compact_click_at(&mut app, &ctx, size, waiting.center());
    assert_eq!(
        app.revision, awaiting_revision,
        "The switch waits for the current revision acknowledgement before sending another declaration"
    );
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot: Arc::new(RuntimeSnapshot {
            key: app.key(),
            revision: app.revision,
            mounted: Some(true),
            ..Default::default()
        }),
    });
    let output = compact_settle(&mut app, &ctx, size);
    assert_eq!(app.declared_mount(), Some(true));
    assert!(
        !text_regions(&output)
            .iter()
            .any(|(text, _)| text == "Mounted")
    );
    let hint = "Mounted. Click to confirm On foot. Pauses automation; sends no game input.";
    let revision = app.revision;
    let generation = app.runtime.stop_generation();
    compact_click_at(
        &mut app,
        &ctx,
        size,
        compact_control_rect(&ctx, hint).center(),
    );
    assert_eq!(
        app.revision,
        revision + 1,
        "Known state toggles to the opposite declaration"
    );
    assert!(app.runtime.stop_generation() > generation);
    assert_eq!(
        app.declared_mount(),
        None,
        "The icon waits for the current revision acknowledgement"
    );
    assert_eq!(app.editor.profile, before);
    app.runtime.shutdown().unwrap();
}

#[test]
fn compact_mount_icons_ignore_clicks_until_connection_acknowledgement() {
    let (mut app, ctx, _dir) = setup();
    app.target = Some(TargetToken {
        handle: 100,
        pid: 201,
        created: 1,
    });
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot: Arc::new(RuntimeSnapshot::default()),
    });
    let size = egui::vec2(400., 500.);
    compact_settle(&mut app, &ctx, size);
    let hint = "Connect a game window first and finish any open capture. Mount confirmation needs no snapshot and sends no game input.";
    let rect = compact_control_rect(&ctx, hint);
    let revision = app.revision;
    let generation = app.runtime.stop_generation();
    compact_click_at(&mut app, &ctx, size, rect.center());
    assert_eq!(app.revision, revision);
    assert_eq!(app.runtime.stop_generation(), generation);
    assert_eq!(app.declared_mount(), None);
    app.runtime.shutdown().unwrap();
}

#[test]
fn compact_visual_run_states_keep_information_and_overlay_without_canvas() {
    let (mut app, ctx, _dir) = setup();
    app.target = Some(TargetToken {
        handle: 100,
        pid: 201,
        created: 1,
    });
    app.tab = Tab::Run;
    {
        widgets::theme(&ctx);
        for (state, running, arming, break_ends_in, attack_remaining) in [
            ("Paused", false, false, None, None),
            ("Starting", false, true, None, None),
            ("Searching", true, false, None, None),
            ("Attacking", true, false, None, Some(2.)),
            ("Looting", true, false, None, None),
            ("Break", true, false, Some(300.), None),
        ] {
            app.window_fixture = Some(window_selection::WindowFixture {
                snapshot: Arc::new(RuntimeSnapshot {
                    key: app.key(),
                    revision: app.revision,
                    state: state.into(),
                    running,
                    arming,
                    reason: "A very long explanatory reason that remains available on hover. "
                        .repeat(4),
                    schedule: metin_core::ScheduleSnapshot {
                        break_ends_in,
                        ..Default::default()
                    },
                    attack_remaining,
                    ..Default::default()
                }),
            });
            let output = compact_settle(&mut app, &ctx, egui::vec2(400., 500.));
            let texts = text_regions(&output);
            assert!(texts.iter().any(|(text, _)| text == state));
            assert!(texts.iter().any(|(text, _)| text == "Game overlay"));
            assert!(
                !texts
                    .iter()
                    .any(|(text, _)| text == "Game preview" || text == "View event history")
            );
            for (text, rect) in texts {
                assert!(
                    rect.left() >= -0.5 && rect.right() <= 400.5,
                    "{state}: {text} {rect:?}"
                );
            }
        }
    }
    app.runtime.shutdown().unwrap();
}

#[test]
fn compact_run_schedule_truncates_long_names_without_losing_click_targets() {
    let (mut app, ctx, _dir) = setup();
    app.target = Some(TargetToken {
        handle: 100,
        pid: 201,
        created: 1,
    });
    app.tab = Tab::Run;
    let timer_name = "A very long named action that must remain inside the schedule row";
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot: Arc::new(RuntimeSnapshot {
            key: app.key(),
            revision: app.revision,
        applied_revision: Some(app.revision),
            running: true,
            state: "Searching".into(),
            action_timers: vec![metin_core::ActionTimer {
                key: metin_core::ScheduledKey::Puller,
                name: timer_name.into(),
                remaining_seconds: Some(100.),
            }],
            ..Default::default()
        }),
    });
    let size = egui::vec2(400., 620.);
    let output = compact_settle(&mut app, &ctx, size);
    let texts = text_regions(&output);
    assert!(
        texts
            .iter()
            .any(|(text, _)| text.starts_with("A very long"))
    );
    assert!(
        texts
            .iter()
            .all(|(_, rect)| rect.left() >= -0.5 && rect.right() <= size.x + 0.5)
    );
    let point = texts
        .iter()
        .find(|(text, _)| text.starts_with("A very long"))
        .unwrap()
        .1
        .center();
    compact_click_at(&mut app, &ctx, size, point);
    assert_eq!(app.tab, Tab::Actions);
    assert_eq!(app.actions_page, ActionsPage::Puller);
    app.runtime.shutdown().unwrap();
}

#[test]
fn compact_mount_buttons_match_start_height_and_selected_state_replaces_footer_labels() {
    let (mut app, ctx, _dir) = setup();
    app.save().unwrap();
    app.target = Some(TargetToken {
        handle: 100,
        pid: 201,
        created: 1,
    });
    {
        widgets::theme(&ctx);
        for mounted in [None, Some(false), Some(true)] {
            app.window_fixture = Some(window_selection::WindowFixture {
                snapshot: Arc::new(RuntimeSnapshot {
                    key: app.key(),
                    revision: app.revision,
                    mounted,
                    ..Default::default()
                }),
            });
            let output = compact_settle(&mut app, &ctx, egui::vec2(400., 500.));
            let text = text_regions(&output);
            assert!(!text.iter().any(|(s, _)| {
                [
                    "F7 Start · F8 Pause",
                    "Mount: unknown",
                    "Mounted",
                    "On foot",
                ]
                .contains(&s.as_str())
            }));
            let start = ctx
                .data(|data| data.get_temp::<egui::Rect>(egui::Id::unique("shell-primary")))
                .unwrap();
            let r = ctx
                .data(|d| d.get_temp::<egui::Rect>(egui::Id::unique("mount-toggle")))
                .unwrap();
            assert!((r.height() - 44.).abs() < 0.1 && (r.top() - start.top()).abs() < 0.1);
            assert_eq!(
                ctx.data(|d| d.get_temp::<Option<bool>>(egui::Id::unique("mount-state")))
                    .unwrap(),
                mounted
            );
        }
    }
    app.runtime.shutdown().unwrap();
}

#[test]
fn compact_progress_slot_is_stable_and_history_belongs_to_recent_activity() {
    let (mut app, ctx, _dir) = setup();
    app.tab = Tab::Run;
    let mut snapshot = RuntimeSnapshot {
        running: true,
        state: "Attacking".into(),
        activity_progress: Some(metin_core::ActivityProgress {
            label: "Combat limit".into(),
            elapsed_seconds: 10.,
            remaining_seconds: Some(20.),
            total_seconds: Some(30.),
        }),
        ..Default::default()
    };
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot: Arc::new(snapshot.clone()),
    });
    let size = egui::vec2(460., 620.);
    let output = compact_settle(&mut app, &ctx, size);
    let texts = text_regions(&output);
    assert!(texts.iter().any(|(text, _)| text == "Combat limit"));
    assert!(texts.iter().any(|(text, _)| text == "00:00:20 left"));
    let y = |label: &str| {
        texts
            .iter()
            .find(|(text, _)| text == label)
            .unwrap()
            .1
            .center()
            .y
    };
    assert!((y("History") - y("Recent activity")).abs() < 3.);
    assert!(y("History") > y("Session"));
    assert_eq!(
        texts.iter().filter(|(text, _)| text == "History").count(),
        1
    );
    let session_y = y("Session");
    snapshot.activity_progress = None;
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot: Arc::new(snapshot),
    });
    let output = compact_settle(&mut app, &ctx, size);
    let paused_session = text_regions(&output)
        .into_iter()
        .find(|(text, _)| text == "Session")
        .unwrap()
        .1
        .center()
        .y;
    app.runtime.shutdown().unwrap();
    assert_eq!(
        paused_session, session_y,
        "Missing countdown must preserve the exact card height"
    );
}

#[test]
fn compact_skill_bars_use_regular_intervals_and_fresh_presence_without_editing() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.buffs.enabled = true;
    app.editor.profile.buffs.skills = vec![
        metin_core::BuffSkill {
            name: "Timer skill".into(),
            shortcut: "F1".into(),
            interval_seconds: Some(120.),
            ..Default::default()
        },
        metin_core::BuffSkill {
            name: "Icon skill".into(),
            shortcut: "F2".into(),
            mode: metin_core::BuffMode::Inactive,
            interval_seconds: None,
            ..Default::default()
        },
    ];
    app.editor.mark_saved(app.editor.profile.clone());
    app.target = Some(TargetToken {
        handle: 7,
        pid: 8,
        created: 9,
    });
    let mut snapshot = RuntimeSnapshot {
        key: app.key(),
        revision: app.revision,
        applied_revision: Some(app.revision),
        timers: vec![Some(60.), None],
        buff_presence: vec![metin_core::Presence::Unknown, metin_core::Presence::Active],
        captured_at: Some(std::time::Instant::now()),
        ..Default::default()
    };
    app.snapshot = Arc::new(snapshot.clone());
    assert_eq!(app.skill_timing(0).1, Some(0.5));
    assert!(app.skill_timing(0).0.contains("paused"));
    assert_eq!(app.skill_timing(1), ("Active".into(), None));
    snapshot.captured_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
    app.snapshot = Arc::new(snapshot.clone());
    assert_eq!(app.skill_timing(1), ("Unknown".into(), None));
    snapshot.captured_at = Some(std::time::Instant::now());
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot: Arc::new(snapshot),
    });
    app.tab = Tab::Actions;
    app.actions_page = ActionsPage::Buffs;
    app.actions_detail = true;
    app.buff_detail = false;
    let before = app.editor.profile.clone();
    let output = compact_settle(&mut app, &ctx, egui::vec2(460., 620.));
    assert!(
        text_regions(&output)
            .iter()
            .any(|(text, _)| text.contains("01:00") && text.contains("paused"))
    );
    assert_eq!(app.editor.profile, before);
    app.runtime.shutdown().unwrap();
}

#[test]
fn recent_activity_uses_only_whole_rows_and_fills_available_height() {
    let (mut app, ctx, _dir) = setup();
    app.snapshot = Arc::new(RuntimeSnapshot {
        running: true,
        events: (0..30)
            .map(|i| metin_core::RunEvent {
                id: i,
                message: format!("Recent event {i:02}"),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    });
    let mut counts = Vec::new();
    for size in [egui::vec2(400., 390.), egui::vec2(460., 700.)] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                ..Default::default()
            },
            |ui| app.run_view(ui),
        );
        output.textures_delta.clear();
        let rows = text_regions(&output)
            .into_iter()
            .filter(|(text, _)| text.starts_with("Recent event"))
            .collect::<Vec<_>>();
        assert!(
            rows.iter()
                .all(|(_, rect)| rect.bottom() <= size.y - 2. && rect.right() <= size.x)
        );
        assert!(rows.iter().any(|(text, _)| text == "Recent event 29"));
        counts.push(rows.len());
    }
    assert!(
        counts[1] > counts[0] && counts[1] > 3,
        "Counts must follow actual available height: {counts:?}"
    );
    let message =
        "A complete warning with a detailed reason and an actionable next step. ".repeat(3);
    app.snapshot = Arc::new(RuntimeSnapshot {
        running: true,
        events: (0..20)
            .map(|i| metin_core::RunEvent {
                id: i,
                message: format!("{message}{i}"),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    });
    let size = egui::vec2(400., 480.);
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        },
        |ui| app.run_view(ui),
    );
    output.textures_delta.clear();
    let rows = text_regions(&output)
        .into_iter()
        .filter(|(text, _)| text.starts_with("A complete warning"))
        .collect::<Vec<_>>();
    assert!(!rows.is_empty());
    assert!(
        rows.iter()
            .all(|(_, rect)| rect.height() > 20. && rect.bottom() <= size.y - 2.)
    );
    assert!(rows.iter().any(|(text, _)| text == &format!("{message}19")));
    app.runtime.shutdown().unwrap();
}

#[test]
fn recent_activity_does_not_load_more_rows_when_other_run_sections_scroll() {
    let (mut app, ctx, _dir) = setup();
    app.target=Some(window_catalog()[0].token);
    app.snapshot = Arc::new(RuntimeSnapshot {
        key:app.key(),revision:app.revision,applied_revision:Some(app.revision),
        running: true,
        action_timers: (0..30)
            .map(|i| metin_core::ActionTimer {
                key: metin_core::ScheduledKey::Periodic(uuid::Uuid::new_v4()),
                name: format!("Scheduled action {i}"),
                remaining_seconds: Some(20.),
            })
            .collect(),
        events: (0..30)
            .map(|i| metin_core::RunEvent {
                id: i,
                message: format!("Overflow event {i}"),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    });
    for frame in 0..8 {
        let events = if frame == 2 {
            vec![
                egui::Event::PointerMoved(egui::pos2(200., 200.)),
                egui::Event::MouseWheel {
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0., -3000.),
                    modifiers: Default::default(),
                },
            ]
        } else {
            vec![]
        };
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(frame as f64),
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400., 400.),
                )),
                ..Default::default()
            },
            |ui| app.run_view(ui),
        );
        output.textures_delta.clear();
        if frame == 7 {
            let id = ctx
                .data(|d| d.get_temp::<egui::Id>(egui::Id::unique("run-scroll-id")))
                .unwrap();
            assert!(
                egui::scroll_area::State::load(&ctx, id)
                    .unwrap()
                    .clamped_offset()
                    .y
                    > 0.,
                "Fixture must actually scroll preceding Run sections"
            );
        }

        assert!(
            !text_regions(&output)
                .iter()
                .any(|(text, _)| text.starts_with("Overflow event")),
            "No spare Run height: scrolling must not continuously reveal new recent events"
        );
    }
    app.runtime.shutdown().unwrap();
}
