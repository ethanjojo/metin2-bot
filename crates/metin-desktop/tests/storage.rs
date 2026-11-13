use metin_core::Profile;
use metin_desktop::storage::ProfileStore;
#[test]
fn legacy_entity_ids_load_without_writing_and_persist_only_on_save() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ProfileStore::open(dir.path()).unwrap();
    let profile = Profile::create("Legacy entities");
    let folder = dir.path().join(profile.id.to_string());
    std::fs::create_dir(&folder).unwrap();
    let path = folder.join("profile.json");
    let mut value = serde_json::to_value(&profile).unwrap();
    value["buffs"]["skills"] = serde_json::json!([{"shortcut":"F1"}]);
    value["masks"] = serde_json::json!([{"left":0.1,"top":0.2,"right":0.3,"bottom":0.4}]);
    let original = value.to_string();
    std::fs::write(&path, &original).unwrap();
    let loaded = store.load(profile.id).unwrap().0;
    assert_eq!(store.load(profile.id).unwrap().0, loaded);
    assert_eq!(store.list().unwrap(), vec![loaded.clone()]);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    let encoded = serde_json::to_value(&loaded).unwrap();
    assert!(encoded["buffs"]["skills"][0]["id"].is_string());
    assert!(encoded["masks"][0]["id"].is_string());
    let mut renamed = loaded.clone();
    renamed.name = "Renamed".into();
    let saved = store.save(&renamed, None).unwrap().profile;
    let on_disk: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(on_disk["buffs"], encoded["buffs"]);
    assert_eq!(on_disk["masks"], encoded["masks"]);
    assert_eq!(store.load(profile.id).unwrap().0, saved);
}

#[test]
fn direct_legacy_save_matches_canonical_profile_load() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ProfileStore::open(dir.path()).unwrap();
    let mut value = serde_json::to_value(Profile::create("Direct legacy input")).unwrap();
    value["buffs"]["skills"] = serde_json::json!([{"shortcut":"F1"}]);
    value["masks"] = serde_json::json!([{"name":"Chat","region":{"left":0.1,"top":0.2,"right":0.3,"bottom":0.4}}]);
    let direct: Profile = serde_json::from_value(value.clone()).unwrap();
    let saved = store.save(&direct, None).unwrap().profile;
    assert_eq!(saved, Profile::from_json(&value.to_string()).unwrap());
    let encoded = serde_json::to_value(saved).unwrap();
    assert!(encoded["buffs"]["skills"][0]["id"].is_string());
    assert!(encoded["masks"][0]["id"].is_string());
}
#[test]
fn obsolete_profile_window_choices_are_consumed_but_never_saved_again() {
    use metin_desktop::storage::UISettings;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui-settings.json");
    let obsolete = serde_json::json!({
        "theme":"dark","ui_scale":1.25,"lite_mode":true,"pin_to_game":false,
        "preview_refresh_ms":450,"last_mounted":true,
        "profile_windows":{(uuid::Uuid::new_v4().to_string()):{"process_name":"client.exe","title":"Old window"}}
    });
    std::fs::write(&path, obsolete.to_string()).unwrap();
    let settings = UISettings::load(dir.path()).unwrap();
    assert_eq!(settings.ui_scale, 1.25);
    assert_eq!(settings.preview_refresh_ms, 450);
    assert!(settings.lite_mode);
    assert!(!settings.pin_to_game);
    settings.save(dir.path()).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        saved,
        serde_json::json!({"ui_scale":1.25,"lite_mode":true,"pin_to_game":false,"preview_refresh_ms":450,"check_updates_on_start":true,"update_channel":"stable","panel_positions":[],"session_rate_display":"per_minute"})
    );
}
#[test]
fn old_settings_receive_explicit_defaults_and_unknown_current_fields_are_rejected() {
    use metin_desktop::storage::UISettings;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui-settings.json");
    std::fs::write(&path, r#"{"theme":"light","last_mounted":false}"#).unwrap();
    let settings = UISettings::load(dir.path()).unwrap();
    assert!(settings.pin_to_game);
    settings.save(dir.path()).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert!(saved.get("profile_windows").is_none());
    assert!(saved.get("last_mounted").is_none());
    assert!(saved.get("theme").is_none());
    std::fs::write(&path, r#"{"theme":"light","ui_scalle":2}"#).unwrap();
    assert!(UISettings::load(dir.path()).is_err());
}
#[test]
fn incomplete_profiles_selection_and_independent_templates_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ProfileStore::open(dir.path()).unwrap();
    let p = Profile::create("Žluťoučký kůň");
    let saved = store.save(&p, None).unwrap();
    assert_eq!(saved.profile, p);
    assert!(saved.cleanup_warning.is_none());
    store.select(p.id).unwrap();
    assert_eq!(store.load(p.id).unwrap().0, p);
    assert_eq!(store.selected().unwrap(), Some(p.id));
    assert!(!store.load(p.id).unwrap().0.ready());
    assert!(ProfileStore::open(dir.path()).is_err());
}
#[test]
fn unsafe_asset_and_incomplete_profile_tree_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ProfileStore::open(dir.path()).unwrap();
    let mut p = Profile::create("Test");
    p.template_file = Some("../outside.png".into());
    assert!(store.save(&p, None).is_err());
    p.template_file = Some("missing.png".into());
    assert!(store.save(&p, None).is_err());
    assert!(store.list().unwrap().is_empty());
    drop(store);
    assert!(
        ProfileStore::open(dir.path())
            .unwrap()
            .list()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn profile_sort_uses_full_unicode_casefold() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ProfileStore::open(dir.path()).unwrap();
    let names = ["Straße", "STRASSE", "σ", "ς", "Žluťoučký"];
    for (i, name) in names.iter().enumerate() {
        let mut p = Profile::create(*name);
        p.id = uuid::Uuid::from_u128(i as u128 + 1);
        store.save(&p, None).unwrap();
    }
    assert_eq!(
        store
            .list()
            .unwrap()
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Straße", "STRASSE", "Žluťoučký", "σ", "ς"]
    );
}

#[cfg(windows)]
#[test]
fn committed_profile_save_succeeds_when_obsolete_template_is_locked() {
    use metin_platform::Frame;
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    // Windows accepts aliases with different casing (and CI may use 8.3 temp paths).
    let alias = std::path::PathBuf::from(dir.path().as_os_str().to_ascii_uppercase());
    let mut store = ProfileStore::open(&alias).unwrap();
    let frame = Frame::new(10, 10, (0..300).map(|n| (n % 251) as u8).collect()).unwrap();
    let original = store
        .save(&Profile::create("Original"), Some(&frame))
        .unwrap()
        .profile;
    let old_asset = alias
        .join(original.id.to_string())
        .join(original.template_file.as_ref().unwrap());
    let canonical_asset = old_asset.canonicalize().unwrap();
    let locked = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&old_asset)
        .unwrap();
    let mut changed = original.clone();
    changed.name = "Committed".into();

    let result = store.save(&changed, Some(&frame));
    assert!(
        result.is_ok(),
        "Committed JSON must be reported as a successful save: {result:?}"
    );
    let outcome = result.unwrap();
    assert!(
        outcome
            .cleanup_warning
            .as_ref()
            .is_some_and(|warning| warning.contains(&canonical_asset.display().to_string())),
        "Cleanup warning must identify the locked asset: {:?}",
        outcome.cleanup_warning
    );
    let committed = store.load(original.id).unwrap().0;
    assert_eq!(outcome.profile, committed);
    assert_eq!(committed.name, "Committed");
    assert_ne!(committed.template_file, original.template_file);
    assert!(old_asset.is_file());
    drop(locked);
}

#[cfg(windows)]
#[test]
fn failed_json_replacement_preserves_previous_template_and_cleans_new_asset() {
    use metin_platform::Frame;
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    let mut store = ProfileStore::open(dir.path()).unwrap();
    let frame = Frame::new(10, 10, (0..300).map(|n| (n % 251) as u8).collect()).unwrap();
    let p = store
        .save(&Profile::create("Atomic"), Some(&frame))
        .unwrap()
        .profile;
    let folder = dir.path().join(p.id.to_string());
    let json = folder.join("profile.json");
    let before = std::fs::read(&json).unwrap();
    // Permit reads but deny delete/replace, so loading succeeds and the final
    // atomic JSON replacement fails after its new asset has been written.
    let locked = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&json)
        .unwrap();
    let mut changed = p.clone();
    changed.name = "Must not commit".into();
    assert!(store.save(&changed, Some(&frame)).is_err());
    assert_eq!(std::fs::read(&json).unwrap(), before);
    assert!(folder.join(p.template_file.as_ref().unwrap()).is_file());
    assert_eq!(
        std::fs::read_dir(&folder)
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "png"))
            .count(),
        1
    );
    drop(locked);
    assert_eq!(store.load(p.id).unwrap().0, p);
}

#[test]
fn damaged_embedded_icon_is_rejected_before_profile_commit() {
    use base64::Engine;
    let dir = tempfile::tempdir().unwrap();
    let mut store = ProfileStore::open(dir.path()).unwrap();
    let mut p = Profile::create("Damaged icon");
    let mut bytes = vec![0; 24];
    bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    bytes[16..20].copy_from_slice(&5_u32.to_be_bytes());
    bytes[20..24].copy_from_slice(&5_u32.to_be_bytes());
    p.buffs.skills.push(metin_core::BuffSkill {
        icon: Some(metin_core::BuffIcon {
            png: base64::engine::general_purpose::STANDARD.encode(bytes),
            region: None,
        }),
        ..Default::default()
    });
    assert!(store.save(&p, None).is_err());
    assert!(store.list().unwrap().is_empty());
}
#[test]
fn selection_uuid_requires_canonical_format_before_loading_existing_profile() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ProfileStore::open(dir.path()).unwrap();
    let mut p = Profile::create("Identity");
    p.id = uuid::Uuid::parse_str("8591a09f-25dc-4ca4-b858-cafe68231dfc").unwrap();
    store.save(&p, None).unwrap();
    let selection = dir.path().join("selection.json");
    for id in [
        p.id.to_string().to_uppercase(),
        p.id.simple().to_string(),
        "invalid".into(),
    ] {
        std::fs::write(&selection, serde_json::json!({"profile_id":id}).to_string()).unwrap();
        assert!(store.selected().is_err());
    }
    store.select(p.id).unwrap();
    assert_eq!(store.selected().unwrap(), Some(p.id));
}

#[test]
fn presentation_settings_roundtrip_and_invalid_values_fail_before_write() {
    use metin_desktop::storage::UISettings;
    let dir = tempfile::tempdir().unwrap();
    let mut settings = UISettings {
        ui_scale: 1.25,
        lite_mode: true,
        preview_refresh_ms: 1500,
        ..Default::default()
    };
    settings.save(dir.path()).unwrap();
    let read = UISettings::load(dir.path()).unwrap();
    assert_eq!(read.ui_scale, 1.25);
    assert!(read.lite_mode);
    assert_eq!(read.preview_refresh_ms, 1500);
    let original = std::fs::read(dir.path().join("ui-settings.json")).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&original).unwrap();
    assert!(saved.get("theme").is_none());
    settings.preview_refresh_ms = 0;
    assert!(settings.save(dir.path()).is_err());
    assert_eq!(
        std::fs::read(dir.path().join("ui-settings.json")).unwrap(),
        original
    );
}

#[test]
fn obsolete_global_mount_hint_is_read_but_never_saved_or_reused() {
    use metin_desktop::storage::UISettings;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui-settings.json");
    std::fs::write(&path, r#"{"theme":"dark","last_mounted":true}"#).unwrap();
    let settings = UISettings::load(dir.path()).unwrap();
    settings.save(dir.path()).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(value.get("last_mounted").is_none());
}

#[test]
fn retired_theme_values_are_ignored_without_losing_current_settings() {
    use metin_desktop::storage::UISettings;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui-settings.json");
    for theme in [
        serde_json::json!("light"),
        serde_json::json!("dark"),
        serde_json::json!("obsolete-value"),
        serde_json::json!({"obsolete": [true, 42]}),
        serde_json::Value::Null,
    ] {
        std::fs::write(
            &path,
            serde_json::json!({
                "theme":theme,"ui_scale":1.5,"preview_refresh_ms":5000,
                "lite_mode":true,"pin_to_game":false
            })
            .to_string(),
        )
        .unwrap();
        let settings = UISettings::load(dir.path()).unwrap();
        assert_eq!(settings.ui_scale, 1.5);
        assert_eq!(settings.preview_refresh_ms, 5000);
        assert!(settings.lite_mode);
        assert!(!settings.pin_to_game);
        settings.save(dir.path()).unwrap();
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            saved,
            serde_json::json!({
                "ui_scale":1.5,"preview_refresh_ms":5000,"lite_mode":true,"pin_to_game":false,"check_updates_on_start":true,"update_channel":"stable","panel_positions":[],"session_rate_display":"per_minute"
            })
        );
        let reloaded = UISettings::load(dir.path()).unwrap();
        assert_eq!(serde_json::to_value(reloaded).unwrap(), saved);
    }
}
