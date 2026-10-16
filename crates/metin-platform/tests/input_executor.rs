use metin_core::{Action, ActionKind, Profile};
use metin_platform::*;
use std::{cell::Cell, time::Instant};
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
