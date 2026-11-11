#[test]
fn qol_update_preferences_are_opt_in_and_survive_a_settings_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = UISettings::load(dir.path()).unwrap();
    assert!(!settings.check_updates_on_start);
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
        app.install_update_with(&ctx, |_, _| panic!(
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
            |_, _| panic!("Cancelled update must not launch")
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
    app.updates =
        crate::updates::UpdateService::fixture(crate::updates::UpdateFixture::Ready).unwrap();
    let mut file = None;
    let result = app.install_update_with(&ctx, |path, args| {
        assert!(path.exists());
        assert_eq!(args[0], "/RESTARTCOMPANION=1");
        assert!(args[1].to_string_lossy().starts_with("/COMPANIONDATADIR="));
        let raw = args[1].to_string_lossy();
        assert!(
            std::path::Path::new(raw.strip_prefix("/COMPANIONDATADIR=").unwrap()).is_dir(),
            "The exact directory passed to Setup must exist without implicit normalization"
        );
        #[cfg(windows)]
        {
            unsafe extern "system" {
                fn GetFileAttributesW(name: *const u16) -> u32;
            }
            let raw = args[1].to_string_lossy();
            let root = raw.strip_prefix("/COMPANIONDATADIR=").unwrap();
            let wide = root.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
            assert_ne!(
                unsafe { GetFileAttributesW(wide.as_ptr()) },
                u32::MAX,
                "Setup receives the raw directory, not Rust's normalized interpretation"
            );
        }
        file = Some(path.to_path_buf());
        Err("Fixture launch refused".into())
    });
    assert_eq!(result.unwrap_err(), "Fixture launch refused");
    assert_eq!(app.editor.profile, original);
    assert!(!file.unwrap().exists());
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
    let original = app.editor.profile.clone();
    app.updates =
        crate::updates::UpdateService::fixture(crate::updates::UpdateFixture::Ready).unwrap();
    let mut file = None;
    app.install_update_with(&ctx, |path, _| {
        #[cfg(windows)]
        assert!(std::fs::OpenOptions::new().write(true).open(path).is_err());
        file = Some(path.to_path_buf());
        Ok(())
    })
    .unwrap();
    assert!(app.update_exit_pending());
    assert!(file.as_ref().unwrap().exists());
    assert_eq!(app.editor.profile, original);
    drop(app);
    assert!(!file.unwrap().exists());
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
        "Pause and release input, resolve unsaved edits, then open the verified installer. Driver consent remains in Setup.",
    );
    let clip=ctx.data(|d|d.get_temp::<egui::Rect>(egui::Id::unique(("control-clip","Pause and release input, resolve unsaved edits, then open the verified installer. Driver consent remains in Setup.")))).unwrap();
    assert!(
        clip.contains_rect(install),
        "Install action must be fully visible: {install:?}; clip {clip:?}"
    );
    app.runtime.shutdown().unwrap();
}
