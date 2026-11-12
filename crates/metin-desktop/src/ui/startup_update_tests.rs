#[test]
fn startup_update_window_chooser_has_no_update_controls_or_ambiguous_footer() {
    let (mut app, ctx, _dir) = setup();
    app.window_fixture = Some(window_selection::WindowFixture { snapshot: Arc::new(RuntimeSnapshot { targets_revision: 1, ..Default::default() }) });
    app.window_chooser.open = true;
    let output = compact_settle(&mut app, &ctx, egui::vec2(780., 600.));
    let text = text_regions(&output);
    assert!(!text.iter().any(|(text, _)| text == "Updates" || text == "Updates automatically"));
    app.runtime.shutdown().unwrap();
}

#[test]
fn startup_update_checks_are_enabled_by_default_with_explicit_opt_out_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = UISettings::load(dir.path()).unwrap();
    assert!(settings.check_updates_on_start);
    settings.check_updates_on_start = false;
    settings.save(dir.path()).unwrap();
    assert!(!UISettings::load(dir.path()).unwrap().check_updates_on_start);
}

fn startup_update_fixture(kind: crate::updates::UpdateFixture) -> (MetinApp, egui::Context, tempfile::TempDir) {
    let (mut app, ctx, dir) = setup();
    app.window_fixture = Some(window_selection::WindowFixture { snapshot: Arc::new(RuntimeSnapshot { targets_revision: 1, ..Default::default() }) });
    app.updates = crate::updates::UpdateService::fixture(kind).unwrap();
    app.update_fixture = true;
    app.startup_update_open = true;
    app.window_chooser.open = true;
    (app, ctx, dir)
}

#[test]
fn startup_update_available_uses_intro_window_and_skip_keeps_profiles() {
    let (mut app, ctx, _dir) = startup_update_fixture(crate::updates::UpdateFixture::Available);
    let profile = app.editor.profile.clone();
    let output = compact_settle(&mut app, &ctx, egui::vec2(400., 500.));
    let text = text_regions(&output);
    assert!(text.iter().any(|(text,_)| text == "Install update"));
    assert!(text.iter().any(|(text,_)| text == "Skip for now"));
    assert!(!text.iter().any(|(text,_)| text == "Release notes" || text == "Choose a window"));
    click_label(&mut app, &ctx, "Skip for now");
    assert!(!app.startup_update_open);
    assert!(app.window_chooser.open);
    assert_eq!(app.editor.profile, profile);
    assert!(!app.update_exit_pending());
    app.runtime.shutdown().unwrap();
}

#[test]
fn startup_update_continue_cancels_check_and_never_reopens_on_a_late_result() {
    let (mut app, ctx, _dir) = startup_update_fixture(crate::updates::UpdateFixture::Checking);
    let output = compact_settle(&mut app, &ctx, egui::vec2(400., 500.));
    assert!(text_regions(&output).iter().any(|(text,_)| text == "Checking for updates"));
    click_label(&mut app, &ctx, "Continue");
    assert!(!app.startup_update_open);
    app.updates = crate::updates::UpdateService::fixture(crate::updates::UpdateFixture::Available).unwrap();
    let output = compact_settle(&mut app, &ctx, egui::vec2(780., 600.));
    assert!(!app.startup_update_open);
    assert!(text_regions(&output).iter().any(|(text,_)| text == "Search window title or application…"));
    app.runtime.shutdown().unwrap();
}

#[test]
fn startup_update_no_release_continues_without_success_dialog() {
    let (mut app, ctx, _dir) = startup_update_fixture(crate::updates::UpdateFixture::UpToDate);
    let output = compact_settle(&mut app, &ctx, egui::vec2(780., 600.));
    assert!(!app.startup_update_open);
    let text=text_regions(&output);
    assert!(text.iter().any(|(text,_)| text == "Search window title or application…"));
    assert!(!text.iter().any(|(text,_)| text == "No compatible newer release"));
    app.runtime.shutdown().unwrap();
}

#[test]
fn startup_update_failures_remain_retryable_and_never_install() {
    let (mut app, ctx, _dir) = startup_update_fixture(crate::updates::UpdateFixture::Failed);
    let output = compact_settle(&mut app, &ctx, egui::vec2(400., 500.));
    assert!(text_regions(&output).iter().any(|(text,_)| text == "Retry check"));
    assert!(app.startup_update_open);
    assert!(!app.update_exit_pending());
    app.runtime.shutdown().unwrap();
}

#[test]
fn startup_update_cancellation_and_failure_revoke_install_intent() {
    for failed in [false, true] {
        let (mut app, ctx, _dir) = startup_update_fixture(crate::updates::UpdateFixture::Available);
        app.update_install_requested = match app.updates.state() { crate::updates::UpdateState::Available(candidate) => Some(candidate.clone()), _ => unreachable!() };
        if failed {
            app.updates = crate::updates::UpdateService::fixture(crate::updates::UpdateFixture::Failed).unwrap();
        } else {
            app.updates.cancel();
        }
        app.sync_updates(&ctx);
        assert!(app.update_install_requested.is_none(), "A cancelled or failed download must revoke the earlier install action");
        app.runtime.shutdown().unwrap();
    }
}

#[test]
fn startup_update_retry_clears_the_previous_helper_error() {
    let (mut app, ctx, _dir) = startup_update_fixture(crate::updates::UpdateFixture::Idle);
    app.update_error = Some("Previous helper failed".into());
    click_label(&mut app, &ctx, "Check for updates");
    assert!(app.update_error.is_none());
    app.runtime.shutdown().unwrap();
}

#[test]
fn startup_update_skip_stays_visible_with_long_errors_and_large_text() {
    for zoom in [1., 1.5, 2.] {
        let (mut app, ctx, _dir) = startup_update_fixture(crate::updates::UpdateFixture::Idle);
        ctx.set_zoom_factor(zoom);
        app.update_error = Some("A long update error with details and recovery information. ".repeat(15));
        let output = compact_settle(&mut app, &ctx, egui::vec2(400., 480.));
        let (_, rect) = text_regions(&output).into_iter().find(|(text,_)| text == "Skip for now").unwrap();
        assert!(ctx.content_rect().contains_rect(rect), "Skip must stay visible at zoom {zoom}: {rect:?}");
        app.runtime.shutdown().unwrap();
    }
}

#[cfg(feature = "ui-smoke")]
#[test]
fn startup_update_helper_failure_keeps_close_visible_at_large_scale() {
    for zoom in [1., 1.5, 2.] {
        let (mut app, ctx, _dir) = setup();
        ctx.set_zoom_factor(zoom);
        app.prepare_update_helper_fixture("failed-long");
        let output = compact_settle(&mut app, &ctx, egui::vec2(400., 480.));
        let (_, rect) = text_regions(&output).into_iter().find(|(text,_)| text == "Close").expect("Update failure needs an accessible Close action");
        assert!(ctx.content_rect().contains_rect(rect), "Close must stay visible at zoom {zoom}: {rect:?}");
        app.runtime.shutdown().unwrap();
    }
}
