//! Single owner of the live controller, timers and input executor.
use metin_core::{ActionKind, Candidate, Controller, Profile, Statistics};
use metin_platform::{
    CapabilityReport, Frame, InputExecutor, PlatformBackend, StopToken, TargetInfo, TargetToken,
};
use metin_vision::{BuffDetector, CandidateTracker};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionKey {
    pub profile_id: Uuid,
    pub target: TargetToken,
}
pub enum Command {
    RefreshTargets,
    Configure {
        profile: Box<Profile>,
        template: Option<Frame>,
        target: TargetToken,
        mounted: bool,
        revision: u64,
    },
    Detach,
    Arm {
        key: SessionKey,
    },
    DeclareMount {
        key: SessionKey,
        mounted: bool,
        revision: u64,
    },
    ResetStatistics {
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
    pub frame: Option<Frame>,
    pub frame_id: u64,
    pub captured_at: Option<Instant>,
    pub candidates: Vec<Candidate>,
    pub selected_target: Option<[u32; 2]>,
    pub next_target: Option<[u32; 2]>,
    pub panel_score: f64,
    pub state: String,
    pub reason: String,
    pub running: bool,
    pub arming: bool,
    pub mounted: Option<bool>,
    pub timers: Vec<Option<f64>>,
    pub statistics: Statistics,
    pub targets: Vec<TargetInfo>,
    pub capabilities: CapabilityReport,
}
impl Default for RuntimeSnapshot {
    fn default() -> Self {
        Self {
            key: None,
            revision: 0,
            frame: None,
            frame_id: 0,
            captured_at: None,
            candidates: vec![],
            selected_target: None,
            next_target: None,
            panel_score: 0.,
            state: "Stopped".into(),
            reason: "Select a profile and game window.".into(),
            running: false,
            arming: false,
            mounted: Some(false),
            timers: vec![],
            statistics: Statistics::default(),
            targets: vec![],
            capabilities: CapabilityReport::default(),
        }
    }
}
struct Envelope {
    generation: u64,
    command: Command,
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
        self.sender
            .try_send(Envelope {
                generation: self.stop.generation(),
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
    key: SessionKey,
    controller: Controller,
    template: Option<Frame>,
    tracker: CandidateTracker,
    buffs: BuffDetector,
    revision: u64,
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
    let publish = |s: &RuntimeSnapshot| {
        if let Ok(mut value) = output.lock() {
            *value = Arc::new(s.clone());
        }
        notify();
    };
    while !shutdown.load(Ordering::SeqCst) {
        let now = origin.elapsed().as_secs_f64();
        let mut generation = stop.generation();
        if observed_generation != generation {
            observed_generation = generation;
            armed = None;
            if let Some(a) = &mut attached {
                a.controller.pause(now, "Paused (F8 / Pause).");
            }
            if let Err(e) = executor.release(backend.as_mut()) {
                snapshot.reason = e.to_string();
            }
            next_sample = Instant::now();
        }
        match receiver.recv_timeout(Duration::from_millis(20)) {
            Ok(envelope) if envelope.generation == stop.generation() => {
                // Retire cancellation received while the channel was asleep
                // before applying a valid command from the new generation.
                if generation != envelope.generation {
                    generation = envelope.generation;
                    observed_generation = generation;
                    armed = None;
                    if let Some(a) = &mut attached {
                        a.controller.pause(now, "Paused (F8 / Pause).");
                    }
                    if let Err(error) = executor.release(backend.as_mut()) {
                        snapshot.reason = error.to_string();
                    }
                    next_sample = Instant::now();
                }
                let requested_key = match &envelope.command {
                    Command::Arm { key }
                    | Command::DeclareMount { key, .. }
                    | Command::ResetStatistics { key }
                    | Command::ResetSkillTimer { key, .. } => Some(*key),
                    _ => None,
                };
                if requested_key.is_some_and(|key| attached.as_ref().is_none_or(|a| a.key != key)) {
                    tracing::warn!("Discarding command for an obsolete game/profile selection");
                    continue;
                }
                let result = (|| -> Result<(), String> {
                    match envelope.command {
                        Command::RefreshTargets => {
                            snapshot.targets = backend.list_targets().map_err(|e| e.to_string())?;
                        }
                        Command::Detach => {
                            armed = None;
                            executor
                                .release(backend.as_mut())
                                .map_err(|e| e.to_string())?;
                            attached = None;
                            snapshot.key = None;
                            snapshot.frame = None;
                            snapshot.candidates.clear();
                            snapshot.running = false;
                            snapshot.state = "Stopped".into();
                            snapshot.reason = "Select a game window.".into();
                        }
                        Command::Configure {
                            profile,
                            template,
                            target,
                            mounted,
                            revision,
                        } => {
                            profile.validate().map_err(|e| e.to_string())?;
                            let key = SessionKey {
                                profile_id: profile.id,
                                target,
                            };
                            armed = None;
                            executor
                                .release(backend.as_mut())
                                .map_err(|e| e.to_string())?;
                            let detector = BuffDetector::new(
                                profile.buffs.clone(),
                                profile.frame_size.unwrap_or([1, 1]),
                                profile.timing.sample_interval,
                            )
                            .map_err(|e| e.to_string())?;
                            let tracker = CandidateTracker::new(
                                profile.color.as_ref().map_or(3, |c| c.stable_frames),
                                profile.timing.sample_interval,
                            );
                            let previous = attached.take();
                            snapshot.key = None;
                            snapshot.frame = None;
                            snapshot.running = false;
                            backend.attach(target).map_err(|e| e.to_string())?;
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
                                    c.profile = *profile;
                                    c
                                } else {
                                    let mut c = Controller::new(*profile);
                                    c.buffs.mounted = Some(mounted);
                                    c
                                };
                            controller.running = false;
                            attached = Some(Attached {
                                key,
                                controller,
                                template,
                                tracker,
                                buffs: detector,
                                revision,
                            });
                            next_sample = Instant::now();
                        }
                        Command::Arm { key } => {
                            let a = attached
                                .as_ref()
                                .filter(|a| a.key == key)
                                .ok_or("The game/profile selection changed.")?;
                            if !a.controller.profile.ready() {
                                return Err("Complete the calibration before starting.".into());
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
                            a.revision = revision;
                        }
                        Command::ResetStatistics { key } => {
                            attached
                                .as_mut()
                                .filter(|a| a.key == key)
                                .ok_or("Stale statistics request.")?
                                .controller
                                .session
                                .reset();
                        }
                        Command::ResetSkillTimer { key, index } => {
                            let a = attached
                                .as_mut()
                                .filter(|a| a.key == key)
                                .ok_or("Stale skill timer request.")?;
                            a.controller
                                .buffs
                                .reset_timer(index, now, !a.controller.running)
                                .map_err(|e| e.to_string())?;
                        }
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    snapshot.arming = armed.is_some();
                    snapshot.state = "Error".into();
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
            let result = (|| -> Result<(), String> {
                let events = hotkeys.map_err(|e| e.to_string())?;
                if events.stop {
                    stop.stop();
                    a.controller.pause(now, "Emergency stop (F8).");
                    armed = None;
                    executor
                        .release(backend.as_mut())
                        .map_err(|e| e.to_string())?;
                }
                let state = backend.inspect().map_err(|e| e.to_string())?;
                if state.foreground {
                    focus_since.get_or_insert(Instant::now());
                } else {
                    focus_since = None;
                }
                if a.controller.running && !state.foreground {
                    a.controller
                        .pause(now, "Game lost focus. Press F7 in the game to resume.");
                    armed = None;
                }
                if events.toggle && !events.stop && state.foreground {
                    armed = None;
                    if a.controller.running {
                        a.controller.pause(now, "Paused (F7).");
                    } else {
                        if !a.controller.profile.ready() {
                            return Err("Complete calibration before starting.".into());
                        }
                        a.controller.start(now).map_err(|e| e.to_string())?;
                        a.tracker.reset();
                        a.buffs.reset();
                    }
                    next_sample = Instant::now();
                }
                if let Some(since) = armed {
                    if since.elapsed() >= Duration::from_secs(12) {
                        armed = None;
                        a.controller.pause(now,"The game did not reach stable focus within 12 s. Activate it and press F7.");
                    } else if since.elapsed() >= Duration::from_secs(3)
                        && focus_since
                            .is_some_and(|focus| focus.elapsed() >= Duration::from_millis(500))
                    {
                        a.controller.start(now).map_err(|e| e.to_string())?;
                        a.tracker.reset();
                        a.buffs.reset();
                        armed = None;
                        next_sample = Instant::now();
                    }
                }
                if Instant::now() < next_sample {
                    return Ok(());
                }
                next_sample = Instant::now()
                    + Duration::from_secs_f64(a.controller.profile.timing.sample_interval);
                let captured = backend
                    .capture(Instant::now() + Duration::from_secs(2), &stop, generation)
                    .map_err(|e| e.to_string())?;
                let now = captured.captured_at.duration_since(origin).as_secs_f64();
                let p = &a.controller.profile;
                if a.controller.running
                    && p.frame_size != Some([captured.frame.width, captured.frame.height])
                {
                    return Err("The game dimensions changed. Recalibrate before resuming.".into());
                }
                let candidates = metin_vision::detect_candidates(&captured.frame, p)
                    .map_err(|e| e.to_string())?;
                let score = if let (Some(template), Some(region)) = (&a.template, p.panel.region) {
                    metin_vision::panel_score(&captured.frame, template, region)
                        .map_err(|e| e.to_string())?
                } else {
                    0.
                };
                let stable = a.tracker.update(candidates.clone(), now);
                let states = a
                    .buffs
                    .observe(&captured.frame, now)
                    .map_err(|e| e.to_string())?;
                let observation = metin_core::Observation {
                    ready: p.ready(),
                    candidates: stable,
                    panel_present: score >= p.panel.threshold,
                    reason: "Incomplete calibration.".into(),
                    buffs: states,
                };
                a.controller.session.advance(now, a.controller.running);
                if a.controller.running
                    && backend.inspect().map_err(|e| e.to_string())? != captured.geometry
                {
                    return Err("Window geometry or focus changed after capture.".into());
                }
                if let Some(action) = a.controller.step(now, &observation) {
                    tracing::info!(action_id=action.id, kind=?action.kind, frame_id=captured.id, panel_score=score, "Executing guarded action");
                    let action_result = executor.execute(
                        &action,
                        &a.controller.profile,
                        backend.as_mut(),
                        &stop,
                        generation,
                    );
                    let completed = origin.elapsed().as_secs_f64();
                    a.controller
                        .session
                        .advance(completed, a.controller.running);
                    if matches!(action.kind, ActionKind::Click { .. }) && executor.click_sent() {
                        a.controller.session.clicks += 1;
                    }
                    a.controller
                        .ack(completed, action.id, action_result.is_ok())
                        .map_err(|e| e.to_string())?;
                    if let Err(error) = action_result {
                        tracing::warn!(action_id=action.id, %error, "Action interrupted");
                        return Err(error.to_string());
                    }
                    tracing::info!(action_id=action.id, mounted=?a.controller.buffs.mounted, "Action completed");
                    a.tracker.reset();
                }
                snapshot.selected_target = a.controller.selected_target();
                snapshot.next_target = a.controller.next_preview_target(&observation.candidates);
                snapshot.frame = Some(captured.frame);
                snapshot.frame_id = captured.id;
                snapshot.captured_at = Some(captured.captured_at);
                snapshot.candidates = candidates;
                snapshot.panel_score = score;
                Ok(())
            })();
            if let Err(error) = result {
                a.controller.pause(origin.elapsed().as_secs_f64(), error);
                armed = None;
                let _ = executor.release(backend.as_mut());
            }
            snapshot.key = Some(a.key);
            snapshot.revision = a.revision;
            snapshot.running = a.controller.running;
            snapshot.mounted = a.controller.buffs.mounted;
            snapshot.statistics = a.controller.session.snapshot();
            snapshot.timers = a
                .controller
                .buffs
                .skill_remaining(origin.elapsed().as_secs_f64(), !a.controller.running);
            snapshot.state = if armed.is_some() {
                "Armed".into()
            } else {
                a.controller.state.clone()
            };
            snapshot.reason = if armed.is_some() {
                "Waiting for stable game focus. Start delay: 3 s; timeout: 12 s.".into()
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
    if let Err(error) = executor.release(backend.as_mut()) {
        snapshot.arming = false;
        snapshot.state = "Release error".into();
        snapshot.reason = error.to_string();
        snapshot.running = false;
        publish(&snapshot);
        return Err(error.to_string());
    }
    Ok(())
}
