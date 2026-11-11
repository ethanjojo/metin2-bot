use super::*;

#[test]
fn startup_requires_explicit_window_selection_and_never_changes_the_profile() {
    let (mut app, ctx, _dir) = setup();
    let original = app.editor.profile.clone();
    app.open_window_chooser();
    pin_windows(&mut app, window_catalog(), 1);
    app.reconcile_windows();
    assert!(app.window_chooser.open);
    assert!(app.target.is_none());
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(780., 600.));
    let text = text_regions(&output);
    assert!(text.iter().any(|(s, _)| s.contains("Žluťoučký server")));
    assert_eq!(app.editor.profile, original);
    app.runtime.shutdown().unwrap();
}

#[test]
fn profile_switch_retains_the_explicit_session_target_without_remembering_it_on_disk() {
    let (mut app, ctx, dir) = setup();
    let a = Profile::create("A");
    let b = Profile::create("B");
    app.store.save(&a, None).unwrap();
    app.store.save(&b, None).unwrap();
    app.replace_profile(a.clone(), None);
    pin_windows(&mut app, window_catalog(), 1);
    let target = app.snapshot.targets[0].token;
    app.select_window(target).unwrap();
    app.switch_profile(b.id);
    assert_eq!(app.target, Some(target));
    assert_eq!(app.key().unwrap().profile_id, b.id);
    assert!(!app.window_chooser.open);
    assert!(!dir.path().join("ui-settings.json").exists());
    app.runtime.shutdown().unwrap();
    drop(app);
    let mut reopened = MetinApp::from_context(&ctx, dir.path().to_owned()).unwrap();
    pin_windows(&mut reopened, window_catalog(), 1);
    reopened.reconcile_windows();
    assert!(reopened.window_chooser.open);
    assert!(reopened.target.is_none());
    reopened.runtime.shutdown().unwrap();
}

#[test]
fn closed_or_reused_window_returns_to_chooser_but_renaming_keeps_exact_token() {
    let (mut app, _ctx, _dir) = setup();
    let windows = window_catalog();
    pin_windows(&mut app, windows.clone(), 1);
    app.select_window(windows[0].token).unwrap();
    let mut renamed = windows[0].clone();
    renamed.title = "Changed name".into();
    pin_windows(&mut app, vec![renamed.clone()], 2);
    app.reconcile_windows();
    assert_eq!(app.target, Some(windows[0].token));
    renamed.token.created += 1;
    pin_windows(&mut app, vec![renamed], 3);
    app.reconcile_windows();
    assert!(app.target.is_none());
    assert!(app.window_chooser.open);
    assert!(app.pending_overlay.is_none());
    app.runtime.shutdown().unwrap();
}

#[test]
fn revert_restores_the_saved_transaction_and_preserves_connection_and_undo() {
    let (mut app, _ctx, _dir) = setup();
    app.save().unwrap();
    let saved = app.editor.profile.clone();
    let target = window_catalog()[0].token;
    app.target = Some(target);
    app.editor.profile.name = "Unsaved edit".into();
    app.editor.record();
    app.revert_profile();
    assert_eq!(app.editor.profile, saved);
    assert!(!app.editor.dirty());
    assert_eq!(app.target, Some(target));
    app.undo_profile();
    assert_eq!(app.editor.profile.name, "Unsaved edit");
    assert_eq!(app.target, Some(target));
    app.runtime.shutdown().unwrap();
}

#[test]
fn recognition_uses_one_native_editor_and_annotations_are_global_across_tabs() {
    let (mut app, ctx, _dir) = setup();
    pin_windows(&mut app, window_catalog(), 1);
    app.target = Some(window_catalog()[0].token);
    let original = app.editor.profile.clone();
    for tab in [
        Tab::Run,
        Tab::Calibration,
        Tab::Actions,
        Tab::Safety,
        Tab::Settings,
    ] {
        app.tab = tab;
        app.run_live = true;
        app.sync_preview_demand();
        assert!(app.preview_demand.unwrap().1);
        let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(460., 620.));
        let text = text_regions(&output);
        assert!(!text.iter().any(|(s, _)| s.contains("Snapshot editor")
            || s == "Game preview"
            || s == "Game window"));
    }
    app.run_live = false;
    app.sync_preview_demand();
    assert!(!app.preview_demand.unwrap().1);
    assert_eq!(app.editor.profile, original);
    app.runtime.shutdown().unwrap();
}

#[test]
fn chooser_search_accepts_unicode_title_process_and_pid_without_changing_connection() {
    let windows = window_catalog();
    for query in ["", " žLUŤOUČKÝ ", "CLIENT-A", "201"] {
        assert!(window_selection::matches_search(&windows[0], query));
    }
    assert!(!window_selection::matches_search(&windows[0], "other"));
    let (mut app, ctx, _dir) = setup();
    pin_windows(&mut app, windows.clone(), 1);
    app.open_window_chooser();
    app.window_search = "OTHER.EXE".into();
    let output = draw_at_size(&mut app, &ctx, &mut vec![], egui::vec2(780., 600.));
    let text = text_regions(&output);
    assert!(text.iter().any(|(s, _)| s == "Second server"));
    assert!(!text.iter().any(|(s, _)| s.contains("Žluťoučký server")));
    assert!(app.target.is_none());
    app.runtime.shutdown().unwrap();
}
