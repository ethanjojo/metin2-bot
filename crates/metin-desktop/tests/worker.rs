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
            foreground: true,
        })
    }
    fn request_activation(&mut self) -> Result<bool> {
        Ok(true)
    }
    fn capture(&mut self, _: Instant, _: &StopToken, _: u64) -> Result<CapturedFrame> {
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
    let template = Frame::new(10, 10, (0..300).map(|n| (n % 251) as u8).collect()).unwrap();
    r.send(Command::Configure {
        profile: Box::new(p),
        template: Some(template),
        target,
        mounted,
        revision,
    })
    .unwrap();
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
