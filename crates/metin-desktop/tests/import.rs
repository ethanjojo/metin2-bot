use metin_core::Profile;
use metin_desktop::storage::{ProfileStore, UISettings, import_data};

#[test]
fn imported_configuration_gets_a_neutral_name_and_keeps_explicit_process_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.json");
    std::fs::write(&path, r#"{"process_name":"custom-client.exe"}"#).unwrap();
    let mut store = ProfileStore::open(&tmp.path().join("profiles")).unwrap();
    let profile = store.migrate_legacy(&path).unwrap().unwrap();
    assert_eq!(profile.name, "Imported profile");
    assert_eq!(profile.process_name, "custom-client.exe");
}
#[test]
fn corrupt_canonical_settings_do_not_trigger_legacy_migration_or_create_profiles() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("config.json"), "{}").unwrap();
    std::fs::write(tmp.path().join("ui-settings.json"), "{broken").unwrap();
    let context = eframe::egui::Context::default();
    assert!(metin_desktop::ui::MetinApp::from_context(&context, tmp.path().to_owned()).is_err());
    assert!(!tmp.path().join("profiles").exists());
    assert!(!tmp.path().join("config.queue-backup.json").exists());
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("config.json")).unwrap(),
        "{}"
    );
    assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 2);
}

#[cfg(unix)]
#[test]
fn dangling_entry_and_unreferenced_nested_symlinks_are_rejected_without_writes() {
    use std::os::unix::fs::symlink;
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("old");
    let destination = tmp.path().join("new");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("ui-settings.json"), "{}").unwrap();
    symlink(tmp.path().join("missing"), source.join("profiles")).unwrap();
    assert!(import_data(&source, &destination).is_err());
    assert!(!destination.exists());
    std::fs::remove_file(source.join("profiles")).unwrap();
    let p = Profile::create("Safe");
    {
        let mut store = ProfileStore::open(&source.join("profiles")).unwrap();
        store.save(&p, None).unwrap();
    }
    symlink(
        tmp.path().join("missing"),
        source
            .join("profiles")
            .join(p.id.to_string())
            .join("unused"),
    )
    .unwrap();
    assert!(metin_desktop::storage::validate_data_directory(&source).is_err());
    assert!(import_data(&source, &destination).is_err());
    assert!(!destination.exists());
}
#[test]
fn adjacent_discovery_requires_known_application_data() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(!metin_desktop::storage::has_legacy_data(tmp.path()));
    std::fs::write(tmp.path().join("random.txt"), "irrelevant").unwrap();
    assert!(!metin_desktop::storage::has_legacy_data(tmp.path()));
    std::fs::write(tmp.path().join("config.queue-backup.json"), "{}").unwrap();
    assert!(metin_desktop::storage::has_legacy_data(tmp.path()));
}
#[test]
fn invalid_backup_and_legacy_conflict_never_publish_import() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("old");
    let destination = tmp.path().join("new");
    std::fs::create_dir(&source).unwrap();
    for value in ["{broken", "[]"] {
        std::fs::write(source.join("config.queue-backup.json"), value).unwrap();
        assert!(import_data(&source, &destination).is_err());
        assert!(!destination.exists());
        assert!(metin_desktop::storage::validate_legacy_files(&source).is_err());
    }
    std::fs::write(source.join("config.queue-backup.json"), "{}").unwrap();
    std::fs::write(source.join("config.json"), "{}").unwrap();
    assert!(import_data(&source, &destination).is_err());
    assert!(!destination.exists());
}
#[test]
fn import_rejects_recursive_destination_without_creating_it() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("old");
    std::fs::create_dir_all(source.join("profiles")).unwrap();
    let destination = source.join("profiles/nested/new");
    assert!(import_data(&source, &destination).is_err());
    assert!(!source.join("profiles/nested").exists());
}

#[test]
fn import_preserves_selection_settings_and_originals() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("old");
    let destination = tmp.path().join("new");
    let p = Profile::create("Původní profil");
    {
        let mut store = ProfileStore::open(&source.join("profiles")).unwrap();
        store.save(&p, None).unwrap();
        store.select(p.id).unwrap();
    }
    UISettings {
        theme: "dark".into(),
        last_mounted: true,
        profile_windows: [(
            p.id,
            metin_desktop::storage::WindowPreference {
                process_name: "client.exe".into(),
                title: "Server".into(),
            },
        )]
        .into(),
    }
    .save(&source)
    .unwrap();
    import_data(&source, &destination).unwrap();
    let imported = ProfileStore::open(&destination.join("profiles")).unwrap();
    assert_eq!(imported.selected().unwrap(), Some(p.id));
    assert_eq!(imported.load(p.id).unwrap().0, p);
    assert!(UISettings::load(&destination).unwrap().last_mounted);
    assert_eq!(
        UISettings::load(&destination).unwrap().profile_windows,
        UISettings::load(&source).unwrap().profile_windows
    );
    assert!(
        source
            .join("profiles")
            .join(p.id.to_string())
            .join("profile.json")
            .is_file()
    );
    assert!(import_data(&source, &destination).is_err());
}

#[test]
fn invalid_import_leaves_destination_and_source_untouched() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("old");
    let destination = tmp.path().join("new");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&destination).unwrap();
    std::fs::write(source.join("ui-settings.json"), "{broken").unwrap();
    assert!(import_data(&source, &destination).is_err());
    assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 0);
    assert_eq!(
        std::fs::read_to_string(source.join("ui-settings.json")).unwrap(),
        "{broken"
    );
}

#[test]
fn interrupted_legacy_migration_reuses_existing_profile_without_overwriting() {
    let tmp = tempfile::tempdir().unwrap();
    let legacy = tmp.path().join("config.json");
    let original = r#"{"process_name":"metin2client.exe"}"#;
    std::fs::write(&legacy, original).unwrap();
    let mut store = ProfileStore::open(&tmp.path().join("profiles")).unwrap();
    let mut p = store.migrate_legacy(&legacy).unwrap().unwrap();
    p.name = "Upravený profil".into();
    store.save(&p, None).unwrap();
    std::fs::remove_file(tmp.path().join("config.queue-backup.json")).unwrap();
    std::fs::write(&legacy, original).unwrap();
    assert_eq!(store.migrate_legacy(&legacy).unwrap().unwrap(), p);
    assert_eq!(store.list().unwrap().len(), 1);
    assert!(!legacy.exists());
    assert_eq!(store.selected().unwrap(), Some(p.id));
}
