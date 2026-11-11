//! Verified installation finalization after setup has written and tested the new bundle.
use crate::file_tree;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

pub const PENDING_INVENTORY: &str = ".companion-upgrade-pending.json";
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct InventoryEntry {
    path: String,
    sha256: String,
}
type Inventory = BTreeMap<String, InventoryEntry>;
fn key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}
fn uninstaller_file(path: &Path) -> bool {
    if path.components().count() != 1 {
        return false;
    }
    let name = key(path);
    name.len() == 12
        && name.starts_with("unins")
        && name.as_bytes()[5..8].iter().all(u8::is_ascii_digit)
        && matches!(&name[8..], ".exe" | ".dat" | ".msg")
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    file_tree::reject_linked_ancestors(path).map_err(|e| e.to_string())?;
    if !fs::metadata(path).map_err(|e| e.to_string())?.is_file()
        || fs::metadata(path).map_err(|e| e.to_string())?.len() > 4 * 1024 * 1024
    {
        return Err("Installed inventory must be a regular file smaller than 4 MiB.".into());
    }
    serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
fn inventory(path: &Path, previous: bool) -> Result<Inventory, String> {
    entries(read_json(path)?, Some(previous))
}
fn entries(entries: Vec<InventoryEntry>, previous: Option<bool>) -> Result<Inventory, String> {
    if (previous.is_some() && entries.is_empty()) || entries.len() > 10000 {
        return Err("Invalid inventory size.".into());
    }
    let mut result = BTreeMap::new();
    for entry in entries {
        let relative = Path::new(&entry.path);
        if relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
            || entry.path.contains(':')
            || entry.path.is_empty()
            || entry.path.split(['/', '\\']).any(|part| {
                part.is_empty() || part == "." || part == ".." || part.ends_with(['.', ' '])
            })
            || entry.sha256.len() != 64
            || !entry.sha256.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("Unsafe installed inventory entry.".into());
        }
        let normalized = key(relative);
        if normalized == "files.json"
            || normalized == PENDING_INVENTORY
            || uninstaller_file(relative)
            || result.insert(normalized, entry).is_some()
        {
            return Err("Duplicate or reserved installed inventory entry.".into());
        }
    }
    if previous.is_some()
        && !result.contains_key("companion.exe")
        && !(previous == Some(true) && result.contains_key("metinbot.exe"))
    {
        return Err("Installed inventory has no Companion executable.".into());
    }
    Ok(result)
}
fn verify_files(
    root: &Path,
    expected: &Inventory,
    retired: &Inventory,
    pending: bool,
) -> Result<(), String> {
    let actual: BTreeMap<_, _> = file_tree::files(root)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|p| (key(&p), p))
        .collect();
    for (name, entry) in expected.iter().chain(retired) {
        let path = actual
            .get(name)
            .ok_or_else(|| format!("Installed file missing: {}", entry.path))?;
        if file_tree::hash(&root.join(path)).map_err(|e| e.to_string())?
            != entry.sha256.to_ascii_lowercase()
        {
            return Err(format!("Installed file checksum mismatch: {}", entry.path));
        }
    }
    for (name, path) in actual {
        if name != "files.json"
            && !(pending && name == PENDING_INVENTORY)
            && !uninstaller_file(&path)
            && !expected.contains_key(&name)
            && !retired.contains_key(&name)
        {
            return Err(format!(
                "Unrecognized installation file retained: {}",
                path.display()
            ));
        }
    }
    Ok(())
}
/// Installed trees may additionally contain Inno's generated uninstall files.
pub fn verify_installed_bundle(root: &Path) -> Result<(), String> {
    verify_files(
        root,
        &inventory(&root.join("files.json"), false)?,
        &Inventory::new(),
        false,
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpgradeSnapshot {
    inventory: Vec<InventoryEntry>,
    historical_notices: Vec<InventoryEntry>,
    archive_id: String,
}
const ARCHIVE_IDENTITY: &str = "upgrade-snapshot.json";
struct UpgradeState {
    backup: PathBuf,
    same: bool,
    package: Inventory,
    retired: Inventory,
    remaining: Inventory,
}
fn validate_archive_identity(
    archive: &Path,
    snapshot: &Path,
    package: &Inventory,
) -> Result<(), String> {
    file_tree::reject_linked_ancestors(&archive.join(ARCHIVE_IDENTITY))
        .map_err(|e| e.to_string())?;
    if fs::read(archive.join(ARCHIVE_IDENTITY)).map_err(|e| e.to_string())?
        != fs::read(snapshot).map_err(|e| e.to_string())?
    {
        return Err("Recovery archive belongs to a different upgrade snapshot.".into());
    }
    if inventory(&archive.join("files.json"), true)? != *package {
        return Err("Recovery archive package inventory changed.".into());
    }
    Ok(())
}
fn verify_relocation_tree(
    root: &Path,
    snapshot: &Path,
    owned: &Inventory,
    receipt_required: bool,
) -> Result<(), String> {
    let receipt = root.join(ARCHIVE_IDENTITY);
    let mut expected = owned.clone();
    if receipt_required || receipt.try_exists().map_err(|e| e.to_string())? {
        file_tree::reject_linked_ancestors(&receipt).map_err(|e| e.to_string())?;
        if fs::read(&receipt).map_err(|e| e.to_string())?
            != fs::read(snapshot).map_err(|e| e.to_string())?
        {
            return Err("Previous installation contains a foreign upgrade receipt.".into());
        }
        expected.insert(
            ARCHIVE_IDENTITY.into(),
            InventoryEntry {
                path: ARCHIVE_IDENTITY.into(),
                sha256: file_tree::hash(snapshot).map_err(|e| e.to_string())?,
            },
        );
    }
    verify_files(root, &expected, &Inventory::new(), false)
}
fn upgrade_state(
    previous: &Path,
    current: &Path,
    snapshot_path: &Path,
) -> Result<UpgradeState, String> {
    if !previous.is_absolute() || !current.is_absolute() || !snapshot_path.is_absolute() {
        return Err("Upgrade paths must be absolute.".into());
    }
    file_tree::reject_linked_ancestors(previous).map_err(|e| e.to_string())?;
    let same = crate::storage::resolve_new_path(previous).map_err(|e| e.to_string())?
        == current.canonicalize().map_err(|e| e.to_string())?;
    if !same
        && (!previous
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("MetinBot"))
            || !current
                .file_name()
                .is_some_and(|n| n.eq_ignore_ascii_case("Companion"))
            || previous.parent().and_then(|p| p.canonicalize().ok())
                != current.parent().and_then(|p| p.canonicalize().ok()))
    {
        return Err(
            "Relocation requires the previous default installation and its Companion sibling."
                .into(),
        );
    }
    let snapshot: UpgradeSnapshot = read_json(snapshot_path)?;
    if snapshot.archive_id.len() != 64
        || !snapshot.archive_id.bytes().all(|c| c.is_ascii_hexdigit())
    {
        return Err("Invalid upgrade archive identity.".into());
    }
    let backup = current
        .parent()
        .ok_or("Installation has no parent.")?
        .join(format!(
            "Companion-install-backup-{}",
            snapshot.archive_id.to_ascii_lowercase()
        ));
    file_tree::reject_linked_ancestors(&backup).map_err(|e| e.to_string())?;
    let package = entries(snapshot.inventory, Some(true))?;
    let notices = entries(snapshot.historical_notices, None)?;
    for (name, notice) in &notices {
        let extension = Path::new(name)
            .extension()
            .unwrap_or_default()
            .to_string_lossy();
        if !name.starts_with("licenses/")
            || !matches!(
                extension.as_ref(),
                "" | "txt" | "md" | "rst" | "html" | "htm"
            )
        {
            return Err("Historical notices must be snapshotted textual license files.".into());
        }
        if let Some(original) = package.get(name)
            && !original.sha256.eq_ignore_ascii_case(&notice.sha256)
        {
            return Err("Historical notice changed while snapshotting.".into());
        }
    }
    let mut owned = package.clone();
    owned.extend(notices);
    let pending = current.join(PENDING_INVENTORY);
    file_tree::reject_linked_ancestors(&pending).map_err(|e| e.to_string())?;
    if fs::read(&pending).map_err(|e| e.to_string())?
        != fs::read(snapshot_path).map_err(|e| e.to_string())?
    {
        return Err("Pending upgrade inventory does not match the pre-write snapshot.".into());
    }
    let new = inventory(&current.join("files.json"), false)?;
    let retired: Inventory = if same {
        owned
            .iter()
            .filter(|(name, _)| !new.contains_key(*name))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    } else {
        Inventory::new()
    };
    let mut remaining = retired.clone();
    if same && backup.try_exists().map_err(|e| e.to_string())? {
        validate_archive_identity(&backup, snapshot_path, &package)?;
        for path in file_tree::files(&backup).map_err(|e| e.to_string())? {
            let name = key(&path);
            if name == ARCHIVE_IDENTITY || name == "files.json" {
                continue;
            }
            let original = retired
                .get(&name)
                .ok_or_else(|| format!("Unrecognized recovery archive file: {}", path.display()))?;
            if file_tree::hash(&backup.join(&path)).map_err(|e| e.to_string())?
                != original.sha256.to_ascii_lowercase()
            {
                return Err(format!(
                    "Recovery archive checksum mismatch: {}",
                    path.display()
                ));
            }
            if current
                .join(&original.path)
                .try_exists()
                .map_err(|e| e.to_string())?
            {
                return Err(
                    "Retired file exists in both installation and recovery archive.".into(),
                );
            }
            remaining.remove(&name);
        }
    } else if !same {
        let source_exists = previous.try_exists().map_err(|e| e.to_string())?;
        let archive_exists = backup.try_exists().map_err(|e| e.to_string())?;
        if source_exists == archive_exists {
            return Err(
                "Expected exactly one previous installation or its identified recovery archive."
                    .into(),
            );
        }
        let source = if source_exists {
            previous
        } else {
            backup.as_path()
        };
        if inventory(&source.join("files.json"), true)? != package {
            return Err("Previous package inventory changed during setup.".into());
        }
        verify_relocation_tree(source, snapshot_path, &owned, archive_exists)?;
    }
    verify_files(current, &new, &remaining, true)?;
    Ok(UpgradeState {
        backup,
        same,
        package,
        retired,
        remaining,
    })
}
fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())
}
/// Resume the exact persisted archive after interruption. Every old file must be
/// hash-verified in either its original location or this snapshot's archive.
pub fn finalize_upgrade(
    previous: &Path,
    current: &Path,
    snapshot: &Path,
    runtime_check: impl FnOnce() -> Result<(), String>,
) -> Result<Option<PathBuf>, String> {
    upgrade_state(previous, current, snapshot)?;
    runtime_check()?;
    let state = upgrade_state(previous, current, snapshot)?;
    if state.same {
        if !state.backup.try_exists().map_err(|e| e.to_string())? {
            // Publish complete archive metadata atomically before moving any payload.
            let stage = tempfile::Builder::new()
                .prefix(".companion-upgrade-")
                .tempdir_in(current.parent().ok_or("Installation has no parent.")?)
                .map_err(|e| e.to_string())?;
            write_synced(
                &stage.path().join(ARCHIVE_IDENTITY),
                &fs::read(snapshot).map_err(|e| e.to_string())?,
            )?;
            write_synced(
                &stage.path().join("files.json"),
                &serde_json::to_vec_pretty(&state.package.values().collect::<Vec<_>>())
                    .map_err(|e| e.to_string())?,
            )?;
            fs::rename(stage.path(), &state.backup).map_err(|e| e.to_string())?;
        }
        for entry in state.remaining.values() {
            let source = current.join(&entry.path);
            let target = state.backup.join(&entry.path);
            file_tree::reject_linked_ancestors(&source).map_err(|e| e.to_string())?;
            file_tree::reject_linked_ancestors(&target).map_err(|e| e.to_string())?;
            if file_tree::hash(&source).map_err(|e| e.to_string())?
                != entry.sha256.to_ascii_lowercase()
                || target.try_exists().map_err(|e| e.to_string())?
            {
                return Err(
                    "Retired payload changed before archiving; upgrade may be retried safely."
                        .into(),
                );
            }
            fs::create_dir_all(target.parent().ok_or("Archive file has no parent.")?)
                .map_err(|e| e.to_string())?;
            fs::rename(&source, &target).map_err(|e| {
                format!(
                    "Upgrade retained recoverable files at {}: {e}",
                    state.backup.display()
                )
            })?;
        }
    } else if previous.try_exists().map_err(|e| e.to_string())? {
        let receipt = previous.join(ARCHIVE_IDENTITY);
        if !receipt.try_exists().map_err(|e| e.to_string())? {
            use std::io::Write;
            let mut stage = tempfile::NamedTempFile::new_in(
                current.parent().ok_or("Installation has no parent.")?,
            )
            .map_err(|e| e.to_string())?;
            stage
                .write_all(&fs::read(snapshot).map_err(|e| e.to_string())?)
                .and_then(|()| stage.as_file().sync_all())
                .map_err(|e| e.to_string())?;
            stage
                .persist_noclobber(&receipt)
                .map_err(|e| e.to_string())?;
        }
        upgrade_state(previous, current, snapshot)?;
        fs::rename(previous, &state.backup)
            .map_err(|e| format!("Previous installation retained: {e}"))?;
    }
    let completed = upgrade_state(previous, current, snapshot)?;
    if completed.same && !completed.remaining.is_empty() {
        return Err("Recovery archive is incomplete; pending upgrade was retained.".into());
    }
    debug_assert!(completed.retired.len() >= completed.remaining.len());
    fs::remove_file(current.join(PENDING_INVENTORY)).map_err(|e| e.to_string())?;
    Ok(Some(state.backup))
}

/// Reserve the fresh report before calling an operation with machine side effects.
pub fn run_reported_operation<T>(
    path: &Path,
    operation: impl FnOnce() -> Result<T, String>,
    report: impl FnOnce(&Result<T, String>) -> serde_json::Value,
) -> Result<T, String> {
    let mut file = reserve_migration_report(path, &[])?;
    let result = operation();
    std::io::Write::write_all(
        &mut file,
        &serde_json::to_vec_pretty(&report(&result)).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    result
}

/// Reserve a fresh report outside all migrating trees before any mutation.
pub fn reserve_migration_report(path: &Path, protected: &[&Path]) -> Result<fs::File, String> {
    if !path.is_absolute() {
        return Err("Migration report path must be absolute.".into());
    }
    let parent = path.parent().ok_or("Migration report has no parent.")?;
    file_tree::reject_linked_ancestors(parent).map_err(|e| e.to_string())?;
    let resolved = parent
        .canonicalize()
        .map_err(|e| e.to_string())?
        .join(path.file_name().ok_or("Invalid report name.")?);
    for root in protected {
        if resolved.starts_with(crate::storage::resolve_new_path(root).map_err(|e| e.to_string())?)
        {
            return Err(
                "Migration report must be outside the application data and installation folders."
                    .into(),
            );
        }
    }
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    options
        .open(path)
        .map_err(|e| format!("A fresh migration report is required: {e}"))
}
