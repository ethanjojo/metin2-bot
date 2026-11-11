use metin_core::Profile;
use metin_desktop::storage::{ProfileStore, prepare_data_directory_at};
use std::{fs, path::Path};

fn old_data(base: &Path) -> Profile {
    let root = base.join("MetinBot");
    let mut store = ProfileStore::open(&root.join("profiles")).unwrap();
    let profile = Profile::create("Existing profile");
    let image =
        metin_platform::Frame::new(6, 6, (0..108).map(|n| (n * 7 % 255) as u8).collect()).unwrap();
    let profile = store.save(&profile, Some(&image)).unwrap().profile;
    store.select(profile.id).unwrap();
    fs::create_dir(root.join("logs")).unwrap();
    fs::write(root.join("logs/run.log"), b"retained log\n").unwrap();
    fs::write(root.join("extra.bin"), [0, 1, 255]).unwrap();
    profile
}

#[test]
fn default_migration_preserves_selection_all_bytes_and_a_complete_backup() {
    let base = tempfile::tempdir().unwrap();
    let profile = old_data(base.path());
    let result = prepare_data_directory_at(base.path(), None).unwrap();
    assert_eq!(result.root, base.path().join("Companion"));
    assert!(!base.path().join("MetinBot").exists());
    let backup = result.backup.unwrap();
    for root in [&result.root, &backup] {
        assert_eq!(
            fs::read(root.join("logs/run.log")).unwrap(),
            b"retained log\n"
        );
        assert_eq!(fs::read(root.join("extra.bin")).unwrap(), [0, 1, 255]);
        let store = ProfileStore::open(&root.join("profiles")).unwrap();
        assert_eq!(store.selected().unwrap(), Some(profile.id));
        assert_eq!(store.load(profile.id).unwrap().0, profile);
        assert!(store.load(profile.id).unwrap().1.is_some());
    }
    let again = prepare_data_directory_at(base.path(), None).unwrap();
    assert_eq!(again.root, result.root);
    assert!(again.backup.is_none());
}

#[test]
fn occupied_destination_is_never_merged_or_overwritten() {
    let base = tempfile::tempdir().unwrap();
    old_data(base.path());
    fs::create_dir(base.path().join("Companion")).unwrap();
    fs::write(base.path().join("Companion/keep.bin"), b"keep").unwrap();
    assert!(prepare_data_directory_at(base.path(), None).is_err());
    assert!(base.path().join("MetinBot/extra.bin").exists());
    assert_eq!(
        fs::read(base.path().join("Companion/keep.bin")).unwrap(),
        b"keep"
    );
    assert_eq!(fs::read_dir(base.path()).unwrap().count(), 2);
}

#[test]
fn busy_old_store_prevents_move_and_backup() {
    let base = tempfile::tempdir().unwrap();
    old_data(base.path());
    let _writer = ProfileStore::open(&base.path().join("MetinBot/profiles")).unwrap();
    assert!(prepare_data_directory_at(base.path(), None).is_err());
    assert!(base.path().join("MetinBot").exists());
    assert!(!base.path().join("Companion").exists());
    assert_eq!(fs::read_dir(base.path()).unwrap().count(), 1);
}

#[test]
fn updater_explicit_old_default_migrates_but_custom_roots_stay_explicit() {
    let base = tempfile::tempdir().unwrap();
    old_data(base.path());
    let custom = base.path().join("custom");
    let result = prepare_data_directory_at(base.path(), Some(&custom)).unwrap();
    assert_eq!(result.root, custom);
    assert!(result.backup.is_none());
    assert!(base.path().join("MetinBot").exists());
    // Update handoff uses canonical paths with the Windows verbatim prefix.
    let canonical_old = base.path().join("MetinBot").canonicalize().unwrap();
    let result = prepare_data_directory_at(base.path(), Some(&canonical_old)).unwrap();
    assert_eq!(result.root, base.path().join("Companion"));
    assert!(result.backup.is_some());
}

#[test]
fn empty_default_does_not_recreate_legacy_storage() {
    let base = tempfile::tempdir().unwrap();
    let result = prepare_data_directory_at(base.path(), None).unwrap();
    assert_eq!(result.root, base.path().join("Companion"));
    assert!(result.backup.is_none());
    assert_eq!(fs::read_dir(base.path()).unwrap().count(), 0);
}

#[cfg(windows)]
#[test]
fn linked_old_tree_is_rejected_before_moving() {
    let base = tempfile::tempdir().unwrap();
    old_data(base.path());
    let outside = tempfile::tempdir().unwrap();
    let link = base.path().join("MetinBot").join("logs").join("external");
    let created = std::process::Command::new("cmd.exe")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(outside.path())
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    assert!(prepare_data_directory_at(base.path(), None).is_err());
    assert!(base.path().join("MetinBot").exists());
    assert!(!base.path().join("Companion").exists());
    fs::remove_dir(link).unwrap();
}

#[cfg(windows)]
#[test]
fn unrelated_custom_store_ignores_a_dangling_previous_default() {
    let base = tempfile::tempdir().unwrap();
    let old = base.path().join("MetinBot");
    let target = base.path().join("missing-target");
    fs::create_dir(&target).unwrap();
    let created = std::process::Command::new("cmd.exe")
        .args(["/C", "mklink", "/J"])
        .arg(&old)
        .arg(&target)
        .output()
        .unwrap();
    assert!(created.status.success());
    fs::remove_dir(target).unwrap();
    let custom = base.path().join("custom");
    fs::create_dir(&custom).unwrap();
    let result = prepare_data_directory_at(base.path(), Some(&custom)).unwrap();
    assert_eq!(result.root, custom);
    assert!(result.backup.is_none());
    fs::remove_dir(old).unwrap();
}
