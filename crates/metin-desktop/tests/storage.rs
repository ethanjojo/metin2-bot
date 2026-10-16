use metin_core::Profile;
use metin_desktop::storage::ProfileStore;
#[test]
fn malformed_window_preferences_are_rejected_before_saving_or_loading() {
    use metin_desktop::storage::{UISettings, WindowPreference};
    let dir = tempfile::tempdir().unwrap();
    for (process, title) in [
        ("../client.exe", "Server"),
        ("client.exe", ""),
        ("", "Server"),
    ] {
        let mut settings = UISettings::default();
        settings.profile_windows.insert(
            uuid::Uuid::new_v4(),
            WindowPreference {
                process_name: process.into(),
                title: title.into(),
            },
        );
        assert!(settings.save(dir.path()).is_err());
        assert!(!dir.path().join("ui-settings.json").exists());
        std::fs::write(
            dir.path().join("ui-settings.json"),
            serde_json::to_vec(&settings).unwrap(),
        )
        .unwrap();
        assert!(UISettings::load(dir.path()).is_err());
        std::fs::remove_file(dir.path().join("ui-settings.json")).unwrap();
    }
}
#[test]
fn per_profile_window_preferences_roundtrip_and_old_settings_still_load() {
    use metin_desktop::storage::UISettings;
    let dir = tempfile::tempdir().unwrap();
    let a = uuid::Uuid::new_v4().to_string();
    let b = uuid::Uuid::new_v4().to_string();
    let expected = serde_json::json!({"theme":"dark","last_mounted":true,"profile_windows":{
        (a):{"process_name":"client-a.exe","title":"Server A"},
        (b):{"process_name":"client-b.exe","title":"Server B"}
    }});
    let path = dir.path().join("ui-settings.json");
    std::fs::write(&path, expected.to_string()).unwrap();
    let settings = UISettings::load(dir.path()).unwrap();
    settings.save(dir.path()).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(&path).unwrap()).unwrap(),
        expected
    );
    std::fs::write(&path, r#"{"theme":"light","last_mounted":false}"#).unwrap();
    let settings = UISettings::load(dir.path()).unwrap();
    settings.save(dir.path()).unwrap();
    let upgraded: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(upgraded["profile_windows"], serde_json::json!({}));
}
#[test]
fn incomplete_profiles_selection_and_independent_templates_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = ProfileStore::open(dir.path()).unwrap();
    let p = Profile::create("Žluťoučký kůň");
    store.save(&p, None).unwrap();
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
fn failed_json_replacement_preserves_previous_template_and_cleans_new_asset() {
    use metin_platform::Frame;
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    let mut store = ProfileStore::open(dir.path()).unwrap();
    let frame = Frame::new(10, 10, (0..300).map(|n| (n % 251) as u8).collect()).unwrap();
    let p = store
        .save(&Profile::create("Atomic"), Some(&frame))
        .unwrap();
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
