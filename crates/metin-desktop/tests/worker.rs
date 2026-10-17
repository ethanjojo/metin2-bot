use metin_core::Profile;
use metin_desktop::runtime::SessionKey;
use metin_desktop::runtime::{Command, RuntimeHandle};
use metin_platform::*;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
#[derive(Default)]
struct Controls {
    hotkey_polls: usize,
    toggle: bool,
    stop: bool,
    fail_key: bool,
    lose_focus: bool,
    fail_capture: bool,
    fail_attach: bool,
    captures: usize,
    capture_limits: Vec<Duration>,
    calls: Vec<(u16, bool)>,
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
        Ok(TargetState {
            target: self.target,
            left: 0,
            top: 0,
            width: 200,
            height: 200,
            visible: true,
            foreground: !self.controls.lock().unwrap().lose_focus,
        })
    }
    fn request_activation(&mut self) -> Result<bool> {
        Ok(true)
    }
    fn capture(&mut self, deadline: Instant, _: &StopToken, _: u64) -> Result<CapturedFrame> {
        {
            let mut c = self.controls.lock().unwrap();
            c.captures += 1;
            c.capture_limits
                .push(deadline.saturating_duration_since(Instant::now()));
            if c.fail_capture {
                return Err(PlatformError("Injected capture failure".into()));
            }
        }
        self.id += 1;
        Ok(CapturedFrame {
            frame: Frame::new(
                200,
                200,
                (0..200 * 200 * 3).map(|n| 128 + (n % 31) as u8).collect(),
            )?,
            target: self.target,
            captured_at: Instant::now(),
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
    fn point_clear(&self, _: [i32; 2]) -> bool {
        true
    }
    fn cursor(&self) -> Result<[i32; 2]> {
        Ok([0, 0])
    }
    fn move_relative(&mut self, _: [i32; 2]) -> Result<()> {
        Ok(())
    }
    fn key(&mut self, key: u16, down: bool, _: bool) -> Result<()> {
        let mut c = self.controls.lock().unwrap();
        c.calls.push((key, down));
        if down && c.fail_key {
            Err(PlatformError("Injected key failure".into()))
        } else {
            Ok(())
        }
    }
    fn mouse(&mut self, _: [u32; 2], _: bool) -> Result<()> {
        Ok(())
    }
    fn hotkeys(&mut self) -> Result<HotkeyEvents> {
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
fn profile() -> Profile {
    let mut p = Profile::from_json(include_str!(
        "../../../tests/parity/profiles/calibrated.json"
    ))
    .unwrap();
    p.frame_size = Some([200, 200]);
    p.timing.sample_interval = 0.1;
    p
}
fn configure(r: &RuntimeHandle, p: Profile, target: TargetToken, mounted: bool, revision: u64) {
    let key = SessionKey {
        profile_id: p.id,
        target,
    };
    let new_session = r.latest().unwrap().key != Some(key);
    let template = Frame::new(10, 10, (0..300).map(|n| (n % 251) as u8).collect()).unwrap();
    r.send(Command::Configure {
        profile: Box::new(p),
        template: Some(template),
        target,
        revision,
    })
    .unwrap();
    wait(r, |s| s.key == Some(key) && s.revision == revision);
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
    r.stop();
    wait(&r, |s| s.state != "Armed");
    assert!(!r.latest().unwrap().arming);
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
    r.send(Command::ResetStatistics { key }).unwrap();
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
    wait(&r, |s| s.reason.contains("one to eight skills"));
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
    wait(&r, |s| s.reason.contains("Complete calibration"));
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
fn reconnecting_same_session_preserves_acknowledged_mount_and_applied_profile() {
    let (r, controls) = runtime();
    let p = profile();
    let key = SessionKey {
        profile_id: p.id,
        target: token(1),
    };
    configure(&r, p, token(1), true, 1);
    r.send(Command::Connect { key, revision: 2 }).unwrap();
    wait(&r, |s| s.revision == 2);
    assert_eq!(r.latest().unwrap().mounted, Some(true));
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
    })
    .unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(controls.lock().unwrap().captures, 0);
    r.send(Command::Snapshot { key }).unwrap();
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
