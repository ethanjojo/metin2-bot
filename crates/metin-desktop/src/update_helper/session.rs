//! Verified staging and typed parent/helper handoff. No installer runs during preparation.
use crate::{
    file_tree,
    updates::{UpdateCandidate, VerifiedInstaller},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Session {
    pub schema: u32,
    pub id: uuid::Uuid,
    pub parent_pid: u32,
    pub parent_created: u64,
    pub candidate: UpdateCandidate,
    pub data_dir: PathBuf,
    pub installation: PathBuf,
    pub bounds: Option<WindowBounds>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WindowBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub zoom: f32,
}
impl WindowBounds {
    pub(super) fn valid(&self) -> bool {
        [self.x, self.y, self.width, self.height, self.zoom]
            .iter()
            .all(|v| v.is_finite())
            && self.x.abs() <= 65536.
            && self.y.abs() <= 65536.
            && (400. ..=3840.).contains(&self.width)
            && (480. ..=2160.).contains(&self.height)
            && (0.75..=2.).contains(&self.zoom)
    }
}

pub struct PreparedUpdate {
    _verified: VerifiedInstaller,
    directory: PathBuf,
    child: Option<Child>,
    started: Option<Instant>,
    session: Option<Session>,
    _locks: Vec<File>,
    committed: bool,
    #[cfg(test)]
    fixture_ready: Option<bool>,
}

impl Drop for PreparedUpdate {
    fn drop(&mut self) {
        // Before process creation there is no second owner. Once launched, only
        // the successful-session cleanup policy may remove the helper runtime.
        if self.child.is_some() {
            if !self.committed
                && let Some(session) = &self.session
            {
                let _ = write_new(
                    &self.directory.join("abort.json"),
                    &Handoff {
                        schema: 1,
                        session: session.id,
                        message: None,
                    },
                );
            }
            return;
        }
        let Some(session) = &self.session else {
            return;
        };
        let Ok(resolved) = self.directory.canonicalize() else {
            return;
        };
        let Ok(temp) = std::env::temp_dir().canonicalize() else {
            return;
        };
        if resolved.parent() != Some(temp.as_path())
            || !resolved
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| {
                    s.starts_with(&format!("companion-update-session-{}-", session.id))
                })
        {
            return;
        }
        self._locks.clear();
        if file_tree::files(&resolved).is_ok() {
            let _ = fs::remove_dir_all(resolved);
        }
    }
}

impl PreparedUpdate {
    pub fn prepare(
        installer: VerifiedInstaller,
        candidate: &UpdateCandidate,
        data_dir: &Path,
    ) -> Result<Self, String> {
        cleanup_completed_sessions();
        let current = std::env::current_exe().map_err(|e| e.to_string())?;
        let installation = current
            .parent()
            .ok_or("The application directory is unavailable.")?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let mut source_locks = Vec::new();
        for relative in file_tree::files(&installation).map_err(|e| e.to_string())? {
            source_locks.push(locked_file(&installation.join(relative))?);
        }
        crate::installation::verify_installed_bundle(&installation)?;
        let trusted =
            file_tree::fingerprints_except(&installation, None).map_err(|e| e.to_string())?;
        file_tree::reject_linked_ancestors(data_dir).map_err(|e| e.to_string())?;
        let data_dir = data_dir.canonicalize().map_err(|e| e.to_string())?;
        let id = uuid::Uuid::new_v4();
        let directory = super::security::create(id)?;
        let root = directory.path();
        let bundle = root.join("runtime");
        let mut locks = vec![locked_directory(root)?];
        super::security::verify(root, true)?;
        fs::create_dir(&bundle).map_err(|e| e.to_string())?;
        locks.push(locked_directory(&bundle)?);
        for relative in file_tree::files(&installation).map_err(|e| e.to_string())? {
            let output = bundle.join(&relative);
            if let Some(parent) = output.parent() {
                create_locked_directories(&bundle, parent, &mut locks)?;
            }
            copy_new_locked(&installation.join(&relative), &output, &mut locks)?;
        }
        locks.extend(lock_tree_against(&bundle, &trusted)?);
        drop(source_locks);
        crate::installation::verify_installed_bundle(&bundle)?;
        let installer_path = root.join(candidate.asset_name());
        copy_new_locked(installer.installer_path(), &installer_path, &mut locks)?;
        if fs::metadata(&installer_path)
            .map_err(|e| e.to_string())?
            .len()
            != candidate.size()
            || file_tree::hash(&installer_path).map_err(|e| e.to_string())?
                != candidate.digest_hex()
        {
            return Err("The staged update did not match the verified download.".into());
        }
        let session = Session {
            schema: 1,
            id,
            parent_pid: std::process::id(),
            parent_created: super::process::creation_time(std::process::id())?,
            candidate: candidate.clone(),
            data_dir,
            installation,
            bounds: None,
        };
        write_new(&root.join("session.json"), &session)?;
        let mut token = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join("session-token.txt"))
            .map_err(|e| e.to_string())?;
        std::io::Write::write_all(&mut token, id.to_string().as_bytes())
            .map_err(|e| e.to_string())?;
        drop(token);
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join("installer-progress.jsonl"))
            .map_err(|e| e.to_string())?;
        // The helper owns this persistent session after spawn. It records completion;
        // its executable cannot be removed while Windows is still running it.
        let directory = directory.keep();
        Ok(Self {
            _verified: installer,
            directory,
            child: None,
            started: None,
            session: Some(session),
            _locks: locks,
            committed: false,
            #[cfg(test)]
            fixture_ready: None,
        })
    }
    pub fn set_window_bounds(&mut self, rect: eframe::egui::Rect, zoom: f32) -> Result<(), String> {
        #[cfg(test)]
        if self.fixture_ready.is_some() {
            return Ok(());
        }
        if self.child.is_some() {
            return Err("The update window is already open.".into());
        }
        let bounds = WindowBounds {
            x: rect.left(),
            y: rect.top(),
            width: rect.width(),
            height: rect.height(),
            zoom,
        };
        if !bounds.valid() {
            return Err("The update window geometry is invalid.".into());
        }
        let path = self.directory.join("session.json");
        let session = self
            .session
            .as_mut()
            .ok_or("Missing canonical update session.")?;
        session.bounds = Some(bounds);
        crate::storage::atomic_write(
            &path,
            &serde_json::to_vec(session).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }
    pub fn spawn(&mut self) -> Result<(), String> {
        #[cfg(test)]
        if self.fixture_ready.is_some() {
            return Ok(());
        }
        if self.child.is_some() {
            return Err("The update helper is already running.".into());
        }
        let canonical = self
            .session
            .as_ref()
            .ok_or("Missing canonical update session.")?;
        self._locks
            .push(locked_file(&self.directory.join("session.json"))?);
        self._locks
            .push(locked_file(&self.directory.join("session-token.txt"))?);
        if fs::read(self.directory.join("session.json")).map_err(|e| e.to_string())?
            != serde_json::to_vec(canonical).map_err(|e| e.to_string())?
            || fs::read(self.directory.join("session-token.txt")).map_err(|e| e.to_string())?
                != canonical.id.to_string().as_bytes()
        {
            return Err("The staged update session changed before launch.".into());
        }
        verify_secure_tree(&self.directory, true)?;
        let mut command = Command::new(self.directory.join("runtime/Companion.exe"));
        command.current_dir(self.directory.join("runtime"));
        command
            .arg("--update-session")
            .arg(self.directory.join("session.json"));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        self.child = Some(
            command
                .spawn()
                .map_err(|e| format!("The update window could not open: {e}"))?,
        );
        self.started = Some(Instant::now());
        Ok(())
    }
    pub fn commit(&mut self) -> Result<(), String> {
        #[cfg(test)]
        if self.fixture_ready.is_some() {
            self.committed = true;
            return Ok(());
        }
        if !self.poll()? {
            return Err("The update helper is not ready to install.".into());
        }
        let id = self
            .session
            .as_ref()
            .ok_or("Missing canonical update session.")?
            .id;
        write_new(
            &self.directory.join("commit.json"),
            &Handoff {
                schema: 1,
                session: id,
                message: None,
            },
        )?;
        self.committed = true;
        Ok(())
    }
    pub fn poll(&mut self) -> Result<bool, String> {
        #[cfg(test)]
        if let Some(ready) = self.fixture_ready {
            return Ok(ready);
        }
        let child = self
            .child
            .as_mut()
            .ok_or("The update helper has not started.")?;
        if let Some(exit) = child.try_wait().map_err(|e| e.to_string())? {
            return Err(format!("The update window closed before handoff ({exit})."));
        }
        if self
            .started
            .is_some_and(|start| start.elapsed() > Duration::from_secs(45))
        {
            return Err("Update verification timed out. Check your connection and retry.".into());
        }
        let id = self
            .session
            .as_ref()
            .ok_or("Missing canonical update session.")?
            .id;
        let error = self.directory.join("handoff-error.json");
        if error.exists() {
            let Ok(record) = read_json::<Handoff>(&error, 4096) else {
                return Ok(false);
            };
            if record.session == id {
                return Err(record
                    .message
                    .unwrap_or_else(|| "The update helper could not initialize.".into()));
            }
        }
        let ready = self.directory.join("ready.json");
        if ready.exists() {
            let Ok(record) = read_json::<Handoff>(&ready, 4096) else {
                return Ok(false);
            };
            if record.schema != 1 || record.session != id || record.message.is_some() {
                return Err("Invalid update handoff acknowledgement.".into());
            }
            return Ok(true);
        }
        if self
            .started
            .is_some_and(|start| start.elapsed() > Duration::from_secs(45))
        {
            return Err("Update verification timed out. Check your connection and retry.".into());
        }
        Ok(false)
    }
    #[cfg(test)]
    pub fn fixture(installer: VerifiedInstaller, ready: bool) -> Self {
        Self {
            _verified: installer,
            directory: PathBuf::new(),
            child: None,
            started: None,
            session: None,
            _locks: Vec::new(),
            committed: false,
            fixture_ready: Some(ready),
        }
    }
    #[cfg(test)]
    pub fn set_fixture_ready(&mut self, ready: bool) {
        self.fixture_ready = Some(ready);
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Handoff {
    pub schema: u32,
    pub session: uuid::Uuid,
    pub message: Option<String>,
}

pub(super) fn write_new<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    use std::io::Write;
    let mut file =
        tempfile::NamedTempFile::new_in(path.parent().ok_or("Missing update record directory.")?)
            .map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist_noclobber(path)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub(super) fn locked_directory(path: &Path) -> Result<File, String> {
    file_tree::reject_linked_ancestors(path).map_err(|e| e.to_string())?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;
        options
            .share_mode(3)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
        let file = options.open(path).map_err(|e| e.to_string())?;
        if file
            .metadata()
            .map_err(|e| e.to_string())?
            .file_attributes()
            & 0x400
            != 0
        {
            return Err("Update staging cannot use reparse points.".into());
        }
        Ok(file)
    }
    #[cfg(not(windows))]
    {
        options.open(path).map_err(|e| e.to_string())
    }
}
fn create_locked_directories(
    root: &Path,
    parent: &Path,
    locks: &mut Vec<File>,
) -> Result<(), String> {
    let mut path = root.to_path_buf();
    for part in parent
        .strip_prefix(root)
        .map_err(|_| "Staged files escaped their directory.")?
        .components()
    {
        if !matches!(part, std::path::Component::Normal(_)) {
            return Err("Invalid staged directory component.".into());
        }
        path.push(part);
        if !path.exists() {
            fs::create_dir(&path).map_err(|e| e.to_string())?;
        }
        locks.push(locked_directory(&path)?);
    }
    Ok(())
}
fn copy_new_locked(source: &Path, destination: &Path, locks: &mut Vec<File>) -> Result<(), String> {
    use std::io::Write;
    let mut source = File::open(source).map_err(|e| e.to_string())?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    let mut output = options.open(destination).map_err(|e| e.to_string())?;
    std::io::copy(&mut source, &mut output).map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())?;
    output.sync_all().map_err(|e| e.to_string())?;
    drop(output);
    locks.push(locked_file(destination)?);
    Ok(())
}
fn lock_tree_against(
    root: &Path,
    trusted: &std::collections::BTreeMap<PathBuf, String>,
) -> Result<Vec<File>, String> {
    let mut locks = Vec::new();
    for relative in file_tree::files(root).map_err(|e| e.to_string())? {
        locks.push(locked_file(&root.join(relative))?);
    }
    if file_tree::fingerprints_except(root, None).map_err(|e| e.to_string())? != *trusted {
        return Err("The staged runtime differs from the trusted installed files.".into());
    }
    Ok(locks)
}

pub(super) fn locked_file(path: &Path) -> Result<File, String> {
    file_tree::reject_linked_ancestors(path).map_err(|e| e.to_string())?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    options.open(path).map_err(|e| e.to_string())
}
pub(super) fn read_json<T: serde::de::DeserializeOwned>(
    path: &Path,
    limit: u64,
) -> Result<T, String> {
    let file = locked_file(path)?;
    if file.metadata().map_err(|e| e.to_string())?.len() > limit {
        return Err("Update metadata exceeded its size limit.".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Update metadata exceeded its size limit.".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "The update metadata is invalid.".into())
}

pub(super) fn load(path: &Path) -> Result<(Session, Vec<File>), String> {
    if !path.is_absolute() || path.file_name().and_then(|s| s.to_str()) != Some("session.json") {
        return Err("The update session path is invalid.".into());
    }
    let root = path.parent().ok_or("Missing update session directory.")?;
    if root.parent().and_then(|p| p.canonicalize().ok()) != std::env::temp_dir().canonicalize().ok()
    {
        return Err(
            "Update sessions must remain in the current user's temporary directory.".into(),
        );
    }
    let session_file = locked_file(path)?;
    let session: Session = read_json(path, 65536)?;
    if session.bounds.is_some_and(|b| !b.valid()) {
        return Err("Invalid update window geometry.".into());
    }
    if session.schema != 1
        || !root
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.starts_with(&format!("companion-update-session-{}-", session.id)))
    {
        return Err("The update session identity is invalid.".into());
    }
    file_tree::reject_linked_ancestors(&session.data_dir).map_err(|e| e.to_string())?;
    file_tree::reject_linked_ancestors(&session.installation).map_err(|e| e.to_string())?;
    if !session.data_dir.is_absolute()
        || !session.data_dir.is_dir()
        || !session.installation.is_absolute()
        || !session.installation.is_dir()
    {
        return Err("The update installation or profile directory is unavailable.".into());
    }
    let current = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let expected = root
        .join("runtime/Companion.exe")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if current != expected {
        return Err("The update helper is not running from its staged runtime.".into());
    }
    crate::installation::verify_installed_bundle(&root.join("runtime"))?;
    verify_secure_tree(root, true)?;
    let mut guards = vec![
        locked_directory(root)?,
        session_file,
        locked_file(&root.join("session-token.txt"))?,
    ];
    for relative in file_tree::files(&root.join("runtime")).map_err(|e| e.to_string())? {
        guards.push(locked_file(&root.join("runtime").join(relative))?);
    }
    crate::updates::reconfirm_candidate(&session.candidate).map_err(|e| {
        format!("Could not recheck the official update. Check your connection and retry: {e}")
    })?;
    let installer = root.join(session.candidate.asset_name());
    if installer.parent() != Some(root) {
        return Err("Invalid update asset filename.".into());
    }
    guards.push(locked_file(&installer)?);
    if fs::metadata(&installer).map_err(|e| e.to_string())?.len() != session.candidate.size()
        || file_tree::hash(&installer).map_err(|e| e.to_string())? != session.candidate.digest_hex()
    {
        return Err("The update installer failed independent verification.".into());
    }
    Ok((session, guards))
}

fn verify_secure_tree(root: &Path, is_root: bool) -> Result<(), String> {
    super::security::verify(root, is_root)?;
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        file_tree::reject_link(&path).map_err(|e| e.to_string())?;
        if path.is_dir() {
            verify_secure_tree(&path, false)?;
        } else {
            super::security::verify(&path, false)?;
        }
    }
    Ok(())
}

pub(super) fn require_commit(root: &Path, id: uuid::Uuid) -> Result<(), String> {
    if root.join("abort.json").exists() {
        return Err("The original Companion cancelled this update. Nothing was installed.".into());
    }
    let record: Handoff = read_json(&root.join("commit.json"), 4096).map_err(
        |_| "The original Companion did not confirm the update handoff. Nothing was installed.",
    )?;
    if record.schema != 1 || record.session != id || record.message.is_some() {
        return Err("The update handoff confirmation is invalid.".into());
    }
    Ok(())
}

/// Completed helper runtimes cannot delete their own executable on Windows.
/// Remove only verified owned, inactive successful sessions after 24 hours;
/// failed sessions and their backups remain available for troubleshooting.
fn cleanup_completed_sessions() {
    let Ok(entries) = fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with("companion-update-session-")
        {
            continue;
        }
        let _ = cleanup_completed(&path);
    }
}
fn cleanup_completed(root: &Path) -> Result<(), String> {
    let record: Handoff = read_json(&root.join("complete.json"), 4096)?;
    if record.schema != 1
        || record.message.is_some()
        || !root.file_name().and_then(|s| s.to_str()).is_some_and(|s| {
            s.starts_with(&format!("companion-update-session-{}-", record.session))
        })
    {
        return Err("A foreign update session was retained.".into());
    }
    let temp = std::env::temp_dir()
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let resolved = root.canonicalize().map_err(|e| e.to_string())?;
    if resolved.parent() != Some(temp.as_path()) {
        return Err("Update cleanup cannot leave the temporary directory.".into());
    }
    file_tree::files(&resolved).map_err(|e| e.to_string())?;
    let modified = fs::metadata(resolved.join("complete.json"))
        .and_then(|m| m.modified())
        .map_err(|e| e.to_string())?;
    if modified.elapsed().map_err(|e| e.to_string())? < Duration::from_secs(86400) {
        return Ok(());
    }
    let mut options = OpenOptions::new();
    options.write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    let inactive = options
        .open(resolved.join("runtime/Companion.exe"))
        .map_err(|_| "The previous helper is still in use.")?;
    drop(inactive);
    fs::remove_dir_all(&resolved).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abandoned_or_cancelled_handoff_never_authorizes_installation() {
        let directory = tempfile::tempdir().unwrap();
        let id = uuid::Uuid::new_v4();
        assert!(require_commit(directory.path(), id).is_err());
        write_new(
            &directory.path().join("commit.json"),
            &Handoff {
                schema: 1,
                session: id,
                message: None,
            },
        )
        .unwrap();
        assert!(require_commit(directory.path(), id).is_ok());
        assert!(require_commit(directory.path(), uuid::Uuid::new_v4()).is_err());
        write_new(
            &directory.path().join("abort.json"),
            &Handoff {
                schema: 1,
                session: id,
                message: None,
            },
        )
        .unwrap();
        assert!(
            require_commit(directory.path(), id).is_err(),
            "Cancellation wins even if a late commit exists"
        );
    }

    #[test]
    fn staged_runtime_uses_original_trusted_hashes_and_keeps_execution_files_locked() {
        let directory = tempfile::tempdir().unwrap();
        let exe = directory.path().join("Companion.exe");
        let manifest = directory.path().join("files.json");
        fs::write(&exe, b"trusted original executable").unwrap();
        fs::write(&manifest, b"trusted original inventory").unwrap();
        let trusted = file_tree::fingerprints_except(directory.path(), None).unwrap();
        fs::write(&exe, b"replaced executable").unwrap();
        fs::write(&manifest, b"attacker adjusted inventory").unwrap();
        assert!(lock_tree_against(directory.path(), &trusted).is_err());
        fs::write(&exe, b"trusted original executable").unwrap();
        fs::write(&manifest, b"trusted original inventory").unwrap();
        let locks = lock_tree_against(directory.path(), &trusted).unwrap();
        #[cfg(windows)]
        {
            assert!(OpenOptions::new().write(true).open(&exe).is_err());
            assert!(fs::remove_file(&exe).is_err());
        }
        drop(locks);
        assert!(OpenOptions::new().write(true).open(&exe).is_ok());
    }

    #[test]
    fn stage_copy_never_overwrites_an_existing_target_and_directory_is_locked() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input");
        let output = directory.path().join("output");
        fs::write(&input, "new").unwrap();
        fs::write(&output, "keep").unwrap();
        let mut locks = Vec::new();
        assert!(copy_new_locked(&input, &output, &mut locks).is_err());
        assert_eq!(fs::read_to_string(&output).unwrap(), "keep");
        let root = directory.path().join("staging");
        fs::create_dir(&root).unwrap();
        let guard = locked_directory(&root).unwrap();
        #[cfg(windows)]
        assert!(fs::rename(&root, directory.path().join("moved")).is_err());
        drop(guard);
        fs::rename(&root, directory.path().join("moved")).unwrap();
    }

    #[test]
    fn handoff_markers_are_published_complete_and_cannot_be_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ready.json");
        let copy = path.clone();
        let id = uuid::Uuid::new_v4();
        let reader = std::thread::spawn(move || {
            loop {
                match fs::read(&copy) {
                    Ok(bytes) => {
                        let record: Handoff = serde_json::from_slice(&bytes)
                            .expect("A published marker is complete JSON");
                        assert_eq!(record.session, id);
                        return;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        std::thread::yield_now()
                    }
                    Err(error) => panic!("{error}"),
                }
            }
        });
        write_new(
            &path,
            &Handoff {
                schema: 1,
                session: id,
                message: Some("x".repeat(2048)),
            },
        )
        .unwrap();
        reader.join().unwrap();
        let original = fs::read(&path).unwrap();
        assert!(
            write_new(
                &path,
                &Handoff {
                    schema: 1,
                    session: uuid::Uuid::new_v4(),
                    message: None
                }
            )
            .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    #[test]
    fn geometry_rejects_nonfinite_and_extreme_or_tiny_windows() {
        let valid = WindowBounds {
            x: -1920.,
            y: 40.,
            width: 460.,
            height: 620.,
            zoom: 1.5,
        };
        assert!(valid.valid());
        for invalid in [
            WindowBounds {
                x: f32::NAN,
                ..valid
            },
            WindowBounds { zoom: 0., ..valid },
            WindowBounds { width: 1., ..valid },
            WindowBounds {
                height: 99999.,
                ..valid
            },
        ] {
            assert!(!invalid.valid());
        }
    }
    #[test]
    fn metadata_read_is_bounded_and_rejects_unexpected_fields() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("record.json");
        fs::write(&file, vec![b' '; 4097]).unwrap();
        assert!(read_json::<Handoff>(&file, 4096).is_err());
        fs::write(&file,serde_json::to_vec(&serde_json::json!({"schema":1,"session":uuid::Uuid::new_v4(),"message":null,"extra":"no"})).unwrap()).unwrap();
        assert!(read_json::<Handoff>(&file, 4096).is_err());
    }
    #[test]
    fn cleanup_never_deletes_incomplete_foreign_or_recent_sessions() {
        let id = uuid::Uuid::new_v4();
        let directory = tempfile::Builder::new()
            .prefix(&format!("companion-update-session-{id}-"))
            .tempdir()
            .unwrap();
        let sentinel = directory.path().join("keep.txt");
        fs::write(&sentinel, "keep").unwrap();
        assert!(cleanup_completed(directory.path()).is_err());
        assert!(sentinel.exists());
        write_new(
            &directory.path().join("complete.json"),
            &Handoff {
                schema: 1,
                session: id,
                message: None,
            },
        )
        .unwrap();
        assert!(cleanup_completed(directory.path()).is_ok());
        assert!(sentinel.exists());
        let foreign = tempfile::tempdir().unwrap();
        write_new(
            &foreign.path().join("complete.json"),
            &Handoff {
                schema: 1,
                session: id,
                message: None,
            },
        )
        .unwrap();
        assert!(cleanup_completed(foreign.path()).is_err());
        assert!(foreign.path().exists());
    }
    #[test]
    fn cleanup_removes_only_old_completed_owned_inactive_session() {
        let id = uuid::Uuid::new_v4();
        let directory = tempfile::Builder::new()
            .prefix(&format!("companion-update-session-{id}-"))
            .tempdir()
            .unwrap();
        fs::create_dir(directory.path().join("runtime")).unwrap();
        fs::write(
            directory.path().join("runtime/Companion.exe"),
            "nonexecutable fixture",
        )
        .unwrap();
        let completed = directory.path().join("complete.json");
        write_new(
            &completed,
            &Handoff {
                schema: 1,
                session: id,
                message: None,
            },
        )
        .unwrap();
        File::options()
            .write(true)
            .open(&completed)
            .unwrap()
            .set_modified(std::time::SystemTime::now() - Duration::from_secs(86401))
            .unwrap();
        let path = directory.keep();
        cleanup_completed(&path).unwrap();
        assert!(!path.exists());
    }
}
