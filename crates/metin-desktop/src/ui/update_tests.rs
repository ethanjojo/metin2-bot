#[test]
fn qol_update_preferences_default_to_startup_checks_and_survive_a_settings_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = UISettings::load(dir.path()).unwrap();
    assert!(settings.check_updates_on_start);
    assert_eq!(
        settings.update_channel,
        crate::updates::UpdateChannel::Stable
    );
    settings.check_updates_on_start = true;
    settings.update_channel = crate::updates::UpdateChannel::Preview;
    settings.save(dir.path()).unwrap();
    let loaded = UISettings::load(dir.path()).unwrap();
    assert!(loaded.check_updates_on_start);
    assert_eq!(
        loaded.update_channel,
        crate::updates::UpdateChannel::Preview
    );
}

#[test]
fn qol_updates_idle_has_an_explicit_check_without_install_authority() {
    let (mut app, ctx, _dir) = setup();
    app.tab = Tab::Settings;
    app.settings_page = SettingsPage::Updates;
    let output = compact_settle(&mut app, &ctx, egui::vec2(400., 500.));
    let text = text_regions(&output);
    assert!(text.iter().any(|(text, _)| text == "Check for updates"));
    assert!(!text.iter().any(|(text, _)| text == "Install and restart"));
    let before = app.editor.profile.clone();
    assert!(
        app.install_update_with(&ctx, |_, _, _| panic!(
            "An unverified file must never reach a launcher"
        ))
        .is_err()
    );
    assert_eq!(app.editor.profile, before);
    assert!(!app.update_exit_pending());
    app.runtime.shutdown().unwrap();
}

#[test]
fn qol_update_cancel_keeps_edits_and_the_verified_download() {
    let (mut app, ctx, _dir) = setup();
    app.updates =
        crate::updates::UpdateService::fixture(crate::updates::UpdateFixture::Ready).unwrap();
    app.editor.profile.name = "Keep my edits".into();
    app.editor.record();
    let original = app.editor.profile.clone();
    assert!(
        app.install_update_after_decision(
            &ctx,
            |_| false,
            |_, _, _| panic!("Cancelled update must not launch")
        )
        .is_err()
    );
    assert_eq!(app.editor.profile, original);
    assert!(matches!(
        app.updates.state(),
        crate::updates::UpdateState::Ready(_)
    ));
    assert!(!app.update_exit_pending());
    app.runtime.shutdown().unwrap();
}

#[test]
fn qol_update_failed_launcher_recovers_a_paused_runtime_and_keeps_profiles() {
    let (mut app, ctx, _dir) = setup();
    app.save().unwrap();
    let original = app.editor.profile.clone();
    app.updates = crate::updates::UpdateService::fixture(crate::updates::UpdateFixture::Ready).unwrap();
    let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
    let capture = captured.clone();
    app.install_update_with(&ctx, move |guard, candidate, root| {
        assert!(guard.installer_path().exists());
        assert!(root.is_dir());
        assert!(!candidate.version().is_empty());
        *capture.lock().unwrap() = Some(guard.installer_path().to_path_buf());
        Err("Fixture launch refused".into())
    }).unwrap();
    let deadline=std::time::Instant::now()+std::time::Duration::from_secs(3);
    while app.update_handoff_waiting() && std::time::Instant::now()<deadline { app.sync_updates(&ctx); std::thread::yield_now(); }
    assert_eq!(app.update_error.as_deref(),Some("Fixture launch refused"));
    assert_eq!(app.editor.profile,original);
    assert!(!captured.lock().unwrap().as_ref().unwrap().exists());
    assert!(!app.update_exit_pending());
    assert!(app.window_chooser.open);
    assert!(!app.snapshot.running);
    app.runtime.send(Command::RefreshTargets).unwrap();
    app.runtime.shutdown().unwrap();
}

#[test]
fn qol_update_success_retains_verification_guard_until_close_without_game_input() {
    let (mut app, ctx, _dir) = setup();
    app.save().unwrap();
    let original=app.editor.profile.clone();
    app.updates=crate::updates::UpdateService::fixture(crate::updates::UpdateFixture::Ready).unwrap();
    let captured=std::sync::Arc::new(std::sync::Mutex::new(None));
    let capture=captured.clone();
    app.install_update_with(&ctx, move |guard, _, _| {
        #[cfg(windows)] assert!(std::fs::OpenOptions::new().write(true).open(guard.installer_path()).is_err());
        *capture.lock().unwrap()=Some(guard.installer_path().to_path_buf());
        Ok(crate::update_helper::PreparedUpdate::fixture(guard,false))
    }).unwrap();
    assert!(!app.update_exit_pending());
    let deadline=std::time::Instant::now()+std::time::Duration::from_secs(3);
    while !matches!(app.update_handoff,Some(update_view::UpdateHandoff::Waiting(_))) && std::time::Instant::now()<deadline { app.sync_updates(&ctx); std::thread::yield_now(); }
    assert!(!app.update_exit_pending(),"Parent must remain open until the independent helper acknowledges readiness");
    assert!(app.update_handoff_waiting());
    let file=captured.lock().unwrap().as_ref().unwrap().clone();
    assert!(file.exists());
    if let Some(update_view::UpdateHandoff::Waiting(helper))=&mut app.update_handoff { helper.set_fixture_ready(true); } else { panic!("Helper did not prepare"); }
    app.sync_updates(&ctx);
    assert!(app.update_exit_pending());
    assert_eq!(app.editor.profile,original);
    assert!(file.exists());
    drop(app);
    assert!(!file.exists());
}

#[test]
fn qol_ready_update_primary_action_fits_above_footer_at_minimum_width() {
    let (mut app, ctx, _dir) = setup();
    app.updates =
        crate::updates::UpdateService::fixture(crate::updates::UpdateFixture::Ready).unwrap();
    app.update_fixture = true;
    app.tab = Tab::Settings;
    app.settings_page = SettingsPage::Updates;
    compact_settle(&mut app, &ctx, egui::vec2(400., 500.));
    let install = compact_control_rect(
        &ctx,
        "Pause and release input, resolve unsaved edits, then update through the verified Companion update window.",
    );
    let clip=ctx.data(|d|d.get_temp::<egui::Rect>(egui::Id::unique(("control-clip","Pause and release input, resolve unsaved edits, then update through the verified Companion update window.")))).unwrap();
    assert!(
        clip.contains_rect(install),
        "Install action must be fully visible: {install:?}; clip {clip:?}"
    );
    app.runtime.shutdown().unwrap();
}
