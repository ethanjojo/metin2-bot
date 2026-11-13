use metin_core::{ColorConfig, Profile};
use metin_desktop::storage::{ProfileStore, validate_data_directory};
use std::fs;

#[test]
fn invalid_preset_catalog_blocks_data_migration_before_any_write() {
    let data = tempfile::tempdir().unwrap();
    fs::create_dir(data.path().join("profiles")).unwrap();
    let catalog = data.path().join("profiles/metin-presets.json");
    fs::write(&catalog, b"{not-json}").unwrap();
    assert!(validate_data_directory(data.path()).is_err());
    assert_eq!(fs::read(&catalog).unwrap(), b"{not-json}");
    assert!(!data.path().join("profiles/.store.lock").exists());
}

#[test]
fn missing_preset_catalog_is_empty_and_read_only() {
    let root = tempfile::tempdir().unwrap();
    let store = ProfileStore::open(root.path()).unwrap();
    assert!(store.metin_presets().unwrap().is_empty());
    assert!(!root.path().join("metin-presets.json").exists());
}

#[test]
fn presets_save_rename_delete_with_stable_ids_without_rewriting_profiles() {
    let root = tempfile::tempdir().unwrap();
    let mut store = ProfileStore::open(root.path()).unwrap();
    let mut profile = Profile::create("Main");
    profile.color = Some(ColorConfig::default());
    store.save(&profile, None).unwrap();
    let path = root
        .path()
        .join(profile.id.to_string())
        .join("profile.json");
    let profile_before = fs::read(&path).unwrap();
    let preset = store
        .save_metin_preset(None, "  Red metin  ", &ColorConfig::default())
        .unwrap();
    assert_eq!(preset.name, "Red metin");
    let color = ColorConfig {
        hsv: [80, 180, 220],
        ..Default::default()
    };
    let renamed = store
        .save_metin_preset(Some(preset.id), "Blue metin", &color)
        .unwrap();
    assert_eq!(renamed.id, preset.id);
    assert_eq!(renamed.color, color);
    assert_eq!(store.metin_presets().unwrap(), vec![renamed]);
    assert_eq!(fs::read(&path).unwrap(), profile_before);
    store.delete_metin_preset(preset.id).unwrap();
    assert!(store.metin_presets().unwrap().is_empty());
    assert_eq!(fs::read(&path).unwrap(), profile_before);
    let before = fs::read(root.path().join("metin-presets.json")).unwrap();
    assert!(store.delete_metin_preset(preset.id).is_err());
    assert!(
        store
            .save_metin_preset(Some(preset.id), "Gone", &color)
            .is_err()
    );
    assert_eq!(
        fs::read(root.path().join("metin-presets.json")).unwrap(),
        before
    );
}

#[test]
fn invalid_preset_changes_leave_previous_catalog_bytes_intact() {
    let root = tempfile::tempdir().unwrap();
    let mut store = ProfileStore::open(root.path()).unwrap();
    let color = ColorConfig::default();
    let preset = store.save_metin_preset(None, "Straße", &color).unwrap();
    let path = root.path().join("metin-presets.json");
    let before = fs::read(&path).unwrap();
    for name in [
        "".to_owned(),
        "  ".to_owned(),
        "A\nB".to_owned(),
        "x".repeat(81),
        "STRASSE".to_owned(),
    ] {
        assert!(
            store.save_metin_preset(None, &name, &color).is_err(),
            "{name:?}"
        );
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    let bad_color = ColorConfig {
        hsv: [180, 200, 200],
        ..color.clone()
    };
    assert!(
        store
            .save_metin_preset(Some(preset.id), "Valid name", &bad_color)
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(
        store
            .save_metin_preset(None, &"ř".repeat(80), &color)
            .is_ok()
    );
}

#[test]
fn catalog_capacity_allows_update_at_limit_but_rejects_new_entry() {
    let root = tempfile::tempdir().unwrap();
    let mut store = ProfileStore::open(root.path()).unwrap();
    let entries: Vec<_> = (0..128)
        .map(|i| {
            serde_json::json!({
                "id":uuid::Uuid::new_v4(),"name":format!("Metin {i}"),"color":ColorConfig::default()
            })
        })
        .collect();
    let first = uuid::Uuid::parse_str(entries[0]["id"].as_str().unwrap()).unwrap();
    let path = root.path().join("metin-presets.json");
    fs::write(
        &path,
        serde_json::json!({"schema":1,"presets":entries}).to_string(),
    )
    .unwrap();
    let before = fs::read(&path).unwrap();
    assert!(
        store
            .save_metin_preset(None, "Overflow", &ColorConfig::default())
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    store
        .save_metin_preset(Some(first), "Updated", &ColorConfig::default())
        .unwrap();
    assert_eq!(store.metin_presets().unwrap().len(), 128);
}

#[test]
fn malformed_oversized_and_duplicate_catalogs_are_never_overwritten() {
    let data = tempfile::tempdir().unwrap();
    let profiles = data.path().join("profiles");
    let mut store = ProfileStore::open(&profiles).unwrap();
    let entry = serde_json::json!({"id":uuid::Uuid::new_v4(),"name":"Stone","color":ColorConfig::default()});
    let mut duplicate_name = entry.clone();
    duplicate_name["id"] = serde_json::json!(uuid::Uuid::new_v4());
    duplicate_name["name"] = serde_json::json!("STONE");
    let mut duplicate_id = entry.clone();
    duplicate_id["name"] = serde_json::json!("Other stone");
    let mut missing_hsv = entry.clone();
    missing_hsv["color"].as_object_mut().unwrap().remove("hsv");
    let mut noncanonical_id = entry.clone();
    noncanonical_id["id"] = serde_json::json!(entry["id"].as_str().unwrap().replace('-', ""));
    let mut documents = vec![
        b"{broken".to_vec(),
        serde_json::json!({"schema":2,"presets":[]})
            .to_string()
            .into_bytes(),
        serde_json::json!({"schema":1,"presets":[entry.clone(),duplicate_name]})
            .to_string()
            .into_bytes(),
        serde_json::json!({"schema":1,"presets":[entry.clone(),duplicate_id]})
            .to_string()
            .into_bytes(),
        serde_json::json!({"schema":1,"presets":[missing_hsv]})
            .to_string()
            .into_bytes(),
        serde_json::json!({"schema":1,"presets":[noncanonical_id]})
            .to_string()
            .into_bytes(),
        serde_json::json!({"schema":1,"presets":(0..129).map(|i| serde_json::json!({
            "id":uuid::Uuid::new_v4(),"name":format!("Unique {i}"),"color":ColorConfig::default()
        })).collect::<Vec<_>>()})
        .to_string()
        .into_bytes(),
    ];
    let mut oversized = br#"{"schema":1,"presets":[]}"#.to_vec();
    oversized.resize(1024 * 1024 + 1, b' ');
    documents.push(oversized);
    let path = profiles.join("metin-presets.json");
    for document in documents {
        fs::write(&path, &document).unwrap();
        assert!(store.metin_presets().is_err());
        assert!(validate_data_directory(data.path()).is_err());
        assert!(
            store
                .save_metin_preset(None, "Safe", &ColorConfig::default())
                .is_err()
        );
        assert!(store.delete_metin_preset(uuid::Uuid::new_v4()).is_err());
        assert_eq!(fs::read(&path).unwrap(), document);
    }
}

#[test]
fn valid_preset_catalog_at_exact_byte_limit_can_be_read() {
    let root = tempfile::tempdir().unwrap();
    let store = ProfileStore::open(root.path()).unwrap();
    let mut document = br#"{"schema":1,"presets":[]}"#.to_vec();
    document.resize(1024 * 1024, b' ');
    fs::write(root.path().join("metin-presets.json"), document).unwrap();
    assert!(store.metin_presets().unwrap().is_empty());
}

#[test]
fn profile_package_keeps_color_when_catalog_is_absent_or_origin_deleted() {
    let source = tempfile::tempdir().unwrap();
    let mut store = ProfileStore::open(source.path()).unwrap();
    let color = ColorConfig {
        hsv: [75, 123, 234],
        ..Default::default()
    };
    let preset = store.save_metin_preset(None, "Portable", &color).unwrap();
    let mut profile = Profile::create("Portable profile");
    profile.color = Some(color.clone());
    profile.color_preset_id = Some(preset.id);
    store.save(&profile, None).unwrap();
    store.delete_metin_preset(preset.id).unwrap();
    let package = store.export_profile(profile.id).unwrap();
    let document = source.path().join("portable.json");
    package.write(&document).unwrap();
    let target = tempfile::tempdir().unwrap();
    let mut imported_store = ProfileStore::open(target.path()).unwrap();
    let read = metin_desktop::storage::ProfilePackage::read(&document).unwrap();
    let imported = imported_store.import_profile(&read).unwrap();
    assert_ne!(imported.id, profile.id);
    assert_eq!(imported.color, Some(color));
    assert_eq!(imported.color_preset_id, Some(preset.id));
    assert!(imported_store.metin_presets().unwrap().is_empty());
    assert!(!target.path().join("metin-presets.json").exists());
}

#[test]
fn catalog_migrates_with_data_root_and_complete_backup_and_legacy_import() {
    use metin_desktop::storage::{import_data, prepare_data_directory_at};
    let base = tempfile::tempdir().unwrap();
    let old = base.path().join("MetinBot");
    let preset = {
        let mut store = ProfileStore::open(&old.join("profiles")).unwrap();
        store
            .save_metin_preset(None, "Preserved", &ColorConfig::default())
            .unwrap()
    };
    let bytes = fs::read(old.join("profiles/metin-presets.json")).unwrap();
    let migrated = prepare_data_directory_at(base.path(), None).unwrap();
    for root in [&migrated.root, &migrated.backup.unwrap()] {
        assert_eq!(
            fs::read(root.join("profiles/metin-presets.json")).unwrap(),
            bytes
        );
        assert_eq!(
            ProfileStore::open(&root.join("profiles"))
                .unwrap()
                .metin_presets()
                .unwrap(),
            vec![preset.clone()]
        );
    }
    let imported = base.path().join("imported");
    import_data(&migrated.root, &imported).unwrap();
    assert_eq!(
        fs::read(imported.join("profiles/metin-presets.json")).unwrap(),
        bytes
    );
}

#[cfg(windows)]
#[test]
fn failed_preset_commit_preserves_catalog_and_supports_retry() {
    let root = tempfile::tempdir().unwrap();
    let mut store = ProfileStore::open(root.path()).unwrap();
    let preset = store
        .save_metin_preset(None, "Original", &ColorConfig::default())
        .unwrap();
    let path = root.path().join("metin-presets.json");
    let before = fs::read(&path).unwrap();
    let original_permissions = fs::metadata(&path).unwrap().permissions();
    let mut permissions = original_permissions.clone();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    let failed = store.save_metin_preset(Some(preset.id), "Retried", &ColorConfig::default());
    fs::set_permissions(&path, original_permissions).unwrap();
    assert!(failed.is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(store.metin_presets().unwrap(), vec![preset.clone()]);
    let retry = store
        .save_metin_preset(Some(preset.id), "Retried", &ColorConfig::default())
        .unwrap();
    assert_eq!(store.metin_presets().unwrap(), vec![retry]);
    assert_eq!(
        fs::read_dir(root.path()).unwrap().count(),
        2,
        "Temporary commit files must be cleaned up"
    );
}

#[cfg(windows)]
#[test]
fn linked_preset_catalog_cannot_be_read_or_replaced() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path().join("profiles");
    let mut store = ProfileStore::open(&root).unwrap();
    let external = tempfile::tempdir().unwrap();
    let link = root.join("metin-presets.json");
    let created = std::process::Command::new("cmd.exe")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(external.path())
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "Could not create isolated junction fixture: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    assert!(store.metin_presets().is_err());
    assert!(
        store
            .save_metin_preset(None, "Linked", &ColorConfig::default())
            .is_err()
    );
    assert!(store.delete_metin_preset(uuid::Uuid::new_v4()).is_err());
    assert!(validate_data_directory(data.path()).is_err());
    fs::remove_dir(link).unwrap();
}
