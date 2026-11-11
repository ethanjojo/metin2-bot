use metin_desktop::installation::verify_installed_bundle;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

fn bundle(root: &Path) {
    fs::create_dir(root).unwrap();
    let entries: Vec<_> = [
        ("Companion.exe", b"application".as_slice()),
        ("bridge.dll", b"native"),
    ]
    .into_iter()
    .map(|(name, bytes)| {
        fs::write(root.join(name), bytes).unwrap();
        serde_json::json!({"path":name,"sha256":format!("{:x}",Sha256::digest(bytes))})
    })
    .collect();
    fs::write(
        root.join("files.json"),
        serde_json::to_vec(&entries).unwrap(),
    )
    .unwrap();
    fs::write(root.join("unins000.exe"), b"uninstaller").unwrap();
    fs::write(root.join("unins000.dat"), b"uninstall history").unwrap();
}

fn retire_previous_installation(
    previous: &Path,
    current: &Path,
) -> Result<Option<std::path::PathBuf>, String> {
    if !previous.exists() {
        verify_installed_bundle(current)?;
        return Ok(None);
    }
    let snapshot = tempfile::NamedTempFile::new().unwrap();
    let inventory: serde_json::Value =
        serde_json::from_slice(&fs::read(previous.join("files.json")).unwrap()).unwrap();
    let notice = previous.join("licenses/removed-dependency/LICENSE.txt");
    let notices = if notice.exists() {
        vec![
            serde_json::json!({"path":"licenses/removed-dependency/LICENSE.txt","sha256":format!("{:x}",Sha256::digest(fs::read(notice).unwrap()))}),
        ]
    } else {
        vec![]
    };
    let bytes = serde_json::to_vec(
        &serde_json::json!({"inventory":inventory,"historical_notices":notices,"archive_id":format!("{:x}",Sha256::digest(uuid::Uuid::new_v4().as_bytes()))}),
    )
    .unwrap();
    fs::write(snapshot.path(), &bytes).unwrap();
    fs::write(current.join(".companion-upgrade-pending.json"), &bytes).unwrap();
    metin_desktop::installation::finalize_upgrade(previous, current, snapshot.path(), || Ok(()))
}

#[test]
fn verified_upgrade_archives_only_the_previous_default_installation() {
    let parent = tempfile::tempdir().unwrap();
    let old = parent.path().join("MetinBot");
    let new = parent.path().join("Companion");
    bundle(&old);
    bundle(&new);
    let backup = retire_previous_installation(&old, &new).unwrap().unwrap();
    assert!(!old.exists());
    assert!(
        backup
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("Companion-install-backup-")
    );
    assert_eq!(
        fs::read(backup.join("Companion.exe")).unwrap(),
        b"application"
    );
    assert_eq!(fs::read(new.join("Companion.exe")).unwrap(), b"application");
    assert!(retire_previous_installation(&old, &new).unwrap().is_none());
}

#[test]
fn corrupt_new_bundle_retains_complete_previous_installation() {
    let parent = tempfile::tempdir().unwrap();
    let old = parent.path().join("MetinBot");
    let new = parent.path().join("Companion");
    bundle(&old);
    bundle(&new);
    fs::write(new.join("bridge.dll"), b"corrupt").unwrap();
    assert!(retire_previous_installation(&old, &new).is_err());
    assert!(old.join("Companion.exe").is_file());
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 2);
}

#[test]
fn unknown_old_files_are_never_moved_or_deleted_automatically() {
    let parent = tempfile::tempdir().unwrap();
    let old = parent.path().join("MetinBot");
    let new = parent.path().join("Companion");
    bundle(&old);
    bundle(&new);
    fs::write(old.join("my-notes.txt"), b"private notes").unwrap();
    assert!(retire_previous_installation(&old, &new).is_err());
    assert_eq!(
        fs::read(old.join("my-notes.txt")).unwrap(),
        b"private notes"
    );
}

#[test]
fn traversal_inventory_is_rejected_without_touching_external_file() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("Companion");
    bundle(&root);
    fs::write(parent.path().join("external"), b"external").unwrap();
    let entries = serde_json::json!([{"path":"../external","sha256":format!("{:x}",Sha256::digest(b"external"))}]);
    fs::write(root.join("files.json"), entries.to_string()).unwrap();
    assert!(verify_installed_bundle(&root).is_err());
    assert_eq!(
        fs::read(parent.path().join("external")).unwrap(),
        b"external"
    );
}

#[test]
fn cleanup_refuses_nested_and_unrelated_installation_roots() {
    let parent = tempfile::tempdir().unwrap();
    let old = parent.path().join("MetinBot");
    let other = parent.path().join("Other");
    bundle(&old);
    bundle(&other);
    assert!(retire_previous_installation(&old, &other).is_err());
    let nested = old.join("Companion");
    bundle(&nested);
    assert!(retire_previous_installation(&old, &nested).is_err());
    assert!(old.is_dir());
    assert!(other.is_dir());
    assert!(nested.is_dir());
}

#[test]
fn inventoried_previous_executable_name_can_be_archived_but_never_used_as_new_install() {
    let parent = tempfile::tempdir().unwrap();
    let old = parent.path().join("MetinBot");
    let new = parent.path().join("Companion");
    bundle(&old);
    bundle(&new);
    fs::rename(old.join("Companion.exe"), old.join("MetinBot.exe")).unwrap();
    let inventory = fs::read_to_string(old.join("files.json"))
        .unwrap()
        .replace("Companion.exe", "MetinBot.exe");
    fs::write(old.join("files.json"), inventory).unwrap();
    assert!(verify_installed_bundle(&old).is_err());
    let backup = retire_previous_installation(&old, &new).unwrap().unwrap();
    assert!(backup.join("MetinBot.exe").is_file());
    assert!(!old.exists());
}

#[cfg(windows)]
#[test]
fn generated_uninstall_log_can_remain_open_during_postinstall_verification() {
    use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt};
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("Companion");
    bundle(&root);
    let _setup_log = OpenOptions::new()
        .write(true)
        .share_mode(0)
        .open(root.join("unins000.dat"))
        .unwrap();
    verify_installed_bundle(&root).unwrap();
}

#[test]
fn report_reservation_never_overwrites_data_or_adds_files_inside_a_migrating_tree() {
    use metin_desktop::installation::reserve_migration_report;
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("Companion");
    bundle(&root);
    let existing = parent.path().join("existing.json");
    fs::write(&existing, b"keep").unwrap();
    assert!(reserve_migration_report(&existing, &[&root]).is_err());
    assert_eq!(fs::read(&existing).unwrap(), b"keep");
    assert!(reserve_migration_report(&root.join("report.json"), &[&root]).is_err());
    assert!(!root.join("report.json").exists());
    let report = parent.path().join("fresh.json");
    let reserved = reserve_migration_report(&report, &[&root]).unwrap();
    assert!(reserve_migration_report(&report, &[&root]).is_err());
    drop(reserved);
}

#[test]
fn snapshotted_previous_license_notices_are_archived_while_new_bundle_extras_are_rejected() {
    let parent = tempfile::tempdir().unwrap();
    let old = parent.path().join("MetinBot");
    let new = parent.path().join("Companion");
    bundle(&old);
    bundle(&new);
    fs::create_dir_all(old.join("licenses/removed-dependency")).unwrap();
    fs::write(
        old.join("licenses/removed-dependency/LICENSE.txt"),
        b"retain obsolete notice",
    )
    .unwrap();
    let backup = retire_previous_installation(&old, &new).unwrap().unwrap();
    assert_eq!(
        fs::read(backup.join("licenses/removed-dependency/LICENSE.txt")).unwrap(),
        b"retain obsolete notice"
    );
    fs::create_dir_all(new.join("licenses/removed-dependency")).unwrap();
    fs::write(
        new.join("licenses/removed-dependency/LICENSE.txt"),
        b"not in new package",
    )
    .unwrap();
    assert!(verify_installed_bundle(&new).is_err());
}

#[test]
fn obsolete_license_exception_does_not_allow_unlisted_binary_payloads() {
    let parent = tempfile::tempdir().unwrap();
    let old = parent.path().join("MetinBot");
    let new = parent.path().join("Companion");
    bundle(&old);
    bundle(&new);
    fs::create_dir(old.join("licenses")).unwrap();
    fs::write(old.join("licenses/unlisted.dll"), b"binary payload").unwrap();
    assert!(retire_previous_installation(&old, &new).is_err());
    assert_eq!(
        fs::read(old.join("licenses/unlisted.dll")).unwrap(),
        b"binary payload"
    );
}

fn legacy_overlay(root: &Path, snapshot: &Path) {
    bundle(root);
    fs::rename(root.join("Companion.exe"), root.join("MetinBot.exe")).unwrap();
    fs::rename(root.join("bridge.dll"), root.join("retired.dll")).unwrap();
    let inventory = fs::read_to_string(root.join("files.json"))
        .unwrap()
        .replace("Companion.exe", "MetinBot.exe")
        .replace("bridge.dll", "retired.dll");
    let snapshot_value = serde_json::json!({"inventory":serde_json::from_str::<serde_json::Value>(&inventory).unwrap(),"historical_notices":[],"archive_id":format!("{:x}",Sha256::digest(uuid::Uuid::new_v4().as_bytes()))});
    fs::write(snapshot, snapshot_value.to_string()).unwrap();
    fs::write(
        root.join(".companion-upgrade-pending.json"),
        snapshot_value.to_string(),
    )
    .unwrap();
    fs::write(root.join("Companion.exe"), b"application").unwrap();
    fs::write(root.join("bridge.dll"), b"native").unwrap();
    let entries = serde_json::json!([
        {"path":"Companion.exe","sha256":format!("{:x}",Sha256::digest(b"application"))},
        {"path":"bridge.dll","sha256":format!("{:x}",Sha256::digest(b"native"))}
    ]);
    fs::write(root.join("files.json"), entries.to_string()).unwrap();
}

#[test]
fn inplace_upgrade_archives_retired_owned_payload_after_runtime_acceptance() {
    let parent = tempfile::tempdir().unwrap();
    for name in ["MetinBot", "Custom install"] {
        let root = parent.path().join(name);
        let snapshot = parent.path().join(format!("{name}-inventory.json"));
        legacy_overlay(&root, &snapshot);
        let backup = metin_desktop::installation::finalize_upgrade(&root, &root, &snapshot, || {
            assert!(root.join("MetinBot.exe").exists());
            Ok(())
        })
        .unwrap()
        .unwrap();
        assert!(backup.join("MetinBot.exe").is_file());
        assert!(backup.join("retired.dll").is_file());
        assert!(backup.join("files.json").is_file());
        assert!(!root.join("MetinBot.exe").exists());
        assert!(!root.join(".companion-upgrade-pending.json").exists());
        verify_installed_bundle(&root).unwrap();
    }
}

#[test]
fn inplace_upgrade_retains_all_files_on_unknown_modified_or_runtime_failure() {
    let parent = tempfile::tempdir().unwrap();
    for case in ["unknown", "modified", "runtime", "snapshot"] {
        let root = parent.path().join(case);
        let snapshot = parent.path().join(format!("{case}.json"));
        legacy_overlay(&root, &snapshot);
        match case {
            "unknown" => fs::write(root.join("notes.txt"), b"private").unwrap(),
            "modified" => fs::write(root.join("retired.dll"), b"edited").unwrap(),
            "snapshot" => fs::write(&snapshot, b"[]").unwrap(),
            _ => (),
        }
        assert!(
            metin_desktop::installation::finalize_upgrade(&root, &root, &snapshot, || {
                if case == "runtime" {
                    Err("runtime rejected".into())
                } else {
                    Ok(())
                }
            })
            .is_err()
        );
        assert!(root.join("MetinBot.exe").is_file());
        assert!(root.join("retired.dll").is_file());
        assert!(root.join(".companion-upgrade-pending.json").is_file());
    }
}

#[test]
fn operation_report_is_reserved_before_any_manager_call() {
    use std::cell::Cell;
    let parent = tempfile::tempdir().unwrap();
    let calls = Cell::new(0);
    let report = parent.path().join("existing.json");
    fs::write(&report, b"keep").unwrap();
    for path in [&report, &parent.path().join("missing/report.json")] {
        assert!(
            metin_desktop::installation::run_reported_operation(
                path,
                || {
                    calls.set(calls.get() + 1);
                    Ok("ran".to_owned())
                },
                |result| serde_json::json!({"success":result.is_ok()})
            )
            .is_err()
        );
    }
    assert_eq!(calls.get(), 0);
    assert_eq!(fs::read(&report).unwrap(), b"keep");
    let fresh = parent.path().join("fresh.json");
    metin_desktop::installation::run_reported_operation(
        &fresh,
        || {
            calls.set(calls.get() + 1);
            Ok("ran".to_owned())
        },
        |result| serde_json::json!({"success":result.is_ok()}),
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(fresh).unwrap()).unwrap()["success"],
        true
    );
}

#[cfg(windows)]
#[test]
fn linked_report_parent_prevents_any_manager_operation() {
    use std::cell::Cell;
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    let link = root.path().join("redirected");
    fs::create_dir(&target).unwrap();
    let result = std::process::Command::new("cmd.exe")
        .args(["/c", "mklink", "/J"])
        .arg(&link)
        .arg(&target)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "Junction fixture creation failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let calls = Cell::new(0);
    let outcome = metin_desktop::installation::run_reported_operation(
        &link.join("report.json"),
        || {
            calls.set(calls.get() + 1);
            Ok(())
        },
        |result| serde_json::json!({"success":result.is_ok()}),
    );
    fs::remove_dir(&link).unwrap();
    assert!(outcome.is_err());
    assert_eq!(calls.get(), 0);
    assert!(!target.join("report.json").exists());
}

#[test]
fn retired_notice_snapshot_rejects_traversal_and_changed_notice_before_moving_payload() {
    let parent = tempfile::tempdir().unwrap();
    for case in ["traversal", "changed", "binary"] {
        let root = parent.path().join(case);
        let snapshot = parent.path().join(format!("{case}.json"));
        legacy_overlay(&root, &snapshot);
        fs::create_dir_all(root.join("licenses")).unwrap();
        fs::write(root.join("licenses/NOTICE.txt"), b"modified notice").unwrap();
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&snapshot).unwrap()).unwrap();
        let notice_path = match case {
            "traversal" => "licenses/../../external",
            "binary" => "licenses/unlisted.dll",
            _ => "licenses/NOTICE.txt",
        };
        value["historical_notices"] = serde_json::json!([{"path":notice_path,"sha256":format!("{:x}",Sha256::digest(b"original notice"))}]);
        fs::write(&snapshot, value.to_string()).unwrap();
        fs::write(
            root.join(".companion-upgrade-pending.json"),
            value.to_string(),
        )
        .unwrap();
        assert!(
            metin_desktop::installation::finalize_upgrade(&root, &root, &snapshot, || Ok(()))
                .is_err()
        );
        assert!(root.join("MetinBot.exe").exists());
        assert_eq!(
            fs::read(root.join("licenses/NOTICE.txt")).unwrap(),
            b"modified notice"
        );
    }
}

fn interrupted_archive(root: &Path, snapshot: &Path, move_file: &str) -> std::path::PathBuf {
    let value: serde_json::Value = serde_json::from_slice(&fs::read(snapshot).unwrap()).unwrap();
    let archive = root.parent().unwrap().join(format!(
        "Companion-install-backup-{}",
        value["archive_id"].as_str().unwrap()
    ));
    fs::create_dir(&archive).unwrap();
    fs::copy(snapshot, archive.join("upgrade-snapshot.json")).unwrap();
    fs::write(archive.join("files.json"), value["inventory"].to_string()).unwrap();
    fs::rename(root.join(move_file), archive.join(move_file)).unwrap();
    archive
}

#[test]
fn interrupted_inplace_archive_resumes_into_the_same_identified_archive() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("Custom install");
    let snapshot = parent.path().join("snapshot.json");
    legacy_overlay(&root, &snapshot);
    let archive = interrupted_archive(&root, &snapshot, "MetinBot.exe");
    assert!(!root.join("MetinBot.exe").exists());
    assert!(root.join("retired.dll").exists());
    let completed =
        metin_desktop::installation::finalize_upgrade(&root, &root, &snapshot, || Ok(()))
            .unwrap()
            .unwrap();
    assert_eq!(completed, archive);
    assert_eq!(
        fs::read(archive.join("MetinBot.exe")).unwrap(),
        b"application"
    );
    assert_eq!(fs::read(archive.join("retired.dll")).unwrap(), b"native");
    verify_installed_bundle(&root).unwrap();
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 3);
}

#[test]
fn interrupted_archive_refuses_foreign_corrupt_duplicate_or_unknown_contents() {
    let parent = tempfile::tempdir().unwrap();
    for case in ["foreign", "corrupt", "duplicate", "unknown"] {
        let root = parent.path().join(case);
        let snapshot = parent.path().join(format!("{case}.json"));
        legacy_overlay(&root, &snapshot);
        let archive = interrupted_archive(&root, &snapshot, "MetinBot.exe");
        match case {
            "foreign" => {
                fs::write(archive.join("upgrade-snapshot.json"), b"foreign snapshot").unwrap()
            }
            "corrupt" => fs::write(archive.join("MetinBot.exe"), b"corrupt").unwrap(),
            "duplicate" => fs::write(root.join("MetinBot.exe"), b"application").unwrap(),
            _ => fs::write(archive.join("private.txt"), b"private").unwrap(),
        }
        assert!(
            metin_desktop::installation::finalize_upgrade(&root, &root, &snapshot, || Ok(()))
                .is_err()
        );
        assert_eq!(fs::read(root.join("retired.dll")).unwrap(), b"native");
        assert!(root.join(".companion-upgrade-pending.json").exists());
    }
}

#[test]
fn interrupted_relocation_accepts_only_its_snapshot_identified_complete_archive() {
    let parent = tempfile::tempdir().unwrap();
    let old = parent.path().join("MetinBot");
    let new = parent.path().join("Companion");
    bundle(&old);
    bundle(&new);
    let original: serde_json::Value =
        serde_json::from_slice(&fs::read(old.join("files.json")).unwrap()).unwrap();
    let snapshot = parent.path().join("snapshot.json");
    let identity = format!("{:x}", Sha256::digest(uuid::Uuid::new_v4().as_bytes()));
    let value =
        serde_json::json!({"inventory":original,"historical_notices":[],"archive_id":identity});
    fs::write(&snapshot, value.to_string()).unwrap();
    fs::write(
        new.join(".companion-upgrade-pending.json"),
        value.to_string(),
    )
    .unwrap();
    let archive = parent
        .path()
        .join(format!("Companion-install-backup-{identity}"));
    fs::copy(&snapshot, old.join("upgrade-snapshot.json")).unwrap();
    fs::rename(&old, &archive).unwrap();
    assert_eq!(
        metin_desktop::installation::finalize_upgrade(&old, &new, &snapshot, || Ok(())).unwrap(),
        Some(archive)
    );
    verify_installed_bundle(&new).unwrap();
}
