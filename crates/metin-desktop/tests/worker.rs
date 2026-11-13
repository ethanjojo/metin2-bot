use metin_core::Profile;
use metin_desktop::runtime::SessionKey;
use metin_desktop::runtime::{Command, RuntimeHandle};
use metin_platform::*;
use std::{
    sync::{Arc, Barrier, Mutex},
    time::{Duration, Instant},
};
#[derive(Default)]
struct Controls {
    hotkey_polls: usize,
    hotkey_barrier: Option<Arc<Barrier>>,
    toggle: bool,
    stop: bool,
    fail_key: bool,
    fail_release: bool,
    lose_focus: bool,
    fail_capture: bool,
    stale_capture: bool,
    focus_change_during_capture: bool,
    fail_attach: bool,
    captures: usize,
    capture_active: bool,
    capture_limits: Vec<Duration>,
    calls: Vec<(u16, bool)>,
    colored_candidates: bool,
    hp_detail: Option<[u32; 2]>,
    frame_override: Option<Frame>,
    target_origin: [i32; 2],
    target_size: Option<[u32; 2]>,
    cursor: [i32; 2],
    obscure_nearest: bool,
    obscure_all: bool,
    obscure_on_hover: bool,
    clicks: Vec<[u32; 2]>,
}
struct Fake {
    controls: Arc<Mutex<Controls>>,
    target: TargetToken,
    id: u64,
}
fn token(n: usize) -> TargetToken {
    TargetToken {
        handle: n,
        pid: n as u32,
        created: n as u64,
    }
}
impl PlatformBackend for Fake {
    fn capabilities(&self) -> CapabilityReport {
        CapabilityReport {
            capture: true,
            identity: true,
            foreground: true,
            obstruction: true,
            manual_input: true,
            input: true,
            emergency_stop: self.controls.lock().unwrap().hotkey_polls > 0,
            reason: String::new(),
        }
    }
    fn list_targets(&mut self) -> Result<Vec<TargetInfo>> {
        Ok(vec![])
    }
    fn attach(&mut self, target: TargetToken) -> Result<()> {
        if self.controls.lock().unwrap().fail_attach {
            return Err(PlatformError("Injected attachment failure".into()));
        }
        self.target = target;
        Ok(())
    }
    fn inspect(&mut self) -> Result<TargetState> {
        let c = self.controls.lock().unwrap();
        Ok(TargetState {
            target: self.target,
            left: c.target_origin[0],
            top: c.target_origin[1],
            width: c.target_size.map_or(200, |size| size[0]),
            height: c.target_size.map_or(200, |size| size[1]),
            visible: true,
            foreground: !c.lose_focus,
        })
    }
    fn stop_capture(&mut self) {
        self.controls.lock().unwrap().capture_active = false;
    }
    fn request_activation(&mut self) -> Result<bool> {
        Ok(true)
    }
    fn capture(&mut self, deadline: Instant, _: &StopToken, _: u64) -> Result<CapturedFrame> {
        {
            let mut c = self.controls.lock().unwrap();
            c.captures += 1;
            c.capture_active = true;
            c.capture_limits
                .push(deadline.saturating_duration_since(Instant::now()));
            if c.fail_capture {
                return Err(PlatformError("Injected capture failure".into()));
            }
            if c.focus_change_during_capture {
                c.focus_change_during_capture = false;
                c.lose_focus = !c.lose_focus;
                return Err(PlatformError("Window focus changed during capture".into()));
            }
        }
        self.id += 1;
        let mut pixels: Vec<_> = (0..200 * 200 * 3).map(|n| 128 + (n % 31) as u8).collect();
        if self.controls.lock().unwrap().colored_candidates {
            pixels.fill(35);
            for (left, top) in [(80, 80), (140, 130)] {
                for y in top..top + 24 {
                    for x in left..left + 20 {
                        let offset = (y * 200 + x) as usize * 3;
                        pixels[offset..offset + 3].copy_from_slice(&[0, 220, 0]);
                    }
                }
            }
        }
        if let Some([left, top]) = self.controls.lock().unwrap().hp_detail {
            let template = hp_template();
            for y in 0..template.height {
                for x in 0..template.width {
                    let offset = ((top + y) * 200 + left + x) as usize * 3;
                    let source = (y * template.width + x) as usize * 3;
                    pixels[offset..offset + 3].copy_from_slice(&template.bgr[source..source + 3]);
                }
            }
        }
        let frame = self
            .controls
            .lock()
            .unwrap()
            .frame_override
            .clone()
            .unwrap_or(Frame::new(200, 200, pixels)?);
        Ok(CapturedFrame {
            frame,
            target: self.target,
            captured_at: if self.controls.lock().unwrap().stale_capture {
                Instant::now() - Duration::from_secs(1)
            } else {
                Instant::now()
            },
            id: self.id,
            geometry: self.inspect()?,
        })
    }
    fn check_input_access(&mut self) -> Result<()> {
        Ok(())
    }
    fn key_held(&self, _: u16) -> bool {
        false
    }
    fn point_clear(&self, point: [i32; 2]) -> bool {
        let c = self.controls.lock().unwrap();
        let client_x = point[0] - c.target_origin[0];
        !(c.obscure_all || (c.obscure_nearest && (80..100).contains(&client_x)))
    }
    fn cursor(&self) -> Result<[i32; 2]> {
        Ok(self.controls.lock().unwrap().cursor)
    }
    fn move_relative(&mut self, delta: [i32; 2]) -> Result<()> {
        let mut c = self.controls.lock().unwrap();
        c.cursor[0] += delta[0];
        c.cursor[1] += delta[1];
        if c.obscure_on_hover {
            c.obscure_all = true;
        }
        Ok(())
    }
    fn key(&mut self, key: u16, down: bool, _: bool) -> Result<()> {
        let mut c = self.controls.lock().unwrap();
        c.calls.push((key, down));
        if !down && c.fail_release {
            Err(PlatformError("Injected release failure".into()))
        } else if down && c.fail_key {
            Err(PlatformError("Injected key failure".into()))
        } else {
            Ok(())
        }
    }
    fn mouse(&mut self, point: [u32; 2], down: bool) -> Result<()> {
        if down {
            self.controls.lock().unwrap().clicks.push(point);
        }
        Ok(())
    }
    fn hotkeys(&mut self) -> Result<HotkeyEvents> {
        let barrier = self.controls.lock().unwrap().hotkey_barrier.take();
        if let Some(barrier) = barrier {
            barrier.wait();
            barrier.wait();
        }
        let mut c = self.controls.lock().unwrap();
        c.hotkey_polls += 1;
        let events = HotkeyEvents {
            toggle: c.toggle,
            stop: c.stop,
        };
        c.toggle = false;
        c.stop = false;
        Ok(events)
    }
}
fn runtime() -> (RuntimeHandle, Arc<Mutex<Controls>>) {
    let controls = Arc::new(Mutex::new(Controls::default()));
    (
        RuntimeHandle::spawn_with_backend(
            Box::new(Fake {
                controls: controls.clone(),
                target: token(1),
                id: 0,
            }),
            || {},
        ),
        controls,
    )
}
#[test]
fn hotkey_capabilities_are_initialized_before_a_window_is_selected() {
    let (r, controls) = runtime();
    wait(&r, |s| s.capabilities.automation_ready());
    assert!(r.latest().unwrap().key.is_none());
    assert_eq!(r.latest().unwrap().targets_revision, 0);
    r.send(Command::RefreshTargets).unwrap();
    wait(&r, |s| s.targets_revision == 1);
    r.send(Command::RefreshTargets).unwrap();
    wait(&r, |s| s.targets_revision == 2);
    assert!(!r.latest().unwrap().running);
    assert!(controls.lock().unwrap().calls.is_empty());
    r.shutdown().unwrap();
}
#[test]
fn f7_requests_the_ui_start_path_without_starting_the_applied_profile() {
    let (runtime, controls) = runtime();
    configure(&runtime, profile(), token(1), false, 1);
    controls.lock().unwrap().toggle = true;
    wait_raw(&runtime, |snapshot| snapshot.start_request.is_some());
    let request = runtime.latest().unwrap().start_request.unwrap();
    assert!(request.matches(runtime.latest().unwrap().key, 1, runtime.stop_generation()));
    assert!(!request.matches(runtime.latest().unwrap().key, 2, runtime.stop_generation()));
    assert!(!request.matches(
        runtime.latest().unwrap().key,
        1,
        runtime.stop_generation() + 1
    ));
    assert!(!request.matches(
        Some(SessionKey {
            target: token(2),
            ..request.key
        }),
        1,
        runtime.stop_generation()
    ));
    assert!(
        !metin_desktop::runtime::StartRequest {
            requested_at: Instant::now() - Duration::from_secs(3),
            ..request
        }
        .matches(runtime.latest().unwrap().key, 1, runtime.stop_generation())
    );
    std::thread::sleep(Duration::from_millis(150));
    assert!(!runtime.latest().unwrap().running && !runtime.latest().unwrap().arming);
    assert!(controls.lock().unwrap().calls.is_empty());
    runtime.stop();
    wait_raw(&runtime, |snapshot| snapshot.start_request.is_none());
    controls.lock().unwrap().toggle = true;
    wait_raw(&runtime, |snapshot| snapshot.start_request.is_some());
    assert!(runtime.latest().unwrap().start_request.unwrap().id > request.id);
    configure(&runtime, profile(), token(2), false, 2);
    assert!(runtime.latest().unwrap().start_request.is_none());
    runtime.shutdown().unwrap();
    let (replacement, controls) = self::runtime();
    configure(&replacement, profile(), token(1), false, 1);
    controls.lock().unwrap().toggle = true;
    wait_raw(&replacement, |snapshot| snapshot.start_request.is_some());
    assert!(replacement.latest().unwrap().start_request.unwrap().id > request.id);
    replacement.shutdown().unwrap();
}
#[test]
fn save_start_ticket_cannot_supersede_a_pause_before_or_after_its_own_stop() {
    let (runtime, controls) = runtime();
    let profile = profile();
    let key = SessionKey {
        profile_id: profile.id,
        target: token(1),
    };
    configure(&runtime, profile, key.target, false, 1);
    let before_save = runtime.stop_generation();
    runtime.stop();
    let paused = runtime.stop_generation();
    assert!(runtime.stop_if_current(before_save).is_err());
    assert_eq!(
        runtime.stop_generation(),
        paused,
        "A cancelled save may not create a newer generation"
    );
    assert!(runtime.arm_in_generation(key, before_save).is_err());
    let own_ticket = runtime.stop_if_current(paused).unwrap();
    assert_eq!(own_ticket, paused + 1);
    runtime.stop();
    assert!(runtime.arm_in_generation(key, own_ticket).is_err());
    wait_raw(&runtime, |snapshot| {
        snapshot.released_generation == Some(runtime.stop_generation())
    });
    assert!(!runtime.latest().unwrap().arming && !runtime.latest().unwrap().running);
    assert!(
        controls
            .lock()
            .unwrap()
            .calls
            .iter()
            .all(|(_, down)| !*down)
    );
    assert!(controls.lock().unwrap().clicks.is_empty());
    runtime.shutdown().unwrap();
}
#[test]
fn current_save_start_ticket_uses_the_existing_arm_countdown() {
    let (runtime, controls) = runtime();
    let profile = profile();
    let key = SessionKey {
        profile_id: profile.id,
        target: token(1),
    };
    configure(&runtime, profile, key.target, false, 1);
    let ticket = runtime.stop_if_current(runtime.stop_generation()).unwrap();
    runtime.arm_in_generation(key, ticket).unwrap();
    wait_raw(&runtime, |snapshot| snapshot.arming);
    assert!(!runtime.latest().unwrap().running);
    runtime.stop();
    wait_raw(&runtime, |snapshot| !snapshot.arming);
    assert!(
        controls
            .lock()
            .unwrap()
            .calls
            .iter()
            .all(|(_, down)| !*down)
    );
    assert!(controls.lock().unwrap().clicks.is_empty());
    runtime.shutdown().unwrap();
}
#[test]
fn connected_revision_cannot_arm_retained_configuration_until_it_is_applied() {
    let (runtime, controls) = runtime();
    let mut profile = candidate_profile();
    profile.buffs.skills.push(Default::default());
    controls.lock().unwrap().colored_candidates = true;
    let key = SessionKey {
        profile_id: profile.id,
        target: token(1),
    };
    configure(&runtime, profile.clone(), key.target, false, 1);
    assert_eq!(runtime.latest().unwrap().applied_revision, Some(1));
    runtime
        .send(Command::SetPreview { key, enabled: true })
        .unwrap();
    wait_raw(&runtime, |snapshot| {
        snapshot.candidates.len() == 2 && snapshot.timers.len() == 1
    });
    let old_frame = runtime.latest().unwrap().frame_id;
    runtime.send(Command::Connect { key, revision: 2 }).unwrap();
    wait_raw(&runtime, |snapshot| snapshot.revision == 2);
    assert_eq!(runtime.latest().unwrap().applied_revision, None);
    assert_eq!(runtime.latest().unwrap().mounted, Some(false));
    runtime
        .send(Command::Snapshot {
            key,
            purpose: metin_desktop::runtime::SnapshotPurpose::UpdateLiveEvidence,
        })
        .unwrap();
    wait_raw(&runtime, |snapshot| {
        snapshot.frame_id > old_frame && snapshot.geometry.is_some()
    });
    let pending = runtime.latest().unwrap();
    assert!(
        pending.frame.is_some(),
        "Raw repair captures remain available"
    );
    assert!(
        pending.candidates.is_empty()
            && pending.timers.is_empty()
            && pending.panel_match.is_none()
            && pending.buff_presence.is_empty(),
        "Old derived recognition/timers cannot represent an unapplied revision"
    );
    runtime.send(Command::Arm { key }).unwrap();
    wait_raw(&runtime, |snapshot| {
        snapshot.reason.contains("Apply the profile configuration")
    });
    assert!(!runtime.latest().unwrap().arming && !runtime.latest().unwrap().running);
    controls.lock().unwrap().toggle = true;
    wait_raw(&runtime, |snapshot| snapshot.start_request.is_some());
    assert!(runtime.latest().unwrap().start_request.unwrap().matches(
        Some(key),
        2,
        runtime.stop_generation()
    ));
    runtime
        .send(Command::DeclareMount {
            key,
            mounted: true,
            revision: 3,
        })
        .unwrap();
    wait_raw(&runtime, |snapshot| {
        snapshot.revision == 3 && snapshot.mounted == Some(true)
    });
    assert_eq!(
        runtime.latest().unwrap().applied_revision,
        None,
        "A mount declaration cannot acknowledge profile configuration"
    );
    configure(&runtime, profile, key.target, true, 3);
    assert_eq!(runtime.latest().unwrap().applied_revision, Some(3));
    runtime
        .arm_in_generation(key, runtime.stop_generation())
        .unwrap();
    wait_raw(&runtime, |snapshot| snapshot.arming);
    runtime.stop();
    wait_raw(&runtime, |snapshot| !snapshot.arming);
    assert!(
        controls
            .lock()
            .unwrap()
            .calls
            .iter()
            .all(|(_, down)| !*down)
    );
    runtime.shutdown().unwrap();
}
fn profile() -> Profile {
    let mut p = Profile::from_json(include_str!(
        "../../../tests/parity/profiles/calibrated.json"
    ))
    .unwrap();
    p.frame_size = Some([200, 200]);
    p.timing.sample_interval = 0.1;
    p
}
fn hp_template() -> Frame {
    Frame::new(10, 10, (0..300).map(|n| (n % 251) as u8).collect()).unwrap()
}

fn configure(r: &RuntimeHandle, p: Profile, target: TargetToken, mounted: bool, revision: u64) {
    let key = SessionKey {
        profile_id: p.id,
        target,
    };
    let new_session = r.latest().unwrap().key != Some(key);
    let template = hp_template();
    r.send(Command::Configure {
        profile: Box::new(p),
        template: Some(template),
        target,
        revision,
    })
    .unwrap();
    wait(r, |s| {
        s.key == Some(key) && s.revision == revision && s.applied_revision == Some(revision)
    });
    if new_session {
        // Fixtures declare their own current state explicitly; Configure no
        // longer supplies a persisted state belonging to another character.
        r.send(Command::DeclareMount {
            key,
            mounted,
            revision,
        })
        .unwrap();
        wait(r, |s| s.mounted == Some(mounted));
    }
}
fn wait(r: &RuntimeHandle, predicate: impl Fn(&metin_desktop::runtime::RuntimeSnapshot) -> bool) {
    // These safety scenarios exercise the controller after a normal UI start.
    // Explicitly model the UI acknowledgement now that F7 only requests it.
    let deadline = Instant::now() + Duration::from_secs(7);
    let mut acknowledged = None;
    while Instant::now() < deadline {
        let snapshot = r.latest().unwrap();
        if let Some(request) = snapshot.start_request
            && acknowledged != Some(request.id)
            && request.matches(snapshot.key, snapshot.revision, r.stop_generation())
        {
            r.send(Command::Arm { key: request.key }).unwrap();
            acknowledged = Some(request.id);
        }
        if predicate(&snapshot) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let last = r.latest().unwrap();
    panic!("Worker condition timed out: {} {}", last.state, last.reason);
}
fn wait_raw(
    r: &RuntimeHandle,
    predicate: impl Fn(&metin_desktop::runtime::RuntimeSnapshot) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline {
        if predicate(&r.latest().unwrap()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let last = r.latest().unwrap();
    panic!("Worker condition timed out: {} {}", last.state, last.reason);
}
#[test]
fn stop_bypasses_full_command_queue_and_shutdown_joins() {
    let (runtime, controls) = runtime();
    for _ in 0..100 {
        let _ = runtime.send(Command::RefreshTargets);
    }
    runtime.stop();
    assert!(runtime.latest().is_ok());
    runtime.shutdown().unwrap();
    assert!(controls.lock().unwrap().calls.is_empty());
}
#[test]
fn simultaneous_f8_and_f7_cannot_start_or_send_input() {
    let (r, controls) = runtime();
    configure(&r, profile(), token(1), false, 1);
    wait(&r, |s| s.revision == 1);
    {
        let mut c = controls.lock().unwrap();
        c.toggle = true;
        c.stop = true;
    }
    wait(&r, |s| s.reason.contains("F8"));
    assert!(!r.latest().unwrap().running);
    assert!(controls.lock().unwrap().calls.is_empty());
    r.shutdown().unwrap();
}
#[test]
fn pause_cancels_pending_start_instead_of_resuming_after_delay() {
    let (r, controls) = runtime();
    let p = profile();
    let key = SessionKey {
        profile_id: p.id,
        target: token(1),
    };
    configure(&r, p, token(1), false, 1);
    wait(&r, |s| s.revision == 1);
    r.send(Command::Arm { key }).unwrap();
    wait(&r, |s| s.state == "Armed");
    assert!(r.latest().unwrap().arming);
    assert_eq!(
        r.latest().unwrap().activity,
        metin_core::ActivityKind::Starting
    );
    assert_eq!(r.latest().unwrap().active_skill, None);
    r.stop();
    wait(&r, |s| s.state != "Armed");
    assert!(!r.latest().unwrap().arming);
    assert_eq!(
        r.latest().unwrap().activity,
        metin_core::ActivityKind::Paused
    );
    assert_eq!(r.latest().unwrap().active_skill, None);
    std::thread::sleep(Duration::from_millis(3200));
    assert!(!r.latest().unwrap().running);
    assert!(controls.lock().unwrap().calls.is_empty());
    r.shutdown().unwrap();
}

#[test]
fn stop_then_new_configuration_is_not_dropped_while_receiver_waits() {
    let (r, _) = runtime();
    let p = profile();
    for revision in 1..=6 {
        std::thread::sleep(Duration::from_millis(5));
        r.stop();
        configure(&r, p.clone(), token(1), false, revision);
        wait(&r, |s| s.revision == revision);
    }
    r.shutdown().unwrap();
}

#[test]
fn calibration_review_blocks_arm_and_f7_but_keeps_snapshot_and_preview_available() {
    let (runtime, controls) = runtime();
    let profile = profile();
    let key = SessionKey {
        profile_id: profile.id,
        target: token(1),
    };
    configure(&runtime, profile, key.target, false, 1);
    runtime.send(Command::Arm { key }).unwrap();
    wait(&runtime, |snapshot| snapshot.arming);
    runtime
        .send(Command::CalibrationReview {
            key,
            revision: 1,
            active: true,
        })
        .unwrap();
    wait(&runtime, |snapshot| snapshot.calibration_review);
    assert!(!runtime.latest().unwrap().arming);
    assert!(!runtime.latest().unwrap().running);
    let before_snapshot = runtime.latest().unwrap().frame_id;
    runtime.send(Command::Arm { key }).unwrap();
    runtime
        .send(Command::Snapshot {
            key,
            purpose: metin_desktop::runtime::SnapshotPurpose::UpdateLiveEvidence,
        })
        .unwrap();
    wait(&runtime, |snapshot| {
        snapshot.frame_id > before_snapshot && snapshot.released_generation.is_some()
    });
    assert!(!runtime.latest().unwrap().arming);
    assert!(!runtime.latest().unwrap().running);
    let before_preview = runtime.latest().unwrap().frame_id;
    controls.lock().unwrap().toggle = true;
    runtime
        .send(Command::SetPreview { key, enabled: true })
        .unwrap();
    wait(&runtime, |snapshot| {
        snapshot.frame_id > before_preview && snapshot.released_generation.is_some()
    });
    let snapshot = runtime.latest().unwrap();
    assert!(snapshot.calibration_review);
    assert!(!snapshot.running && !snapshot.arming);
    assert!(snapshot.released_generation.is_some());
    assert!(controls.lock().unwrap().calls.iter().all(|(_, down)| !down));
    runtime.shutdown().unwrap();
}

#[test]
fn calibration_review_survives_focus_resize_restore_and_stop_generation_changes() {
    let (runtime, controls) = runtime();
    let profile = profile();
    let key = SessionKey {
        profile_id: profile.id,
        target: token(1),
    };
    configure(&runtime, profile, key.target, false, 1);
    runtime
        .send(Command::CalibrationReview {
            key,
            revision: 1,
            active: true,
        })
        .unwrap();
    wait(&runtime, |snapshot| snapshot.calibration_review);
    for (focus_lost, size) in [(true, [240, 200]), (false, [240, 200]), (false, [200, 200])] {
        {
            let mut control = controls.lock().unwrap();
            control.lose_focus = focus_lost;
            control.target_size = Some(size);
            control.toggle = true;
        }
        let previous = runtime.latest().unwrap().frame_id;
        runtime
            .send(Command::Snapshot {
                key,
                purpose: metin_desktop::runtime::SnapshotPurpose::UpdateLiveEvidence,
            })
            .unwrap();
        wait(&runtime, |snapshot| snapshot.frame_id > previous);
        let snapshot = runtime.latest().unwrap();
        assert!(snapshot.calibration_review);
        assert!(!snapshot.running && !snapshot.arming);
    }
    runtime.stop();
    wait(&runtime, |snapshot| {
        snapshot.released_generation == Some(runtime.stop_generation())
    });
    controls.lock().unwrap().toggle = true;
    let previous = runtime.latest().unwrap().frame_id;
    runtime
        .send(Command::Snapshot {
            key,
            purpose: metin_desktop::runtime::SnapshotPurpose::UpdateLiveEvidence,
        })
        .unwrap();
    wait(&runtime, |snapshot| snapshot.frame_id > previous);
    assert!(runtime.latest().unwrap().calibration_review);
    assert!(!runtime.latest().unwrap().running);
    assert!(controls.lock().unwrap().calls.iter().all(|(_, down)| !down));
    runtime.shutdown().unwrap();
}

#[test]
fn calibration_review_ignores_stale_begin_and_end_and_matching_cancel_allows_start() {
    let (runtime, controls) = runtime();
    let profile = profile();
    let key = SessionKey {
        profile_id: profile.id,
        target: token(1),
    };
    configure(&runtime, profile.clone(), key.target, false, 1);
    runtime
        .send(Command::CalibrationReview {
            key,
            revision: 1,
            active: true,
        })
        .unwrap();
    wait(&runtime, |snapshot| snapshot.calibration_review);
    runtime
        .send(Command::CalibrationReview {
            key,
            revision: 1,
            active: false,
        })
        .unwrap();
    wait(&runtime, |snapshot| !snapshot.calibration_review);
    configure(&runtime, profile, key.target, false, 2);
    let previous = runtime.latest().unwrap().frame_id;
    runtime
        .send(Command::CalibrationReview {
            key,
            revision: 1,
            active: true,
        })
        .unwrap();
    runtime
        .send(Command::Snapshot {
            key,
            purpose: metin_desktop::runtime::SnapshotPurpose::UpdateLiveEvidence,
        })
        .unwrap();
    wait(&runtime, |snapshot| snapshot.frame_id > previous);
    assert!(!runtime.latest().unwrap().calibration_review);
    runtime
        .send(Command::CalibrationReview {
            key,
            revision: 2,
            active: true,
        })
        .unwrap();
    wait(&runtime, |snapshot| snapshot.calibration_review);
    for (stale_key, revision) in [
        (key, 1),
        (
            SessionKey {
                target: token(2),
                ..key
            },
            2,
        ),
    ] {
        runtime
            .send(Command::CalibrationReview {
                key: stale_key,
                revision,
                active: false,
            })
            .unwrap();
    }
    runtime.send(Command::Arm { key }).unwrap();
    let previous = runtime.latest().unwrap().frame_id;
    runtime
        .send(Command::Snapshot {
            key,
            purpose: metin_desktop::runtime::SnapshotPurpose::UpdateLiveEvidence,
        })
        .unwrap();
    wait(&runtime, |snapshot| snapshot.frame_id > previous);
    assert!(runtime.latest().unwrap().calibration_review);
    assert!(!runtime.latest().unwrap().arming);
    runtime
        .send(Command::CalibrationReview {
            key,
            revision: 2,
            active: false,
        })
        .unwrap();
    wait(&runtime, |snapshot| !snapshot.calibration_review);
    controls.lock().unwrap().toggle = true;
    wait(&runtime, |snapshot| snapshot.running);
    runtime.stop();
    wait(&runtime, |snapshot| !snapshot.running);
    runtime.send(Command::Arm { key }).unwrap();
    wait(&runtime, |snapshot| snapshot.arming);
    runtime.shutdown().unwrap();
}

#[test]
fn calibration_review_resets_on_reconnect_and_session_replacement() {
    let (runtime, _) = runtime();
    let profile = profile();
    let key = SessionKey {
        profile_id: profile.id,
        target: token(1),
    };
    configure(&runtime, profile.clone(), key.target, false, 1);
    runtime
        .send(Command::CalibrationReview {
            key,
            revision: 1,
            active: true,
        })
        .unwrap();
    wait(&runtime, |snapshot| snapshot.calibration_review);
    runtime.send(Command::Connect { key, revision: 2 }).unwrap();
    wait(&runtime, |snapshot| snapshot.revision == 2);
    assert!(!runtime.latest().unwrap().calibration_review);
    assert_eq!(runtime.latest().unwrap().applied_revision, None);
    configure(&runtime, profile.clone(), key.target, false, 2);
    runtime.send(Command::Arm { key }).unwrap();
    wait(&runtime, |snapshot| snapshot.arming);
    runtime
        .send(Command::CalibrationReview {
            key,
            revision: 2,
            active: true,
        })
        .unwrap();
    wait(&runtime, |snapshot| snapshot.calibration_review);
    configure(&runtime, profile, token(2), false, 3);
    assert!(!runtime.latest().unwrap().calibration_review);
    let replacement = runtime.latest().unwrap().key.unwrap();
    runtime
        .send(Command::CalibrationReview {
            key: replacement,
            revision: 3,
            active: true,
        })
        .unwrap();
    wait(&runtime, |snapshot| snapshot.calibration_review);
    runtime.send(Command::Detach).unwrap();
    wait(&runtime, |snapshot| snapshot.key.is_none());
    assert!(!runtime.latest().unwrap().calibration_review);
    runtime.shutdown().unwrap();
}

#[test]
fn calibration_review_cannot_be_bypassed_by_reapplying_the_same_session() {
    let (runtime, controls) = runtime();
    let profile = profile();
    let key = SessionKey {
        profile_id: profile.id,
        target: token(1),
    };
    configure(&runtime, profile.clone(), key.target, false, 1);
    runtime
        .send(Command::CalibrationReview {
            key,
            revision: 1,
            active: true,
        })
        .unwrap();
    wait(&runtime, |snapshot| snapshot.calibration_review);
    runtime
        .send(Command::Configure {
            profile: Box::new(profile),
            template: Some(hp_template()),
            target: key.target,
            revision: 1,
        })
        .unwrap();
    controls.lock().unwrap().toggle = true;
    runtime
        .send(Command::Snapshot {
            key,
            purpose: metin_desktop::runtime::SnapshotPurpose::UpdateLiveEvidence,
        })
        .unwrap();
    wait(&runtime, |snapshot| snapshot.frame_id > 0);
    assert!(runtime.latest().unwrap().calibration_review);
    assert!(!runtime.latest().unwrap().running);
    assert!(controls.lock().unwrap().calls.iter().all(|(_, down)| !down));
    runtime.shutdown().unwrap();
}

#[test]
fn calibration_review_commands_survive_stop_generation_changes_in_fifo_order() {
    let (runtime, controls) = runtime();
    let profile = profile();
    let key = SessionKey {
        profile_id: profile.id,
        target: token(1),
    };
    configure(&runtime, profile.clone(), key.target, false, 1);
    for active in [true, false] {
        let barrier = Arc::new(Barrier::new(2));
        controls.lock().unwrap().hotkey_barrier = Some(barrier.clone());
        barrier.wait();
        runtime
            .send(Command::CalibrationReview {
                key,
                revision: 1,
                active,
            })
            .unwrap();
        // Force the control command into the old generation before allowing
        // the worker to receive it; release/stop must never discard review state.
        runtime.stop();
        barrier.wait();
        wait(&runtime, |snapshot| {
            snapshot.calibration_review == active
                && snapshot.released_generation == Some(runtime.stop_generation())
        });
    }
    // The save path cancels holds after queueing the matching review end.
    // Both end and the later configuration must survive in FIFO order.
    runtime
        .send(Command::CalibrationReview {
            key,
            revision: 1,
            active: true,
        })
        .unwrap();
    wait(&runtime, |snapshot| snapshot.calibration_review);
    let barrier = Arc::new(Barrier::new(2));
    controls.lock().unwrap().hotkey_barrier = Some(barrier.clone());
    barrier.wait();
    runtime
        .send(Command::CalibrationReview {
            key,
            revision: 1,
            active: false,
        })
        .unwrap();
    runtime.stop();
    runtime
        .send(Command::Configure {
            profile: Box::new(profile),
            template: Some(hp_template()),
            target: key.target,
            revision: 2,
        })
        .unwrap();
    runtime.send(Command::Arm { key }).unwrap();
    barrier.wait();
    wait(&runtime, |snapshot| {
        snapshot.revision == 2 && snapshot.arming
    });
    assert!(!runtime.latest().unwrap().calibration_review);
    runtime.shutdown().unwrap();
}

#[test]
fn calibration_review_pauses_and_releases_an_owned_attack() {
    let (runtime, controls) = runtime();
    let mut profile = profile();
    profile.timing.search_timeout = 120.;
    profile.out_of_target.during_search = true;
    profile.out_of_target.search_after_seconds = 1.;
    profile.out_of_target.search_attack_seconds = 3.;
    let key = SessionKey {
        profile_id: profile.id,
        target: token(1),
    };
    configure(&runtime, profile, key.target, false, 1);
    controls.lock().unwrap().toggle = true;
    wait(&runtime, |snapshot| {
        snapshot.state == "Out-of-target attack"
    });
    assert!(controls.lock().unwrap().calls.contains(&(0x20, true)));
    runtime
        .send(Command::CalibrationReview {
            key,
            revision: 1,
            active: true,
        })
        .unwrap();
    wait(&runtime, |snapshot| {
        snapshot.calibration_review && snapshot.released_generation.is_some()
    });
    assert!(!runtime.latest().unwrap().running);
    assert!(controls.lock().unwrap().calls.contains(&(0x20, false)));
    runtime.shutdown().unwrap();
}

#[test]
fn same_target_save_preserves_declared_mount_and_stale_declaration_is_rejected() {
    let (r, _) = runtime();
    let p = profile();
    let key = SessionKey {
        profile_id: p.id,
        target: token(1),
    };
    configure(&r, p.clone(), token(1), true, 1);
    wait(&r, |s| s.revision == 1);
    r.send(Command::DeclareMount {
        key,
        mounted: false,
        revision: 2,
    })
    .unwrap();
    wait(&r, |s| s.revision == 2 && s.mounted == Some(false));
    configure(&r, p.clone(), token(1), true, 3);
    wait(&r, |s| s.revision == 3);
    assert_eq!(r.latest().unwrap().mounted, Some(false));
    configure(&r, p, token(2), true, 4);
    wait(&r, |s| s.revision == 4);
    r.send(Command::Arm {
        key: SessionKey {
            profile_id: key.profile_id,
            target: token(2),
        },
    })
    .unwrap();
    wait(&r, |s| s.state == "Armed");
    r.send(Command::DeclareMount {
        key,
        mounted: false,
        revision: 5,
    })
    .unwrap();
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(r.latest().unwrap().state, "Armed");
    assert_eq!(r.latest().unwrap().mounted, Some(true));
    r.shutdown().unwrap();
}

#[test]
fn a_new_profile_or_window_requires_its_own_mount_declaration() {
    let (r, controls) = runtime();
    let mut p = profile();
    p.buffs.enabled = true;
    p.buffs.skills.push(Default::default());
    configure(&r, p.clone(), token(1), true, 1);
    wait(&r, |s| s.revision == 1);
    p.id = Profile::create("Another character").id;
    r.send(Command::Configure {
        profile: Box::new(p),
        template: None,
        target: token(2),
        revision: 2,
    })
    .unwrap();
    wait(&r, |s| s.revision == 2);
    assert!(
        r.latest().unwrap().mounted.is_none(),
        "Never inherit a declaration from another session"
    );
    controls.lock().unwrap().toggle = true;
    wait(&r, |s| s.reason.contains("Mount state is unknown"));
    assert!(controls.lock().unwrap().calls.iter().all(|(_, down)| !down));
    r.shutdown().unwrap();
}

#[test]
fn failed_mount_remains_unknown_across_refresh_until_explicit_declaration() {
    let (r, controls) = runtime();
    let mut p = profile();
    p.buffs.enabled = true;
    p.buffs.skills.push(Default::default());
    let key = SessionKey {
        profile_id: p.id,
        target: token(1),
    };
    configure(&r, p.clone(), token(1), true, 1);
    wait(&r, |s| s.revision == 1);
    {
        let mut c = controls.lock().unwrap();
        c.fail_key = true;
        c.toggle = true;
    }
    wait(&r, |s| s.mounted.is_none());
    assert!(!r.latest().unwrap().running);
    configure(&r, p, token(1), true, 2);
    wait(&r, |s| s.revision == 2);
    assert_eq!(r.latest().unwrap().mounted, None);
    r.send(Command::DeclareMount {
        key,
        mounted: true,
        revision: 3,
    })
    .unwrap();
    wait(&r, |s| s.revision == 3 && s.mounted == Some(true));
    assert!(
        controls
            .lock()
            .unwrap()
            .calls
            .iter()
            .any(|(_, down)| !*down)
    );
    r.shutdown().unwrap();
}

#[test]
fn history_clear_and_statistics_reset_keep_safety_budget_and_reject_stale_keys() {
    let (r, controls) = runtime();
    let mut p = profile();
    p.safety.stop_enabled = true;
    let key = SessionKey {
        profile_id: p.id,
        target: token(1),
    };
    configure(&r, p.clone(), token(1), false, 1);
    wait(&r, |s| s.revision == 1);
    controls.lock().unwrap().toggle = true;
    wait(&r, |s| s.running && s.schedule.active_seconds > 0.2);
    r.stop();
    wait(&r, |s| !s.running && !s.events.is_empty());
    let active = r.latest().unwrap().schedule.active_seconds;
    r.send(Command::ResetSession { key }).unwrap();
    r.send(Command::ClearHistory {
        key: SessionKey {
            profile_id: key.profile_id,
            target: token(2),
        },
    })
    .unwrap();
    std::thread::sleep(Duration::from_millis(250));
    assert!(!r.latest().unwrap().events.is_empty());
    r.send(Command::ClearHistory { key }).unwrap();
    wait(&r, |s| s.events.is_empty());
    assert_eq!(r.latest().unwrap().schedule.active_seconds, active);
    assert_eq!(r.latest().unwrap().statistics.active_seconds, 0.);
    configure(&r, p.clone(), token(1), false, 2);
    wait(&r, |s| s.revision == 2);
    assert_eq!(r.latest().unwrap().schedule.active_seconds, active);
    p.safety.stop_after_seconds = 3600.;
    configure(&r, p, token(1), false, 3);
    wait(&r, |s| s.revision == 3);
    assert_eq!(r.latest().unwrap().schedule.active_seconds, 0.);
    assert_eq!(r.latest().unwrap().schedule.stop_remaining, Some(3600.));
    r.shutdown().unwrap();
}

#[test]
fn session_reset_releases_held_attack_and_does_not_claim_success_when_release_fails() {
    for fail_release in [false, true] {
        let (runtime, controls) = runtime();
        let mut profile = profile();
        profile.safety.stop_enabled = true;
        profile.timing.search_timeout = 120.;
        profile.out_of_target.during_search = true;
        profile.out_of_target.search_after_seconds = 1.;
        profile.out_of_target.search_attack_seconds = 3.;
        let key = SessionKey {
            profile_id: profile.id,
            target: token(1),
        };
        configure(&runtime, profile, key.target, false, 1);
        controls.lock().unwrap().toggle = true;
        wait(&runtime, |snapshot| {
            snapshot.state == "Out-of-target attack"
        });
        let budget_before = runtime.latest().unwrap().schedule.active_seconds;
        assert!(budget_before > 0.);
        controls.lock().unwrap().fail_release = fail_release;
        runtime.send(Command::ResetSession { key }).unwrap();
        if fail_release {
            wait(&runtime, |snapshot| {
                !snapshot.running && snapshot.reason.contains("release failure")
            });
            let snapshot = runtime.latest().unwrap();
            assert!(snapshot.statistics.active_seconds > 0.);
            assert!(
                !snapshot
                    .events
                    .iter()
                    .any(|event| event.message.starts_with("Session reset"))
            );
            assert!(snapshot.released_generation.is_none());
            controls.lock().unwrap().fail_release = false;
        } else {
            wait(&runtime, |snapshot| {
                !snapshot.running
                    && snapshot.events.len() == 1
                    && snapshot.events[0].message.starts_with("Session reset")
            });
            let snapshot = runtime.latest().unwrap();
            assert_eq!(snapshot.statistics, metin_core::Statistics::default());
            assert!(snapshot.schedule.active_seconds >= budget_before);
            assert_eq!(snapshot.mounted, Some(false));
            assert!(snapshot.activity_progress.is_none());
            assert!(snapshot.buff_presence.is_empty());
        }
        assert!(controls.lock().unwrap().calls.contains(&(0x20, false)));
        runtime.shutdown().unwrap();
    }
}

#[test]
fn session_reset_interrupts_a_blocking_rotation_before_its_hold_finishes() {
    let (runtime, controls) = runtime();
    let mut profile = profile();
    profile.buffs.enabled = false;
    profile.timing.search_timeout = 2.;
    profile.timing.rotate_seconds = 1.;
    let key = SessionKey {
        profile_id: profile.id,
        target: token(1),
    };
    configure(&runtime, profile, key.target, false, 1);
    controls.lock().unwrap().toggle = true;
    wait(&runtime, |snapshot| snapshot.running);
    let deadline = Instant::now() + Duration::from_secs(4);
    while !controls.lock().unwrap().calls.contains(&(0x45, true)) {
        assert!(Instant::now() < deadline, "Rotation never started");
        std::thread::sleep(Duration::from_millis(5));
    }
    let requested = Instant::now();
    runtime.send(Command::ResetSession { key }).unwrap();
    let deadline = requested + Duration::from_millis(700);
    while !controls.lock().unwrap().calls.contains(&(0x45, false)) {
        assert!(
            Instant::now() < deadline,
            "Reset waited for the full blocking hold"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    wait(&runtime, |snapshot| {
        !snapshot.running
            && snapshot.events.len() == 1
            && snapshot.events[0].message.starts_with("Session reset")
    });
    assert_eq!(
        runtime.latest().unwrap().statistics,
        metin_core::Statistics::default()
    );
    runtime.shutdown().unwrap();
}

#[test]
fn recognition_snapshot_publishes_pixels_without_creating_live_evidence() {
    let (runtime, controls) = runtime();
    let mut profile = profile();
    profile.buffs.skills.push(metin_core::BuffSkill::default());
    let key = SessionKey {
        profile_id: profile.id,
        target: token(1),
    };
    configure(&runtime, profile, token(1), false, 1);
    let before = runtime.latest().unwrap();
    runtime
        .send(Command::Snapshot {
            key,
            purpose: metin_desktop::runtime::SnapshotPurpose::ReadOnlyRecognition,
        })
        .unwrap();
    wait_raw(&runtime, |snapshot| snapshot.frame_id > before.frame_id);
    let after = runtime.latest().unwrap();
    assert!(after.frame.is_some());
    assert_eq!(
        after.buff_presence, before.buff_presence,
        "A read-only snapshot must not advance or replace live buff evidence"
    );
    assert_eq!(
        after.buff_last_active_age_seconds,
        before.buff_last_active_age_seconds
    );
    assert_eq!(after.timers, before.timers);
    assert!(controls.lock().unwrap().calls.is_empty());
    runtime.shutdown().unwrap();
}
#[test]
fn failed_readonly_capture_preserves_paused_controller_and_previous_pixels() {
    let (runtime, controls) = runtime();
    let profile = profile();
    let key = SessionKey {
        profile_id: profile.id,
        target: token(1),
    };
    configure(&runtime, profile, key.target, false, 1);
    runtime
        .send(Command::Snapshot {
            key,
            purpose: metin_desktop::runtime::SnapshotPurpose::ReadOnlyRecognition,
        })
        .unwrap();
    wait_raw(&runtime, |snapshot| snapshot.frame.is_some());
    let before = runtime.latest().unwrap();
    let captures = controls.lock().unwrap().captures;
    controls.lock().unwrap().fail_capture = true;
    runtime
        .send(Command::Snapshot {
            key,
            purpose: metin_desktop::runtime::SnapshotPurpose::ReadOnlyRecognition,
        })
        .unwrap();
    // The following command is a worker barrier: observe the failure, then inspect state.
    runtime.send(Command::RefreshTargets).unwrap();
    wait_raw(&runtime, |snapshot| {
        snapshot.targets_revision > before.targets_revision
            && controls.lock().unwrap().captures > captures
    });
    let after = runtime.latest().unwrap();
    assert_eq!(after.reason, before.reason);
    assert_eq!(after.state, before.state);
    assert_eq!(after.pause_info, before.pause_info);
    assert_eq!(after.frame_id, before.frame_id);
    assert_eq!(after.geometry, before.geometry);
    assert_eq!(after.timers, before.timers);
    assert_eq!(after.events, before.events);
    assert!(controls.lock().unwrap().calls.is_empty());
    runtime.shutdown().unwrap();
}
#[test]
fn readonly_snapshots_preserve_seeded_active_evidence_and_its_original_expiry() {
    use base64::Engine;
    let (runtime, controls) = runtime();
    let active = Frame::new(
        200,
        200,
        (0..200 * 200 * 3)
            .map(|index| 128 + (index % 31) as u8)
            .collect(),
    )
    .unwrap();
    let changed = Frame::new(
        200,
        200,
        (0..200 * 200 * 3)
            .map(|index| ((index * 17 + index / 7 * 113) % 251) as u8)
            .collect(),
    )
    .unwrap();
    let area = metin_core::Region {
        left: 0.,
        top: 0.,
        right: 0.05,
        bottom: 0.05,
    };
    let reference = metin_vision::crop(&active, area).unwrap();
    let mut profile = profile();
    profile.buffs.icon_region = Some(area);
    profile.buffs.skills.push(metin_core::BuffSkill {
        mode: metin_core::BuffMode::Inactive,
        interval_seconds: None,
        active_hold_seconds: 1.5,
        icon: Some(metin_core::BuffIcon {
            png: base64::engine::general_purpose::STANDARD
                .encode(metin_vision::encode_png(&reference).unwrap()),
            region: Some(area),
        }),
        ..Default::default()
    });
    let key = SessionKey {
        profile_id: profile.id,
        target: token(1),
    };
    controls.lock().unwrap().frame_override = Some(active.clone());
    configure(&runtime, profile, token(1), false, 1);
    runtime
        .send(Command::Snapshot {
            key,
            purpose: metin_desktop::runtime::SnapshotPurpose::UpdateLiveEvidence,
        })
        .unwrap();
    wait_raw(&runtime, |snapshot| {
        snapshot.buff_presence == vec![metin_core::Presence::Active]
    });
    let seeded = runtime.latest().unwrap();
    let source = seeded
        .recognition_evidence
        .expect("Actual live Active evidence");
    controls.lock().unwrap().frame_override = Some(changed);
    let mut after = seeded.clone();
    while source.captured_at.elapsed() < Duration::from_millis(700) {
        let frame_id = after.frame_id;
        runtime
            .send(Command::Snapshot {
                key,
                purpose: metin_desktop::runtime::SnapshotPurpose::ReadOnlyRecognition,
            })
            .unwrap();
        wait_raw(&runtime, |snapshot| snapshot.frame_id > frame_id);
        after = runtime.latest().unwrap();
        if let Some(still_fresh) = after.recognition_evidence {
            assert_eq!(
                still_fresh, source,
                "The request must not renew live evidence provenance"
            );
            assert_eq!(
                after.buff_last_active_age_seconds,
                seeded.buff_last_active_age_seconds
            );
            assert_eq!(after.buff_presence, seeded.buff_presence);
        }
        assert_eq!(after.timers, seeded.timers);
        // Repeated checks of an actually Active image must not renew its hold.
        controls.lock().unwrap().frame_override = Some(active.clone());
    }
    assert!(after.captured_at.unwrap().elapsed() < Duration::from_millis(500));
    assert!(
        after.recognition_evidence.is_none() && after.buff_presence.is_empty(),
        "Raw checks must not keep original live evidence fresh beyond500ms"
    );
    assert!(controls.lock().unwrap().calls.is_empty());
    runtime.shutdown().unwrap();
}
#[test]
fn buff_presence_is_published_with_fresh_pixels_and_retired_when_the_frame_expires() {
    let (runtime, _) = runtime();
    let mut profile = profile();
    profile.buffs.skills.push(metin_core::BuffSkill::default());
    let key = SessionKey {
        profile_id: profile.id,
        target: token(1),
    };
    configure(&runtime, profile, key.target, false, 1);
    runtime
        .send(Command::Snapshot {
            key,
            purpose: metin_desktop::runtime::SnapshotPurpose::UpdateLiveEvidence,
        })
        .unwrap();
    wait(&runtime, |snapshot| {
        snapshot.frame_id > 0 && snapshot.buff_presence == vec![metin_core::Presence::Unknown]
    });
    wait(&runtime, |snapshot| {
        snapshot.captured_at.is_some() && snapshot.buff_presence.is_empty()
    });
    assert!(runtime.latest().unwrap().captured_at.unwrap().elapsed() >= Duration::from_millis(500));
    runtime.shutdown().unwrap();
}

#[test]
fn held_attack_keeps_capturing_and_releases_on_stop_focus_loss_or_capture_error() {
    for fault in [0, 1, 2] {
        let (r, controls) = runtime();
        let mut p = profile();
        p.timing.search_timeout = 120.;
        p.out_of_target.during_search = true;
        p.out_of_target.search_after_seconds = 1.;
        p.out_of_target.search_attack_seconds = 3.;
        configure(&r, p, token(1), false, 1);
        wait(&r, |s| s.revision == 1);
        controls.lock().unwrap().toggle = true;
        wait(&r, |s| s.state == "Out-of-target attack");
        assert_eq!(
            r.latest().unwrap().activity,
            metin_core::ActivityKind::OutOfTarget
        );
        let before = controls.lock().unwrap().captures;
        std::thread::sleep(Duration::from_millis(250));
        {
            let c = controls.lock().unwrap();
            assert!(c.captures > before);
            assert!(c.calls.contains(&(0x20, true)));
            assert!(!c.calls.contains(&(0x20, false)));
            assert!(
                c.capture_limits
                    .iter()
                    .any(|d| *d <= Duration::from_millis(100))
            );
        }
        match fault {
            0 => r.stop(),
            1 => controls.lock().unwrap().lose_focus = true,
            _ => controls.lock().unwrap().fail_capture = true,
        }
        wait(&r, |s| !s.running);
        assert_eq!(
            r.latest().unwrap().activity,
            if fault == 0 {
                metin_core::ActivityKind::Paused
            } else {
                metin_core::ActivityKind::Error
            }
        );
        assert_eq!(r.latest().unwrap().active_skill, None);
        assert!(controls.lock().unwrap().calls.contains(&(0x20, false)));
        r.shutdown().unwrap();
    }
}

#[test]
fn a_paused_session_without_preview_demand_does_not_capture() {
    let (r, controls) = runtime();
    configure(&r, profile(), token(1), false, 1);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(controls.lock().unwrap().captures, 0);
    r.shutdown().unwrap();
}

fn candidate_profile() -> Profile {
    let mut p = profile();
    p.panel.threshold = 0.99;
    p.color.as_mut().unwrap().hsv = [60, 255, 220];
    p.timing.search_timeout = 120.;
    p
}

#[test]
fn obscured_new_candidate_is_skipped_for_a_clear_candidate_in_physical_coordinates() {
    let (r, controls) = runtime();
    {
        let mut c = controls.lock().unwrap();
        c.colored_candidates = true;
        c.obscure_nearest = true;
        c.target_origin = [-250, 300];
        c.cursor = [-150, 400];
    }
    configure(&r, candidate_profile(), token(1), false, 1);
    controls.lock().unwrap().toggle = true;
    wait(&r, |_| !controls.lock().unwrap().clicks.is_empty());
    let clicks = controls.lock().unwrap().clicks.clone();
    assert_eq!(clicks, vec![[149, 141]]);
    assert!(r.latest().unwrap().running);
    wait(&r, |s| s.candidates.len() == 2);
    r.shutdown().unwrap();
}

#[test]
fn all_obscured_candidates_remain_visible_in_preview_without_new_clicks() {
    let (r, controls) = runtime();
    {
        let mut c = controls.lock().unwrap();
        c.colored_candidates = true;
        c.obscure_all = true;
    }
    configure(&r, candidate_profile(), token(1), false, 1);
    controls.lock().unwrap().toggle = true;
    wait(&r, |s| {
        s.candidates.len() == 2 && s.schedule.active_seconds > 0.8
    });
    assert!(r.latest().unwrap().running);
    assert!(r.latest().unwrap().next_target.is_none());
    assert!(controls.lock().unwrap().clicks.is_empty());
    r.shutdown().unwrap();
}

#[test]
fn candidate_obstruction_appearing_during_hover_still_pauses_before_mouse_down() {
    let (r, controls) = runtime();
    {
        let mut c = controls.lock().unwrap();
        c.colored_candidates = true;
        c.obscure_on_hover = true;
    }
    configure(&r, candidate_profile(), token(1), false, 1);
    controls.lock().unwrap().toggle = true;
    wait(&r, |s| !s.running && s.reason.contains("obstructs"));
    assert!(controls.lock().unwrap().clicks.is_empty());
    r.shutdown().unwrap();
}

#[test]
fn arming_retries_preview_focus_transition_then_starts_from_a_fresh_frame() {
    let (r, controls) = runtime();
    let p = profile();
    let key = SessionKey {
        profile_id: p.id,
        target: token(1),
    };
    configure(&r, p, token(1), false, 1);
    r.send(Command::SetPreview { key, enabled: true }).unwrap();
    wait(&r, |s| s.frame_id > 0);
    {
        let mut c = controls.lock().unwrap();
        c.lose_focus = true;
        c.focus_change_during_capture = true;
    }
    r.send(Command::Arm { key }).unwrap();
    std::thread::sleep(Duration::from_millis(400));
    assert!(r.latest().unwrap().arming, "{}", r.latest().unwrap().reason);
    assert!(!r.latest().unwrap().running);
    assert!(controls.lock().unwrap().calls.iter().all(|(_, down)| !down));
    wait(&r, |s| s.running && s.frame_id > 1);
    assert!(r.latest().unwrap().geometry.unwrap().foreground);
    r.shutdown().unwrap();
}

#[test]
fn arming_waits_for_successful_capture_even_after_focus_delay() {
    let (r, controls) = runtime();
    let p = profile();
    let key = SessionKey {
        profile_id: p.id,
        target: token(1),
    };
    configure(&r, p, token(1), false, 1);
    controls.lock().unwrap().fail_capture = true;
    r.send(Command::Arm { key }).unwrap();
    std::thread::sleep(Duration::from_millis(3300));
    assert!(r.latest().unwrap().arming);
    assert!(!r.latest().unwrap().running);
    assert!(controls.lock().unwrap().calls.iter().all(|(_, down)| !down));
    controls.lock().unwrap().fail_capture = false;
    wait(&r, |s| s.running && s.frame_id > 0);
    r.shutdown().unwrap();
}

#[test]
fn stale_capture_cannot_start_armed_input_or_trigger_a_running_action() {
    for arm in [true, false] {
        let (r, controls) = runtime();
        let p = profile();
        let key = SessionKey {
            profile_id: p.id,
            target: token(1),
        };
        configure(&r, p, token(1), false, 1);
        if arm {
            controls.lock().unwrap().stale_capture = true;
            r.send(Command::Arm { key }).unwrap();
            std::thread::sleep(Duration::from_millis(3300));
            assert!(r.latest().unwrap().arming);
            assert!(!r.latest().unwrap().running);
        } else {
            controls.lock().unwrap().toggle = true;
            wait(&r, |s| s.running);
            controls.lock().unwrap().stale_capture = true;
            wait(&r, |s| !s.running && s.reason.contains("stale"));
        }
        assert!(controls.lock().unwrap().calls.iter().all(|(_, down)| !down));
        r.shutdown().unwrap();
    }
}

#[test]
fn capture_snapshot_has_current_geometry_and_matching_release_ack() {
    let (r, _) = runtime();
    let p = profile();
    let key = SessionKey {
        profile_id: p.id,
        target: token(1),
    };
    configure(&r, p, token(1), false, 1);
    r.stop();
    let generation = r.stop_generation();
    r.send(Command::Snapshot {
        key,
        purpose: metin_desktop::runtime::SnapshotPurpose::UpdateLiveEvidence,
    })
    .unwrap();
    wait(&r, |s| {
        s.frame_id > 0 && s.released_generation == Some(generation)
    });
    assert_eq!(r.latest().unwrap().geometry.unwrap().target, token(1));
    r.send(Command::Detach).unwrap();
    wait(&r, |s| s.key.is_none());
    assert!(r.latest().unwrap().geometry.is_none());
    r.shutdown().unwrap();
}

#[test]
fn oversized_hp_detail_still_publishes_raw_snapshots_for_repair_but_blocks_running() {
    let (r, controls) = runtime();
    let mut p = profile();
    // An older imported profile may lack reference crop metadata, so its
    // externally loaded PNG dimensions are known only to runtime recognition.
    p.template_region = None;
    let key = SessionKey {
        profile_id: p.id,
        target: token(1),
    };
    configure(&r, p.clone(), token(1), false, 1);
    // A malformed imported reference can exceed the complete client frame.
    let template = Frame::new(
        201,
        201,
        (0..201 * 201 * 3).map(|n| (n % 251) as u8).collect(),
    )
    .unwrap();
    r.send(Command::Configure {
        profile: Box::new(p),
        template: Some(template),
        target: token(1),
        revision: 2,
    })
    .unwrap();
    wait(&r, |s| s.revision == 2);
    r.send(Command::Snapshot {
        key,
        purpose: metin_desktop::runtime::SnapshotPurpose::UpdateLiveEvidence,
    })
    .unwrap();
    wait(&r, |s| s.frame_id > 0 && s.geometry.is_some());
    let snapshot = r.latest().unwrap();
    assert!(snapshot.frame.is_some());
    assert_eq!(snapshot.panel_score, 0.);
    assert!(snapshot.panel_match.is_none());
    assert!(
        snapshot.reason.contains("Template is larger"),
        "{}",
        snapshot.reason
    );
    assert_eq!(snapshot.released_generation, Some(r.stop_generation()));
    let frame_id = snapshot.frame_id;
    r.send(Command::SetPreview { key, enabled: true }).unwrap();
    wait(&r, |s| s.frame_id > frame_id && s.geometry.is_some());
    controls.lock().unwrap().toggle = true;
    wait(&r, |s| {
        !s.running
            && s.events.iter().any(|event| {
                event.notify && event.message == "Template is larger than search region."
            })
    });
    assert!(controls.lock().unwrap().calls.iter().all(|(_, down)| !down));
    r.shutdown().unwrap();
}

#[test]
fn shrunk_client_with_enabled_visual_reference_still_provides_raw_repair_captures() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let (r, controls) = runtime();
    let mut p = profile();
    p.frame_size = Some([300, 300]);
    let reference = Frame::new(
        128,
        128,
        (0..128 * 128 * 3).map(|n| (n % 251) as u8).collect(),
    )
    .unwrap();
    let mut rule = metin_core::SafetyRule::new("Visual reference before resize");
    rule.enabled = true;
    rule.reference = Some(metin_core::BuffIcon {
        png: STANDARD.encode(metin_vision::encode_png(&reference).unwrap()),
        region: Some(metin_core::Region {
            left: 0.,
            top: 0.,
            right: 127. / 300.,
            bottom: 127. / 300.,
        }),
    });
    p.safety.rules.push(rule);
    let key = SessionKey {
        profile_id: p.id,
        target: token(1),
    };
    configure(&r, p, token(1), false, 1);
    r.send(Command::Snapshot {
        key,
        purpose: metin_desktop::runtime::SnapshotPurpose::UpdateLiveEvidence,
    })
    .unwrap();
    wait(&r, |s| s.frame_id > 0 && s.geometry.is_some());
    let snapshot = r.latest().unwrap();
    assert_eq!(
        [
            snapshot.frame.as_ref().unwrap().width,
            snapshot.geometry.unwrap().width
        ],
        [200, 200]
    );
    assert!(snapshot.safety_scores.is_empty());
    assert!(snapshot.reason.contains("Recognition needs repair"));
    assert!(snapshot.reason.contains("Template is larger"));
    assert_eq!(snapshot.released_generation, Some(r.stop_generation()));
    assert!(!snapshot.running && !snapshot.arming);
    let frame_id = snapshot.frame_id;
    r.send(Command::SetPreview { key, enabled: true }).unwrap();
    wait(&r, |s| s.frame_id > frame_id && s.geometry.is_some());
    controls.lock().unwrap().toggle = true;
    wait(&r, |s| {
        !s.running
            && s.events
                .iter()
                .any(|event| event.notify && event.message.contains("dimensions changed"))
    });
    assert!(controls.lock().unwrap().calls.iter().all(|(_, down)| !down));
    assert!(controls.lock().unwrap().clicks.is_empty());
    r.shutdown().unwrap();
}

#[test]
fn failed_owned_release_never_acknowledges_capture_safety() {
    let (r, controls) = runtime();
    let mut p = profile();
    p.timing.search_timeout = 120.;
    p.out_of_target.during_search = true;
    p.out_of_target.search_after_seconds = 1.;
    p.out_of_target.search_attack_seconds = 3.;
    configure(&r, p, token(1), false, 1);
    controls.lock().unwrap().toggle = true;
    wait(&r, |s| s.state == "Out-of-target attack");
    controls.lock().unwrap().fail_release = true;
    r.stop();
    wait(&r, |s| !s.running && s.reason.contains("release failure"));
    assert!(r.latest().unwrap().released_generation.is_none());
    controls.lock().unwrap().fail_release = false;
    wait(&r, |s| s.released_generation == Some(r.stop_generation()));
    r.shutdown().unwrap();
}

#[test]
fn mount_declaration_does_not_need_a_frame_template_or_calibration() {
    let (r, controls) = runtime();
    let mut p = Profile::create("Uncalibrated character");
    p.buffs.enabled = true;
    assert!(
        p.validate().is_err(),
        "An unfinished buff draft must not prevent connecting"
    );
    let key = SessionKey {
        profile_id: p.id,
        target: token(1),
    };
    r.send(Command::Connect { key, revision: 1 }).unwrap();
    wait(&r, |s| s.key == Some(key) && s.revision == 1);
    // Even an explicit rejected configuration cannot invalidate the independent connection.
    r.send(Command::Configure {
        profile: Box::new(p),
        template: None,
        target: token(1),
        revision: 1,
    })
    .unwrap();
    wait(&r, |s| s.reason.contains("one to eight buffs"));
    assert!(r.latest().unwrap().frame.is_none());
    for (revision, mounted) in [(2, false), (3, true)] {
        r.send(Command::DeclareMount {
            key,
            mounted,
            revision,
        })
        .unwrap();
        wait(&r, |s| s.revision == revision && s.mounted == Some(mounted));
    }
    controls.lock().unwrap().toggle = true;
    wait(&r, |s| s.reason.contains("Apply the profile configuration"));
    let c = controls.lock().unwrap();
    assert_eq!(c.captures, 0);
    assert!(c.calls.iter().all(|(_, down)| !down));
    drop(c);
    r.send(Command::Detach).unwrap();
    wait(&r, |s| s.key.is_none());
    let detached = r.latest().unwrap();
    assert_eq!(detached.mounted, None);
    assert!(detached.timers.is_empty());
    assert!(detached.action_timers.is_empty());
    assert_eq!(detached.statistics, Default::default());
    r.shutdown().unwrap();
}

#[test]
fn reconnecting_same_session_preserves_mount_but_requires_profile_apply_before_start() {
    let (r, controls) = runtime();
    let p = profile();
    let key = SessionKey {
        profile_id: p.id,
        target: token(1),
    };
    configure(&r, p.clone(), token(1), true, 1);
    r.send(Command::Connect { key, revision: 2 }).unwrap();
    wait(&r, |s| s.revision == 2);
    assert_eq!(r.latest().unwrap().mounted, Some(true));
    assert_eq!(r.latest().unwrap().applied_revision, None);
    configure(&r, p, token(1), true, 2);
    controls.lock().unwrap().toggle = true;
    wait(&r, |s| s.running);
    r.shutdown().unwrap();
}

#[test]
fn failed_connection_to_different_session_retires_old_controller_and_mount() {
    let (r, controls) = runtime();
    let p = profile();
    let key = SessionKey {
        profile_id: p.id,
        target: token(1),
    };
    configure(&r, p, token(1), true, 1);
    controls.lock().unwrap().fail_attach = true;
    r.send(Command::Connect {
        key: SessionKey {
            target: token(2),
            ..key
        },
        revision: 2,
    })
    .unwrap();
    wait(&r, |s| {
        s.key.is_none() && s.reason.contains("Injected attachment failure")
    });
    assert_eq!(r.latest().unwrap().mounted, None);
    assert_eq!(
        r.latest().unwrap().activity,
        metin_core::ActivityKind::Error
    );
    assert_eq!(r.latest().unwrap().active_skill, None);
    controls.lock().unwrap().toggle = true;
    std::thread::sleep(Duration::from_millis(300));
    assert!(!r.latest().unwrap().running);
    assert_eq!(r.latest().unwrap().key, None);
    assert!(controls.lock().unwrap().calls.iter().all(|(_, down)| !down));
    r.shutdown().unwrap();
}

#[test]
fn preview_and_snapshot_are_explicit_and_old_session_requests_are_ignored() {
    let (r, controls) = runtime();
    let p = profile();
    let key = SessionKey {
        profile_id: p.id,
        target: token(1),
    };
    configure(&r, p, token(1), false, 1);
    r.send(Command::Snapshot {
        key: SessionKey {
            target: token(2),
            ..key
        },
        purpose: metin_desktop::runtime::SnapshotPurpose::UpdateLiveEvidence,
    })
    .unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(controls.lock().unwrap().captures, 0);
    r.send(Command::Snapshot {
        key,
        purpose: metin_desktop::runtime::SnapshotPurpose::UpdateLiveEvidence,
    })
    .unwrap();
    wait(&r, |s| s.frame_id > 0);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        controls.lock().unwrap().captures,
        1,
        "Snapshot is a single request"
    );
    r.send(Command::SetPreview { key, enabled: true }).unwrap();
    wait(&r, |s| s.frame_id > 2);
    r.send(Command::SetPreview {
        key,
        enabled: false,
    })
    .unwrap();
    std::thread::sleep(Duration::from_millis(150));
    let before = controls.lock().unwrap().captures;
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(controls.lock().unwrap().captures, before);
    controls.lock().unwrap().toggle = true;
    wait(&r, |s| s.running && s.frame_id > before as u64);
    r.stop();
    wait(&r, |s| !s.running);
    std::thread::sleep(Duration::from_millis(150));
    let stopped = controls.lock().unwrap().captures;
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(controls.lock().unwrap().captures, stopped);
    r.shutdown().unwrap();
}

#[test]
fn idle_snapshot_detach_and_shutdown_close_capture_resources_but_preview_can_resume() {
    let (runtime, controls) = runtime();
    let profile = profile();
    let key = SessionKey {
        profile_id: profile.id,
        target: token(1),
    };
    configure(&runtime, profile.clone(), key.target, false, 1);
    runtime
        .send(Command::SetPreview { key, enabled: true })
        .unwrap();
    wait(&runtime, |snapshot| {
        snapshot.frame_id > 0 && controls.lock().unwrap().capture_active
    });
    runtime
        .send(Command::SetPreview {
            key,
            enabled: false,
        })
        .unwrap();
    wait(&runtime, |_| !controls.lock().unwrap().capture_active);
    let previous = runtime.latest().unwrap().frame_id;
    runtime
        .send(Command::Snapshot {
            key,
            purpose: metin_desktop::runtime::SnapshotPurpose::UpdateLiveEvidence,
        })
        .unwrap();
    wait(&runtime, |snapshot| {
        snapshot.frame_id > previous && !controls.lock().unwrap().capture_active
    });
    controls.lock().unwrap().fail_capture = true;
    let previous_attempts = controls.lock().unwrap().captures;
    runtime
        .send(Command::Snapshot {
            key,
            purpose: metin_desktop::runtime::SnapshotPurpose::UpdateLiveEvidence,
        })
        .unwrap();
    wait(&runtime, |snapshot| {
        snapshot.reason.contains("Injected capture failure")
            && controls.lock().unwrap().captures > previous_attempts
    });
    runtime.stop();
    wait(&runtime, |snapshot| {
        snapshot.released_generation == Some(runtime.stop_generation())
    });
    let stopped_attempts = controls.lock().unwrap().captures;
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(
        controls.lock().unwrap().captures,
        stopped_attempts,
        "Cancel must retire a failed one-shot, not continue capturing in the background"
    );
    controls.lock().unwrap().fail_capture = false;
    runtime
        .send(Command::SetPreview { key, enabled: true })
        .unwrap();
    wait(&runtime, |_| controls.lock().unwrap().capture_active);
    runtime.send(Command::Detach).unwrap();
    wait(&runtime, |snapshot| {
        snapshot.key.is_none() && !controls.lock().unwrap().capture_active
    });
    configure(&runtime, profile, key.target, false, 2);
    runtime
        .send(Command::SetPreview { key, enabled: true })
        .unwrap();
    wait(&runtime, |_| controls.lock().unwrap().capture_active);
    runtime.shutdown().unwrap();
    assert!(!controls.lock().unwrap().capture_active);
}

#[test]
fn hp_matching_tracks_all_connected_client_corners_independent_of_target_area_and_masks() {
    let (r, controls) = runtime();
    let mut p = profile();
    p.region = Some(metin_core::Region {
        left: 0.4,
        top: 0.4,
        right: 0.6,
        bottom: 0.6,
    });
    p.masks.push(
        metin_core::Region {
            left: 0.,
            top: 0.,
            right: 1.,
            bottom: 1.,
        }
        .into(),
    );
    let key = SessionKey {
        profile_id: p.id,
        target: token(1),
    };
    controls.lock().unwrap().target_origin = [-350, 125];
    configure(&r, p, token(1), false, 1);
    for position in [[0, 0], [190, 0], [0, 190], [190, 190]] {
        controls.lock().unwrap().hp_detail = Some(position);
        r.send(Command::Snapshot {
            key,
            purpose: metin_desktop::runtime::SnapshotPurpose::UpdateLiveEvidence,
        })
        .unwrap();
        wait(&r, |s| {
            s.panel_match
                .is_some_and(|found| found.position == position && found.score > 0.9999)
        });
        let snapshot = r.latest().unwrap();
        let geometry = snapshot.geometry.unwrap();
        assert_eq!([geometry.left, geometry.top], [-350, 125]);
        assert_eq!([geometry.width, geometry.height], [200, 200]);
        assert!(snapshot.candidates.is_empty());
        assert!(!snapshot.running);
    }
    assert!(controls.lock().unwrap().calls.iter().all(|(_, down)| !down));
    r.shutdown().unwrap();
}
