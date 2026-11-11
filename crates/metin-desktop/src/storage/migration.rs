use super::*;

#[derive(Debug, Serialize)]
pub struct DataDirectory {
    pub root: PathBuf,
    pub backup: Option<PathBuf>,
}

pub fn prepare_data_directory(requested: Option<&Path>) -> Result<DataDirectory> {
    if let Some(root) = requested
        && !is_default_name(root)
    {
        return Ok(DataDirectory {
            root: root.to_path_buf(),
            backup: None,
        });
    }
    prepare_data_directory_at(&data_parent_directory()?, requested)
}

fn is_default_name(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("Companion") || n.eq_ignore_ascii_case("MetinBot"))
}

pub fn migration_protected_paths(requested: Option<&Path>) -> Result<Vec<PathBuf>> {
    if let Some(root) = requested
        && !is_default_name(root)
    {
        return Ok(vec![root.to_path_buf()]);
    }
    let base = data_parent_directory()?;
    if let Some(root) = requested
        && resolve_new_path(root.parent().unwrap_or(Path::new(".")))? != resolve_new_path(&base)?
    {
        return Ok(vec![root.to_path_buf()]);
    }
    let mut paths = vec![base.join("Companion"), base.join("MetinBot")];
    if let Some(root) = requested {
        paths.push(root.to_path_buf());
    }
    Ok(paths)
}

/// Migrate only the historical default; explicitly selected custom stores stay put.
pub fn prepare_data_directory_at(base: &Path, requested: Option<&Path>) -> Result<DataDirectory> {
    let old = base.join("MetinBot");
    let new = base.join("Companion");
    let root = requested.unwrap_or(&new).to_path_buf();
    if !is_default_name(&root)
        || resolve_new_path(root.parent().unwrap_or(Path::new(".")))? != resolve_new_path(base)?
    {
        return Ok(DataDirectory { root, backup: None });
    }
    let resolved = resolve_new_path(&root)?;
    if resolved != resolve_new_path(&old)? && resolved != resolve_new_path(&new)? {
        return Ok(DataDirectory { root, backup: None });
    }
    if !present(&old)? {
        return Ok(DataDirectory {
            root: new,
            backup: None,
        });
    }
    if present(&new)? {
        reject_link(&new)?;
        if !new.is_dir() || fs::read_dir(&new)?.next().is_some() {
            return Err(invalid(
                "Both the previous and Companion data folders exist. No files were merged or overwritten. Back up both folders and resolve the conflict before starting Companion.",
            ));
        }
    }
    validate_data_directory(&old)?;
    let lock_parent = if old.join("profiles").is_dir() {
        old.join("profiles")
    } else {
        old.clone()
    };
    let lock_path = lock_parent.join(".store.lock");
    if present(&lock_path)? {
        reject_link(&lock_path)?;
    }
    let mut lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)?;
    lock.try_lock_exclusive()
        .map_err(|_| invalid("Close the previous application before migrating its data."))?;
    let mut lock_bytes = Vec::new();
    std::io::Read::read_to_end(&mut lock, &mut lock_bytes)?;
    // Windows byte-range locks also block reads/copies through another handle.
    // Preserve the lock file's bytes separately while holding its original handle.
    let before = crate::file_tree::fingerprints_except(&old, Some(&lock_path))?;
    let backup_stage = tempfile::Builder::new()
        .prefix("Companion-data-backup-")
        .tempdir_in(base)?;
    let backup_data = backup_stage.path().join("data");
    copy_tree_except(&old, &backup_data, Some(&lock_path))?;
    let backup_lock = backup_data.join(lock_path.strip_prefix(&old).expect("lock inside old data"));
    fs::write(&backup_lock, lock_bytes)?;
    if before != crate::file_tree::fingerprints_except(&backup_data, Some(&backup_lock))?
        || before != crate::file_tree::fingerprints_except(&old, Some(&lock_path))?
    {
        return Err(invalid(
            "Data changed during migration. The original folder was left in place.",
        ));
    }
    let backup = backup_stage.keep().join("data");
    if present(&new)? {
        fs::remove_dir(&new)?;
    }
    // Windows refuses to rename a directory with an open child handle. The old
    // writer lock protects backup/verification; close it immediately before the
    // atomic rename. A newly opened old writer makes that rename fail safely.
    drop(lock);
    fs::rename(&old, &new).map_err(|error| invalid(&format!("Could not move application data: {error}. Original files and backup at {} were retained.", backup.display())))?;
    Ok(DataDirectory {
        root: new,
        backup: Some(backup),
    })
}
