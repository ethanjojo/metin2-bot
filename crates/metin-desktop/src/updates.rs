//! Fixed-source, explicitly confirmed Companion updates. No executable is launched here.
//!
//! Network and file verification run on one bounded worker. A ready artifact retains
//! a Windows file handle denying write/delete sharing until the caller launches it.

use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::{Duration, Instant},
};

const RELEASES_API: &str =
    "https://api.github.com/repos/ethanjojo/metin2-bot/releases?per_page=100";
const RELEASE_ROOT: &str = "https://github.com/ethanjojo/metin2-bot/releases";
const MAX_METADATA: u64 = 4 * 1024 * 1024;
const MAX_INSTALLER: u64 = 512 * 1024 * 1024;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Preview,
}

/// Neutral screenshots/tests only. The fixture installer contains plain text,
/// and callers must disable actual installation when running UI smoke mode.
#[cfg(any(test, feature = "ui-smoke"))]
#[derive(Clone, Copy, Debug)]
pub enum UpdateFixture {
    Idle,
    Available,
    Downloading,
    Ready,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    #[error("The installed application version is invalid.")]
    InvalidCurrentVersion,
    #[error("GitHub returned invalid or oversized release metadata.")]
    InvalidMetadata,
    #[error("The update request failed. Check your connection and try again.")]
    Network,
    #[error("GitHub returned HTTP {0}. Try again later.")]
    HttpStatus(u16),
    #[error("The update download redirect was not an approved GitHub asset URL.")]
    UnsafeRedirect,
    #[error("The downloaded installer did not match its published size and SHA256.")]
    Integrity,
    #[error("The private installer file could not be written or locked.")]
    FileAccess,
    #[error("The update operation was cancelled.")]
    Cancelled,
    #[error("The previous update operation is still stopping. Try again shortly.")]
    Busy,
    #[error("No verified installer is ready.")]
    NotReady,
    #[error("The update worker is unavailable. Restart Companion and try again.")]
    WorkerUnavailable,
}

#[derive(Clone, Debug)]
pub struct UpdateCandidate {
    version: Version,
    version_text: String,
    notes: String,
    release_url: String,
    asset_url: String,
    asset_name: String,
    size: u64,
    digest: [u8; 32],
}

impl UpdateCandidate {
    pub fn version(&self) -> &str {
        &self.version_text
    }
    pub fn release_notes(&self) -> &str {
        &self.notes
    }
    pub fn release_url(&self) -> &str {
        &self.release_url
    }
    pub fn size(&self) -> u64 {
        self.size
    }
}

#[derive(Clone, Debug)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available(UpdateCandidate),
    Downloading {
        candidate: UpdateCandidate,
        downloaded: u64,
        total: u64,
    },
    Ready(UpdateCandidate),
    Cancelled,
    Failed(UpdateError),
}

/// This guard, rather than a copied path, is the authority to install. Keep it
/// alive through process creation. Constructing it from an arbitrary path is impossible.
#[derive(Debug)]
pub struct VerifiedInstaller {
    // Field order deliberately closes the file before TempDir attempts cleanup.
    _file: File,
    _directory: tempfile::TempDir,
    path: PathBuf,
}

impl VerifiedInstaller {
    pub fn installer_path(&self) -> &Path {
        &self.path
    }
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    published_at: Option<String>,
    html_url: String,
    body: Option<String>,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    state: String,
    digest: Option<String>,
}

fn candidate_from_release(
    release: Release,
    current: &Version,
    channel: UpdateChannel,
) -> Option<UpdateCandidate> {
    if release.draft || release.assets.len() > 256 || release.tag_name.len() > 100 {
        return None;
    }
    chrono::DateTime::parse_from_rfc3339(release.published_at.as_deref()?).ok()?;
    let version = Version::parse(release.tag_name.strip_prefix('v')?).ok()?;
    if release.tag_name != format!("v{version}")
        || !version.cmp_precedence(current).is_gt()
        || release.prerelease != !version.pre.is_empty()
        || (channel == UpdateChannel::Stable && release.prerelease)
        || release.html_url != format!("{RELEASE_ROOT}/tag/v{version}")
    {
        return None;
    }
    let asset_name = format!("CompanionSetup-{version}-windows-x64.exe");
    let mut assets = release.assets.into_iter().filter(|a| a.name == asset_name);
    let asset = assets.next()?;
    if assets.next().is_some()
        || asset.state != "uploaded"
        || asset.size == 0
        || asset.size > MAX_INSTALLER
        || asset.browser_download_url != format!("{RELEASE_ROOT}/download/v{version}/{asset_name}")
    {
        return None;
    }
    let hash = asset.digest.as_deref()?.strip_prefix("sha256:")?;
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let mut digest = [0; 32];
    for (i, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hash[i * 2..i * 2 + 2], 16).ok()?;
    }
    let notes = release.body.unwrap_or_default();
    let notes: String = notes
        .chars()
        .take(16_384)
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t' | '\r'))
        .collect();
    Some(UpdateCandidate {
        version_text: version.to_string(),
        version,
        notes,
        release_url: release.html_url,
        asset_url: asset.browser_download_url,
        asset_name,
        size: asset.size,
        digest,
    })
}

fn parse_releases(
    json: &[u8],
    current: &Version,
    channel: UpdateChannel,
) -> Result<Option<UpdateCandidate>, UpdateError> {
    if json.len() as u64 > MAX_METADATA {
        return Err(UpdateError::InvalidMetadata);
    }
    let releases: Vec<Release> =
        serde_json::from_slice(json).map_err(|_| UpdateError::InvalidMetadata)?;
    if releases.len() > 100 {
        return Err(UpdateError::InvalidMetadata);
    }
    Ok(releases
        .into_iter()
        .filter_map(|r| candidate_from_release(r, current, channel))
        .max_by(|a, b| a.version.cmp_precedence(&b.version)))
}

#[cfg(test)]
fn select_release(
    json: &[u8],
    current: &str,
    preview: bool,
) -> Result<Option<String>, UpdateError> {
    let current = Version::parse(current).map_err(|_| UpdateError::InvalidCurrentVersion)?;
    parse_releases(
        json,
        &current,
        if preview {
            UpdateChannel::Preview
        } else {
            UpdateChannel::Stable
        },
    )
    .map(|candidate| candidate.map(|c| c.version().to_owned()))
}

fn allowed_asset_redirect(url: &str) -> bool {
    if url.len() > 8192 || url.contains('#') {
        return false;
    }
    let Ok(uri) = url.parse::<ureq::http::Uri>() else {
        return false;
    };
    uri.scheme_str() == Some("https")
        && uri.authority().map(|a| a.as_str()) == Some("release-assets.githubusercontent.com")
        && uri.path().starts_with("/github-production-release-asset/")
        && !uri.path().contains(['\\', '%'])
        && !uri.path().split('/').any(|part| matches!(part, "." | ".."))
}

fn check_generation(generation: &AtomicU64, id: u64) -> Result<(), UpdateError> {
    if generation.load(Ordering::Acquire) == id {
        Ok(())
    } else {
        Err(UpdateError::Cancelled)
    }
}

fn download_stream(
    mut reader: impl Read,
    candidate: &UpdateCandidate,
    generation: &AtomicU64,
    id: u64,
    mut progress: impl FnMut(u64),
) -> Result<VerifiedInstaller, UpdateError> {
    check_generation(generation, id)?;
    let directory = tempfile::Builder::new()
        .prefix("companion-update-")
        .tempdir()
        .map_err(|_| UpdateError::FileAccess)?;
    let path = directory.path().join(&candidate.asset_name);
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1); // FILE_SHARE_READ only, never WRITE or DELETE.
    }
    let mut file = options.open(&path).map_err(|_| UpdateError::FileAccess)?;
    let started = Instant::now();
    let mut received = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_generation(generation, id)?;
        if started.elapsed() > DOWNLOAD_TIMEOUT {
            return Err(UpdateError::Network);
        }
        let count = reader.read(&mut buffer).map_err(|_| UpdateError::Network)?;
        if count == 0 {
            break;
        }
        received = received
            .checked_add(count as u64)
            .ok_or(UpdateError::Integrity)?;
        if received > candidate.size || received > MAX_INSTALLER {
            return Err(UpdateError::Integrity);
        }
        file.write_all(&buffer[..count])
            .map_err(|_| UpdateError::FileAccess)?;
        progress(received);
    }
    if received != candidate.size {
        return Err(UpdateError::Integrity);
    }
    file.sync_all().map_err(|_| UpdateError::FileAccess)?;
    drop(file);

    // Reopen without write access and deny write/delete sharing BEFORE hashing.
    // Any alteration in the close/reopen interval is detected by this final hash.
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    let mut file = options.open(&path).map_err(|_| UpdateError::FileAccess)?;
    if file.metadata().map_err(|_| UpdateError::FileAccess)?.len() != candidate.size {
        return Err(UpdateError::Integrity);
    }
    let mut hash = Sha256::new();
    loop {
        check_generation(generation, id)?;
        if started.elapsed() > DOWNLOAD_TIMEOUT {
            return Err(UpdateError::Network);
        }
        let count = file
            .read(&mut buffer)
            .map_err(|_| UpdateError::FileAccess)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    if hash.finalize().as_slice() != candidate.digest {
        return Err(UpdateError::Integrity);
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| UpdateError::FileAccess)?;
    check_generation(generation, id)?;
    Ok(VerifiedInstaller {
        _file: file,
        _directory: directory,
        path,
    })
}

trait UpdateSource: Send {
    fn releases(&mut self) -> Result<Vec<u8>, UpdateError>;
    fn installer(
        &mut self,
        candidate: &UpdateCandidate,
    ) -> Result<Box<dyn Read + Send>, UpdateError>;
}

#[cfg(any(test, feature = "ui-smoke"))]
const FIXTURE_BYTES: &[u8; 32] = b"Companion updater smoke fixture\n";

#[cfg(any(test, feature = "ui-smoke"))]
const FIXTURE_METADATA: &[u8] = br#"[{
    "tag_name":"v0.2.0","draft":false,"prerelease":false,
    "published_at":"2026-11-11T12:00:00Z",
    "html_url":"https://github.com/ethanjojo/metin2-bot/releases/tag/v0.2.0",
    "body":"A neutral update preview for Companion. This is a UI fixture, not an executable installer.",
    "assets":[{
        "name":"CompanionSetup-0.2.0-windows-x64.exe","size":32,"state":"uploaded",
        "browser_download_url":"https://github.com/ethanjojo/metin2-bot/releases/download/v0.2.0/CompanionSetup-0.2.0-windows-x64.exe",
        "digest":"sha256:b0dc024194067df36cfd5a879c44c2370e1280b8a76772ad1ecb00e58eda4f09"
    }]
}]"#;

#[cfg(any(test, feature = "ui-smoke"))]
struct FixtureSource;

#[cfg(any(test, feature = "ui-smoke"))]
impl UpdateSource for FixtureSource {
    fn releases(&mut self) -> Result<Vec<u8>, UpdateError> {
        Ok(FIXTURE_METADATA.to_vec())
    }

    fn installer(&mut self, _: &UpdateCandidate) -> Result<Box<dyn Read + Send>, UpdateError> {
        Ok(Box::new(std::io::Cursor::new(FIXTURE_BYTES.as_slice())))
    }
}

struct GitHubSource {
    agent: ureq::Agent,
}

impl GitHubSource {
    fn new() -> Self {
        Self {
            agent: ureq::Agent::config_builder()
                .https_only(true)
                .max_redirects(0)
                .http_status_as_error(false)
                .proxy(None)
                .user_agent("Companion-updater")
                .max_idle_connections(1)
                .max_response_header_size(32 * 1024)
                .timeout_resolve(Some(Duration::from_secs(10)))
                .timeout_connect(Some(Duration::from_secs(10)))
                .timeout_per_read(Some(Duration::from_secs(10)))
                .timeout_per_write(Some(Duration::from_secs(10)))
                .timeout_global(Some(DOWNLOAD_TIMEOUT))
                .build()
                .new_agent(),
        }
    }
}

impl UpdateSource for GitHubSource {
    fn releases(&mut self) -> Result<Vec<u8>, UpdateError> {
        let mut response = self
            .agent
            .get(RELEASES_API)
            .header("Accept", "application/vnd.github+json")
            .header("Accept-Encoding", "identity")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .config()
            .timeout_global(Some(Duration::from_secs(20)))
            .build()
            .call()
            .map_err(|_| UpdateError::Network)?;
        if response.status().as_u16() != 200 {
            return Err(UpdateError::HttpStatus(response.status().as_u16()));
        }
        response
            .body_mut()
            .with_config()
            .limit(MAX_METADATA)
            .read_to_vec()
            .map_err(|_| UpdateError::InvalidMetadata)
    }

    fn installer(
        &mut self,
        candidate: &UpdateCandidate,
    ) -> Result<Box<dyn Read + Send>, UpdateError> {
        let deadline = Instant::now() + DOWNLOAD_TIMEOUT;
        let mut url = candidate.asset_url.clone();
        for redirects in 0..=3 {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or(UpdateError::Network)?;
            let response = self
                .agent
                .get(&url)
                .header("Accept", "application/octet-stream")
                .header("Accept-Encoding", "identity")
                .config()
                .timeout_global(Some(remaining))
                .build()
                .call()
                .map_err(|_| UpdateError::Network)?;
            let status = response.status().as_u16();
            if matches!(status, 301 | 302 | 303 | 307 | 308) {
                let location = response
                    .headers()
                    .get("location")
                    .and_then(|h| h.to_str().ok())
                    .ok_or(UpdateError::UnsafeRedirect)?;
                if redirects == 3 || !allowed_asset_redirect(location) {
                    return Err(UpdateError::UnsafeRedirect);
                }
                url = location.to_owned();
                continue;
            }
            if status != 200 {
                return Err(UpdateError::HttpStatus(status));
            }
            if response
                .body()
                .content_length()
                .is_some_and(|size| size != candidate.size)
            {
                return Err(UpdateError::Integrity);
            }
            return Ok(Box::new(response.into_body().into_reader()));
        }
        Err(UpdateError::UnsafeRedirect)
    }
}

enum Operation {
    Check(UpdateChannel),
    Download(UpdateCandidate),
}
struct Job {
    id: u64,
    operation: Operation,
}
enum Completion {
    Checked(Option<UpdateCandidate>),
    Downloaded(UpdateCandidate, VerifiedInstaller),
}
#[derive(Default)]
struct Mailbox {
    progress: Option<(u64, u64)>,
    completion: Option<(u64, Result<Completion, UpdateError>)>,
}

pub struct UpdateService {
    commands: SyncSender<Job>,
    generation: Arc<AtomicU64>,
    mailbox: Arc<Mutex<Mailbox>>,
    state: UpdateState,
    ready: Option<VerifiedInstaller>,
}

impl UpdateService {
    #[cfg(any(test, feature = "ui-smoke"))]
    pub fn fixture(kind: UpdateFixture) -> Result<Self, UpdateError> {
        let mut service = Self::from_source("0.1.0", Box::new(FixtureSource))?;
        let candidate = parse_releases(
            FIXTURE_METADATA,
            &Version::new(0, 1, 0),
            UpdateChannel::Stable,
        )?
        .ok_or(UpdateError::InvalidMetadata)?;
        service.state = match kind {
            UpdateFixture::Idle => UpdateState::Idle,
            UpdateFixture::Available => UpdateState::Available(candidate),
            UpdateFixture::Downloading => UpdateState::Downloading {
                downloaded: 16,
                total: candidate.size,
                candidate,
            },
            UpdateFixture::Ready => {
                // The same disk/hash/Windows-lock production path builds this
                // intentionally nonexecutable artifact, only in explicit fixture mode.
                service.ready = Some(download_stream(
                    std::io::Cursor::new(FIXTURE_BYTES.as_slice()),
                    &candidate,
                    &service.generation,
                    0,
                    |_| {},
                )?);
                UpdateState::Ready(candidate)
            }
            UpdateFixture::Failed => UpdateState::Failed(UpdateError::Network),
        };
        Ok(service)
    }

    pub fn new(current_version: &str) -> Result<Self, UpdateError> {
        Self::from_source(current_version, Box::new(GitHubSource::new()))
    }

    fn from_source(current: &str, source: Box<dyn UpdateSource>) -> Result<Self, UpdateError> {
        let current = Version::parse(current).map_err(|_| UpdateError::InvalidCurrentVersion)?;
        let (commands, receiver) = mpsc::sync_channel(1);
        let generation = Arc::new(AtomicU64::new(0));
        let mailbox = Arc::new(Mutex::new(Mailbox::default()));
        let worker_generation = Arc::clone(&generation);
        let worker_mailbox = Arc::clone(&mailbox);
        std::thread::Builder::new()
            .name("companion-updates".into())
            .spawn(move || {
                update_worker(receiver, current, source, worker_generation, worker_mailbox)
            })
            .map_err(|_| UpdateError::WorkerUnavailable)?;
        Ok(Self {
            commands,
            generation,
            mailbox,
            state: UpdateState::Idle,
            ready: None,
        })
    }
    pub fn state(&self) -> &UpdateState {
        &self.state
    }

    /// Consume only the most recent worker result/progress. This never performs network or file I/O.
    pub fn poll(&mut self) -> bool {
        let Ok(mut mailbox) = self.mailbox.lock() else {
            self.state = UpdateState::Failed(UpdateError::WorkerUnavailable);
            return true;
        };
        let current = self.generation.load(Ordering::Acquire);
        if let Some((id, result)) = mailbox.completion.take() {
            mailbox.progress = None;
            if id != current {
                return false;
            }
            self.state = match result {
                Ok(Completion::Checked(Some(candidate))) => UpdateState::Available(candidate),
                Ok(Completion::Checked(None)) => UpdateState::UpToDate,
                Ok(Completion::Downloaded(candidate, installer)) => {
                    self.ready = Some(installer);
                    UpdateState::Ready(candidate)
                }
                Err(UpdateError::Cancelled) => UpdateState::Cancelled,
                Err(error) => UpdateState::Failed(error),
            };
            return true;
        }
        if let Some((id, received)) = mailbox.progress.take()
            && id == current
            && let UpdateState::Downloading { downloaded, .. } = &mut self.state
        {
            *downloaded = received;
            return true;
        }
        false
    }

    fn start(&mut self, operation: Operation, state: UpdateState) -> Result<(), UpdateError> {
        self.cancel();
        let id = self.generation.load(Ordering::Acquire);
        match self.commands.try_send(Job { id, operation }) {
            Ok(()) => {
                self.state = state;
                Ok(())
            }
            Err(mpsc::TrySendError::Full(_)) => {
                self.state = UpdateState::Failed(UpdateError::Busy);
                Err(UpdateError::Busy)
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.state = UpdateState::Failed(UpdateError::WorkerUnavailable);
                Err(UpdateError::WorkerUnavailable)
            }
        }
    }

    pub fn check(&mut self, channel: UpdateChannel) -> Result<(), UpdateError> {
        self.start(Operation::Check(channel), UpdateState::Checking)
    }

    pub fn download(&mut self) -> Result<(), UpdateError> {
        let UpdateState::Available(candidate) = &self.state else {
            return Err(UpdateError::NotReady);
        };
        let candidate = candidate.clone();
        let state = UpdateState::Downloading {
            downloaded: 0,
            total: candidate.size,
            candidate: candidate.clone(),
        };
        self.start(Operation::Download(candidate), state)
    }

    pub fn cancel(&mut self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.ready = None;
        if let Ok(mut mailbox) = self.mailbox.lock() {
            *mailbox = Mailbox::default();
        }
        self.state = UpdateState::Cancelled;
    }

    /// Transfer the same already hashed Windows write/delete-locked handle. No file is reopened.
    pub fn take_verified_installer(&mut self) -> Result<VerifiedInstaller, UpdateError> {
        if !matches!(self.state, UpdateState::Ready(_)) {
            return Err(UpdateError::NotReady);
        }
        let installer = self.ready.take().ok_or(UpdateError::NotReady)?;
        self.state = UpdateState::Idle;
        Ok(installer)
    }
}

impl Drop for UpdateService {
    fn drop(&mut self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}

fn update_worker(
    receiver: Receiver<Job>,
    current: Version,
    mut source: Box<dyn UpdateSource>,
    generation: Arc<AtomicU64>,
    mailbox: Arc<Mutex<Mailbox>>,
) {
    while let Ok(job) = receiver.recv() {
        if check_generation(&generation, job.id).is_err() {
            continue;
        }
        let result = match job.operation {
            Operation::Check(channel) => source.releases().and_then(|bytes| {
                check_generation(&generation, job.id)?;
                parse_releases(&bytes, &current, channel).map(Completion::Checked)
            }),
            Operation::Download(candidate) => source.installer(&candidate).and_then(|reader| {
                let installer =
                    download_stream(reader, &candidate, &generation, job.id, |received| {
                        if let Ok(mut mailbox) = mailbox.lock()
                            && check_generation(&generation, job.id).is_ok()
                        {
                            mailbox.progress = Some((job.id, received));
                        }
                    })?;
                Ok(Completion::Downloaded(candidate, installer))
            }),
        };
        if let Ok(mut mailbox) = mailbox.lock()
            && check_generation(&generation, job.id).is_ok()
        {
            mailbox.completion = Some((job.id, result));
        }
    }
}

#[cfg(test)]
#[path = "updates/tests.rs"]
mod tests;
