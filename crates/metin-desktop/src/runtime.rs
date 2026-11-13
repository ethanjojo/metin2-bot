//! Single owner of the live controller, timers and input executor.
use metin_core::{
    ActionKind, ActivityKind, Candidate, Controller, Profile, RunEvent, ScheduleSnapshot,
    Statistics,
};
use metin_platform::{
    CapabilityReport, Frame, InputExecutor, PlatformBackend, StopToken, TargetInfo, TargetState,
    TargetToken,
};
use metin_vision::{BuffDetector, CandidateTracker};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use uuid::Uuid;

// The same deadlines govern both arming behavior and its UI countdown.
const ARM_START_DELAY: Duration = Duration::from_secs(3);
const ARM_FOCUS_TIMEOUT: Duration = Duration::from_secs(12);
const START_CANCELLED_REASON: &str =
    "Start cancelled by Pause/F8. Check the game and press Start again when ready.";
// A recoverable update failure can replace the worker while the GUI survives.
// Request identities therefore outlive any one worker/session.
static NEXT_START_REQUEST: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionKey {
    pub profile_id: Uuid,
    pub target: TargetToken,
}
/// A global F7 press is a request for the visible UI's save/validate/start path.
/// The worker never decides that an old applied profile represents the draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartRequest {
    pub id: u64,
    pub key: SessionKey,
    pub revision: u64,
    pub generation: u64,
    pub requested_at: Instant,
}
impl StartRequest {
    pub fn matches(self, key: Option<SessionKey>, revision: u64, generation: u64) -> bool {
        key == Some(self.key)
            && revision == self.revision
            && generation == self.generation
            && Instant::now()
                .checked_duration_since(self.requested_at)
                .is_some_and(|age| age <= Duration::from_secs(2))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseCause {
    FocusLost,
    WindowChanged,
    StaleFrame,
    CaptureInterrupted,
    RuntimeFailure,
}
impl PauseCause {
    pub fn label(self) -> &'static str {
        match self {
            Self::FocusLost => "Game lost focus",
            Self::WindowChanged => "Game window changed",
            Self::StaleFrame => "No fresh game image",
            Self::CaptureInterrupted => "Capture interrupted",
            Self::RuntimeFailure => "Runtime interrupted",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PauseInfo {
    pub cause: PauseCause,
    pub details: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotPurpose {
    /// Existing capture/preview controls also refresh the live recognition evidence.
    UpdateLiveEvidence,
    /// Inspection captures pixels without advancing temporal recognition or timers.
    ReadOnlyRecognition,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecognitionEvidenceSource {
    pub key: SessionKey,
    pub revision: u64,
    pub frame_id: u64,
    pub captured_at: Instant,
    pub geometry: TargetState,
}
pub enum Command {
    TestNotification {
        sound: bool,
        system: bool,
    },
    RefreshTargets,
    Connect {
        key: SessionKey,
        revision: u64,
    },
    Configure {
        profile: Box<Profile>,
        template: Option<Frame>,
        target: TargetToken,
        revision: u64,
    },
    Detach,
    SetPreview {
        key: SessionKey,
        enabled: bool,
    },
    Snapshot {
        key: SessionKey,
        purpose: SnapshotPurpose,
    },
    CalibrationReview {
        key: SessionKey,
        revision: u64,
        active: bool,
    },
    Arm {
        key: SessionKey,
    },
    DeclareMount {
        key: SessionKey,
        mounted: bool,
        revision: u64,
    },
    ResetSession {
        key: SessionKey,
    },
    ClearHistory {
        key: SessionKey,
    },
    ResetSkillTimer {
        key: SessionKey,
        index: usize,
    },
}
#[derive(Clone, PartialEq)]
pub struct RuntimeSnapshot {
    pub key: Option<SessionKey>,
    pub revision: u64,
    /// A connection/mount acknowledgement is not proof of applied profile data.
    pub applied_revision: Option<u64>,
    pub start_request: Option<StartRequest>,
    pub pause_info: Option<PauseInfo>,
    /// Worker-owned start gate for the current session's calibration draft review.
    pub calibration_review: bool,
    pub frame: Option<Frame>,
    pub geometry: Option<TargetState>,
    /// Set only after all owned input releases succeeded in a stopped generation.
    pub released_generation: Option<u64>,
    pub frame_id: u64,
    pub captured_at: Option<Instant>,
    pub candidates: Vec<Candidate>,
    pub selected_target: Option<[u32; 2]>,
    pub next_target: Option<[u32; 2]>,
    pub panel_score: f64,
    pub panel_match: Option<metin_vision::TemplateMatch>,
    pub state: String,
    pub activity: ActivityKind,
    pub active_skill: Option<usize>,
    pub reason: String,
    pub running: bool,
    pub arming: bool,
    pub mounted: Option<bool>,
    pub timers: Vec<Option<f64>>,
    pub statistics: Statistics,
    pub schedule: ScheduleSnapshot,
    pub events: Vec<RunEvent>,
    pub action_timers: Vec<metin_core::ActionTimer>,
    pub attack_remaining: Option<f64>,
    pub activity_progress: Option<metin_core::ActivityProgress>,
    pub buff_presence: Vec<metin_core::Presence>,
    pub buff_last_active_age_seconds: Vec<Option<f64>>,
    /// Provenance of the derived recognition pass, independent of newer raw snapshots.
    pub recognition_evidence: Option<RecognitionEvidenceSource>,
    pub safety_scores: Vec<(Uuid, f64)>,
    pub recoveries: u32,
    pub notification_status: String,
    pub targets: Vec<TargetInfo>,
    pub targets_revision: u64,
    pub capabilities: CapabilityReport,
}
impl Default for RuntimeSnapshot {
    fn default() -> Self {
        Self {
            key: None,
            revision: 0,
            applied_revision: None,
            start_request: None,
            pause_info: None,
            calibration_review: false,
            frame: None,
            geometry: None,
            released_generation: None,
            frame_id: 0,
            captured_at: None,
            candidates: vec![],
            selected_target: None,
            next_target: None,
            panel_score: 0.,
            panel_match: None,
            state: "Stopped".into(),
            activity: ActivityKind::Stopped,
            active_skill: None,
            reason: "Select a profile and game window.".into(),
            running: false,
            arming: false,
            mounted: None,
            timers: vec![],
            statistics: Statistics::default(),
            schedule: Default::default(),
            events: vec![],
            action_timers: vec![],
            attack_remaining: None,
            activity_progress: None,
            buff_presence: Vec::new(),
            buff_last_active_age_seconds: Vec::new(),
            recognition_evidence: None,
            safety_scores: vec![],
            recoveries: 0,
            notification_status: "No notifications sent.".into(),
            targets: vec![],
            targets_revision: 0,
            capabilities: CapabilityReport::default(),
        }
    }
}
impl RuntimeSnapshot {
    pub fn recognition_evidence_is_fresh(&self) -> bool {
        self.recognition_evidence.is_some_and(|source| {
            self.key == Some(source.key)
                && self.revision == source.revision
                && self.geometry == Some(source.geometry)
                && source.geometry.target == source.key.target
                && source.geometry.visible
                && Instant::now()
                    .checked_duration_since(source.captured_at)
                    .is_some_and(|age| age <= Duration::from_millis(500))
        })
    }
    pub fn recognition_evidence_matches_frame(&self) -> bool {
        self.recognition_evidence_is_fresh()
            && self.recognition_evidence.is_some_and(|source| {
                source.frame_id == self.frame_id && Some(source.captured_at) == self.captured_at
            })
    }
    fn update_activity(
        &mut self,
        controller: Option<&Controller>,
        arming: bool,
        failed: bool,
        now: f64,
    ) {
        self.activity = if failed {
            ActivityKind::Error
        } else if arming {
            ActivityKind::Starting
        } else {
            controller.map_or(ActivityKind::Stopped, Controller::activity)
        };
        self.active_skill = if arming || failed {
            None
        } else {
            controller.and_then(|c| c.active_skill(now))
        };
    }
}
struct Envelope {
    generation: u64,
    command: Command,
}

const CALIBRATION_REVIEW_REASON: &str = "Finish or cancel the calibration review before starting.";

fn start_controller(controller: &mut Controller, now: f64) -> Result<(), String> {
    let clock = controller
        .profile
        .safety
        .clock_enabled
        .then(|| {
            next_clock_delay(
                chrono::Local::now(),
                controller.profile.safety.stop_at_minute,
            )
        })
        .transpose()?;
    controller
        .start_with_clock(now, clock)
        .map_err(|e| e.to_string())
}

/// Fix a wall-clock choice to a monotonic deadline. DST gaps have no occurrence;
/// ambiguous times choose the first future occurrence, including the second fold.
fn next_clock_delay<T: chrono::TimeZone>(
    now: chrono::DateTime<T>,
    minute: u32,
) -> Result<f64, String> {
    use chrono::LocalResult;
    if minute >= 1440 {
        return Err("Invalid local stop time.".into());
    }
    for day in 0..=2 {
        let date = now.date_naive() + chrono::Duration::days(day);
        let local = date
            .and_hms_opt(minute / 60, minute % 60, 0)
            .ok_or("Invalid local stop time.")?;
        let candidates = match now.timezone().from_local_datetime(&local) {
            LocalResult::Single(a) => vec![a],
            LocalResult::Ambiguous(a, b) => vec![a, b],
            LocalResult::None => vec![],
        };
        if let Some(next) = candidates.into_iter().filter(|d| d > &now).min() {
            return Ok((next - now).num_milliseconds() as f64 / 1000.);
        }
    }
    Err("No valid occurrence of the local stop time was found.".into())
}

pub struct RuntimeHandle {
    sender: SyncSender<Envelope>,
    stop: StopToken,
    shutdown_flag: Arc<AtomicBool>,
    latest: Arc<Mutex<Arc<RuntimeSnapshot>>>,
    thread: Mutex<Option<JoinHandle<Result<(), String>>>>,
}
impl RuntimeHandle {
    pub fn spawn() -> Self {
        Self::spawn_with_notify(|| {})
    }
    pub fn spawn_with_notify(notify: impl Fn() + Send + 'static) -> Self {
        Self::spawn_with_backend(metin_platform::backend(), notify)
    }
    pub fn spawn_with_backend(
        backend: Box<dyn PlatformBackend>,
        notify: impl Fn() + Send + 'static,
    ) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<Envelope>(32);
        let stop = StopToken::default();
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let latest = Arc::new(Mutex::new(Arc::new(RuntimeSnapshot::default())));
        let (cancel, shutdown, output) = (stop.clone(), shutdown_flag.clone(), latest.clone());
        let thread = thread::Builder::new()
            .name("metin-runtime".into())
            .spawn(move || run(backend, receiver, cancel, shutdown, output, notify))
            .expect("Unable to create runtime worker");
        Self {
            sender,
            stop,
            shutdown_flag,
            latest,
            thread: Mutex::new(Some(thread)),
        }
    }
    pub fn send(&self, command: Command) -> Result<(), String> {
        if let Command::ResetSession { key } = &command {
            if self.latest()?.key != Some(*key) {
                return Err("The session reset belongs to an obsolete game/profile.".into());
            }
            // Interrupt blocking holds before queueing the atomic reset. A full
            // queue still leaves input cancelled without claiming a data reset.
            self.stop.stop();
        }
        self.enqueue(command, self.stop.generation())
    }
    fn enqueue(&self, command: Command, generation: u64) -> Result<(), String> {
        self.sender
            .try_send(Envelope {
                generation,
                command,
            })
            .map_err(|e| match e {
                TrySendError::Full(_) => "The runtime command queue is full. Try again.".into(),
                TrySendError::Disconnected(_) => "The runtime worker has stopped.".into(),
            })
    }
    pub fn stop(&self) {
        self.stop.stop();
    }
    pub fn stop_if_current(&self, expected: u64) -> Result<u64, String> {
        self.stop
            .stop_if_current(expected)
            .ok_or_else(|| START_CANCELLED_REASON.into())
    }
    /// Preserve the caller's exact authority through queueing. In particular,
    /// never stamp an Arm with the newer generation of an intervening F8.
    pub fn arm_in_generation(&self, key: SessionKey, expected: u64) -> Result<(), String> {
        if self.stop.cancelled(expected) {
            return Err(START_CANCELLED_REASON.into());
        }
        self.enqueue(Command::Arm { key }, expected)
    }
    pub fn stop_generation(&self) -> u64 {
        self.stop.generation()
    }
    pub fn latest(&self) -> Result<Arc<RuntimeSnapshot>, String> {
        self.latest
            .lock()
            .map(|value| value.clone())
            .map_err(|_| "Runtime snapshot is unavailable.".into())
    }
    pub fn shutdown(&self) -> Result<(), String> {
        self.shutdown_flag.store(true, Ordering::SeqCst);
        self.stop();
        if let Some(thread) = self
            .thread
            .lock()
            .map_err(|_| "Runtime join lock poisoned.")?
            .take()
        {
            thread.join().map_err(|_| "Runtime worker panicked.")??;
        }
        Ok(())
    }
}
impl Drop for RuntimeHandle {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
struct Attached {
    safety: metin_vision::SafetyDetector,
    key: SessionKey,
    controller: Controller,
    template: Option<Frame>,
    tracker: CandidateTracker,
    buffs: BuffDetector,
    revision: u64,
    applied_revision: Option<u64>,
    preview: bool,
    calibration_review: bool,
    snapshot_requested: Option<SnapshotPurpose>,
}
impl Attached {
    fn new(
        key: SessionKey,
        controller: Controller,
        template: Option<Frame>,
        revision: u64,
    ) -> Result<Self, String> {
        let profile = &controller.profile;
        Ok(Self {
            buffs: BuffDetector::new(
                profile.buffs.clone(),
                profile.frame_size.unwrap_or([1, 1]),
                profile.timing.sample_interval,
            )
            .map_err(|e| e.to_string())?,
            tracker: CandidateTracker::new(
                profile.color.as_ref().map_or(3, |c| c.stable_frames),
                profile.timing.sample_interval,
            ),
            safety: metin_vision::SafetyDetector::new(&profile.safety.rules)
                .map_err(|e| e.to_string())?,
            key,
            controller,
            template,
            revision,
            applied_revision: None,
            preview: false,
            calibration_review: false,
            snapshot_requested: None,
        })
    }
}

fn clear_session_snapshot(snapshot: &mut RuntimeSnapshot) {
    *snapshot = RuntimeSnapshot {
        targets: std::mem::take(&mut snapshot.targets),
        targets_revision: snapshot.targets_revision,
        capabilities: snapshot.capabilities.clone(),
        ..Default::default()
    };
}

fn publish_captured_frame(snapshot: &mut RuntimeSnapshot, captured: metin_platform::CapturedFrame) {
    snapshot.frame = Some(captured.frame);
    snapshot.geometry = Some(captured.geometry);
    snapshot.frame_id = captured.id;
    snapshot.captured_at = Some(captured.captured_at);
}

/// Stopped calibration must retain raw pixels even when saved references need
/// repair. Only vision results use this policy; platform/input errors stay strict.
fn recognition_for_capture<T: Default>(
    result: metin_vision::Result<T>,
    allow_repair: bool,
    diagnostics: &mut Vec<String>,
) -> Result<T, String> {
    match result {
        Ok(value) => Ok(value),
        Err(error) if allow_repair => {
            diagnostics.push(error.to_string());
            Ok(T::default())
        }
        Err(error) => Err(error.to_string()),
    }
}

fn pause_after_failure(controller: &mut Controller, now: f64, error: &str, during_start: bool) {
    let prior_event = controller.events.back().map(|event| event.id);
    controller.pause_on_runtime_error(now, error);
    if during_start {
        // Repeating a failed explicit Start still needs a new event/alert,
        // whereas ordinary paused repair captures must remain silent.
        if controller.events.back().map(|event| event.id) == prior_event {
            controller.event("Start interrupted", error, true);
        }
        if let Some(event) = controller.events.back_mut() {
            event.notify = true;
            event.destination = metin_core::EventDestination::Calibration;
        }
    }
}

fn run(
    mut backend: Box<dyn PlatformBackend>,
    receiver: mpsc::Receiver<Envelope>,
    stop: StopToken,
    shutdown: Arc<AtomicBool>,
    output: Arc<Mutex<Arc<RuntimeSnapshot>>>,
    notify: impl Fn(),
) -> Result<(), String> {
    let mut executor = InputExecutor::default();
    let mut attached: Option<Attached> = None;
    let mut snapshot = RuntimeSnapshot::default();
    let origin = Instant::now();
    let mut observed_generation = stop.generation();
    let mut armed: Option<Instant> = None;
    let mut focus_since: Option<Instant> = None;
    let mut next_sample = Instant::now();
    let mut last_published = RuntimeSnapshot::default();
    let mut last_publish_time = Instant::now() - Duration::from_secs(1);
    let mut logged_event: Option<(SessionKey, u64)> = None;
    let notifications = crate::notifications::NotificationService::default();
    let publish = |s: &RuntimeSnapshot| {
        if let Ok(mut value) = output.lock() {
            *value = Arc::new(s.clone());
        }
        notify();
    };
    while !shutdown.load(Ordering::SeqCst) {
        // Any attempted command/input invalidates the prior acknowledgement;
        // the stopped path below renews it only after a successful release.
        snapshot.released_generation = None;
        let now = origin.elapsed().as_secs_f64();
        let mut generation = stop.generation();
        if observed_generation != generation {
            snapshot.start_request = None;
            snapshot.pause_info = None;
            observed_generation = generation;
            armed = None;
            if let Some(a) = &mut attached {
                a.snapshot_requested = None;
                a.controller.pause(now, "Paused (F8 / Pause).");
            }
            if let Err(e) = executor.release(backend.as_mut()) {
                snapshot.reason = e.to_string();
            }
            next_sample = Instant::now();
        }
        match receiver.recv_timeout(Duration::from_millis(20)) {
            Ok(envelope)
                if envelope.generation == stop.generation()
                    || matches!(&envelope.command, Command::CalibrationReview { .. }) =>
            {
                // Review controls change no game input and are guarded by the
                // session/revision below. Preserve their FIFO order across Stop,
                // especially End review queued immediately before Save cancels input.
                let current_generation = stop.generation();
                // Retire cancellation received while the channel was asleep
                // before applying a valid command from the new generation.
                if generation != current_generation {
                    snapshot.start_request = None;
                    snapshot.pause_info = None;
                    generation = current_generation;
                    observed_generation = generation;
                    armed = None;
                    if let Some(a) = &mut attached {
                        a.snapshot_requested = None;
                        a.controller.pause(now, "Paused (F8 / Pause).");
                    }
                    if let Err(error) = executor.release(backend.as_mut()) {
                        snapshot.reason = error.to_string();
                    }
                    next_sample = Instant::now();
                }
                let requested_key = match &envelope.command {
                    Command::Arm { key }
                    | Command::SetPreview { key, .. }
                    | Command::Snapshot { key, .. }
                    | Command::CalibrationReview { key, .. }
                    | Command::DeclareMount { key, .. }
                    | Command::ResetSession { key }
                    | Command::ClearHistory { key }
                    | Command::ResetSkillTimer { key, .. } => Some(*key),
                    _ => None,
                };
                if requested_key.is_some_and(|key| attached.as_ref().is_none_or(|a| a.key != key)) {
                    tracing::warn!("Discarding command for an obsolete game/profile selection");
                    continue;
                }
                let result = (|| -> Result<(), String> {
                    match envelope.command {
                        Command::TestNotification { sound, system } => {
                            notifications.send(crate::notifications::Notice {
                                title: format!("{} notification test", crate::PRODUCT_NAME),
                                body: "Local notification test; no game input is sent.".into(),
                                sound,
                                system,
                            })?
                        }
                        Command::RefreshTargets => {
                            snapshot.targets = backend.list_targets().map_err(|e| e.to_string())?;
                            snapshot.targets_revision += 1;
                        }
                        Command::Connect { key, revision } => {
                            snapshot.applied_revision = None;
                            if let Some(attached) = &mut attached {
                                attached.applied_revision = None;
                            }
                            snapshot.start_request = None;
                            snapshot.pause_info = None;
                            snapshot.recognition_evidence = None;
                            snapshot.buff_presence.clear();
                            snapshot.buff_last_active_age_seconds.clear();
                            armed = None;
                            if attached.as_ref().is_some_and(|a| a.key == key) {
                                executor
                                    .release(backend.as_mut())
                                    .map_err(|e| e.to_string())?;
                                let a = attached.as_mut().expect("matching session");
                                a.controller.pause(
                                    now,
                                    "Game window connected; configuration changes apply on Save.",
                                );
                                a.revision = revision;
                                a.calibration_review = false;
                            } else {
                                // Retire the old character before any fallible attachment.
                                attached = None;
                                focus_since = None;
                                logged_event = None;
                                clear_session_snapshot(&mut snapshot);
                                executor
                                    .release(backend.as_mut())
                                    .map_err(|e| e.to_string())?;
                                backend.attach(key.target).map_err(|e| e.to_string())?;
                                let mut profile = Profile::create("Unconfigured profile");
                                profile.id = key.profile_id;
                                attached = Some(Attached::new(
                                    key,
                                    Controller::new(profile),
                                    None,
                                    revision,
                                )?);
                            }
                            next_sample = Instant::now();
                        }
                        Command::SetPreview { enabled, .. } => {
                            let a = attached.as_mut().expect("validated session");
                            a.preview = enabled;
                            if enabled {
                                next_sample = Instant::now();
                            }
                        }
                        Command::Snapshot { purpose, .. } => {
                            let a = attached.as_mut().expect("validated session");
                            if purpose == SnapshotPurpose::ReadOnlyRecognition
                                && (a.controller.running || armed.is_some())
                            {
                                return Ok(());
                            }
                            a.snapshot_requested = Some(purpose);
                            if purpose == SnapshotPurpose::UpdateLiveEvidence {
                                next_sample = Instant::now();
                            }
                        }
                        Command::CalibrationReview {
                            revision, active, ..
                        } => {
                            let a = attached.as_mut().expect("validated session");
                            if a.revision != revision {
                                return Ok(());
                            }
                            let was_active = a.calibration_review;
                            a.calibration_review = active;
                            if active {
                                snapshot.start_request = None;
                                armed = None;
                                focus_since = None;
                                a.controller.pause(now, CALIBRATION_REVIEW_REASON);
                                executor
                                    .release(backend.as_mut())
                                    .map_err(|error| error.to_string())?;
                            } else if was_active {
                                a.controller
                                    .pause(now, "Calibration review closed; press Start or F7.");
                            }
                        }
                        Command::Detach => {
                            armed = None;
                            backend.stop_capture();
                            executor
                                .release(backend.as_mut())
                                .map_err(|e| e.to_string())?;
                            attached = None;
                            logged_event = None;
                            clear_session_snapshot(&mut snapshot);
                            snapshot.reason = "Select a game window.".into();
                        }
                        Command::Configure {
                            profile,
                            template,
                            target,
                            revision,
                        } => {
                            snapshot.applied_revision = None;
                            if let Some(attached) = &mut attached {
                                attached.applied_revision = None;
                            }
                            snapshot.start_request = None;
                            snapshot.pause_info = None;
                            snapshot.recognition_evidence = None;
                            snapshot.buff_presence.clear();
                            snapshot.buff_last_active_age_seconds.clear();
                            profile.validate().map_err(|e| e.to_string())?;
                            let key = SessionKey {
                                profile_id: profile.id,
                                target,
                            };
                            if attached
                                .as_ref()
                                .is_some_and(|a| a.key == key && a.calibration_review)
                            {
                                return Err(CALIBRATION_REVIEW_REASON.into());
                            }
                            armed = None;
                            executor
                                .release(backend.as_mut())
                                .map_err(|e| e.to_string())?;
                            let previous = attached.take();
                            snapshot.key = None;
                            snapshot.frame = None;
                            snapshot.geometry = None;
                            snapshot.captured_at = None;
                            snapshot.panel_match = None;
                            snapshot.running = false;
                            if previous.as_ref().is_none_or(|old| old.key != key) {
                                backend.attach(target).map_err(|e| e.to_string())?;
                            }
                            let mut controller =
                                if let Some(old) = previous.filter(|old| old.key == key) {
                                    let mut c = old.controller;
                                    c.pause(now, "Configuration saved; press Start or F7.");
                                    if !c.profile.buffs.execution_eq(&profile.buffs) {
                                        let mounted = c.buffs.mounted;
                                        c.buffs = metin_core::BuffCycle::new(profile.buffs.clone());
                                        c.buffs.mounted = mounted;
                                    } else {
                                        c.buffs.config = profile.buffs.clone();
                                    }
                                    if !c.profile.safety.schedule_eq(&profile.safety) {
                                        c.reset_schedule();
                                        c.event(
                                        "Safety settings",
                                        "Safety settings changed; active-time limits restarted.",
                                        false,
                                    );
                                    }
                                    c.actions.reconfigure(&c.profile, &profile);
                                    c.profile = *profile;
                                    c
                                } else {
                                    Controller::new(*profile)
                                };
                            controller.running = false;
                            let mut configured =
                                Attached::new(key, controller, template, revision)?;
                            configured.applied_revision = Some(revision);
                            attached = Some(configured);
                            next_sample = Instant::now();
                        }
                        Command::Arm { key } => {
                            snapshot.start_request = None;
                            snapshot.pause_info = None;
                            let a = attached
                                .as_ref()
                                .filter(|a| a.key == key)
                                .ok_or("The game/profile selection changed.")?;
                            if a.calibration_review {
                                return Err(CALIBRATION_REVIEW_REASON.into());
                            }
                            if a.applied_revision != Some(a.revision) {
                                return Err(
                                    "Apply the profile configuration before starting.".into()
                                );
                            }
                            if !a.controller.profile.ready() {
                                return Err("Complete calibration before starting.".into());
                            }
                            if !backend.capabilities().automation_ready() {
                                return Err(backend.capabilities().reason);
                            }
                            let activated =
                                backend.request_activation().map_err(|e| e.to_string())?;
                            armed = Some(Instant::now());
                            focus_since = None;
                            if !activated {
                                snapshot.reason = "Activate the game manually and press F7.".into();
                            }
                        }
                        Command::DeclareMount {
                            key,
                            mounted,
                            revision,
                        } => {
                            let a = attached
                                .as_mut()
                                .filter(|a| a.key == key)
                                .ok_or("The mount declaration belongs to a stale game/profile.")?;
                            armed = None;
                            executor
                                .release(backend.as_mut())
                                .map_err(|e| e.to_string())?;
                            a.controller.pause(
                                now,
                                "Mount state declared; check the game before resuming.",
                            );
                            a.controller.buffs.mounted = Some(mounted);
                            a.applied_revision = a
                                .applied_revision
                                .filter(|applied| *applied == a.revision)
                                .map(|_| revision);
                            a.revision = revision;
                        }
                        Command::ResetSession { key } => {
                            let a = attached
                                .as_mut()
                                .filter(|a| a.key == key)
                                .ok_or("Stale session reset request.")?;
                            armed = None;
                            executor
                                .release(backend.as_mut())
                                .map_err(|error| error.to_string())?;
                            a.controller.reset_session(now);
                            a.snapshot_requested = None;
                            snapshot.recognition_evidence = None;
                            snapshot.buff_presence.clear();
                            snapshot.buff_last_active_age_seconds.clear();
                            snapshot.activity_progress = None;
                        }
                        Command::ClearHistory { key } => {
                            attached
                                .as_mut()
                                .filter(|a| a.key == key)
                                .ok_or("Stale history request.")?
                                .controller
                                .events
                                .clear();
                        }
                        Command::ResetSkillTimer { key, index } => {
                            let a = attached
                                .as_mut()
                                .filter(|a| a.key == key)
                                .ok_or("Stale skill timer request.")?;
                            a.controller
                                .buffs
                                .reset_timer(index, now, !a.controller.is_active())
                                .map_err(|e| e.to_string())?;
                        }
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    snapshot.start_request = None;
                    snapshot.pause_info = Some(PauseInfo {
                        cause: PauseCause::RuntimeFailure,
                        details: error.clone(),
                    });
                    backend.stop_capture();
                    snapshot.arming = armed.is_some();
                    snapshot.state = "Error".into();
                    snapshot.update_activity(None, false, true, now);
                    snapshot.reason = error.clone();
                    if let Some(a) = &mut attached {
                        a.controller.pause(now, error);
                    }
                    publish(&snapshot);
                }
            }
            Ok(_) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        let hotkeys = backend.hotkeys();
        if let Some(a) = &mut attached {
            let mut failure_cause = PauseCause::RuntimeFailure;
            let result = (|| -> Result<(), String> {
                let read_only_capture = a.snapshot_requested
                    == Some(SnapshotPurpose::ReadOnlyRecognition)
                    && !a.controller.running
                    && armed.is_none();
                if !read_only_capture {
                    a.controller.tick(origin.elapsed().as_secs_f64());
                }
                let events = hotkeys.map_err(|e| e.to_string())?;
                if events.stop {
                    snapshot.start_request = None;
                    snapshot.pause_info = None;
                    stop.stop();
                    a.controller.pause(now, "Emergency stop (F8).");
                    armed = None;
                    executor
                        .release(backend.as_mut())
                        .map_err(|e| e.to_string())?;
                }
                let state = backend.inspect().map_err(|e| e.to_string())?;
                if snapshot.geometry.is_some_and(|captured| captured != state) {
                    snapshot.geometry = None;
                }
                if state.foreground {
                    focus_since.get_or_insert(Instant::now());
                } else {
                    focus_since = None;
                }
                if a.controller.running && !state.foreground {
                    snapshot.pause_info = Some(PauseInfo {
                        cause: PauseCause::FocusLost,
                        details: "The selected game is no longer in the foreground. Inputs have been released; check the window before starting again.".into(),
                    });
                    a.controller.pause_on_runtime_error(
                        now,
                        "Game lost focus. Check the selected window before starting again.",
                    );
                    armed = None;
                }
                if events.toggle && !events.stop && !a.calibration_review {
                    if a.controller.running || armed.is_some() {
                        armed = None;
                        snapshot.start_request = None;
                        snapshot.pause_info = None;
                        a.controller.pause(now, "Paused (F7).");
                    } else {
                        let start_request_id = NEXT_START_REQUEST
                            .try_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                                value.checked_add(1)
                            })
                            .map_err(
                                |_| "Start request identity exhausted; restart the application.",
                            )?
                            + 1;
                        snapshot.start_request = Some(StartRequest {
                            id: start_request_id,
                            key: a.key,
                            revision: a.revision,
                            generation,
                            requested_at: Instant::now(),
                        });
                    }
                    next_sample = Instant::now();
                }
                if let Some(since) = armed
                    && since.elapsed() >= ARM_FOCUS_TIMEOUT
                {
                    armed = None;
                    a.controller.pause(now, format!("The game did not reach stable focus within {} s. Activate it and press F7.", ARM_FOCUS_TIMEOUT.as_secs()));
                }
                if !a.controller.running {
                    executor
                        .release(backend.as_mut())
                        .map_err(|e| e.to_string())?;
                    if armed.is_none() {
                        snapshot.released_generation = Some(generation);
                    }
                }
                executor
                    .poll_attack(backend.as_mut(), &stop, generation)
                    .map_err(|e| e.to_string())?;
                if !a.controller.running
                    && armed.is_none()
                    && !a.preview
                    && a.snapshot_requested.is_none()
                {
                    backend.stop_capture();
                    return Ok(());
                }
                if Instant::now() < next_sample && !read_only_capture {
                    return Ok(());
                }
                executor.set_deadline(if a.controller.running {
                    a.controller
                        .input_budget(origin.elapsed().as_secs_f64())
                        .map(|left| Instant::now() + Duration::from_secs_f64(left))
                } else {
                    None
                });
                let Some(capture_budget) = executor.capture_budget() else {
                    return Ok(());
                };
                if !read_only_capture {
                    next_sample = Instant::now()
                        + Duration::from_secs_f64(a.controller.profile.timing.sample_interval);
                }
                a.snapshot_requested = None;
                let captured =
                    match backend.capture(Instant::now() + capture_budget, &stop, generation) {
                        Ok(captured) => captured,
                        Err(error) if read_only_capture => {
                            tracing::warn!(%error, "Read-only recognition capture failed");
                            return Ok(());
                        }
                        // WGC can reject a frame when activation changes focus. No
                        // input has started while armed; wait for a fresh stable
                        // capture within the existing arming timeout. Running/HID
                        // failures never use this retry path.
                        Err(error) if armed.is_some() && !a.controller.running => {
                            snapshot.geometry = None;
                            focus_since = None;
                            tracing::debug!(%error, "Retrying capture while arming");
                            return Ok(());
                        }
                        Err(error) => {
                            failure_cause = PauseCause::CaptureInterrupted;
                            return Err(error.to_string());
                        }
                    };
                if captured.captured_at.elapsed() > Duration::from_millis(500) {
                    if read_only_capture {
                        tracing::warn!("Read-only recognition capture returned a stale frame");
                        return Ok(());
                    }
                    snapshot.geometry = None;
                    if armed.is_some() && !a.controller.running {
                        focus_since = None;
                        return Ok(());
                    }
                    failure_cause = PauseCause::StaleFrame;
                    return Err("The game frame is stale. Waiting for a fresh capture is required before input.".into());
                }
                if read_only_capture {
                    publish_captured_frame(&mut snapshot, captured);
                    return Ok(());
                }
                executor
                    .poll_attack(backend.as_mut(), &stop, generation)
                    .map_err(|e| e.to_string())?;
                let now = captured.captured_at.duration_since(origin).as_secs_f64();
                // Check dimensions before recognition throughout the complete
                // start countdown, so a resized client's references cannot fail
                // first with a misleading template-size error.
                if (a.controller.running || armed.is_some())
                    && a.controller.profile.frame_size
                        != Some([captured.frame.width, captured.frame.height])
                {
                    failure_cause = PauseCause::WindowChanged;
                    return Err("The game dimensions changed. Recalibrate before resuming.".into());
                }
                if armed.is_some_and(|since| since.elapsed() >= ARM_START_DELAY)
                    && focus_since
                        .is_some_and(|since| since.elapsed() >= Duration::from_millis(500))
                    && captured.geometry.foreground
                    && captured.geometry == backend.inspect().map_err(|e| e.to_string())?
                {
                    start_controller(&mut a.controller, origin.elapsed().as_secs_f64())?;
                    a.tracker.reset();
                    a.buffs.reset();
                    armed = None;
                    snapshot.released_generation = None;
                }
                let p = &a.controller.profile;
                let allow_repair = !a.controller.running && armed.is_none();
                let mut recognition_diagnostics = Vec::new();
                let panel_match = recognition_for_capture(
                    a.template
                        .as_ref()
                        .map(|template| metin_vision::panel_match(&captured.frame, template))
                        .transpose(),
                    allow_repair,
                    &mut recognition_diagnostics,
                )?;
                let score = panel_match.map_or(0., |found| found.score);
                let candidates = recognition_for_capture(
                    metin_vision::detect_candidates_excluding(
                        &captured.frame,
                        p,
                        panel_match
                            .as_ref()
                            .filter(|found| found.score >= p.panel.threshold),
                    ),
                    allow_repair,
                    &mut recognition_diagnostics,
                )?;
                executor
                    .poll_attack(backend.as_mut(), &stop, generation)
                    .map_err(|e| e.to_string())?;
                let stable = a.tracker.update(candidates.clone(), now);
                let states = recognition_for_capture(
                    a.buffs.observe(&captured.frame, now),
                    allow_repair,
                    &mut recognition_diagnostics,
                )?;
                executor
                    .poll_attack(backend.as_mut(), &stop, generation)
                    .map_err(|e| e.to_string())?;
                let safety_scores = recognition_for_capture(
                    a.safety.observe(&captured.frame),
                    allow_repair,
                    &mut recognition_diagnostics,
                )?;
                if !recognition_diagnostics.is_empty() {
                    a.controller.reason = format!(
                        "Recognition needs repair: {}",
                        recognition_diagnostics.join("; ")
                    );
                }
                executor
                    .poll_attack(backend.as_mut(), &stop, generation)
                    .map_err(|e| e.to_string())?;
                snapshot.buff_presence = states.clone();
                snapshot.buff_last_active_age_seconds = a.buffs.last_active_ages(now);
                snapshot.recognition_evidence = Some(RecognitionEvidenceSource {
                    key: a.key,
                    revision: a.revision,
                    frame_id: captured.id,
                    captured_at: captured.captured_at,
                    geometry: captured.geometry,
                });
                let mut observation = metin_core::Observation {
                    ready: p.ready(),
                    candidates: stable,
                    panel_present: score >= p.panel.threshold,
                    reason: "Incomplete calibration.".into(),
                    buffs: states,
                    safety_scores: safety_scores.clone(),
                };
                a.controller.tick(origin.elapsed().as_secs_f64());
                if a.controller.running
                    && captured.captured_at.elapsed() > Duration::from_millis(500)
                {
                    failure_cause = PauseCause::StaleFrame;
                    return Err("The game frame became stale during recognition. Resume with a fresh capture.".into());
                }
                if a.controller.running {
                    let verified = backend.inspect().map_err(|e| e.to_string())?;
                    if !captured.geometry.foreground || !verified.foreground {
                        failure_cause = PauseCause::FocusLost;
                        return Err("Game focus changed after capture. Activate the selected game before starting again.".into());
                    }
                    if verified != captured.geometry {
                        failure_cause = PauseCause::WindowChanged;
                        return Err("Game window geometry changed after capture. Review its position and calibration before starting again.".into());
                    }
                }
                if a.controller.running {
                    // WGC also sees client pixels behind the control panel or
                    // another window. Choose new targets only at clear screen
                    // points; the final input guard still checks again before
                    // mouse-down and protects already selected targets.
                    observation.candidates.retain(|candidate| {
                        let x = captured
                            .geometry
                            .left
                            .checked_add(candidate.point[0] as i32);
                        let y = captured.geometry.top.checked_add(candidate.point[1] as i32);
                        match (x, y) {
                            (Some(x), Some(y)) => backend.point_clear([x, y]),
                            _ => false,
                        }
                    });
                }
                if let Some(action) = a
                    .controller
                    .step(origin.elapsed().as_secs_f64(), &observation)
                {
                    tracing::info!(action_id=action.id, kind=?action.kind, frame_id=captured.id, panel_score=score, "Executing guarded action");
                    executor.set_deadline(
                        a.controller
                            .input_budget(origin.elapsed().as_secs_f64())
                            .map(|left| Instant::now() + Duration::from_secs_f64(left)),
                    );
                    let visual_rule = a.controller.visual_rule_for_input(&action).cloned();
                    // Input preparation can move/hover the cursor. Recheck the visual
                    // precondition immediately before down, using a new captured frame.
                    let mut verify=|backend:&mut dyn metin_platform::PlatformBackend| -> metin_platform::Result<()> {
                        let rule=visual_rule.as_ref().expect("visual verifier only");
                        let fresh=backend.capture(Instant::now()+Duration::from_millis(500),&stop,generation)?;
                        if fresh.geometry != captured.geometry || [fresh.frame.width,fresh.frame.height] != [captured.frame.width,captured.frame.height] {
                            return Err(metin_platform::PlatformError("Window changed while verifying visual input.".into()));
                        }
                        let scores=a.safety.observe(&fresh.frame).map_err(|e|metin_platform::PlatformError(e.to_string()))?;
                        if !scores.iter().any(|(id,score)|*id==rule.id && score.is_finite() && *score>=rule.threshold) {
                            return Err(metin_platform::PlatformError("Visual event changed before input. Check the game before resuming.".into()));
                        }
                        Ok(())
                    };
                    let action_result = executor.execute_verified(
                        &action,
                        &a.controller.profile,
                        backend.as_mut(),
                        &stop,
                        generation,
                        if visual_rule.is_some() {
                            Some(&mut verify)
                        } else {
                            None
                        },
                    );
                    let completed = origin.elapsed().as_secs_f64();
                    if matches!(action.kind, ActionKind::Click { .. }) && executor.click_sent() {
                        a.controller.session.clicks += 1;
                    }
                    a.controller
                        .ack(completed, action.id, action_result.is_ok())
                        .map_err(|e| e.to_string())?;
                    a.controller.tick(completed);
                    if let Err(error) = action_result {
                        tracing::warn!(action_id=action.id, %error, "Action interrupted");
                        return Err(error.to_string());
                    }
                    tracing::info!(action_id=action.id, mounted=?a.controller.buffs.mounted, "Action completed");
                    a.tracker.reset();
                }
                snapshot.selected_target = a.controller.selected_target();
                snapshot.next_target = a.controller.next_preview_target(&observation.candidates);
                publish_captured_frame(&mut snapshot, captured);
                snapshot.candidates = candidates;
                snapshot.panel_score = score;
                snapshot.panel_match = panel_match;
                snapshot.safety_scores = safety_scores;
                Ok(())
            })();
            let failed = result.is_err();
            if let Err(error) = result {
                snapshot.pause_info = Some(PauseInfo {
                    cause: failure_cause,
                    details: error.clone(),
                });
                snapshot.start_request = None;
                backend.stop_capture();
                snapshot.geometry = None;
                snapshot.released_generation = None;
                pause_after_failure(
                    &mut a.controller,
                    origin.elapsed().as_secs_f64(),
                    &error,
                    armed.is_some(),
                );
                armed = None;
                let _ = executor.release(backend.as_mut());
            }
            snapshot.key = Some(a.key);
            snapshot.revision = a.revision;
            snapshot.applied_revision = a.applied_revision;
            snapshot.calibration_review = a.calibration_review;
            snapshot.running = a.controller.running;
            snapshot.update_activity(
                Some(&a.controller),
                armed.is_some(),
                failed,
                origin.elapsed().as_secs_f64(),
            );
            snapshot.mounted = a.controller.buffs.mounted;
            snapshot.statistics = a.controller.session.snapshot();
            snapshot.action_timers = a.controller.action_timers();
            snapshot.activity_progress = a
                .controller
                .activity_progress(origin.elapsed().as_secs_f64());
            snapshot.attack_remaining = if a.controller.phase == metin_core::Phase::OutOfTarget {
                a.controller.remaining(origin.elapsed().as_secs_f64())
            } else {
                None
            };
            snapshot.recoveries = a.controller.recoveries;
            snapshot.schedule = a.controller.schedule(origin.elapsed().as_secs_f64());
            for event in &a.controller.events {
                if logged_event.is_none_or(|(key, id)| key != a.key || event.id > id) {
                    tracing::info!(event_id=event.id, activity=%event.activity, message=%event.message, "Session event");
                    if event.notify
                        && (a.controller.profile.safety.sound
                            || a.controller.profile.safety.system_notification)
                        && let Err(error) = notifications.send(crate::notifications::Notice {
                            title: format!("{}: {}", crate::PRODUCT_NAME, event.activity),
                            body: event.message.clone(),
                            sound: a.controller.profile.safety.sound,
                            system: a.controller.profile.safety.system_notification,
                        })
                    {
                        tracing::warn!(%error,"Notification was not queued");
                    }
                    logged_event = Some((a.key, event.id));
                }
            }
            snapshot.events = a.controller.events.iter().cloned().collect();
            snapshot.timers = a
                .controller
                .buffs
                .skill_remaining(origin.elapsed().as_secs_f64(), !a.controller.is_active());
            snapshot.state = if armed.is_some() {
                "Armed".into()
            } else {
                a.controller.state.clone()
            };
            snapshot.reason = if armed.is_some() {
                format!(
                    "Waiting for stable game focus. Start delay: {} s; timeout: {} s.",
                    ARM_START_DELAY.as_secs(),
                    ARM_FOCUS_TIMEOUT.as_secs()
                )
            } else {
                a.controller.reason.clone()
            };
        }
        if let Some(a) = &attached {
            snapshot.selected_target = a.controller.selected_target();
        }
        if !snapshot.running {
            snapshot.selected_target = None;
            snapshot.next_target = None;
        }
        snapshot.arming = armed.is_some();
        if snapshot.applied_revision != Some(snapshot.revision) {
            snapshot.candidates.clear();
            snapshot.selected_target = None;
            snapshot.next_target = None;
            snapshot.panel_score = 0.;
            snapshot.panel_match = None;
            snapshot.recognition_evidence = None;
            snapshot.buff_presence.clear();
            snapshot.buff_last_active_age_seconds.clear();
            snapshot.safety_scores.clear();
            snapshot.timers.clear();
            snapshot.action_timers.clear();
            snapshot.active_skill = None;
            snapshot.activity_progress = None;
        }
        if snapshot.start_request.is_some_and(|request| {
            !request.matches(snapshot.key, snapshot.revision, stop.generation())
        }) {
            snapshot.start_request = None;
        }
        if let Some(started) = armed {
            snapshot.update_activity(None, true, false, now);
            snapshot.activity_progress = Some(arming_progress(started.elapsed()));
        }
        if !snapshot.recognition_evidence_is_fresh() {
            snapshot.recognition_evidence = None;
            snapshot.buff_presence.clear();
            snapshot.buff_last_active_age_seconds.clear();
            if let Some(a) = &mut attached {
                a.buffs.reset();
            }
        }
        snapshot.notification_status = notifications.status();
        snapshot.capabilities = backend.capabilities();
        if snapshot.key != last_published.key
            || snapshot.state != last_published.state
            || snapshot.reason != last_published.reason
            || snapshot.mounted != last_published.mounted
        {
            tracing::info!(state=%snapshot.state, reason=%snapshot.reason, mounted=?snapshot.mounted, frame_id=snapshot.frame_id, panel_score=snapshot.panel_score, candidates=snapshot.candidates.len(), targets_ended=snapshot.statistics.completed, "Runtime state");
        }
        if last_publish_time.elapsed() >= Duration::from_millis(200) && snapshot != last_published {
            publish(&snapshot);
            last_published = snapshot.clone();
            last_publish_time = Instant::now();
        }
    }
    let released = executor.release(backend.as_mut());
    backend.stop_capture();
    if let Err(error) = released {
        snapshot.arming = false;
        snapshot.state = "Release error".into();
        snapshot.update_activity(None, false, true, origin.elapsed().as_secs_f64());
        snapshot.reason = error.to_string();
        snapshot.running = false;
        publish(&snapshot);
        return Err(error.to_string());
    }
    Ok(())
}

fn arming_progress(elapsed: Duration) -> metin_core::ActivityProgress {
    let (label, stage_elapsed, stage_duration) = if elapsed < ARM_START_DELAY {
        ("Starting", elapsed, ARM_START_DELAY)
    } else {
        (
            "Waiting for game focus",
            elapsed.saturating_sub(ARM_START_DELAY),
            ARM_FOCUS_TIMEOUT.saturating_sub(ARM_START_DELAY),
        )
    };
    metin_core::ActivityProgress {
        label: label.into(),
        elapsed_seconds: stage_elapsed.as_secs_f64(),
        remaining_seconds: Some(stage_duration.saturating_sub(stage_elapsed).as_secs_f64()),
        total_seconds: Some(stage_duration.as_secs_f64()),
    }
}

#[cfg(test)]
mod schedule_tests {
    use super::*;
    #[test]
    fn arm_envelope_keeps_exact_authority_when_f8_follows_the_enqueue_check() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let runtime = RuntimeHandle {
            sender,
            stop: StopToken::default(),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            latest: Arc::new(Mutex::new(Arc::new(RuntimeSnapshot::default()))),
            thread: Mutex::new(None),
        };
        let key = SessionKey {
            profile_id: Uuid::new_v4(),
            target: TargetToken {
                handle: 1,
                pid: 2,
                created: 3,
            },
        };
        let ticket = runtime.stop_generation();
        runtime.arm_in_generation(key, ticket).unwrap();
        runtime.stop();
        let envelope = receiver.try_recv().unwrap();
        assert_eq!(envelope.generation, ticket);
        assert!(runtime.stop.cancelled(envelope.generation));
        assert!(matches!(envelope.command,Command::Arm {key:queued} if queued==key));
        assert!(runtime.arm_in_generation(key, ticket).is_err());
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn explicit_start_failures_notify_once_per_attempt_while_paused_repair_stays_silent() {
        let mut controller = Controller::new(Profile::create("Failure presentation"));
        pause_after_failure(
            &mut controller,
            0.,
            "Template is larger than search region.",
            false,
        );
        assert!(!controller.events.back().unwrap().notify);
        let before = controller.events.back().unwrap().id;
        pause_after_failure(
            &mut controller,
            1.,
            "Template is larger than search region.",
            true,
        );
        assert!(controller.events.back().unwrap().notify);
        assert!(controller.events.back().unwrap().id > before);
        let first = controller.events.back().unwrap().id;
        pause_after_failure(
            &mut controller,
            2.,
            "Template is larger than search region.",
            true,
        );
        assert!(controller.events.back().unwrap().id > first);
        assert!(controller.events.back().unwrap().notify);
    }
    #[test]
    fn arming_countdown_switches_from_start_delay_to_remaining_focus_timeout() {
        for (elapsed, label, stage_elapsed, remaining, total) in [
            (0., "Starting", 0., 3., 3.),
            (2.5, "Starting", 2.5, 0.5, 3.),
            (3., "Waiting for game focus", 0., 9., 9.),
            (6.5, "Waiting for game focus", 3.5, 5.5, 9.),
            (12., "Waiting for game focus", 9., 0., 9.),
            (13., "Waiting for game focus", 10., 0., 9.),
        ] {
            let progress = arming_progress(Duration::from_secs_f64(elapsed));
            assert_eq!(progress.label, label);
            assert_eq!(progress.remaining_seconds, Some(remaining));
            assert_eq!(progress.total_seconds, Some(total));
            assert_eq!(progress.elapsed_seconds, stage_elapsed);
        }
    }
    #[test]
    fn presentation_clears_active_skill_for_arming_errors_and_detachment() {
        let mut profile = Profile::create("Presentation");
        profile.buffs.enabled = false;
        let mut controller = Controller::new(profile);
        controller.start(0.).unwrap();
        controller.phase = metin_core::Phase::Scheduled;
        let mut snapshot = RuntimeSnapshot::default();
        snapshot.update_activity(Some(&controller), false, false, 0.);
        assert_eq!(snapshot.activity, metin_core::ActivityKind::Periodic);
        snapshot.active_skill = Some(3);
        snapshot.update_activity(Some(&controller), true, false, 0.);
        assert_eq!(snapshot.activity, metin_core::ActivityKind::Starting);
        assert_eq!(snapshot.active_skill, None);
        snapshot.active_skill = Some(3);
        snapshot.update_activity(Some(&controller), false, true, 0.);
        assert_eq!(snapshot.activity, metin_core::ActivityKind::Error);
        assert_eq!(snapshot.active_skill, None);
        snapshot.update_activity(None, false, false, 0.);
        assert_eq!(snapshot.activity, metin_core::ActivityKind::Stopped);
        assert_eq!(snapshot.active_skill, None);
        controller.pause(1., "Stopped");
        snapshot.update_activity(Some(&controller), false, false, 1.);
        assert_eq!(snapshot.activity, metin_core::ActivityKind::Paused);
    }
    #[test]
    fn full_reset_queue_still_cancels_input_but_never_reports_a_completed_reset() {
        let (sender, _receiver) = mpsc::sync_channel(0);
        let key = SessionKey {
            profile_id: Uuid::new_v4(),
            target: TargetToken {
                handle: 7,
                pid: 8,
                created: 9,
            },
        };
        let mut snapshot = RuntimeSnapshot {
            key: Some(key),
            ..Default::default()
        };
        snapshot.statistics.completed = 4;
        snapshot.schedule.break_ends_in = Some(6.);
        let runtime = RuntimeHandle {
            sender,
            stop: StopToken::default(),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            latest: Arc::new(Mutex::new(Arc::new(snapshot.clone()))),
            thread: Mutex::new(None),
        };
        let wrong = SessionKey {
            profile_id: Uuid::new_v4(),
            ..key
        };
        assert!(runtime.send(Command::ResetSession { key: wrong }).is_err());
        assert_eq!(runtime.stop_generation(), 0);
        assert!(
            runtime
                .send(Command::ResetSession { key })
                .unwrap_err()
                .contains("queue is full")
        );
        assert_eq!(runtime.stop_generation(), 1);
        assert!(*runtime.latest().unwrap() == snapshot);
    }
    #[test]
    fn clock_time_uses_next_occurrence_and_rejects_invalid_time() {
        use chrono::TimeZone;
        let now = chrono::Utc
            .with_ymd_and_hms(2026, 10, 16, 23, 59, 30)
            .unwrap();
        assert_eq!(next_clock_delay(now, 0).unwrap(), 30.);
        assert_eq!(next_clock_delay(now, 23 * 60 + 59).unwrap(), 86370.);
        assert!(next_clock_delay(now, 1440).is_err());
    }
}
