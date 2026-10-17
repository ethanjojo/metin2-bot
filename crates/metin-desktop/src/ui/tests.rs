use super::*;
#[cfg(windows)]
#[test]
fn committed_save_marks_editor_saved_and_configures_despite_cleanup_warning() {
    use std::os::windows::fs::OpenOptionsExt;
    let (mut app, _ctx, dir) = setup();
    app.editor.template = Some(
        metin_platform::Frame::new(10, 10, (0..300).map(|n| (n % 251) as u8).collect()).unwrap(),
    );
    app.save().unwrap();
    let original = app.editor.profile.clone();
    let old_asset = dir
        .path()
        .join("profiles")
        .join(original.id.to_string())
        .join(original.template_file.as_ref().unwrap());
    let locked = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(old_asset)
        .unwrap();
    app.editor.profile.name = "Committed despite cleanup warning".into();
    app.target = Some(TargetToken {
        handle: 100,
        pid: 201,
        created: 1,
    });
    let revision = app.revision;

    assert!(app.save().is_ok());
    assert!(!app.editor.dirty());
    assert_eq!(app.editor.profile.name, "Committed despite cleanup warning");
    assert_ne!(app.editor.profile.template_file, original.template_file);
    assert_eq!(app.revision, revision + 1);
    assert_eq!(app.store.load(original.id).unwrap().0, app.editor.profile);
    assert!(
        app.error
            .as_ref()
            .is_some_and(|warning| warning.contains("cleanup"))
    );
    drop(locked);
    app.runtime.shutdown().unwrap();
}
#[cfg(feature = "ui-smoke")]
#[test]
fn window_selection_native_fixture_really_opens_the_picker_and_keeps_start_blocked() {
    let (mut app, ctx, _dir) = setup();
    app.set_window_picker_fixture(WindowPickerFixture::All);
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    for _ in 0..4 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    }
    assert!(
        text_regions(&output)
            .iter()
            .any(|(s, _)| s == "Search title or process…")
    );
    assert!(
        text_regions(&output)
            .iter()
            .any(|(s, _)| s.contains("Forest · Warrior · server-a.exe"))
    );
    assert!(app.start_blocker().is_some());
    app.runtime.shutdown().unwrap();
}
fn window_catalog() -> Vec<metin_platform::TargetInfo> {
    vec![
        metin_platform::TargetInfo {
            token: TargetToken {
                handle: 100,
                pid: 201,
                created: 1,
            },
            title: "Žluťoučký server".into(),
            process_name: "client-a.exe".into(),
        },
        metin_platform::TargetInfo {
            token: TargetToken {
                handle: 200,
                pid: 202,
                created: 2,
            },
            title: "Second server".into(),
            process_name: "other.exe".into(),
        },
    ]
}
fn pin_windows(app: &mut MetinApp, targets: Vec<metin_platform::TargetInfo>, revision: u64) {
    app.snapshot = Arc::new(RuntimeSnapshot {
        targets,
        targets_revision: revision,
        ..Default::default()
    });
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot: app.snapshot.clone(),
        open_popup: false,
    });
}
#[test]
fn window_selection_search_is_temporary_and_can_select_any_process_via_real_clicks() {
    let (mut app, ctx, dir) = setup();
    let before = app.editor.profile.clone();
    let windows = window_catalog();
    pin_windows(&mut app, windows.clone(), 1);
    click_label(&mut app, &ctx, "Select a game window");
    draw(
        &mut app,
        &ctx,
        vec![egui::Event::Text("  OTHER.EXE  ".into())],
    );
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    let text = text_regions(&output);
    assert!(
        text.iter()
            .any(|(s, _)| s.contains("Second server · other.exe")),
        "{text:?}"
    );
    assert!(
        !text
            .iter()
            .any(|(s, _)| s.contains("Žluťoučký server · client-a.exe"))
    );
    click_label(&mut app, &ctx, "Second server · other.exe · PID 202");
    assert_eq!(app.target, Some(windows[1].token));
    assert_eq!(app.editor.profile, before);
    let settings = UISettings::load(dir.path()).unwrap();
    let remembered = settings.profile_windows.get(&before.id).unwrap();
    assert_eq!(remembered.process_name, "other.exe");
    let persisted = std::fs::read_to_string(dir.path().join("ui-settings.json")).unwrap();
    assert!(!persisted.contains("OTHER.EXE") && !persisted.contains("window_search"));
    click_label(&mut app, &ctx, "Second server · other.exe");
    click_label(&mut app, &ctx, "Clear");
    assert!(app.window_search.is_empty());
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert!(
        text_regions(&output)
            .iter()
            .any(|(s, _)| s.contains("Žluťoučký server · client-a.exe"))
    );
    draw(
        &mut app,
        &ctx,
        vec![egui::Event::Text("no-such-window".into())],
    );
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert_eq!(app.window_search, "no-such-window");
    assert!(
        text_regions(&output)
            .iter()
            .any(|(s, _)| s == "No windows match this search.")
    );
    assert_eq!(
        app.target,
        Some(windows[1].token),
        "Filtering does not change the selected window"
    );
    app.runtime.shutdown().unwrap();
}
#[test]
fn window_selection_preferences_survive_restart_and_are_independent_for_each_profile() {
    let (mut app, ctx, dir) = setup();
    let a = Profile::create("A");
    let b = Profile::create("B");
    app.store.save(&a, None).unwrap();
    app.store.save(&b, None).unwrap();
    app.store.select(a.id).unwrap();
    app.replace_profile(a.clone(), None);
    let windows = window_catalog();
    pin_windows(&mut app, windows.clone(), 1);
    app.select_window(windows[0].token).unwrap();
    app.switch_profile(b.id);
    app.select_window(windows[1].token).unwrap();
    app.window_search = "old search".into();
    app.switch_profile(a.id);
    assert!(app.window_search.is_empty());
    app.reconcile_windows();
    assert_eq!(app.target, Some(windows[0].token));
    assert!(!app.snapshot.running && !app.snapshot.arming);
    app.runtime.shutdown().unwrap();
    drop(app);
    let mut reopened = MetinApp::from_context(&ctx, dir.path().to_owned()).unwrap();
    let mut restarted = windows.clone();
    restarted[0].token = TargetToken {
        handle: 900,
        pid: 901,
        created: 99,
    };
    pin_windows(&mut reopened, restarted.clone(), 1);
    reopened.reconcile_windows();
    assert_eq!(reopened.target, Some(restarted[0].token));
    assert_eq!(reopened.settings.profile_windows.len(), 2);
    reopened.switch_profile(b.id);
    reopened.reconcile_windows();
    assert_eq!(reopened.target, Some(restarted[1].token));
    assert_eq!(reopened.editor.profile, b);
    reopened.runtime.shutdown().unwrap();
}
#[test]
fn window_selection_restores_only_unique_exact_matches_and_retires_closed_tokens() {
    let (mut app, ctx, _dir) = setup();
    let windows = window_catalog();
    app.settings.profile_windows.insert(
        app.editor.profile.id,
        crate::storage::WindowPreference {
            process_name: windows[0].process_name.to_uppercase(),
            title: windows[0].title.clone(),
        },
    );
    let mut duplicate = windows[0].clone();
    duplicate.token = windows[1].token;
    pin_windows(&mut app, vec![windows[0].clone(), duplicate], 1);
    app.reconcile_windows();
    assert!(app.target.is_none());
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert!(
        text_regions(&output)
            .iter()
            .any(|(s, _)| s.contains("Several windows match"))
    );
    pin_windows(&mut app, windows.clone(), 2);
    app.reconcile_windows();
    assert_eq!(app.target, Some(windows[0].token));
    let mut renamed = windows[0].clone();
    renamed.title = "Changed title".into();
    pin_windows(&mut app, vec![renamed], 3);
    app.reconcile_windows();
    assert_eq!(
        app.target,
        Some(windows[0].token),
        "A running window keeps its actual token when its title changes"
    );
    pin_windows(&mut app, vec![], 4);
    app.reconcile_windows();
    assert!(app.target.is_none());
    assert!(!app.live && app.snapshot_requested.is_none());
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert!(
        text_regions(&output)
            .iter()
            .any(|(s, _)| s.contains("Remembered window not found"))
    );
    let mut wrong_title = windows[0].clone();
    wrong_title.title = "Different".into();
    pin_windows(&mut app, vec![wrong_title], 5);
    app.reconcile_windows();
    assert!(app.target.is_none());
    assert!(!app.snapshot.running && !app.snapshot.arming);
    app.runtime.shutdown().unwrap();
}
#[test]
fn window_selection_failed_preference_write_does_not_claim_a_saved_selection() {
    let (mut app, _ctx, dir) = setup();
    let windows = window_catalog();
    pin_windows(&mut app, windows.clone(), 1);
    std::fs::create_dir(dir.path().join("ui-settings.json")).unwrap();
    assert!(app.select_window(windows[0].token).is_err());
    assert!(app.target.is_none());
    assert!(app.settings.profile_windows.is_empty());
    app.runtime.shutdown().unwrap();
}
#[test]
fn window_selection_unicode_search_and_exact_preference_have_different_purposes() {
    let windows = window_catalog();
    for search in ["", " žLUŤOUČKÝ ", "CLIENT-A", "201"] {
        assert!(window_selection::matches_search(&windows[0], search));
    }
    assert!(!window_selection::matches_search(&windows[0], "other"));
    let preference = crate::storage::WindowPreference {
        process_name: "CLIENT-A.EXE".into(),
        title: windows[0].title.clone(),
    };
    assert_eq!(
        window_selection::remembered_matches(&windows, &preference).len(),
        1
    );
}
#[test]
fn window_selection_has_search_and_settings_no_longer_have_a_process_filter() {
    let (mut app, ctx, _dir) = setup();
    app.navigate(Tab::Settings);
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert!(
        !text_regions(&output)
            .iter()
            .any(|(text, _)| text == "Game window filter")
    );
    app.navigate(Tab::Calibration);
    app.snapshot = Arc::new(RuntimeSnapshot {
        targets: vec![metin_platform::TargetInfo {
            token: TargetToken {
                handle: 100,
                pid: 200,
                created: 300,
            },
            title: "Different server".into(),
            process_name: "other.exe".into(),
        }],
        ..Default::default()
    });
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot: app.snapshot.clone(),
        open_popup: false,
    });
    click_label(&mut app, &ctx, "Select a game window");
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    let text = text_regions(&output);
    assert!(
        text.iter().any(|(s, _)| s == "Search title or process…"),
        "{text:?}"
    );
    assert!(
        text.iter()
            .any(|(s, _)| s.contains("Different server") && s.contains("other.exe")),
        "{text:?}"
    );
    app.runtime.shutdown().unwrap();
}
#[test]
fn refinement_tool_changes_do_not_move_canvas_or_expand_inline_help() {
    let (mut app, ctx, _dir) = setup();
    let mut previous = None;
    for mode in [
        EditMode::World,
        EditMode::Color,
        EditMode::Panel,
        EditMode::Template,
        EditMode::Mask(None),
    ] {
        app.edit_area(mode);
        for editing in [false, true] {
            app.editing = editing;
            let mut output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
            for _ in 0..3 {
                output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
            }
            let canvas = app.canvas_rect.unwrap();
            if let Some(old) = previous {
                assert_eq!(canvas, old, "Canvas moved for {mode:?}, editing={editing}");
            }
            previous = Some(canvas);
            assert!(
                !text_regions(&output)
                    .iter()
                    .any(|(text, _)| text.contains("Drag around the game area")),
                "Help belongs in a hover tooltip"
            );
        }
    }
    app.runtime.shutdown().unwrap();
}

#[test]
fn refinement_sizes_checkbox_is_independent_of_detections() {
    let (mut app, ctx, _dir) = setup();
    let region = metin_core::Region {
        left: 0.1,
        top: 0.2,
        right: 0.4,
        bottom: 0.5,
    };
    app.editor.set_region(EditMode::World, region).unwrap();
    let f = app.editor.frozen.as_ref().unwrap();
    let [x1, y1, x2, y2] = region.pixels(f.width, f.height);
    let expected = format!("{} × {} px", x2 - x1 + 1, y2 - y1 + 1);
    for (detections, show) in [(false, false), (false, true), (true, false), (true, true)] {
        app.show_detections = detections;
        app.show_sizes = show;
        let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
        assert_eq!(
            text_regions(&output)
                .iter()
                .any(|(text, _)| text == &expected),
            show
        );
    }
    app.runtime.shutdown().unwrap();
}
#[test]
fn mount_declaration_is_read_only_for_current_acknowledged_session() {
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
    assert_eq!(app.declared_mount(), Some(true));
    app.revision = 5;
    assert_eq!(app.declared_mount(), None);
    app.target = None;
    assert_eq!(app.declared_mount(), None);
    assert!(!dir.path().join("ui-settings.json").exists());
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
    app.edit_area(EditMode::World);
    (app, ctx, dir)
}

#[test]
fn run_can_confirm_mount_without_snapshot_or_calibration() {
    let (mut app, ctx, _dir) = setup();
    app.editor = Editor::new(Profile::create("Not calibrated"), None);
    app.editor.profile.buffs.enabled = true;
    app.tab = Tab::Run;
    app.editing = false;
    app.target = Some(TargetToken {
        handle: 100,
        pid: 201,
        created: 1,
    });
    let snapshot = Arc::new(RuntimeSnapshot {
        key: app.key(),
        revision: app.revision,
        mounted: None,
        ..Default::default()
    });
    app.snapshot = snapshot.clone();
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot,
        open_popup: false,
    });
    let revision = app.revision;
    let profile = app.editor.profile.clone();
    click_label(&mut app, &ctx, "On foot");
    assert_eq!(app.revision, revision + 1);
    assert!(app.editor.frozen.is_none());
    assert!(app.editor.template.is_none());
    assert_eq!(app.editor.profile, profile);
    assert_eq!(
        app.declared_mount(),
        None,
        "Wait for runtime acknowledgement rather than assuming success"
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn run_mount_waits_for_connection_acknowledgement_without_cancelling_connect() {
    let (mut app, ctx, _dir) = setup();
    app.tab = Tab::Run;
    app.target = Some(TargetToken {
        handle: 100,
        pid: 201,
        created: 1,
    });
    let snapshot = Arc::new(RuntimeSnapshot::default());
    app.snapshot = snapshot.clone();
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot,
        open_popup: false,
    });
    let revision = app.revision;
    let generation = app.runtime.stop_generation();
    click_label(&mut app, &ctx, "On foot");
    assert_eq!(app.revision, revision);
    assert_eq!(app.runtime.stop_generation(), generation);
    assert_eq!(app.declared_mount(), None);
    app.runtime.shutdown().unwrap();
}

#[test]
fn run_readiness_keeps_mount_independent_from_calibration_and_routes_directly() {
    let (mut app, _ctx, _dir) = setup();
    app.editor = Editor::new(Profile::create("Incomplete"), None);
    app.editor.profile.buffs.enabled = true;
    app.target = Some(TargetToken {
        handle: 100,
        pid: 201,
        created: 1,
    });
    let steps = app.run_setup_steps();
    assert!(steps.iter().any(|step| step.action == SetupAction::Mount));
    assert_eq!(
        steps
            .iter()
            .filter(|step| step.action == SetupAction::Calibration)
            .count(),
        1
    );
    assert!(!steps.iter().any(|step| step.action == SetupAction::Window));
    app.snapshot = Arc::new(RuntimeSnapshot {
        key: app.key(),
        revision: app.revision,
        mounted: Some(false),
        ..Default::default()
    });
    assert!(
        !app.run_setup_steps()
            .iter()
            .any(|step| step.action == SetupAction::Mount)
    );
    assert!(
        app.run_setup_steps()
            .iter()
            .any(|step| step.action == SetupAction::Calibration)
    );
    app.open_setup(SetupAction::Mount);
    assert_eq!(app.tab, Tab::Actions);
    assert_eq!(app.actions_page, ActionsPage::Mount);
    app.open_setup(SetupAction::Calibration);
    assert_eq!(app.tab, Tab::Calibration);
    app.runtime.shutdown().unwrap();
}

#[test]
fn run_setup_routes_visual_errors_once_to_the_visual_rule_editor() {
    let (mut app, _ctx, _dir) = setup();
    app.editor
        .profile
        .safety
        .rules
        .push(metin_core::SafetyRule::new(""));
    let steps = app.run_setup_steps();
    let visual: Vec<_> = steps
        .iter()
        .filter(|s| s.action == SetupAction::VisualRules)
        .collect();
    assert_eq!(visual.len(), 1);
    assert!(visual[0].detail.contains("name"));
    assert!(
        !steps
            .iter()
            .any(|s| s.action != SetupAction::VisualRules && s.detail == visual[0].detail)
    );
    app.open_setup(SetupAction::VisualRules);
    assert_eq!(app.tab, Tab::Actions);
    assert_eq!(app.actions_page, ActionsPage::VisualRules);
    app.runtime.shutdown().unwrap();
}

#[test]
fn run_setup_opens_invalid_periodic_action_even_with_enabled_puller() {
    let (mut app, _ctx, _dir) = setup();
    app.editor.profile.puller.enabled = true;
    let action = metin_core::PeriodicAction::new("");
    app.editor.profile.periodic_actions.push(action);
    let step = app
        .run_setup_steps()
        .into_iter()
        .find(|step| step.action == SetupAction::Actions(ActionsPage::Periodic))
        .expect("periodic setup issue");
    assert!(step.detail.contains("name"));
    app.open_setup(step.action);
    assert_eq!(app.tab, Tab::Actions);
    assert_eq!(app.actions_page, ActionsPage::Periodic);
    app.runtime.shutdown().unwrap();
}

#[test]
fn run_preview_toggle_updates_capture_demand_without_changing_automation_or_profile() {
    let (mut app, ctx, _dir) = setup();
    app.tab = Tab::Run;
    app.target = Some(TargetToken {
        handle: 100,
        pid: 201,
        created: 1,
    });
    let snapshot = Arc::new(RuntimeSnapshot {
        key: app.key(),
        revision: app.revision,
        mounted: Some(false),
        frame: app.editor.frozen.clone(),
        running: true,
        ..Default::default()
    });
    app.snapshot = snapshot.clone();
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot,
        open_popup: false,
    });
    let profile = app.editor.profile.clone();
    click_label(&mut app, &ctx, "Live preview");
    assert!(!app.run_live);
    assert!(!app.preview_demand.unwrap().1);
    assert!(app.snapshot.running);
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert!(
        text_regions(&output)
            .iter()
            .any(|(text, _)| text == "Preview off")
    );
    click_label(&mut app, &ctx, "Live preview");
    assert!(app.run_live);
    assert!(app.preview_demand.unwrap().1);
    assert_eq!(app.editor.profile, profile);
    app.runtime.shutdown().unwrap();
}

#[test]
fn action_links_have_readable_contrast_in_both_themes() {
    fn luminance(color: egui::Color32) -> f64 {
        let linear = |value: u8| {
            let x = f64::from(value) / 255.;
            if x <= 0.04045 {
                x / 12.92
            } else {
                ((x + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    }
    let ctx = egui::Context::default();
    for dark in [false, true] {
        widgets::theme(&ctx, dark);
        let style = ctx.style_of(if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        });
        let visuals = &style.visuals;
        for background in [
            visuals.panel_fill,
            visuals.faint_bg_color,
            visuals.window_fill,
        ] {
            let a = luminance(visuals.hyperlink_color);
            let b = luminance(background);
            let ratio = (a.max(b) + 0.05) / (a.min(b) + 0.05);
            assert!(ratio >= 4.5, "link contrast {ratio:.2}: dark={dark}");
        }
    }
}

#[test]
fn run_groups_confirmed_mount_with_activity_and_replaces_the_buff_only_shortcut() {
    let (mut app, ctx, _dir) = setup();
    app.tab = Tab::Run;
    app.target = Some(TargetToken {
        handle: 100,
        pid: 201,
        created: 1,
    });
    let snapshot = Arc::new(RuntimeSnapshot {
        key: app.key(),
        revision: app.revision,
        mounted: Some(true),
        running: true,
        ..Default::default()
    });
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot,
        open_popup: false,
    });
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    for _ in 0..3 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    }
    let texts = text_regions(&output);
    assert!(
        texts.iter().any(|(t, _)| t == "Change"),
        "confirmed mount needs a compact Change action"
    );
    assert!(
        !texts.iter().any(|(t, _)| t == "On foot"),
        "choices are hidden until Change"
    );
    assert!(!texts.iter().any(|(t, _)| t == "Edit buffs"));
    assert!(texts.iter().any(|(t, _)| t == "Configure actions"));
    let profile = app.editor.profile.clone();
    click_label(&mut app, &ctx, "Change");
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert!(text_regions(&output).iter().any(|(t, _)| t == "On foot"));
    assert_eq!(app.editor.profile, profile);
    app.runtime.shutdown().unwrap();
}

#[test]
fn run_history_opens_a_dialog_and_escape_restores_the_same_overview_position() {
    let (mut app, ctx, _dir) = setup();
    app.tab = Tab::Run;
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot: Arc::new(RuntimeSnapshot {
            running: true,
            events: (0..12)
                .map(|i| metin_core::RunEvent {
                    id: i,
                    message: format!("Dialog record {i}"),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        open_popup: false,
    });
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    for _ in 0..3 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    }
    let preview = text_regions(&output)
        .into_iter()
        .find(|(t, _)| t == "Game preview")
        .unwrap()
        .1;
    click_label(&mut app, &ctx, "View event history (12)");
    for _ in 0..4 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    }
    assert!(
        text_regions(&output)
            .iter()
            .any(|(t, _)| t == "Close history"),
        "full history must be a dialog"
    );
    assert!(app.snapshot.running);
    draw(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    for _ in 0..4 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    }
    assert!(
        !text_regions(&output)
            .iter()
            .any(|(t, _)| t == "Close history")
    );
    let restored = text_regions(&output)
        .into_iter()
        .find(|(t, _)| t == "Game preview")
        .unwrap()
        .1;
    assert_eq!(restored, preview);
    app.runtime.shutdown().unwrap();
}

#[test]
fn run_schedule_links_open_the_exact_action_skill_and_safety_page() {
    let (mut app, ctx, _dir) = setup();
    app.target = Some(TargetToken {
        handle: 100,
        pid: 201,
        created: 1,
    });
    app.run_live = false;
    app.editor.profile.buffs.enabled = true;
    app.editor.profile.loot.enabled = true;
    app.editor.profile.buffs.skills = vec![
        metin_core::BuffSkill {
            name: "First skill".into(),
            ..Default::default()
        },
        metin_core::BuffSkill {
            name: "Armor spell".into(),
            shortcut: "F2".into(),
            ..Default::default()
        },
    ];
    let action = metin_core::PeriodicAction::new("Supply action");
    let id = action.id;
    app.editor.profile.periodic_actions.push(action);
    app.editor.mark_saved(app.editor.profile.clone());
    let snapshot = Arc::new(RuntimeSnapshot {
        key: app.key(),
        revision: app.revision,
        mounted: Some(false),
        running: true,
        recoveries: 1,
        action_timers: vec![
            metin_core::ActionTimer {
                key: metin_core::ScheduledKey::Periodic(id),
                name: "Supply action".into(),
                remaining_seconds: Some(10.),
            },
            metin_core::ActionTimer {
                key: metin_core::ScheduledKey::Puller,
                name: "Auto-puller".into(),
                remaining_seconds: Some(3.),
            },
        ],
        ..Default::default()
    });
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot,
        open_popup: false,
    });
    let before = app.editor.profile.clone();
    let generation = app.runtime.stop_generation();
    let reveal_schedule = |app: &mut MetinApp| {
        app.tab = Tab::Run;
        draw(
            app,
            &ctx,
            vec![
                egui::Event::PointerMoved(egui::pos2(750., 700.)),
                egui::Event::MouseWheel {
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0., -2000.),
                    modifiers: Default::default(),
                },
            ],
        );
        for _ in 0..20 {
            draw(app, &ctx, vec![]);
        }
    };
    for (label, expected) in [
        ("Armor spell", ActionsPage::Buffs),
        ("Supply action", ActionsPage::Periodic),
        ("Auto-puller", ActionsPage::Puller),
        ("Loot", ActionsPage::Loot),
    ] {
        reveal_schedule(&mut app);
        click_label(&mut app, &ctx, label);
        assert_eq!(app.tab, Tab::Actions, "link: {label}");
        assert_eq!(app.actions_page, expected, "link: {label}");
        if label == "Armor spell" {
            assert_eq!(app.selected_skill, 1);
        }
        if label == "Supply action" {
            assert_eq!(app.selected_periodic, Some(id));
        }
    }
    for (label, expected) in [
        ("Safety schedule", SafetyPage::Schedule),
        ("Recovery", SafetyPage::Recovery),
    ] {
        reveal_schedule(&mut app);
        click_label(&mut app, &ctx, label);
        assert_eq!(app.tab, Tab::Safety);
        assert_eq!(app.safety_page, expected);
    }
    assert_eq!(app.editor.profile, before);
    assert_eq!(
        app.runtime.stop_generation(),
        generation,
        "navigation does not pause a running bot"
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn history_dialog_close_and_backdrop_do_not_activate_underlying_run_controls() {
    let (mut app, ctx, _dir) = setup();
    app.tab = Tab::Run;
    click_label(&mut app, &ctx, "View event history (0)");
    assert!(app.history_open);
    let generation = app.runtime.stop_generation();
    click_label(&mut app, &ctx, "Close history");
    assert!(!app.history_open);
    click_label(&mut app, &ctx, "View event history (0)");
    click_label(&mut app, &ctx, "Actions"); // outside the modal: closes its backdrop only
    assert_eq!(app.tab, Tab::Run);
    assert!(!app.history_open);
    assert_eq!(app.runtime.stop_generation(), generation);
    app.runtime.shutdown().unwrap();
}

#[test]
fn run_history_keeps_five_in_overview_and_filters_full_dialog() {
    let (mut app, ctx, _dir) = setup();
    app.tab = Tab::Run;
    app.run_live = false;
    app.target = Some(TargetToken {
        handle: 100,
        pid: 201,
        created: 1,
    });
    let snapshot = Arc::new(RuntimeSnapshot {
        key: app.key(),
        running: true,
        events: (0..12)
            .map(|i| metin_core::RunEvent {
                id: i,
                activity: "Action".into(),
                message: format!("History record {i}"),
                level: if i % 3 == 0 {
                    metin_core::EventLevel::Info
                } else {
                    metin_core::EventLevel::Warning
                },
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    });
    app.snapshot = snapshot.clone();
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot,
        open_popup: false,
    });
    let size = egui::vec2(1280., 1500.);
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], size);
    for _ in 0..3 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], size);
    }
    let records = |output: &egui::FullOutput| {
        text_regions(output)
            .into_iter()
            .filter(|(text, _)| text.starts_with("History record "))
            .map(|(text, _)| text)
            .collect::<Vec<_>>()
    };
    let compact = records(&output);
    assert_eq!(compact.len(), 5);
    assert_eq!(compact[0], "History record 11");
    let pos = text_regions(&output)
        .into_iter()
        .find(|(text, _)| text == "Show full history")
        .unwrap()
        .1
        .center();
    for pressed in [true, false] {
        draw_at_size(
            &mut app,
            &ctx,
            &mut vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
            size,
        );
    }
    assert!(app.history_open);
    app.warnings_only = true;
    // Inspect the dialog's own layer: the unchanged Run behind its backdrop
    // deliberately still contains its unfiltered five-event summary.
    let render_dialog = |app: &mut MetinApp, events: Vec<egui::Event>| {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ui| app.run_history_dialog(ui.ctx()),
        )
    };
    let mut output = render_dialog(&mut app, vec![]);
    for _ in 0..3 {
        output = render_dialog(&mut app, vec![]);
    }
    let warnings = records(&output);
    assert!(warnings.len() >= 3);
    assert!(!warnings.iter().any(|text| {
        [
            "History record 9",
            "History record 6",
            "History record 3",
            "History record 0",
        ]
        .contains(&text.as_str())
    }));
    let pos = text_regions(&output)
        .into_iter()
        .find(|(text, _)| text == "History record 11")
        .unwrap()
        .1
        .center();
    let mut seen = warnings;
    for _ in 0..12 {
        let output = render_dialog(
            &mut app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::MouseWheel {
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0., -80.),
                    modifiers: Default::default(),
                },
            ],
        );
        seen.extend(records(&output));
    }
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 8, "All warnings remain accessible by scrolling");
    assert!(seen.contains(&"History record 1".into()));
    app.runtime.shutdown().unwrap();
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
        for tab in [
            Tab::Run,
            Tab::Calibration,
            Tab::Actions,
            Tab::Safety,
            Tab::Settings,
        ] {
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
            for tab in [
                Tab::Run,
                Tab::Calibration,
                Tab::Actions,
                Tab::Safety,
                Tab::Settings,
            ] {
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
        ("Actions", Tab::Actions),
        ("Run", Tab::Run),
        ("Settings", Tab::Settings),
        ("Safety", Tab::Safety),
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
    app.navigate(Tab::Actions);
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    let label = text_regions(&output)
        .into_iter()
        .find(|(s, _)| s == "1. Skill 1")
        .unwrap()
        .1;
    let [up, down, trash] = skill_row_actions(&output, label);
    assert_eq!(up.center().x, down.center().x);
    assert!(up.bottom() <= down.top());
    assert!(trash.left() > down.right());
    assert!(
        (down.bottom() - up.top() - 32.).abs() < 0.1,
        "up={up:?}, down={down:?}, trash={trash:?}"
    );
    assert!((trash.top() - up.top()).abs() < 0.1);
    assert!((trash.bottom() - down.bottom()).abs() < 0.1);
    click_position(&mut app, &ctx, down.center());
    assert_eq!(app.selected_skill, 1);
    assert_eq!(app.editor.profile.buffs.skills[1].shortcut, "F1");
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert!(!text_regions(&output).iter().any(|(s, _)| s == "Remove"));
    let label = text_regions(&output)
        .into_iter()
        .find(|(s, _)| s == "2. Skill 2")
        .unwrap()
        .1;
    click_position(
        &mut app,
        &ctx,
        skill_row_actions(&output, label)[2].center(),
    );
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
    app.navigate(Tab::Actions);
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
        Some(std::time::Instant::now() - std::time::Duration::from_secs(1)),
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
    assert!(app.start_blocker().unwrap().action == SetupAction::Calibration);
    app.editor.profile.name = "Valid".into();
    app.editor.profile.buffs.enabled = true;
    app.editor.profile.buffs.skills.push(metin_core::BuffSkill {
        mode: metin_core::BuffMode::Inactive,
        interval_seconds: None,
        ..Default::default()
    });
    assert!(app.start_blocker().unwrap().action == SetupAction::Actions(ActionsPage::Buffs));
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
    app.switch_profile(other.profile.id);
    assert!(app.mode == EditMode::World);
    assert!(!app.return_to_buffs);
    assert!(app.editor.profile.buffs.skills.is_empty());
    app.runtime.shutdown().unwrap();
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
    for label in [
        "Pause · F8",
        "Run",
        "Calibration",
        "Actions",
        "Safety",
        "Settings",
    ] {
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
    app.editor.profile.buffs.skills[0].mode = metin_core::BuffMode::Inactive;
    app.editor.profile.buffs.skills[0].interval_seconds = None;
    app.navigate(Tab::Actions);
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
    click_label(&mut app, &ctx, "Done & back to buffs");
    assert!(app.tab == Tab::Actions);
    assert_eq!(app.selected_skill, 0);
    app.runtime.shutdown().unwrap();
}
#[test]
fn wide_run_overview_keeps_game_preview_visible_without_scrolling() {
    let (mut app, ctx, _dir) = setup();
    app.navigate(Tab::Run);
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    for _ in 0..3 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    }
    let connection = text_regions(&output)
        .into_iter()
        .find(|(text, _)| text == "Game preview");
    assert!(
        connection.is_some_and(|(_, rect)| rect.bottom() < 740.),
        "Game preview should be visible in a wide Run overview"
    );
    app.runtime.shutdown().unwrap();
}

// User-review probes use the painted control locations, followed by real
// egui pointer/key events. No live game target is attached by these tests.
fn review_hover_label(app: &mut MetinApp, ctx: &egui::Context, label: &str) -> Vec<String> {
    let mut output = draw_at_size(app, ctx, &mut vec![], egui::vec2(1280., 820.));
    for _ in 0..3 {
        output = draw_at_size(app, ctx, &mut vec![], egui::vec2(1280., 820.));
    }
    let pos = text_regions(&output)
        .into_iter()
        .find(|(text, _)| text == label)
        .unwrap_or_else(|| panic!("Missing hover target: {label}"))
        .1
        .center();
    let start = ctx.input(|i| i.time);
    let mut texts = vec![];
    for frame in 0..4 {
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(start + f64::from(frame) + 1.),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280., 820.),
                )),
                events: if frame == 0 {
                    vec![egui::Event::PointerMoved(pos)]
                } else {
                    vec![]
                },
                ..Default::default()
            },
            |ui| app.render(ui),
        );
        texts.extend(text_regions(&output).into_iter().map(|(text, _)| text));
        output.textures_delta.clear();
    }
    texts
}

fn review_key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

#[test]
fn user_review_disabled_save_and_start_explain_their_state_on_actual_hover() {
    let (mut app, ctx, _dir) = setup();
    app.save().unwrap();
    // Keep the tooltip's input state stable while the real worker finishes
    // initial hotkey registration; this test exercises hover, not its timing.
    pin_windows(&mut app, vec![], 1);
    assert!(!app.has_unsaved_profile());
    let save_texts = review_hover_label(&mut app, &ctx, "Save");
    assert!(
        save_texts
            .iter()
            .any(|text| text.contains("Save all profile changes")),
        "Disabled Save should retain its explanation: {save_texts:?}"
    );
    let blocker = app.start_blocker().unwrap().message;
    let start_texts = review_hover_label(&mut app, &ctx, "Start");
    assert!(
        start_texts.contains(&blocker),
        "Disabled Start should show its actual blocker: {start_texts:?}"
    );
    click_label(&mut app, &ctx, "Start");
    assert!(!app.snapshot.running);
    assert!(app.target.is_none());
    app.runtime.shutdown().unwrap();
}

#[test]
fn user_review_four_calibration_steps_hover_select_and_preserve_edits() {
    let (mut app, ctx, _dir) = setup();
    let before = app.editor.profile.clone();
    for (label, mode, hint) in [
        (
            "1. Search area",
            EditMode::World,
            "Drag around the game area",
        ),
        (
            "2. Metin color",
            EditMode::Color,
            "Click a representative Metin pixel",
        ),
        (
            "3. Target panel",
            EditMode::Panel,
            "Draw around the target information panel",
        ),
        (
            "4. Reference crop",
            EditMode::Template,
            "Draw a small, distinctive fixed part",
        ),
    ] {
        let old = app.mode;
        let texts = review_hover_label(&mut app, &ctx, label);
        assert!(app.mode == old, "Hover must not select the step");
        assert!(app.hover_mode == Some(mode));
        assert!(
            texts.iter().any(|text| text.contains(hint)),
            "Missing step instructions: {texts:?}"
        );
        click_label(&mut app, &ctx, label);
        assert!(app.mode == mode);
    }
    for _ in 0..3 {
        draw(&mut app, &ctx, vec![]);
    }
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert!(
        !text_regions(&output)
            .iter()
            .any(|(text, _)| text == "Hide steps" || text == "Show steps")
    );
    assert!(app.mode == EditMode::Template);
    assert_eq!(app.editor.profile, before);
    app.runtime.shutdown().unwrap();
}

#[test]
fn user_review_shortcut_recording_cancel_reserved_key_and_clear_are_real_key_events() {
    let (mut app, ctx, _dir) = setup();
    app.add_skill();
    app.navigate(Tab::Actions);
    click_label(&mut app, &ctx, "F1");
    assert!(app.binding.is_some());
    draw(
        &mut app,
        &ctx,
        vec![review_key(egui::Key::Escape, Default::default())],
    );
    assert!(app.binding.is_none());
    assert_eq!(app.editor.profile.buffs.skills[0].shortcut, "F1");
    click_label(&mut app, &ctx, "F1");
    draw(
        &mut app,
        &ctx,
        vec![review_key(egui::Key::F7, Default::default())],
    );
    assert!(
        app.binding.is_some(),
        "A reserved key must not finish recording"
    );
    assert!(app.error.is_some());
    assert_eq!(app.editor.profile.buffs.skills[0].shortcut, "F1");
    draw(
        &mut app,
        &ctx,
        vec![review_key(
            egui::Key::A,
            egui::Modifiers {
                ctrl: true,
                ..Default::default()
            },
        )],
    );
    assert!(app.binding.is_none());
    assert_eq!(app.editor.profile.buffs.skills[0].shortcut, "Ctrl+A");
    click_label(&mut app, &ctx, "Ctrl+A");
    draw(
        &mut app,
        &ctx,
        vec![review_key(egui::Key::Backspace, Default::default())],
    );
    assert!(app.binding.is_none());
    assert!(app.editor.profile.buffs.skills[0].shortcut.is_empty());
    assert_eq!(app.editor.profile.buffs.skills.len(), 1);
    app.runtime.shutdown().unwrap();
}

#[test]
fn user_review_buff_mode_and_unit_menus_apply_to_the_selected_skill_only() {
    let (mut app, ctx, _dir) = setup();
    app.add_skill();
    app.add_skill();
    app.navigate(Tab::Actions);
    let first = app.editor.profile.buffs.skills[0].clone();
    app.editor.profile.buffs.skills[1].interval_seconds = Some(180.);
    click_label(&mut app, &ctx, "seconds");
    click_label(&mut app, &ctx, "minutes");
    assert!(app.units[1]);
    assert_eq!(
        app.editor.profile.buffs.skills[1].interval_seconds,
        Some(180.)
    );
    click_label(&mut app, &ctx, "Refresh periodically");
    click_label(&mut app, &ctx, "Activate when inactive");
    assert_eq!(
        app.editor.profile.buffs.skills[1].mode,
        metin_core::BuffMode::Inactive
    );
    assert_eq!(app.editor.profile.buffs.skills[1].interval_seconds, None);
    click_label(&mut app, &ctx, "Activate when inactive");
    click_label(&mut app, &ctx, "Refresh periodically");
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
fn user_review_pending_snapshot_timeout_and_error_dismiss_offer_recovery() {
    let (mut app, ctx, _dir) = setup();
    app.snapshot_requested = Some(std::time::Instant::now());
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert!(
        text_regions(&output)
            .iter()
            .any(|(text, _)| text == "Capturing...")
    );
    assert!(app.snapshot_requested.is_some());
    app.snapshot_requested = Some(std::time::Instant::now() - std::time::Duration::from_secs(4));
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert!(app.snapshot_requested.is_none());
    assert!(
        app.error
            .as_ref()
            .is_some_and(|text| text.contains("No fresh frame arrived"))
    );
    assert!(
        text_regions(&output)
            .iter()
            .any(|(text, _)| text == "Snapshot")
    );
    click_label(&mut app, &ctx, "Dismiss");
    assert!(app.error.is_none());
    assert!(
        app.editor.frozen.is_some(),
        "A failed capture must keep the previous editable snapshot"
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn user_review_saved_profile_switch_uses_the_combo_and_clears_previous_canvas() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.name = "First saved profile".into();
    app.editor.record();
    app.save().unwrap();
    let second = Profile::create("Second saved profile");
    app.store.save(&second, None).unwrap();
    app.profiles = app.store.list().unwrap();
    app.navigate(Tab::Run);
    click_label(&mut app, &ctx, "First saved profile");
    click_label(&mut app, &ctx, "Second saved profile");
    assert_eq!(app.editor.profile.id, second.id);
    assert!(!app.has_unsaved_profile());
    assert!(app.editor.frozen.is_none());
    assert!(app.target.is_none());
    assert!(!app.return_to_buffs);
    assert!(app.mode == EditMode::World);
    assert_eq!(app.store.selected().unwrap(), Some(second.id));
    app.runtime.shutdown().unwrap();
}

#[test]
fn user_review_keyboard_delete_and_redo_restore_the_same_selected_calibration_area() {
    let (mut app, ctx, _dir) = setup();
    let region = metin_core::Region {
        left: 0.1,
        top: 0.2,
        right: 0.8,
        bottom: 0.9,
    };
    app.editor.set_region(EditMode::World, region).unwrap();
    app.mode = EditMode::World;
    for _ in 0..3 {
        draw(&mut app, &ctx, vec![]);
    }
    draw(
        &mut app,
        &ctx,
        vec![review_key(egui::Key::Delete, Default::default())],
    );
    assert!(app.editor.profile.region.is_none());
    let ctrl = egui::Modifiers {
        ctrl: true,
        ..Default::default()
    };
    draw(&mut app, &ctx, vec![review_key(egui::Key::Z, ctrl)]);
    assert_eq!(app.editor.profile.region, Some(region));
    draw(&mut app, &ctx, vec![review_key(egui::Key::Y, ctrl)]);
    assert!(app.editor.profile.region.is_none());
    app.runtime.shutdown().unwrap();
}

#[test]
fn user_review_armed_countdown_is_visible_without_offering_another_start() {
    let (mut app, ctx, _dir) = setup();
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
    app.target = Some(TargetToken {
        handle: 1,
        pid: 2,
        created: 3,
    });
    app.snapshot = Arc::new(RuntimeSnapshot {
        key: app.key(),
        arming: true,
        state: "Armed".into(),
        reason: "Waiting for stable game focus. Start delay: 3 s; timeout: 12 s.".into(),
        capabilities: metin_platform::CapabilityReport {
            capture: true,
            identity: true,
            foreground: true,
            obstruction: true,
            manual_input: true,
            input: true,
            emergency_stop: true,
            reason: String::new(),
        },
        ..Default::default()
    });
    // Render the synthetic published state directly, without polling a worker
    // or sending Arm/Configure to any game window.
    let mut texts = vec![];
    for _ in 0..3 {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280., 820.),
                )),
                ..Default::default()
            },
            |ui| {
                app.toolbar(ui);
                app.run_view(ui);
            },
        );
        texts = text_regions(&output)
            .into_iter()
            .map(|(text, _)| text)
            .collect();
        output.textures_delta.clear();
    }
    assert!(
        texts.iter().any(|text| text.starts_with("Starting")),
        "Countdown must replace Start: {texts:?}"
    );
    assert!(texts.iter().any(|text| text.contains("Start delay: 3 s")));
    assert!(texts.iter().any(|text| text == "Pause · F8"));
    assert!(!texts.iter().any(|text| text == "Ready to start"));
    app.runtime.shutdown().unwrap();
}

#[test]
fn user_review_unknown_mount_confirmation_is_visible_and_applies_without_dirtying_profile() {
    let (mut app, ctx, dir) = setup();
    app.target = Some(TargetToken {
        handle: 1,
        pid: 2,
        created: 3,
    });
    app.snapshot = Arc::new(RuntimeSnapshot {
        key: app.key(),
        mounted: None,
        ..Default::default()
    });
    let before = app.editor.profile.clone();
    let old_revision = app.revision;
    let mut button_pos = None;
    for _ in 0..3 {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280., 820.),
                )),
                ..Default::default()
            },
            |ui| app.mount_view(ui),
        );
        let texts = text_regions(&output);
        assert!(
            texts
                .iter()
                .any(|(text, _)| text.contains("Mount state unknown"))
        );
        button_pos = texts
            .iter()
            .find(|(text, _)| text == "On foot")
            .map(|(_, rect)| rect.center());
        output.textures_delta.clear();
    }
    let pos = button_pos.expect("Unknown mount must offer an explicit On foot declaration");
    for pressed in [true, false] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280., 820.),
                )),
                events: vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
                ..Default::default()
            },
            |ui| app.mount_view(ui),
        );
        output.textures_delta.clear();
    }
    assert_eq!(app.revision, old_revision + 1);
    assert_eq!(app.editor.profile, before);
    assert!(!app.editor.dirty());
    assert!(!dir.path().join("ui-settings.json").exists());
    app.runtime.shutdown().unwrap();
}

#[test]
fn user_review_sampled_color_can_be_cleared_with_the_same_visible_delete_control() {
    let (mut app, ctx, _dir) = setup();
    app.editor.color([50, 50]).unwrap();
    let sampled = app.editor.profile.color.clone();
    click_label(&mut app, &ctx, "2. Metin color");
    assert!(app.mode == EditMode::Color);
    click_row_trash(&mut app, &ctx, "2. Metin color");
    assert!(
        app.editor.profile.color.is_none(),
        "The visible delete action must also clear the sampled color"
    );
    draw(
        &mut app,
        &ctx,
        vec![review_key(
            egui::Key::Z,
            egui::Modifiers {
                ctrl: true,
                ..Default::default()
            },
        )],
    );
    assert_eq!(app.editor.profile.color, sampled);
    app.runtime.shutdown().unwrap();
}

#[test]
fn user_review_nested_calibration_labels_remain_readable_when_hover_changes_focus() {
    let (mut app, ctx, _dir) = setup();
    // A reference crop often shares the panel's top-left corner. Both labels
    // must not be painted over the same pixels, including during checklist hover.
    app.editor.profile.panel.region = Some(metin_core::Region {
        left: 0.1,
        top: 0.1,
        right: 0.7,
        bottom: 0.4,
    });
    app.editor.profile.template_region = Some(metin_core::Region {
        left: 0.1,
        top: 0.1,
        right: 0.4,
        bottom: 0.25,
    });
    app.mode = EditMode::Panel;
    let labels = |output: &egui::FullOutput| {
        text_regions(output)
            .into_iter()
            .filter(|(text, _)| text == "Target panel" || text == "Reference crop")
            .collect::<Vec<_>>()
    };
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    for _ in 0..3 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    }
    let selected = labels(&output);
    assert!(selected.iter().any(|(text, _)| text == "Target panel"));
    for (i, (_, rect)) in selected.iter().enumerate() {
        for (_, other) in selected.iter().skip(i + 1) {
            assert!(
                !rect.intersects(*other),
                "Nested area labels overlap: {selected:?}"
            );
        }
    }
    let hover_pos = text_regions(&output)
        .into_iter()
        .find(|(text, _)| text.starts_with("4. Reference crop"))
        .expect("Reference crop checklist entry")
        .1
        .center();
    output = draw_at_size(
        &mut app,
        &ctx,
        &mut vec![egui::Event::PointerMoved(hover_pos)],
        egui::vec2(1280., 820.),
    );
    let hovered = labels(&output);
    assert!(app.hover_mode == Some(EditMode::Template));
    assert!(hovered.iter().any(|(text, _)| text == "Reference crop"));
    for (i, (_, rect)) in hovered.iter().enumerate() {
        for (_, other) in hovered.iter().skip(i + 1) {
            assert!(
                !rect.intersects(*other),
                "Hovered area labels overlap: {hovered:?}"
            );
        }
    }
    assert!(
        app.mode == EditMode::Panel,
        "Hover must not change selection"
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn feedback_one_wheel_notch_does_not_repeat_zoom_during_smoothing() {
    let (mut app, ctx, _dir) = setup();
    for _ in 0..3 {
        draw(&mut app, &ctx, vec![]);
    }
    let pos = app.canvas_rect.unwrap().center();
    draw(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::MouseWheel {
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Unknown,
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0., 1.),
                modifiers: Default::default(),
            },
        ],
    );
    for _ in 0..90 {
        draw(&mut app, &ctx, vec![]);
    }
    assert!(
        app.viewport.zoom > 1. && app.viewport.zoom <= 1.3,
        "One notch must be a bounded proportional zoom: {}",
        app.viewport.zoom
    );
    app.runtime.shutdown().unwrap();
}
#[test]
fn feedback_periodic_skills_do_not_offer_unused_icon_recognition() {
    let (mut app, ctx, _dir) = setup();
    app.add_skill();
    app.navigate(Tab::Actions);
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    for _ in 0..3 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    }
    assert!(
        !text_regions(&output)
            .iter()
            .any(|(s, _)| s == "Icon recognition")
    );
    app.editor.profile.buffs.skills[0].mode = metin_core::BuffMode::Inactive;
    for _ in 0..3 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    }
    assert!(
        text_regions(&output)
            .iter()
            .any(|(s, _)| s == "Icon recognition")
    );
    app.runtime.shutdown().unwrap();
}
#[test]
fn feedback_run_has_read_only_game_preview_instead_of_duplicate_connection_card() {
    let (mut app, ctx, _dir) = setup();
    app.navigate(Tab::Run);
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    for _ in 0..3 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    }
    let text = text_regions(&output);
    assert!(text.iter().any(|(s, _)| s == "Game preview"));
    assert!(!text.iter().any(|(s, _)| s == "Connected profile"));
    assert!(app.canvas_rect.is_none());
    app.runtime.shutdown().unwrap();
}

#[test]
fn feedback_tools_have_explicit_done_state_and_hit_test_cursors() {
    let (mut app, ctx, _dir) = setup();
    let region = metin_core::Region {
        left: 0.2,
        top: 0.2,
        right: 0.7,
        bottom: 0.7,
    };
    app.editor.set_region(EditMode::World, region).unwrap();
    for _ in 0..3 {
        draw(&mut app, &ctx, vec![]);
    }
    // Derive pixel positions from actual fixture size, not presumed DPI.
    let size = [
        app.editor.frozen.as_ref().unwrap().width,
        app.editor.frozen.as_ref().unwrap().height,
    ];
    let position = |app: &MetinApp, x: f64, y: f64| {
        let p = app
            .viewport
            .to_canvas([x * f64::from(size[0]), y * f64::from(size[1])]);
        app.canvas_rect.unwrap().min + egui::vec2(p[0] as f32, p[1] as f32)
    };
    for (x, y, cursor) in [
        (0.2, 0.2, egui::CursorIcon::ResizeNwSe),
        (0.45, 0.2, egui::CursorIcon::ResizeVertical),
        (0.45, 0.45, egui::CursorIcon::Move),
        (0.9, 0.9, egui::CursorIcon::Crosshair),
    ] {
        let pos = position(&app, x, y);
        let output = draw_at_size(
            &mut app,
            &ctx,
            &mut vec![egui::Event::PointerMoved(pos)],
            egui::vec2(1280., 820.),
        );
        assert_eq!(output.platform_output.cursor_icon, cursor);
    }
    click_label(&mut app, &ctx, "Done editing");
    assert!(!app.editing);
    let pos = position(&app, 0.3, 0.3);
    let before = app.editor.profile.clone();
    for pressed in [true, false] {
        draw(
            &mut app,
            &ctx,
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
    assert_eq!(app.editor.profile, before);
    assert!(app.gesture.is_none());
    click_label(&mut app, &ctx, "2. Metin color");
    assert!(app.editing);
    let pos = position(&app, 0.4, 0.4);
    let output = draw_at_size(
        &mut app,
        &ctx,
        &mut vec![egui::Event::PointerMoved(pos)],
        egui::vec2(1280., 820.),
    );
    assert_eq!(
        output.platform_output.cursor_icon,
        egui::CursorIcon::Crosshair
    );
    app.runtime.shutdown().unwrap();
}
#[test]
fn feedback_color_value_is_visible_and_excluded_area_announces_drawing() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.color = Some(metin_core::ColorConfig {
        hsv: [90, 200, 180],
        ..Default::default()
    });
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    for _ in 0..3 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    }
    assert!(
        text_regions(&output)
            .iter()
            .any(|(s, _)| s == "HSV 90, 200, 180")
    );
    click_label(&mut app, &ctx, "Draw excluded area");
    assert!(app.editing);
    assert!(app.mode == EditMode::Mask(None));
    output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert!(
        text_regions(&output)
            .iter()
            .any(|(s, _)| s == "Editing: Draw excluded area")
    );
    assert!(
        !text_regions(&output)
            .iter()
            .any(|(s, _)| s.contains("Draw excluded area: drag a rectangle"))
    );
    app.runtime.shutdown().unwrap();
}

fn click_row_trash(app: &mut MetinApp, ctx: &egui::Context, label: &str) {
    let mut output = draw_at_size(app, ctx, &mut vec![], egui::vec2(1280., 820.));
    for _ in 0..3 {
        output = draw_at_size(app, ctx, &mut vec![], egui::vec2(1280., 820.));
    }
    let rect = text_regions(&output)
        .into_iter()
        .find(|(text, _)| text == label)
        .unwrap()
        .1;
    let trash = row_trash_rect(&output, rect);
    let pos = trash.center();
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
fn refinement_row_trash_removes_that_mask_and_preserves_other_selection_and_undo() {
    let (mut app, ctx, _dir) = setup();
    let masks = [
        metin_core::Region {
            left: 0.1,
            top: 0.1,
            right: 0.2,
            bottom: 0.2,
        },
        metin_core::Region {
            left: 0.3,
            top: 0.3,
            right: 0.4,
            bottom: 0.4,
        },
        metin_core::Region {
            left: 0.5,
            top: 0.5,
            right: 0.6,
            bottom: 0.6,
        },
    ];
    for mask in masks {
        app.editor.set_region(EditMode::Mask(None), mask).unwrap();
    }
    app.edit_area(EditMode::Mask(Some(2)));
    click_row_trash(&mut app, &ctx, "Excluded area 1");
    assert_eq!(app.editor.profile.masks, vec![masks[1], masks[2]]);
    assert!(app.mode == EditMode::Mask(Some(1)));
    assert_eq!(app.editor.region(app.mode), Some(masks[2]));
    app.undo_profile();
    assert_eq!(app.editor.profile.masks, masks);
    app.edit_area(EditMode::Mask(Some(1)));
    click_row_trash(&mut app, &ctx, "Excluded area 2");
    assert!(!app.editing);
    assert!(app.mode == EditMode::Mask(None));
    assert_eq!(app.editor.profile.masks, vec![masks[0], masks[2]]);
    app.runtime.shutdown().unwrap();
}

#[test]
fn refinement_info_icon_has_hover_help_without_taking_layout_space_when_open() {
    let ctx = egui::Context::default();
    let mut pos = egui::Pos2::ZERO;
    let mut found = false;
    let mut before = None;
    for step in 0..5 {
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(f64::from(step)),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 560.),
                )),
                events: if step == 1 {
                    vec![egui::Event::PointerMoved(pos)]
                } else {
                    vec![]
                },
                ..Default::default()
            },
            |ui| {
                let r =
                    crate::ui_controls::help(ui, "Draw an excluded rectangle on the frozen image.");
                pos = r.rect.center();
                if let Some(rect) = before {
                    assert_eq!(r.rect, rect);
                }
                before = Some(r.rect);
            },
        );
        found |= text_regions(&output)
            .iter()
            .any(|(s, _)| s == "Draw an excluded rectangle on the frozen image.");
        output.textures_delta.clear();
    }
    assert!(found);
}

#[test]
fn feedback_color_picker_preserves_opencv_srgb_instead_of_linear_brightness() {
    for rgb in [
        [255, 0, 0],
        [0, 255, 0],
        [0, 0, 255],
        [90, 160, 210],
        [123, 55, 201],
        [180, 180, 180],
        [0, 0, 0],
    ] {
        let frame = metin_platform::Frame::new(1, 1, vec![rgb[2], rgb[1], rgb[0]]).unwrap();
        let hsv = metin_vision::pick_color(&frame, [0, 0]).unwrap();
        let displayed = crate::ui_controls::sampled_hsv_to_srgb(hsv);
        for i in 0..3 {
            assert!(
                displayed[i].abs_diff(rgb[i]) <= 4,
                "Sampled swatch differs: {rgb:?} -> {hsv:?} -> {displayed:?}"
            );
        }
        let selected = crate::ui_controls::sampled_srgb_to_hsv(rgb);
        let hue = selected[0].abs_diff(hsv[0]);
        assert!(hue.min(180 - hue) <= 1);
        assert!(selected[1].abs_diff(hsv[1]) <= 1);
        assert!(selected[2].abs_diff(hsv[2]) <= 1);
    }
}

#[test]
fn feedback_run_preview_draws_distinct_current_next_and_other_markers_without_editing() {
    let (mut app, ctx, _dir) = setup();
    app.navigate(Tab::Run);
    app.target = Some(TargetToken {
        handle: 1,
        pid: 2,
        created: 3,
    });
    app.snapshot = Arc::new(RuntimeSnapshot {
        key: app.key(),
        frame: Some(metin_platform::Frame::new(300, 300, vec![0; 300 * 300 * 3]).unwrap()),
        captured_at: Some(std::time::Instant::now()),
        running: true,
        selected_target: Some([50, 50]),
        next_target: Some([150, 150]),
        candidates: [50, 150, 250]
            .map(|p| metin_core::Candidate {
                x: p - 20,
                y: p - 20,
                width: 40,
                height: 40,
                point: [p, p],
            })
            .to_vec(),
        ..Default::default()
    });
    fn rectangles(shape: &egui::Shape, colors: &mut Vec<egui::Color32>) {
        match shape {
            egui::Shape::Rect(r) => colors.push(r.stroke.color),
            egui::Shape::Vec(shapes) => {
                for s in shapes {
                    rectangles(s, colors);
                }
            }
            _ => {}
        }
    }
    let before = app.editor.profile.clone();
    let mut colors = vec![];
    for _ in 0..3 {
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
        colors.clear();
        for shape in &output.shapes {
            rectangles(&shape.shape, &mut colors);
        }
        output.textures_delta.clear();
    }
    for color in [widgets::AMBER, widgets::GREEN, widgets::BLUE] {
        assert_eq!(
            colors.iter().filter(|c| **c == color).count(),
            1,
            "Each detection class needs one distinct rectangle"
        );
    }
    assert_eq!(app.editor.profile, before);
    assert!(app.canvas_rect.is_none());
    assert!(!app.editing);
    app.runtime.shutdown().unwrap();
}

fn row_trash_rect(output: &egui::FullOutput, label: egui::Rect) -> egui::Rect {
    fn visit(shape: &egui::Shape, row: egui::Rect, found: &mut Vec<egui::Rect>) {
        match shape {
            egui::Shape::Rect(s)
                if (s.rect.width() - 26.).abs() < 5.
                    && (s.rect.height() - 28.).abs() < 5.
                    && (s.rect.center().y - row.center().y).abs() < 4.
                    && s.rect.left() > row.right() =>
            {
                found.push(s.rect)
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    visit(shape, row, found);
                }
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    for shape in &output.shapes {
        visit(&shape.shape, label, &mut found);
    }
    *found
        .iter()
        .max_by(|a, b| a.right().total_cmp(&b.right()))
        .expect("row trash button")
}

#[test]
fn toolbar_sizes_toggle_and_right_alignment_keep_canvas_stable() {
    let (mut app, ctx, _dir) = setup();
    assert!(!app.show_sizes);
    for _ in 0..4 {
        draw(&mut app, &ctx, vec![]);
    }
    let before = app.canvas_rect.unwrap();
    click_label(&mut app, &ctx, "Sizes (px)");
    assert!(app.show_sizes);
    draw(&mut app, &ctx, vec![]);
    assert_eq!(app.canvas_rect.unwrap(), before);
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    let texts = text_regions(&output);
    let rect = |label: &str| texts.iter().find(|(s, _)| s == label).unwrap().1;
    assert!((rect("Done editing").center().y - rect("Fit").center().y).abs() < 4.);
    assert!(rect("Done editing").right() < rect("Detections").left());
    let trash = [
        "1. Search area",
        "2. Metin color",
        "3. Target panel",
        "4. Reference crop",
    ]
    .map(|label| row_trash_rect(&output, rect(label)));
    assert!(
        trash
            .iter()
            .all(|r| (r.right() - trash[0].right()).abs() < 1.)
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn buff_capture_shows_context_switches_states_and_returns_to_same_skill() {
    let (mut app, ctx, _dir) = setup();
    app.editor.profile.buffs.skills.push(metin_core::BuffSkill {
        name: "Aura".into(),
        mode: metin_core::BuffMode::Inactive,
        interval_seconds: None,
        ..Default::default()
    });
    app.edit_area(EditMode::BuffIcon {
        index: 0,
        inactive: false,
    });
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert!(text_regions(&output).iter().any(|(s, _)| s == "Aura"));
    click_label(&mut app, &ctx, "Inactive icon (optional)");
    assert_eq!(
        app.mode,
        EditMode::BuffIcon {
            index: 0,
            inactive: true
        }
    );
    let region = metin_core::Region {
        left: 0.1,
        top: 0.1,
        right: 0.2,
        bottom: 0.2,
    };
    app.editor
        .freeze(
            metin_platform::Frame::new(
                128,
                128,
                (0..128 * 128)
                    .flat_map(|i| {
                        if (i / 128 + i % 128) % 2 == 0 {
                            [0, 0, 0]
                        } else {
                            [255, 255, 255]
                        }
                    })
                    .collect(),
            )
            .unwrap(),
        )
        .unwrap();
    app.editor.profile.buffs.icon_region = Some(metin_core::Region {
        left: 0.,
        top: 0.,
        right: 1.,
        bottom: 1.,
    });
    app.editor.set_region(app.mode, region).unwrap();
    draw(&mut app, &ctx, vec![]);
    assert_eq!(app.tab, Tab::Calibration);
    let mut small = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(800., 560.));
    for _ in 0..3 {
        small = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(800., 560.));
    }
    let labels = text_regions(&small);
    assert!(
        labels
            .iter()
            .any(|(text, rect)| text == "Done & back to buffs" && rect.bottom() < 520.)
    );
    assert!(!labels.iter().any(|(text, _)| text == "Profile name"));
    click_label(&mut app, &ctx, "Done & back to buffs");
    assert_eq!(app.tab, Tab::Actions);
    assert!(!app.editing && !app.return_to_buffs);
    assert_eq!(app.selected_skill, 0);
    assert!(app.editor.profile.buffs.skills[0].inactive_icon.is_some());
    app.editor.profile.buffs.skills[0]
        .inactive_icon
        .as_mut()
        .unwrap()
        .region = None;
    assert!(app.calibration_ready(EditMode::BuffIcon {
        index: 0,
        inactive: true
    }));
    assert!(app.editor.dirty());
    app.runtime.shutdown().unwrap();
}

fn click_position(app: &mut MetinApp, ctx: &egui::Context, pos: egui::Pos2) {
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
fn skill_row_actions(output: &egui::FullOutput, label: egui::Rect) -> [egui::Rect; 3] {
    fn visit(shape: &egui::Shape, label: egui::Rect, found: &mut Vec<egui::Rect>) {
        match shape {
            egui::Shape::Rect(s)
                if (25. ..=33.).contains(&s.rect.width())
                    && (14. ..=33.).contains(&s.rect.height())
                    && s.rect.left() > label.right()
                    && s.rect.right() < label.right() + 170.
                    && s.rect.center().y > label.center().y - 17.
                    && s.rect.center().y < label.center().y + 17. =>
            {
                found.push(s.rect)
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    visit(shape, label, found);
                }
            }
            _ => {}
        }
    }
    let mut found = vec![];
    for shape in &output.shapes {
        visit(&shape.shape, label, &mut found);
    }
    found.sort_by(|a, b| {
        a.left()
            .total_cmp(&b.left())
            .then(a.top().total_cmp(&b.top()))
    });
    found.dedup();
    found.try_into().expect("up/down/trash in the skill row")
}
#[test]
fn skill_row_trash_deletes_its_own_skill_and_last_skill_can_be_restored() {
    let (mut app, ctx, _dir) = setup();
    for shortcut in ["F1", "F2"] {
        app.editor.profile.buffs.skills.push(metin_core::BuffSkill {
            shortcut: shortcut.into(),
            ..Default::default()
        });
    }
    app.editor.record();
    app.selected_skill = 1;
    app.navigate(Tab::Actions);
    for _ in 0..3 {
        draw(&mut app, &ctx, vec![]);
    }
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    let label = text_regions(&output)
        .into_iter()
        .find(|(s, _)| s == "1. Skill 1")
        .unwrap()
        .1;
    click_position(
        &mut app,
        &ctx,
        skill_row_actions(&output, label)[2].center(),
    );
    assert_eq!(app.selected_skill, 0);
    assert_eq!(app.editor.profile.buffs.skills[0].shortcut, "F2");
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    let label = text_regions(&output)
        .into_iter()
        .find(|(s, _)| s == "1. Skill 1")
        .unwrap()
        .1;
    click_position(
        &mut app,
        &ctx,
        skill_row_actions(&output, label)[2].center(),
    );
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert!(app.editor.profile.buffs.skills.is_empty());
    assert!(
        text_regions(&output)
            .iter()
            .any(|(s, _)| s == "No skills yet")
    );
    app.undo_profile();
    assert_eq!(app.editor.profile.buffs.skills[0].shortcut, "F2");
    app.runtime.shutdown().unwrap();
}

#[test]
fn saved_theme_and_live_theme_changes_send_matching_native_window_icons() {
    let (mut app, ctx, dir) = setup();
    let before = app.editor.profile.clone();
    for turn in 0..3 {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.label("Theme test");
            if turn > 0 {
                app.toggle_theme(&ctx);
            }
        });
        output.textures_delta.clear();
        let expected = if turn == 1 { "dark" } else { "light" };
        assert_eq!(app.settings.theme, expected);
        let icon = output
            .viewport_output
            .values()
            .flat_map(|v| &v.commands)
            .filter_map(|command| match command {
                egui::ViewportCommand::Icon(Some(icon)) => Some(icon),
                _ => None,
            })
            .next_back()
            .expect("native window icon command");
        let rgb = if expected == "dark" { 255 } else { 0 };
        assert!(
            icon.rgba
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[3] > 0)
                .all(|p| p[..3] == [rgb, rgb, rgb])
        );
        if turn > 0 {
            assert_eq!(UISettings::load(dir.path()).unwrap().theme, expected);
        }
        assert_eq!(app.editor.profile, before);
    }
    app.runtime.shutdown().unwrap();
}

#[cfg(windows)]
#[test]
fn native_windows_taskbar_icon_updates_black_white_black() {
    use winit::{
        application::ApplicationHandler,
        event_loop::{ActiveEventLoop, EventLoop},
        platform::windows::EventLoopBuilderExtWindows,
        raw_window_handle::{HasWindowHandle, RawWindowHandle},
    };
    struct Probe {
        app: MetinApp,
        ctx: egui::Context,
    }
    impl ApplicationHandler for Probe {
        fn resumed(&mut self, events: &ActiveEventLoop) {
            let window = Arc::new(
                events
                    .create_window(winit::window::Window::default_attributes().with_visible(false))
                    .unwrap(),
            );
            let RawWindowHandle::Win32(handle) = window.window_handle().unwrap().as_raw() else {
                panic!("Win32 window required")
            };
            self.app.native_window = Some(window);
            for dark in [false, true, false] {
                self.app.apply_theme(&self.ctx, dark).unwrap();
                let pixels = unsafe { taskbar_pixels(handle.hwnd.get() as _) };
                let opaque = pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|p| p[3] > 200)
                    .collect::<Vec<_>>();
                assert!(!opaque.is_empty());
                let rgb = if dark { 255 } else { 0 };
                assert!(opaque.iter().all(|p| p[..3] == [rgb, rgb, rgb]));
            }
            self.app.native_window = None;
            events.exit();
        }
        fn window_event(
            &mut self,
            _: &ActiveEventLoop,
            _: winit::window::WindowId,
            _: winit::event::WindowEvent,
        ) {
        }
    }
    // Read the actual ICON_BIG bitmap. The window and its icon are owned by
    // this test; GetIconInfo creates bitmap copies that must be released.
    unsafe fn taskbar_pixels(hwnd: windows_sys::Win32::Foundation::HWND) -> Vec<u8> {
        use windows_sys::Win32::{Graphics::Gdi::*, UI::WindowsAndMessaging::*};
        unsafe {
            let icon = SendMessageW(hwnd, WM_GETICON, ICON_BIG as usize, 0) as HICON;
            assert!(!icon.is_null(), "native taskbar icon is missing");
            let mut icon_info: ICONINFO = std::mem::zeroed();
            assert_ne!(GetIconInfo(icon, &mut icon_info), 0);
            let mut bitmap: BITMAP = std::mem::zeroed();
            assert_ne!(
                GetObjectW(
                    icon_info.hbmColor,
                    std::mem::size_of::<BITMAP>() as i32,
                    (&mut bitmap as *mut BITMAP).cast()
                ),
                0
            );
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = bitmap.bmWidth;
            info.bmiHeader.biHeight = -bitmap.bmHeight;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            let mut pixels = vec![0u8; (bitmap.bmWidth * bitmap.bmHeight * 4) as usize];
            let dc = GetDC(std::ptr::null_mut());
            let lines = GetDIBits(
                dc,
                icon_info.hbmColor,
                0,
                bitmap.bmHeight as u32,
                pixels.as_mut_ptr().cast(),
                &mut info,
                DIB_RGB_COLORS,
            );
            ReleaseDC(std::ptr::null_mut(), dc);
            DeleteObject(icon_info.hbmColor);
            DeleteObject(icon_info.hbmMask);
            assert_eq!(lines, bitmap.bmHeight);
            pixels
        }
    }
    let (app, ctx, _dir) = setup();
    let events = EventLoop::builder().with_any_thread(true).build().unwrap();
    let mut probe = Probe { app, ctx };
    events.run_app(&mut probe).unwrap();
    probe.app.runtime.shutdown().unwrap();
}

#[test]
fn theme_toggle_stays_at_right_edge_and_switches_with_target_tooltip() {
    let (mut app, ctx, _dir) = setup();
    for width in [800., 1280.] {
        let size = egui::vec2(width, 820.);
        let mut output = draw_at_size(&mut app, &ctx, &mut vec![], size);
        for _ in 0..3 {
            output = draw_at_size(&mut app, &ctx, &mut vec![], size);
        }
        let mut buttons = vec![];
        fn visit(shape: &egui::Shape, buttons: &mut Vec<egui::Rect>) {
            match shape {
                egui::Shape::Rect(r)
                    if (r.rect.width() - 32.).abs() < 1.
                        && (r.rect.height() - 32.).abs() < 1.
                        && r.rect.center().y < 50. =>
                {
                    buttons.push(r.rect)
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        visit(shape, buttons);
                    }
                }
                _ => {}
            }
        }
        for shape in &output.shapes {
            visit(&shape.shape, &mut buttons);
        }
        let button = buttons
            .into_iter()
            .max_by(|a, b| a.right().total_cmp(&b.right()))
            .unwrap();
        assert!((button.right() - (width - 8.)).abs() < 2., "{button:?}");
        let before = app.settings.theme.clone();
        let hint = if before == "dark" {
            "Switch to light mode"
        } else {
            "Switch to dark mode"
        };
        let start = ctx.input(|i| i.time);
        let mut tooltip = false;
        for frame in 0..4 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(start + f64::from(frame) + 1.),
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                    events: if frame == 0 {
                        vec![egui::Event::PointerMoved(button.center())]
                    } else {
                        vec![]
                    },
                    ..Default::default()
                },
                |ui| app.render(ui),
            );
            tooltip |= text_regions(&output).iter().any(|(text, _)| text == hint);
            output.textures_delta.clear();
        }
        assert!(tooltip, "missing target-mode tooltip: {hint}");
        for pressed in [true, false] {
            draw_at_size(
                &mut app,
                &ctx,
                &mut vec![
                    egui::Event::PointerMoved(button.center()),
                    egui::Event::PointerButton {
                        pos: button.center(),
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
                size,
            );
        }
        assert_ne!(app.settings.theme, before);
    }
    app.runtime.shutdown().unwrap();
}

#[test]
fn actions_subpages_and_safety_preserve_edits_and_expose_all_controls() {
    let (mut app, ctx, _dir) = setup();
    app.add_skill();
    app.navigate(Tab::Actions);
    click_label(&mut app, &ctx, "Loot");
    assert_eq!(app.actions_page, ActionsPage::Loot);
    click_label(&mut app, &ctx, "Automatic loot");
    assert!(app.editor.profile.loot.enabled);
    click_label(&mut app, &ctx, "Mount");
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(800., 560.));
    assert!(
        text_regions(&output)
            .iter()
            .any(|(s, _)| s == "Mount state")
    );
    click_label(&mut app, &ctx, "Safety");
    click_label(&mut app, &ctx, "Stop after active time");
    click_label(&mut app, &ctx, "Take regular breaks");
    assert!(app.editor.profile.safety.stop_enabled);
    assert!(app.editor.profile.safety.breaks_enabled);
    assert!(app.editor.profile.loot.enabled);
    assert_eq!(app.editor.profile.buffs.skills.len(), 1);
    assert!(app.editor.dirty());
    app.runtime.shutdown().unwrap();
}
#[test]
fn loot_key_recording_uses_reserved_key_validation_and_cancels_on_navigation() {
    let (mut app, ctx, _dir) = setup();
    app.navigate(Tab::Actions);
    click_label(&mut app, &ctx, "Loot");
    click_label(&mut app, &ctx, "Automatic loot");
    click_label(&mut app, &ctx, "Z");
    draw(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Y,
            physical_key: Some(egui::Key::Y),
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    assert_eq!(app.editor.profile.loot.shortcut, "Y");
    click_label(&mut app, &ctx, "Y");
    click_label(&mut app, &ctx, "Safety");
    assert!(app.binding.is_none());
    app.runtime.shutdown().unwrap();
}
#[test]
fn every_actions_page_and_safety_fit_both_themes_at_minimum_size_without_mutation() {
    let (mut app, ctx, _dir) = setup();
    app.add_skill();
    let before = app.editor.profile.clone();
    for dark in [false, true] {
        ctx.set_visuals(if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        app.tab = Tab::Actions;
        for page in [ActionsPage::Buffs, ActionsPage::Loot, ActionsPage::Mount] {
            app.actions_page = page;
            for _ in 0..3 {
                draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(800., 560.));
            }
        }
        app.tab = Tab::Safety;
        for _ in 0..3 {
            draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(800., 560.));
        }
    }
    assert_eq!(app.editor.profile, before);
    app.runtime.shutdown().unwrap();
}

#[test]
fn break_countdown_and_history_jump_are_visible_and_do_not_offer_another_start() {
    let (mut app, ctx, _dir) = setup();
    let snapshot = Arc::new(RuntimeSnapshot {
        events: (0..8)
            .map(|i| metin_core::RunEvent {
                id: i,
                at_seconds: i as f64,
                level: metin_core::EventLevel::Info,
                activity: "Action".into(),
                message: format!("Recorded event {i}"),
                ..Default::default()
            })
            .collect(),
        running: true,
        state: "On break".into(),
        schedule: metin_core::ScheduleSnapshot {
            break_ends_in: Some(60.),
            ..Default::default()
        },
        ..Default::default()
    });
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot,
        open_popup: false,
    });
    app.tab = Tab::Run;
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    for _ in 0..3 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    }
    let texts = text_regions(&output);
    assert!(texts.iter().any(|(s, _)| s == "Continues in 00:01:00"));
    assert!(!texts.iter().any(|(s, _)| s == "Before you start"));
    click_label(&mut app, &ctx, "View event history (8)");
    for _ in 0..20 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    }
    assert!(
        text_regions(&output)
            .iter()
            .filter(|(s, _)| s.starts_with("Recorded event "))
            .count()
            >= 3,
        "History must show several events after jumping to it: {:?}",
        text_regions(&output)
    );
    assert!(app.snapshot.running);
    app.runtime.shutdown().unwrap();
}

#[test]
fn clock_only_schedule_explains_pending_start_instead_of_claiming_no_limits() {
    let mut profile = Profile::create("Clock only");
    profile.safety.clock_enabled = true;
    let schedule = metin_core::Controller::new(profile).schedule(0.);
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        safety::schedule_labels(ui, &schedule, false)
    });
    let text = text_regions(&output);
    output.textures_delta.clear();
    assert!(
        text.iter()
            .any(|(s, _)| s.contains("Local-time stop: 23:00"))
    );
    assert!(!text.iter().any(|(s, _)| s.contains("No automatic stop")));
}

#[test]
fn remaining_pages_are_reachable_and_keep_profile_values_across_navigation() {
    let (mut app, ctx, _dir) = setup();
    app.navigate(Tab::Actions);
    click_label(&mut app, &ctx, "Periodic actions");
    click_label(&mut app, &ctx, "Add action");
    assert_eq!(app.editor.profile.periodic_actions.len(), 1);
    assert_eq!(app.editor.profile.periodic_actions[0].name, "New action");
    click_label(&mut app, &ctx, "Enable this action");
    click_label(&mut app, &ctx, "Auto-puller");
    click_label(&mut app, &ctx, "Automatic puller");
    click_label(&mut app, &ctx, "Repeat during combat");
    assert_eq!(
        app.editor.profile.puller.mode,
        metin_core::PullerMode::CombatPeriodic
    );
    click_label(&mut app, &ctx, "Settings");
    for label in [
        "Combat & timing",
        "Appearance & performance",
        "Diagnostics",
        "Detection",
    ] {
        click_label(&mut app, &ctx, label);
    }
    assert!(app.editor.profile.periodic_actions[0].enabled);
    click_label(&mut app, &ctx, "Actions");
    click_label(&mut app, &ctx, "Visual rules");
    click_label(&mut app, &ctx, "Add rule");
    assert!(!app.editor.profile.safety.rules[0].enabled);
    click_label(&mut app, &ctx, "Capture reference");
    assert_eq!(app.tab, Tab::Calibration);
    assert!(app.return_to_safety);
    click_label(&mut app, &ctx, "Done & back to Actions");
    assert_eq!(app.actions_page, ActionsPage::VisualRules);
    assert_eq!(app.tab, Tab::Actions);
    app.runtime.shutdown().unwrap();
}
#[test]
fn all_remaining_subpages_render_in_both_themes_at_small_size_without_edits() {
    let (mut app, ctx, _dir) = setup();
    let before = app.editor.profile.clone();
    for dark in [false, true] {
        ctx.set_visuals(if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        app.tab = Tab::Actions;
        for page in [
            ActionsPage::Periodic,
            ActionsPage::Puller,
            ActionsPage::VisualRules,
        ] {
            app.actions_page = page;
            for _ in 0..3 {
                draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(800., 560.));
            }
        }
        app.tab = Tab::Settings;
        for page in [
            SettingsPage::Detection,
            SettingsPage::Combat,
            SettingsPage::Appearance,
            SettingsPage::Diagnostics,
        ] {
            app.settings_page = page;
            for _ in 0..3 {
                draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(800., 560.));
            }
        }
        app.tab = Tab::Safety;
        for page in [
            SafetyPage::Schedule,
            SafetyPage::Recovery,
            SafetyPage::Notifications,
        ] {
            app.safety_page = page;
            for _ in 0..3 {
                draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(800., 560.));
            }
        }
    }
    assert_eq!(app.editor.profile, before);
    app.runtime.shutdown().unwrap();
}
#[test]
fn lite_preview_preserves_capture_settings_and_reuses_one_coherent_snapshot() {
    let (mut app, ctx, _dir) = setup();
    let before = app.editor.profile.timing.clone();
    app.settings.lite_mode = true;
    let first = app.presentation_snapshot();
    let mut newer = (*first).clone();
    newer.frame_id = 42;
    app.snapshot = Arc::new(newer);
    let held = app.presentation_snapshot();
    assert_eq!(held.frame_id, first.frame_id);
    app.preview_updated = std::time::Instant::now() - std::time::Duration::from_secs(2);
    assert_eq!(app.presentation_snapshot().frame_id, 42);
    assert_eq!(app.editor.profile.timing, before);
    draw(&mut app, &ctx, vec![]);
    app.runtime.shutdown().unwrap();
}

#[test]
fn incomplete_imported_safety_reference_does_not_offer_a_snapshot_test() {
    let (mut app, ctx, _dir) = setup();
    app.editor
        .profile
        .safety
        .rules
        .push(metin_core::SafetyRule::new("Imported"));
    app.editor
        .freeze(
            metin_platform::Frame::new(100, 100, (0..30000).map(|n| (n % 251) as u8).collect())
                .unwrap(),
        )
        .unwrap();
    app.editor
        .set_region(
            EditMode::SafetyReference(0),
            metin_core::Region {
                left: 0.1,
                top: 0.1,
                right: 0.2,
                bottom: 0.2,
            },
        )
        .unwrap();
    app.editor.profile.safety.rules[0]
        .reference
        .as_mut()
        .unwrap()
        .region = None;
    app.tab = Tab::Actions;
    app.actions_page = ActionsPage::VisualRules;
    click_label(&mut app, &ctx, "Test on snapshot");
    assert!(app.snapshot_score.is_none());
    assert!(!app.editor.profile.safety.rules[0].enabled);
    app.runtime.shutdown().unwrap();
}

#[test]
fn disabled_puller_cannot_change_mode_or_record_a_key_but_preserves_its_values() {
    let (mut app, ctx, _dir) = setup();
    app.tab = Tab::Actions;
    app.actions_page = ActionsPage::Puller;
    let before = app.editor.profile.puller.clone();
    click_label(&mut app, &ctx, "Repeat during combat");
    click_label(&mut app, &ctx, "F4");
    assert!(app.binding.is_none());
    assert_eq!(app.editor.profile.puller, before);
    click_label(&mut app, &ctx, "Automatic puller");
    click_label(&mut app, &ctx, "Repeat during combat");
    assert_eq!(
        app.editor.profile.puller.mode,
        metin_core::PullerMode::CombatPeriodic
    );
    click_label(&mut app, &ctx, "Automatic puller");
    click_label(&mut app, &ctx, "At combat start");
    assert_eq!(
        app.editor.profile.puller.mode,
        metin_core::PullerMode::CombatPeriodic
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn combat_options_show_only_relevant_fields_and_record_space() {
    let (mut app, ctx, _dir) = setup();
    app.tab = Tab::Actions;
    app.actions_page = ActionsPage::Puller;
    click_label(&mut app, &ctx, "Automatic puller");
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert!(
        !text_regions(&output)
            .iter()
            .any(|(t, _)| t == "Repeat interval (s)")
    );
    click_label(&mut app, &ctx, "Repeat during combat");
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert!(
        text_regions(&output)
            .iter()
            .any(|(t, _)| t == "Repeat interval (s)")
    );
    click_label(&mut app, &ctx, "Out-of-target attack");
    click_label(&mut app, &ctx, "Attack after a target ends");
    click_label(&mut app, &ctx, "Space");
    draw(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Space,
            physical_key: Some(egui::Key::Space),
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    assert_eq!(app.editor.profile.out_of_target.shortcut, "Space");
    assert!(app.binding.is_none());
    draw(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Space,
            physical_key: Some(egui::Key::Space),
            pressed: false,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    assert!(app.binding.is_none());
    click_label(&mut app, &ctx, "Attack when searching takes too long");
    assert!(app.editor.profile.out_of_target.during_search);
    let before = app.editor.profile.clone();
    for dark in [false, true] {
        widgets::theme(&ctx, dark);
        draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(800., 560.));
    }
    assert_eq!(app.editor.profile, before);
    app.runtime.shutdown().unwrap();
}

#[test]
fn small_attack_form_scrolls_to_the_search_limits_and_event_link_opens_it() {
    let (mut app, ctx, _dir) = setup();
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot: Arc::new(RuntimeSnapshot {
            events: vec![metin_core::RunEvent {
                activity: "Out-of-target attack".into(),
                message: "Attack key released.".into(),
                destination: metin_core::EventDestination::OutOfTarget,
                ..Default::default()
            }],
            ..Default::default()
        }),
        open_popup: false,
    });
    app.tab = Tab::Run;
    click_label(&mut app, &ctx, "View event history (1)");
    for _ in 0..20 {
        draw(&mut app, &ctx, vec![]);
    }
    click_label(&mut app, &ctx, "Open setup");
    assert_eq!(app.tab, Tab::Actions);
    assert_eq!(app.actions_page, ActionsPage::OutOfTarget);
    app.editor.profile.out_of_target.during_search = true;
    let size = egui::vec2(800., 560.);
    for _ in 0..3 {
        draw_at_size(&mut app, &ctx, &mut vec![], size);
    }
    draw_at_size(
        &mut app,
        &ctx,
        &mut vec![
            egui::Event::PointerMoved(egui::pos2(600., 450.)),
            egui::Event::MouseWheel {
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Unknown,
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., -1200.),
                modifiers: Default::default(),
            },
        ],
        size,
    );
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], size);
    for _ in 0..20 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], size);
    }
    let limits = text_regions(&output)
        .into_iter()
        .find(|(t, _)| t == "Maximum search attacks")
        .expect("search limit rendered");
    assert!(
        limits.1.top() > 137. && limits.1.bottom() < 520.,
        "Limit must be accessible after scrolling: {:?}",
        limits.1
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn periodic_key_click_first_run_and_capture_use_one_input() {
    let (mut app, ctx, _dir) = setup();
    app.navigate(Tab::Actions);
    app.actions_page = ActionsPage::Periodic;
    click_label(&mut app, &ctx, "Add action");
    assert!(!app.editor.profile.periodic_actions[0].enabled);
    assert_eq!(
        app.editor.profile.periodic_actions[0].first_run,
        metin_core::FirstRun::AfterInterval
    );
    click_label(&mut app, &ctx, "Enable this action");
    click_label(&mut app, &ctx, "Immediately when available");
    assert_eq!(
        app.editor.profile.periodic_actions[0].first_run,
        metin_core::FirstRun::Immediate
    );
    click_label(&mut app, &ctx, "F4");
    draw(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::F6,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }],
    );
    assert_eq!(
        app.editor.profile.periodic_actions[0].input.shortcut(),
        Some("F6")
    );
    click_label(&mut app, &ctx, "Click");
    assert!(!app.editor.profile.periodic_actions[0].enabled);
    let id = app.editor.profile.periodic_actions[0].id;
    click_label(&mut app, &ctx, "Select click target");
    assert_eq!(app.mode, EditMode::PeriodicClick(id));
    app.editor
        .freeze(metin_platform::Frame::new(100, 100, vec![50; 30000]).unwrap())
        .unwrap();
    let region = metin_core::Region {
        left: 0.2,
        top: 0.3,
        right: 0.4,
        bottom: 0.5,
    };
    app.editor.set_region(app.mode, region).unwrap();
    assert_eq!(app.editor.region(app.mode), Some(region));
    click_label(&mut app, &ctx, "Done & back to Actions");
    assert_eq!(app.tab, Tab::Actions);
    assert_eq!(app.actions_page, ActionsPage::Periodic);
    click_label(&mut app, &ctx, "Enable this action");
    assert!(app.editor.profile.periodic_actions[0].enabled);
    app.editor
        .freeze(metin_platform::Frame::new(200, 100, vec![50; 60000]).unwrap())
        .unwrap();
    assert_eq!(
        app.editor.profile.periodic_actions[0].input,
        metin_core::ActionInput::Click { region: None }
    );
    assert!(!app.editor.profile.periodic_actions[0].enabled);
    app.runtime.shutdown().unwrap();
}

#[test]
fn visual_execution_controls_keep_disabled_fields_and_capture_return() {
    let (mut app, ctx, _dir) = setup();
    app.navigate(Tab::Actions);
    app.actions_page = ActionsPage::VisualRules;
    click_label(&mut app, &ctx, "Add rule");
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert!(
        !text_regions(&output)
            .iter()
            .any(|(text, _)| text == "Wait only")
    );
    assert_eq!(
        app.editor.profile.safety.rules[0].execution.verification,
        metin_core::Verification::ReferenceGone
    );
    click_label(&mut app, &ctx, "Pause immediately");
    click_label(&mut app, &ctx, "Perform action");
    assert_eq!(
        app.editor.profile.safety.rules[0].reaction,
        metin_core::RuleReactions {
            notify: true,
            pause: false,
            execute: true
        }
    );
    click_label(&mut app, &ctx, "Wait only");
    assert_eq!(
        app.editor.profile.safety.rules[0].execution.verification,
        metin_core::Verification::Delay
    );
    click_label(&mut app, &ctx, "Continue from a fresh frame");
    assert_eq!(
        app.editor.profile.safety.rules[0].execution.after_success,
        metin_core::AfterSuccess::Continue
    );
    click_label(&mut app, &ctx, "F4");
    draw(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::F5,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }],
    );
    assert_eq!(
        app.editor.profile.safety.rules[0]
            .execution
            .input
            .shortcut(),
        Some("F5")
    );
    click_label(&mut app, &ctx, "Click");
    click_label(&mut app, &ctx, "Select click target");
    assert_eq!(app.mode, EditMode::VisualClick(0));
    assert!(app.return_to_safety);
    click_label(&mut app, &ctx, "Done & back to Actions");
    assert_eq!(app.actions_page, ActionsPage::VisualRules);
    assert_eq!(app.tab, Tab::Actions);
    assert!(!app.editor.profile.safety.rules[0].enabled);
    app.runtime.shutdown().unwrap();
}

#[test]
fn visual_event_setup_link_opens_the_matching_rule_in_actions() {
    let (mut app, ctx, _dir) = setup();
    let rule = metin_core::SafetyRule::new("Dialog");
    let id = rule.id;
    app.editor.profile.safety.rules.push(rule);
    app.window_fixture = Some(window_selection::WindowFixture {
        snapshot: Arc::new(RuntimeSnapshot {
            events: vec![metin_core::RunEvent {
                activity: "Visual rule".into(),
                message: "Dialog found".into(),
                destination: metin_core::EventDestination::Safety,
                subject: Some(id),
                ..Default::default()
            }],
            ..Default::default()
        }),
        open_popup: false,
    });
    app.tab = Tab::Run;
    click_label(&mut app, &ctx, "View event history (1)");
    for _ in 0..20 {
        draw(&mut app, &ctx, vec![]);
    }
    click_label(&mut app, &ctx, "Open setup");
    assert_eq!(app.tab, Tab::Actions);
    assert_eq!(app.actions_page, ActionsPage::VisualRules);
    assert_eq!(app.editor.profile.safety.rules[app.selected_safety].id, id);
    app.runtime.shutdown().unwrap();
}

#[test]
fn narrow_visual_execution_scrolls_to_completion_controls() {
    let (mut app, ctx, _dir) = setup();
    let mut rule = metin_core::SafetyRule::new("Dialog");
    rule.reaction = metin_core::RuleReactions {
        notify: true,
        pause: false,
        execute: true,
    };
    app.editor.profile.safety.rules.push(rule);
    app.tab = Tab::Actions;
    app.actions_page = ActionsPage::VisualRules;
    let size = egui::vec2(800., 560.);
    for _ in 0..3 {
        draw_at_size(&mut app, &ctx, &mut vec![], size);
    }
    draw_at_size(
        &mut app,
        &ctx,
        &mut vec![
            egui::Event::PointerMoved(egui::pos2(600., 450.)),
            egui::Event::MouseWheel {
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Unknown,
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., -2000.),
                modifiers: Default::default(),
            },
        ],
        size,
    );
    let mut output = draw_at_size(&mut app, &ctx, &mut vec![], size);
    for _ in 0..20 {
        output = draw_at_size(&mut app, &ctx, &mut vec![], size);
    }
    assert!(
        text_regions(&output)
            .iter()
            .any(|(text, rect)| text == "Continue from a fresh frame" && rect.right() <= 800.)
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn visual_reaction_checkboxes_keep_notify_independent_and_block_pause_action_conflict() {
    let (mut app, ctx, _dir) = setup();
    app.navigate(Tab::Actions);
    app.actions_page = ActionsPage::VisualRules;
    click_label(&mut app, &ctx, "Add rule");
    click_label(&mut app, &ctx, "Perform action");
    assert!(!app.editor.profile.safety.rules[0].reaction.execute);
    assert!(app.editor.profile.safety.rules[0].reaction.pause);
    click_label(&mut app, &ctx, "Notify");
    assert!(!app.editor.profile.safety.rules[0].reaction.notify);
    click_label(&mut app, &ctx, "Pause immediately");
    assert!(!app.editor.profile.safety.rules[0].reaction.any());
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(1280., 820.));
    assert!(
        text_regions(&output)
            .iter()
            .any(|(t, _)| t.contains("Choose at least one reaction"))
    );
    click_label(&mut app, &ctx, "Enable this rule");
    assert!(!app.editor.profile.safety.rules[0].enabled);
    click_label(&mut app, &ctx, "Perform action");
    assert!(app.editor.profile.safety.rules[0].reaction.execute);
    click_label(&mut app, &ctx, "Pause immediately");
    assert!(!app.editor.profile.safety.rules[0].reaction.pause);
    click_label(&mut app, &ctx, "Notify");
    assert!(app.editor.profile.safety.rules[0].reaction.notify);
    assert!(app.editor.profile.safety.rules[0].reaction.execute);
    click_label(&mut app, &ctx, "Configure notifications");
    assert_eq!(app.tab, Tab::Safety);
    assert_eq!(app.safety_page, SafetyPage::Notifications);
    app.runtime.shutdown().unwrap();
}
