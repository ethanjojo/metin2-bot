use metin_core::{Action, ActionKind, Profile};
use metin_platform::*;
use std::time::Duration;
use std::{cell::Cell, time::Instant};

#[test]
fn loot_uses_guarded_shortcut_and_releases_at_safety_deadline() {
    let mut backend = Fake::new();
    let mut profile = Profile::create("Test");
    profile.loot.shortcut = "Ctrl+Z".into();
    profile.loot.hold_seconds = 0.5;
    let action = Action {
        id: 1,
        kind: ActionKind::Loot {
            shortcut: profile.loot.shortcut.clone(),
        },
    };
    let mut executor = InputExecutor::default();
    executor.set_deadline(Some(Instant::now() + std::time::Duration::from_millis(160)));
    let stop = StopToken::default();
    let result = executor.execute(&action, &profile, &mut backend, &stop, stop.generation());
    assert!(result.unwrap_err().to_string().contains("safety limit"));
    assert!(backend.events.contains(&(b'Z' as u16, true)));
    assert_eq!(backend.events.last(), Some(&(0x11, false)));
    assert!(backend.events.contains(&(b'Z' as u16, false)));
}
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Fault {
    #[default]
    None,
    Focus,
    Geometry,
    Emergency,
    ManualMouse,
}
struct Fake {
    state: TargetState,
    events: Vec<(u16, bool)>,
    fail_down: bool,
    fail_up: bool,
    denied: bool,
    blocked: bool,
    held: Option<u16>,
    fault: Fault,
    emergency: bool,
    cursor_position: Cell<[i32; 2]>,
    mouse_events: Vec<bool>,
}
impl Fake {
    fn new() -> Self {
        Self {
            state: TargetState {
                target: TargetToken {
                    handle: 1,
                    pid: 2,
                    created: 3,
                },
                left: 0,
                top: 0,
                width: 800,
                height: 600,
                visible: true,
                foreground: true,
            },
            events: vec![],
            fail_down: false,
            fail_up: false,
            denied: false,
            blocked: false,
            held: None,
            fault: Fault::None,
            emergency: false,
            cursor_position: Cell::new([0, 0]),
            mouse_events: vec![],
        }
    }
}
impl PlatformBackend for Fake {
    fn capabilities(&self) -> CapabilityReport {
        Default::default()
    }
    fn list_targets(&mut self) -> Result<Vec<TargetInfo>> {
        Ok(vec![])
    }
    fn attach(&mut self, _: TargetToken) -> Result<()> {
        Ok(())
    }
    fn stop_capture(&mut self) {}
    fn inspect(&mut self) -> Result<TargetState> {
        Ok(self.state)
    }
    fn request_activation(&mut self) -> Result<bool> {
        Ok(true)
    }
    fn capture(&mut self, _: Instant, _: &StopToken, _: u64) -> Result<CapturedFrame> {
        unreachable!()
    }
    fn check_input_access(&mut self) -> Result<()> {
        if self.denied {
            return Err(PlatformError("permission denied".into()));
        }
        Ok(())
    }
    fn key_held(&self, key: u16) -> bool {
        self.held == Some(key)
    }
    fn point_clear(&self, _: [i32; 2]) -> bool {
        !self.blocked
    }
    fn cursor(&self) -> Result<[i32; 2]> {
        Ok(self.cursor_position.get())
    }
    fn move_relative(&mut self, delta: [i32; 2]) -> Result<()> {
        let p = self.cursor_position.get();
        self.cursor_position.set([p[0] + delta[0], p[1] + delta[1]]);
        Ok(())
    }
    fn key(&mut self, key: u16, down: bool, _: bool) -> Result<()> {
        self.events.push((key, down));
        if down {
            match self.fault {
                Fault::Focus => self.state.foreground = false,
                Fault::Geometry => self.state.width += 1,
                Fault::Emergency => self.emergency = true,
                _ => {}
            }
        }
        if !down && self.fail_up {
            return Err(PlatformError("injected release error".into()));
        }
        if down && self.fail_down {
            Err(PlatformError("injected unknown send outcome".into()))
        } else {
            Ok(())
        }
    }
    fn mouse(&mut self, _: [u32; 2], down: bool) -> Result<()> {
        self.mouse_events.push(down);
        if down && self.fault == Fault::ManualMouse {
            self.cursor_position.set([99, 99]);
        }
        Ok(())
    }
    fn emergency_pressed(&mut self) -> Result<bool> {
        Ok(self.emergency)
    }
    fn hotkeys(&mut self) -> Result<HotkeyEvents> {
        Ok(HotkeyEvents {
            toggle: false,
            stop: false,
        })
    }
}

fn execute(fake: &mut Fake, kind: ActionKind) -> Result<()> {
    InputExecutor::default().execute(
        &Action { id: 1, kind },
        &Profile::create("Test"),
        fake,
        &StopToken::default(),
        0,
    )
}
#[test]
fn loss_of_focus_geometry_or_short_emergency_during_hold_releases_the_key() {
    for fault in [Fault::Focus, Fault::Geometry, Fault::Emergency] {
        let mut fake = Fake::new();
        fake.fault = fault;
        assert!(execute(&mut fake, ActionKind::Rotate).is_err());
        assert_eq!(fake.events, vec![(0x45, true), (0x45, false)]);
    }
}
#[test]
fn permission_and_physically_held_keys_block_all_down_events() {
    for denied in [false, true] {
        let mut fake = Fake::new();
        fake.denied = denied;
        fake.held = Some(0x11);
        assert!(execute(&mut fake, ActionKind::Rotate).is_err());
        assert!(fake.events.is_empty());
    }
}
#[test]
fn obstruction_prevents_mouse_down_and_manual_motion_releases_a_pressed_button() {
    let mut fake = Fake::new();
    fake.blocked = true;
    assert!(execute(&mut fake, ActionKind::Click { point: [0, 0] }).is_err());
    assert!(fake.mouse_events.is_empty());
    fake.blocked = false;
    fake.fault = Fault::ManualMouse;
    assert!(execute(&mut fake, ActionKind::Click { point: [0, 0] }).is_err());
    assert_eq!(fake.mouse_events, vec![true, false]);
}
#[test]
fn modifiers_release_in_reverse_order_after_main_key() {
    let mut fake = Fake::new();
    execute(
        &mut fake,
        ActionKind::Skill {
            shortcut: "Ctrl+Shift+Alt+G".into(),
            index: 0,
        },
    )
    .unwrap();
    assert_eq!(
        fake.events,
        vec![
            (0x10, true),
            (0x11, true),
            (0x12, true),
            (0x47, true),
            (0x47, false),
            (0x12, false),
            (0x11, false),
            (0x10, false)
        ]
    );
}
#[test]
fn failed_cleanup_blocks_next_action_until_release_succeeds_and_is_idempotent() {
    let mut fake = Fake::new();
    fake.fail_up = true;
    let mut executor = InputExecutor::default();
    let action = Action {
        id: 1,
        kind: ActionKind::Rotate,
    };
    let profile = Profile::create("Test");
    let stop = StopToken::default();
    assert!(
        executor
            .execute(&action, &profile, &mut fake, &stop, 0)
            .is_err()
    );
    let count = fake.events.len();
    assert!(
        executor
            .execute(&action, &profile, &mut fake, &stop, 0)
            .is_err()
    );
    assert_eq!(fake.events.len(), count);
    fake.fail_up = false;
    executor.release(&mut fake).unwrap();
    let count = fake.events.len();
    executor.release(&mut fake).unwrap();
    assert_eq!(fake.events.len(), count);
}
#[test]
fn failed_down_is_still_released() {
    let mut fake = Fake::new();
    fake.fail_down = true;
    let stop = StopToken::default();
    let mut executor = InputExecutor::default();
    assert!(
        executor
            .execute(
                &Action {
                    id: 1,
                    kind: ActionKind::Rotate
                },
                &Profile::create("Test"),
                &mut fake,
                &stop,
                0
            )
            .is_err()
    );
    assert_eq!(fake.events, vec![(0x45, true), (0x45, false)]);
}
#[test]
fn cancelled_generation_sends_no_down() {
    let mut fake = Fake::new();
    let stop = StopToken::default();
    stop.stop();
    assert!(
        InputExecutor::default()
            .execute(
                &Action {
                    id: 1,
                    kind: ActionKind::Rotate
                },
                &Profile::create("Test"),
                &mut fake,
                &stop,
                0
            )
            .is_err()
    );
    assert!(fake.events.is_empty());
}
#[test]
fn lost_focus_sends_no_down() {
    let mut fake = Fake::new();
    fake.state.foreground = false;
    let stop = StopToken::default();
    assert!(
        InputExecutor::default()
            .execute(
                &Action {
                    id: 1,
                    kind: ActionKind::Rotate
                },
                &Profile::create("Test"),
                &mut fake,
                &stop,
                0
            )
            .is_err()
    );
    assert!(fake.events.is_empty());
}

#[test]
fn scheduled_and_recovery_actions_release_keys_on_emergency() {
    for kind in [
        ActionKind::Scheduled {
            input: metin_core::ActionInput::Key {
                shortcut: "Ctrl+G".into(),
                hold_seconds: 0.3,
            },
            key: metin_core::ScheduledKey::Puller,
        },
        ActionKind::Recover,
    ] {
        let mut fake = Fake::new();
        fake.fault = Fault::Emergency;
        assert!(execute(&mut fake, kind).is_err());
        assert!(!fake.events.is_empty());
        for &(key, down) in &fake.events {
            if down {
                assert!(fake.events.contains(&(key, false)));
            }
        }
        assert!(
            !fake.events.contains(&(0x45, true)),
            "Recovery must stop before camera movement"
        );
    }
}

#[test]
fn attack_hold_persists_without_blocking_and_releases_on_cancel_focus_or_deadline() {
    for fault in [Fault::Focus, Fault::Geometry, Fault::Emergency] {
        let mut fake = Fake::new();
        let stop = StopToken::default();
        let mut executor = InputExecutor::default();
        executor
            .execute(
                &Action {
                    id: 1,
                    kind: ActionKind::Attack {
                        shortcut: "Space".into(),
                        duration_seconds: 3.,
                    },
                },
                &Profile::create("Hold"),
                &mut fake,
                &stop,
                0,
            )
            .unwrap();
        assert_eq!(fake.events, vec![(0x20, true)]);
        match fault {
            Fault::Focus => fake.state.foreground = false,
            Fault::Geometry => fake.state.width += 1,
            Fault::Emergency => fake.emergency = true,
            _ => unreachable!(),
        }
        assert!(executor.poll_attack(&mut fake, &stop, 0).is_err());
        assert_eq!(fake.events, vec![(0x20, true), (0x20, false)]);
    }
    let mut fake = Fake::new();
    let stop = StopToken::default();
    let mut executor = InputExecutor::default();
    executor
        .execute(
            &Action {
                id: 1,
                kind: ActionKind::Attack {
                    shortcut: "Space".into(),
                    duration_seconds: 0.02,
                },
            },
            &Profile::create("Hold"),
            &mut fake,
            &stop,
            0,
        )
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(25));
    executor.poll_attack(&mut fake, &stop, 0).unwrap();
    assert_eq!(fake.events, vec![(0x20, true), (0x20, false)]);
    assert!(!executor.attack_held());
}
#[test]
fn attack_key_is_released_before_another_action_and_on_generation_change() {
    let mut fake = Fake::new();
    let stop = StopToken::default();
    let mut executor = InputExecutor::default();
    let p = Profile::create("Hold");
    executor
        .execute(
            &Action {
                id: 1,
                kind: ActionKind::Attack {
                    shortcut: "Space".into(),
                    duration_seconds: 3.,
                },
            },
            &p,
            &mut fake,
            &stop,
            0,
        )
        .unwrap();
    executor
        .execute(
            &Action {
                id: 2,
                kind: ActionKind::Rotate,
            },
            &p,
            &mut fake,
            &stop,
            0,
        )
        .unwrap();
    assert_eq!(
        &fake.events[..3],
        &[(0x20, true), (0x20, false), (0x45, true)]
    );
    executor
        .execute(
            &Action {
                id: 3,
                kind: ActionKind::Attack {
                    shortcut: "Space".into(),
                    duration_seconds: 3.,
                },
            },
            &p,
            &mut fake,
            &stop,
            0,
        )
        .unwrap();
    stop.stop();
    assert!(executor.poll_attack(&mut fake, &stop, 0).is_err());
    assert_eq!(fake.events.last(), Some(&(0x20, false)));
}

#[test]
fn safety_deadline_interrupts_a_persistent_attack_and_failed_release_blocks_followup() {
    let mut fake = Fake::new();
    let stop = StopToken::default();
    let mut e = InputExecutor::default();
    let p = Profile::create("Hold");
    e.set_deadline(Some(Instant::now() + std::time::Duration::from_millis(15)));
    e.execute(
        &Action {
            id: 1,
            kind: ActionKind::Attack {
                shortcut: "Space".into(),
                duration_seconds: 3.,
            },
        },
        &p,
        &mut fake,
        &stop,
        0,
    )
    .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    fake.fail_up = true;
    assert!(e.poll_attack(&mut fake, &stop, 0).is_err());
    let old = fake.events.len();
    assert!(
        e.execute(
            &Action {
                id: 2,
                kind: ActionKind::Rotate
            },
            &p,
            &mut fake,
            &stop,
            0
        )
        .is_err()
    );
    assert!(!fake.events[old..].iter().any(|(_, down)| *down));
    fake.fail_up = false;
    e.release(&mut fake).unwrap();
    assert!(!e.attack_held());
}
#[test]
fn configured_click_and_key_share_guarded_execution_and_release() {
    use metin_core::{ActionInput, Region, ScheduledKey};
    let mut profile = Profile::create("Configured");
    profile.frame_size = Some([800, 600]);
    let region = Region {
        left: 0.2,
        top: 0.3,
        right: 0.4,
        bottom: 0.5,
    };
    for kind in [
        ActionKind::Configured {
            input: ActionInput::Click {
                region: Some(region),
            },
        },
        ActionKind::Scheduled {
            input: ActionInput::Click {
                region: Some(region),
            },
            key: ScheduledKey::Puller,
        },
    ] {
        let mut backend = Fake::new();
        let stop = StopToken::default();
        let mut executor = InputExecutor::default();
        executor
            .execute(
                &Action {
                    id: 1,
                    kind: kind.clone(),
                },
                &profile,
                &mut backend,
                &stop,
                stop.generation(),
            )
            .unwrap();
        assert!(
            backend
                .cursor_position
                .get()
                .into_iter()
                .all(|n| n.abs_diff(240) <= 3)
        );
        assert_eq!(backend.mouse_events, vec![true, false]);
        let mut backend = Fake::new();
        backend.blocked = true;
        assert!(
            executor
                .execute(
                    &Action {
                        id: 2,
                        kind: kind.clone()
                    },
                    &profile,
                    &mut backend,
                    &stop,
                    stop.generation()
                )
                .is_err()
        );
        assert!(backend.mouse_events.is_empty());
        let mut backend = Fake::new();
        backend.state.width += 1;
        assert!(
            executor
                .execute(
                    &Action { id: 3, kind },
                    &profile,
                    &mut backend,
                    &stop,
                    stop.generation()
                )
                .is_err()
        );
        assert!(backend.mouse_events.is_empty());
    }
    let mut backend = Fake::new();
    let mut executor = InputExecutor::default();
    let stop = StopToken::default();
    executor
        .execute(
            &Action {
                id: 4,
                kind: ActionKind::Configured {
                    input: ActionInput::Key {
                        shortcut: "Ctrl+G".into(),
                        hold_seconds: 0.05,
                    },
                },
            },
            &profile,
            &mut backend,
            &stop,
            stop.generation(),
        )
        .unwrap();
    assert_eq!(
        backend.events,
        vec![
            (0x11, true),
            (b'G' as u16, true),
            (b'G' as u16, false),
            (0x11, false)
        ]
    );
}
#[test]
fn visual_precondition_is_checked_after_preparation_and_cancels_down() {
    use metin_core::ActionInput;
    for input in [
        ActionInput::Click {
            region: Some(metin_core::Region {
                left: 0.2,
                top: 0.3,
                right: 0.4,
                bottom: 0.5,
            }),
        },
        ActionInput::Key {
            shortcut: "Ctrl+G".into(),
            hold_seconds: 0.1,
        },
    ] {
        let mut profile = Profile::create("Verify");
        profile.frame_size = Some([800, 600]);
        profile.timing.click_hover_seconds = 0.01;
        let mut backend = Fake::new();
        let mut executor = InputExecutor::default();
        let stop = StopToken::default();
        let mut calls = 0;
        let mut verify = |backend: &mut dyn PlatformBackend| {
            calls += 1;
            let _ = backend.inspect()?;
            Err(PlatformError("event disappeared".into()))
        };
        let error = executor
            .execute_verified(
                &Action {
                    id: 1,
                    kind: ActionKind::Configured { input },
                },
                &profile,
                &mut backend,
                &stop,
                stop.generation(),
                Some(&mut verify),
            )
            .unwrap_err();
        assert!(error.to_string().contains("event disappeared"));
        assert_eq!(calls, 1);
        assert!(backend.mouse_events.is_empty());
        assert!(!backend.events.contains(&(b'G' as u16, true)));
        for &(key, down) in &backend.events {
            if down {
                assert!(backend.events.contains(&(key, false)));
            }
        }
    }
}
#[test]
fn cancelled_verification_after_capture_never_sends_input() {
    let mut profile = Profile::create("Verify");
    profile.frame_size = Some([800, 600]);
    let mut backend = Fake::new();
    let mut executor = InputExecutor::default();
    let stop = StopToken::default();
    let generation = stop.generation();
    let mut verify = |_: &mut dyn PlatformBackend| {
        stop.stop();
        Ok(())
    };
    let action = Action {
        id: 1,
        kind: ActionKind::Configured {
            input: metin_core::ActionInput::Key {
                shortcut: "G".into(),
                hold_seconds: 0.1,
            },
        },
    };
    assert!(
        executor
            .execute_verified(
                &action,
                &profile,
                &mut backend,
                &stop,
                generation,
                Some(&mut verify)
            )
            .is_err()
    );
    assert!(backend.events.is_empty());
}
#[test]
fn continuous_space_attack_sends_one_down_and_remains_held_across_worker_polls() {
    let mut backend = Fake::new();
    let stop = StopToken::default();
    let mut executor = InputExecutor::default();
    executor
        .execute(
            &Action {
                id: 1,
                kind: ActionKind::Attack {
                    shortcut: "Space".into(),
                    duration_seconds: 10.,
                },
            },
            &Profile::create("Continuous"),
            &mut backend,
            &stop,
            stop.generation(),
        )
        .unwrap();
    for _ in 0..5 {
        executor
            .poll_attack(&mut backend, &stop, stop.generation())
            .unwrap();
        assert!(executor.attack_held());
        assert_eq!(backend.events, vec![(0x20, true)]);
    }
    executor.release(&mut backend).unwrap();
    assert_eq!(backend.events, vec![(0x20, true), (0x20, false)]);
}

#[test]
fn capture_is_deferred_near_safety_deadline_even_if_attack_has_time_left() {
    let mut backend = Fake::new();
    let mut executor = InputExecutor::default();
    let stop = StopToken::default();
    executor
        .execute(
            &Action {
                id: 1,
                kind: ActionKind::Attack {
                    shortcut: "Space".into(),
                    duration_seconds: 1.,
                },
            },
            &Profile::create("Deadline"),
            &mut backend,
            &stop,
            stop.generation(),
        )
        .unwrap();
    executor.set_deadline(Some(Instant::now() + Duration::from_millis(10)));
    assert!(
        executor.capture_budget().is_none(),
        "Do not start capture when the stop deadline is imminent"
    );
    executor.release(&mut backend).unwrap();
    assert_eq!(backend.events, vec![(0x20, true), (0x20, false)]);
}

#[test]
fn paused_attack_free_capture_budget_is_also_limited_by_safety_deadline() {
    let mut executor = InputExecutor::default();
    executor.set_deadline(Some(Instant::now() + Duration::from_millis(250)));
    assert!(executor.capture_budget().unwrap() <= Duration::from_millis(150));
}

#[test]
fn a_slow_budgeted_capture_is_skipped_near_stop_and_attack_releases_automatically() {
    let mut backend = Fake::new();
    let mut executor = InputExecutor::default();
    let stop = StopToken::default();
    executor
        .execute(
            &Action {
                id: 1,
                kind: ActionKind::Attack {
                    shortcut: "Space".into(),
                    duration_seconds: 1.,
                },
            },
            &Profile::create("Slow capture"),
            &mut backend,
            &stop,
            stop.generation(),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_millis(10);
    executor.set_deadline(Some(deadline));
    let mut captures = 0;
    loop {
        if executor
            .poll_attack(&mut backend, &stop, stop.generation())
            .is_err()
        {
            break;
        }
        if let Some(budget) = executor.capture_budget() {
            captures += 1;
            // Original audit capture took 90 ms inside its 100-ms allowance.
            std::thread::sleep(budget.min(Duration::from_millis(90)));
        } else {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    assert_eq!(
        captures, 0,
        "Do not grant a capture that could run past stop"
    );
    assert!(!executor.attack_held());
    assert_eq!(backend.events, vec![(0x20, true), (0x20, false)]);
}
