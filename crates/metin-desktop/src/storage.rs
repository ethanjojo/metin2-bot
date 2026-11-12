//! Canonical storage. New assets commit before JSON, obsolete assets after JSON.
use fs2::FileExt;
use metin_core::Profile;
use metin_platform::Frame;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Profile(#[from] metin_core::ValidationError),
    #[error("{0}")]
    Vision(#[from] metin_vision::VisionError),
    #[error("{0}")]
    Invalid(String),
}
pub type Result<T> = std::result::Result<T, StorageError>;
mod migration;
pub use migration::{
    DataDirectory, migration_protected_paths, prepare_data_directory, prepare_data_directory_at,
};
pub const DATA_ENTRIES: [&str; 4] = [
    "profiles",
    "ui-settings.json",
    "config.json",
    "config.queue-backup.json",
];
fn invalid(text: &str) -> StorageError {
    StorageError::Invalid(text.into())
}
fn present(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}
pub(crate) fn resolve_new_path(path: &Path) -> Result<PathBuf> {
    if present(path)? {
        return Ok(path.canonicalize()?);
    }
    let name = path
        .file_name()
        .ok_or_else(|| invalid("Invalid data path."))?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    Ok(resolve_new_path(parent)?.join(name))
}
fn read_legacy_object(path: &Path) -> Result<serde_json::Map<String, serde_json::Value>> {
    reject_link(path)?;
    match serde_json::from_slice(&fs::read(path)?)? {
        serde_json::Value::Object(object) => Ok(object),
        _ => Err(invalid("Legacy configuration must be a JSON object.")),
    }
}
pub fn validate_legacy_files(root: &Path) -> Result<()> {
    if present(&root.join("config.json"))? && present(&root.join("config.queue-backup.json"))? {
        return Err(invalid("Legacy configuration and backup both exist."));
    }
    for name in ["config.json", "config.queue-backup.json"] {
        let path = root.join(name);
        if present(&path)? {
            read_legacy_object(&path)?;
        }
    }
    Ok(())
}
fn reject_link(path: &Path) -> Result<()> {
    let m = fs::symlink_metadata(path)?;
    if m.file_type().is_symlink() {
        return Err(invalid("Application data cannot contain symbolic links."));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 != 0 {
            return Err(invalid("Application data cannot contain reparse points."));
        }
    }
    Ok(())
}
fn reject_tree_links(path: &Path) -> Result<()> {
    reject_link(path)?;
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            reject_tree_links(&entry?.path())?;
        }
    }
    Ok(())
}
/// Read-only validation happens before migration, logs, or writer-lock creation.
pub fn validate_data_directory(root: &Path) -> Result<()> {
    if !present(root)? {
        return Ok(());
    }
    reject_link(root)?;
    if !root.is_dir() {
        return Err(invalid("Application data must be a directory."));
    }
    validate_legacy_files(root)?;
    UISettings::load(root)?;
    let profiles = root.join("profiles");
    if present(&profiles)? {
        reject_tree_links(&profiles)?;
        let profiles = profiles.canonicalize()?;
        list_profiles(&profiles)?;
        read_selection(&profiles)?;
    }
    Ok(())
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("File has no parent directory."))?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    atomic_write(path, &bytes)
}
fn validate_icons(profile: &Profile) -> Result<()> {
    for reference in profile
        .safety
        .rules
        .iter()
        .filter_map(|r| r.reference.as_ref())
    {
        metin_vision::validate_template(&metin_vision::decode_png(&reference.png_bytes()?)?)?;
    }
    for skill in &profile.buffs.skills {
        for icon in [&skill.icon, &skill.inactive_icon].into_iter().flatten() {
            metin_vision::validate_template(&metin_vision::decode_png(&icon.png_bytes()?)?)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "StoredUISettings")]
pub struct UISettings {
    pub ui_scale: f32,
    pub preview_refresh_ms: u32,
    pub lite_mode: bool,
    pub pin_to_game: bool,
    pub check_updates_on_start: bool,
    pub update_channel: crate::updates::UpdateChannel,
}
/// Consume obsolete persisted hints only when loading existing user data.
/// Serialization has one source of truth and never writes these hints again.
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct StoredUISettings {
    #[serde(rename = "theme")]
    _obsolete_theme: Option<serde::de::IgnoredAny>,
    #[serde(rename = "last_mounted")]
    _obsolete_mount_hint: Option<bool>,
    #[serde(rename = "profile_windows")]
    _obsolete_profile_windows: Option<serde::de::IgnoredAny>,
    ui_scale: f32,
    preview_refresh_ms: u32,
    lite_mode: bool,
    pin_to_game: bool,
    check_updates_on_start: bool,
    update_channel: crate::updates::UpdateChannel,
}
impl Default for StoredUISettings {
    fn default() -> Self {
        let settings = UISettings::default();
        Self {
            _obsolete_theme: None,
            _obsolete_mount_hint: None,
            _obsolete_profile_windows: None,
            ui_scale: settings.ui_scale,
            preview_refresh_ms: settings.preview_refresh_ms,
            lite_mode: settings.lite_mode,
            pin_to_game: settings.pin_to_game,
            check_updates_on_start: settings.check_updates_on_start,
            update_channel: settings.update_channel,
        }
    }
}
impl From<StoredUISettings> for UISettings {
    fn from(s: StoredUISettings) -> Self {
        Self {
            ui_scale: s.ui_scale,
            preview_refresh_ms: s.preview_refresh_ms,
            lite_mode: s.lite_mode,
            pin_to_game: s.pin_to_game,
            check_updates_on_start: s.check_updates_on_start,
            update_channel: s.update_channel,
        }
    }
}
impl Default for UISettings {
    fn default() -> Self {
        Self {
            ui_scale: 1.,
            preview_refresh_ms: 200,
            lite_mode: false,
            pin_to_game: true,
            check_updates_on_start: true,
            update_channel: crate::updates::UpdateChannel::Stable,
        }
    }
}
impl UISettings {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join("ui-settings.json");
        if !present(&path)? {
            return Ok(Self::default());
        }
        reject_link(&path)?;
        let s: Self = serde_json::from_slice(&fs::read(path)?)?;
        s.validate()?;
        Ok(s)
    }
    fn validate(&self) -> Result<()> {
        if !self.ui_scale.is_finite()
            || !(0.75..=1.5).contains(&self.ui_scale)
            || !(100..=5000).contains(&self.preview_refresh_ms)
        {
            return Err(invalid("Invalid UI scale or preview refresh interval."));
        }
        Ok(())
    }
    pub fn save(&self, root: &Path) -> Result<()> {
        self.validate()?;
        atomic_json(&root.join("ui-settings.json"), self)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    profile_id: String,
}
fn profile_folder(root: &Path, id: Uuid) -> Result<PathBuf> {
    let p = root.join(id.to_string());
    if present(&p)? {
        reject_link(&p)?;
        if !p.canonicalize()?.starts_with(root) {
            return Err(invalid("Profile is outside the store."));
        }
    }
    Ok(p)
}
fn read_profile(root: &Path, id: Uuid) -> Result<(Profile, Option<Frame>)> {
    let folder = profile_folder(root, id)?;
    let json = folder.join("profile.json");
    reject_link(&json)?;
    let p = Profile::from_json(&fs::read_to_string(json)?)?;
    if p.id != id {
        return Err(invalid("Profile identity does not match its folder."));
    }
    validate_icons(&p)?;
    let template = if let Some(name) = &p.template_file {
        let asset = folder.join(name);
        reject_link(&asset)?;
        let image = metin_vision::load_image(&asset)?;
        metin_vision::validate_template(&image)?;
        Some(image)
    } else {
        None
    };
    Ok((p, template))
}
fn list_profiles(root: &Path) -> Result<Vec<Profile>> {
    let mut profiles = vec![];
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        reject_link(&path)?;
        if path.is_dir() {
            let id = path
                .file_name()
                .and_then(|s| s.to_str())
                .and_then(|s| metin_core::validation::canonical_uuid(s).ok())
                .ok_or_else(|| invalid("Invalid profile directory name."))?;
            profiles.push(read_profile(root, id)?.0);
        }
    }
    let mapper = icu_casemap::CaseMapper::new();
    profiles.sort_by(|a, b| {
        mapper
            .fold_string(&a.name)
            .cmp(&mapper.fold_string(&b.name))
            .then(a.id.cmp(&b.id))
    });
    Ok(profiles)
}
fn read_selection(root: &Path) -> Result<Option<Uuid>> {
    let path = root.join("selection.json");
    if !present(&path)? {
        return Ok(None);
    }
    reject_link(&path)?;
    let selected: Selection = serde_json::from_slice(&fs::read(path)?)?;
    let id = metin_core::validation::canonical_uuid(&selected.profile_id)?;
    read_profile(root, id)?;
    Ok(Some(id))
}
pub struct ProfileStore {
    root: PathBuf,
    lock: File,
}
/// A committed profile save; obsolete asset cleanup cannot undo the commit.
#[derive(Debug)]
pub struct SaveOutcome {
    pub profile: Profile,
    pub cleanup_warning: Option<String>,
}
impl ProfileStore {
    pub fn open(root: &Path) -> Result<Self> {
        fs::create_dir_all(root)?;
        reject_link(root)?;
        let path = root.join(".store.lock");
        if present(&path)? {
            reject_link(&path)?;
        }
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        lock.try_lock_exclusive().map_err(|_| {
            invalid(&format!(
                "Another {} instance is using this profile store.",
                crate::PRODUCT_NAME
            ))
        })?;
        Ok(Self {
            root: root.canonicalize()?,
            lock,
        })
    }
    fn folder(&self, id: Uuid) -> Result<PathBuf> {
        profile_folder(&self.root, id)
    }
    pub fn load(&self, id: Uuid) -> Result<(Profile, Option<Frame>)> {
        read_profile(&self.root, id)
    }
    pub fn list(&self) -> Result<Vec<Profile>> {
        list_profiles(&self.root)
    }
    pub fn save(&mut self, profile: &Profile, template: Option<&Frame>) -> Result<SaveOutcome> {
        profile.validate()?;
        validate_icons(profile)?;
        let destination = self.folder(profile.id)?;
        let staging = if present(&destination)? {
            None
        } else {
            Some(
                tempfile::Builder::new()
                    .prefix(".Companion-profile-")
                    .tempdir_in(
                        self.root
                            .parent()
                            .ok_or_else(|| invalid("Profile store has no parent."))?,
                    )?,
            )
        };
        let folder = staging.as_ref().map_or(destination.as_path(), |s| s.path());
        let old = if present(&folder.join("profile.json"))? {
            self.load(profile.id)?.0.template_file
        } else {
            None
        };
        let mut profile = profile.clone();
        let mut created = None;
        if let Some(image) = template {
            metin_vision::validate_template(image)?;
            let name = format!("{}.png", Uuid::new_v4());
            let path = folder.join(&name);
            atomic_write(&path, &metin_vision::encode_png(image)?)?;
            profile.template_file = Some(name);
            created = Some(path);
        } else if let Some(name) = &profile.template_file {
            let path = folder.join(name);
            reject_link(&path)?;
            metin_vision::validate_template(&metin_vision::load_image(&path)?)?;
        }
        if let Err(error) = atomic_json(&folder.join("profile.json"), &profile) {
            if let Some(path) = created {
                let _ = fs::remove_file(path);
            }
            return Err(error);
        }
        let obsolete = old
            .filter(|name| Some(name) != profile.template_file.as_ref())
            .map(|name| folder.join(name));
        if let Some(staging) = staging {
            fs::rename(staging.path(), destination)?;
        }
        let cleanup_warning = obsolete.and_then(|path| {
            reject_link(&path)
                .and_then(|()| fs::remove_file(&path).map_err(Into::into))
                .err()
                .map(|error| {
                    format!(
                        "Profile saved, but obsolete template cleanup failed for {}: {error}",
                        path.display()
                    )
                })
        });
        Ok(SaveOutcome {
            profile,
            cleanup_warning,
        })
    }
    pub fn select(&self, id: Uuid) -> Result<()> {
        self.load(id)?;
        atomic_json(
            &self.root.join("selection.json"),
            &Selection {
                profile_id: id.to_string(),
            },
        )
    }
    pub fn selected(&self) -> Result<Option<Uuid>> {
        read_selection(&self.root)
    }
    pub fn migrate_legacy(&mut self, path: &Path) -> Result<Option<Profile>> {
        self.migrate_legacy_with_identity(path, path)
    }
    fn migrate_legacy_with_identity(
        &mut self,
        path: &Path,
        identity: &Path,
    ) -> Result<Option<Profile>> {
        if !present(path)? {
            return Ok(None);
        }
        reject_link(path)?;
        let backup = path.with_file_name("config.queue-backup.json");
        if present(&backup)? {
            return Err(invalid("Legacy configuration and backup both exist."));
        }
        let object = read_legacy_object(path)?;
        let resolved = identity.canonicalize()?;
        let identity = resolved.to_string_lossy();
        // Stable imported identities use normal DOS/UNC paths without extended prefixes.
        #[cfg(windows)]
        let identity = if let Some(unc) = identity.strip_prefix("\\\\?\\UNC\\") {
            format!("\\\\{unc}")
        } else {
            identity
                .strip_prefix("\\\\?\\")
                .unwrap_or(&identity)
                .to_owned()
        };
        let id = Uuid::new_v5(&Uuid::NAMESPACE_URL, identity.as_bytes());
        let profile = if present(&self.folder(id)?.join("profile.json"))? {
            self.load(id)?.0
        } else {
            let mut p = Profile::create("Imported profile");
            p.id = id;
            if let Some(region) = object.get("region").filter(|r| !r.is_null()) {
                p.region = Some(serde_json::from_value(region.clone())?);
            }
            self.save(&p, None)?.profile
        };
        self.select(id)?;
        fs::rename(path, backup)?;
        Ok(Some(profile))
    }
}
impl Drop for ProfileStore {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.lock);
    }
}

fn data_parent_directory() -> Result<PathBuf> {
    #[cfg(windows)]
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| invalid("LOCALAPPDATA must be an absolute user-data directory."))?;
    #[cfg(target_os = "macos")]
    let root = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| invalid("HOME must be absolute."))?
        .join("Library/Application Support");
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let root = if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
        let path = PathBuf::from(data);
        if !path.is_absolute() {
            return Err(invalid("XDG_DATA_HOME must be absolute."));
        }
        path
    } else {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or_else(|| invalid("HOME must be absolute."))?
            .join(".local/share")
    };
    Ok(root)
}
pub fn default_data_directory() -> Result<PathBuf> {
    Ok(data_parent_directory()?.join("Companion"))
}
pub fn legacy_data_directory() -> Result<PathBuf> {
    if cfg!(debug_assertions) {
        Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()?)
    } else {
        std::env::current_exe()?
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| invalid("Executable has no parent directory."))
    }
}
pub fn has_legacy_data(root: &Path) -> bool {
    DATA_ENTRIES
        .iter()
        .any(|name| fs::symlink_metadata(root.join(name)).is_ok())
}
fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    copy_tree_except(source, destination, None)
}
fn copy_tree_except(source: &Path, destination: &Path, excluded: Option<&Path>) -> Result<()> {
    reject_link(source)?;
    if Some(source) == excluded {
        return Ok(());
    }
    if source.is_dir() {
        fs::create_dir(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_tree_except(
                &entry.path(),
                &destination.join(entry.file_name()),
                excluded,
            )?;
        }
    } else {
        fs::copy(source, destination)?;
    }
    Ok(())
}
pub fn import_data(source: &Path, destination: &Path) -> Result<()> {
    reject_link(source)?;
    validate_data_directory(source)?;
    if present(destination)? {
        reject_link(destination)?;
    }
    if resolve_new_path(destination)?.starts_with(source.canonicalize()?) {
        return Err(invalid(
            "Import destination cannot be inside the source folder.",
        ));
    }
    if present(destination)? {
        reject_link(destination)?;
        if fs::read_dir(destination)?.next().is_some() {
            return Err(invalid(
                "Import requires an empty destination; existing profiles are never overwritten.",
            ));
        }
    }
    let parent = destination
        .parent()
        .ok_or_else(|| invalid("Import destination has no parent."))?;
    fs::create_dir_all(parent)?;
    let stage = tempfile::Builder::new()
        .prefix(".Companion-import-")
        .tempdir_in(parent)?;
    let mut copied = false;
    for name in DATA_ENTRIES {
        let path = source.join(name);
        if present(&path)? {
            copy_tree(&path, &stage.path().join(name))?;
            copied = true;
        }
    }
    if !copied {
        return Err(invalid(&format!(
            "The import folder contains no {} profiles or settings.",
            crate::PRODUCT_NAME
        )));
    }
    validate_data_directory(stage.path())?;
    {
        let mut store = ProfileStore::open(&stage.path().join("profiles"))?;
        store.list()?;
        store.selected()?;
        store.migrate_legacy_with_identity(
            &stage.path().join("config.json"),
            &source.join("config.json"),
        )?;
    }
    if present(destination)? {
        fs::remove_dir(destination)?;
    }
    let staged = stage.keep();
    if let Err(error) = fs::rename(&staged, destination) {
        let _ = fs::remove_dir_all(&staged);
        return Err(error.into());
    }
    Ok(())
}
