//! Shared safety policy; transport remains behind the selected OS backend.
use crate::*;
use metin_core::{Action, ActionKind, Key, Profile, Shortcut};
use std::{thread, time::Duration};

#[derive(Default)]
pub struct InputExecutor {
    owned: Vec<u16>,
    mouse: Option<[u32; 2]>,
    expected_cursor: Option<[i32; 2]>,
    click_sent: bool,
}
impl InputExecutor {
    pub fn click_sent(&self) -> bool {
        self.click_sent
    }
    fn guard(
        &self,
        backend: &mut dyn PlatformBackend,
        baseline: TargetState,
        stop: &StopToken,
        generation: u64,
        point: Option<[i32; 2]>,
    ) -> Result<()> {
        if stop.cancelled(generation) || backend.emergency_pressed()? {
            return Err(PlatformError("Emergency stop (F8 / Pause).".into()));
        }
        let current = backend.inspect()?;
        if current != baseline || !current.foreground || !current.visible {
            return Err(PlatformError(
                "The window geometry or foreground focus changed.".into(),
            ));
        }
        if point.is_some_and(|p| !backend.point_clear(p)) {
            return Err(PlatformError(
                "Another window obstructs the selection area.".into(),
            ));
        }
        if let Some(expected) = self.expected_cursor {
            let actual = backend.cursor()?;
            if actual[0].abs_diff(expected[0]) > 3 || actual[1].abs_diff(expected[1]) > 3 {
                return Err(PlatformError(
                    "Manual mouse movement interrupted selection.".into(),
                ));
            }
        }
        Ok(())
    }
    fn wait(
        &self,
        seconds: f64,
        backend: &mut dyn PlatformBackend,
        baseline: TargetState,
        stop: &StopToken,
        generation: u64,
        point: Option<[i32; 2]>,
    ) -> Result<()> {
        let end = Instant::now() + Duration::from_secs_f64(seconds);
        loop {
            self.guard(backend, baseline, stop, generation, point)?;
            let now = Instant::now();
            if now >= end {
                return Ok(());
            }
            thread::sleep((end - now).min(Duration::from_millis(10)));
        }
    }
    fn down(&mut self, key: u16, backend: &mut dyn PlatformBackend) -> Result<()> {
        let alt = self.owned.contains(&0x12);
        self.owned.push(key);
        backend.key(key, true, alt)
    }
    pub fn release(&mut self, backend: &mut dyn PlatformBackend) -> Result<()> {
        let mut errors = vec![];
        if let Some(point) = self.mouse {
            match backend.mouse(point, false) {
                Ok(()) => self.mouse = None,
                Err(error) => errors.push(error.to_string()),
            }
        }
        for i in (0..self.owned.len()).rev() {
            let key = self.owned[i];
            match backend.key(key, false, self.owned.contains(&0x12)) {
                Ok(()) => {
                    self.owned.remove(i);
                }
                Err(error) => errors.push(error.to_string()),
            }
        }
        self.expected_cursor = None;
        if errors.is_empty() {
            Ok(())
        } else {
            Err(PlatformError(format!(
                "Failed to release owned input: {}",
                errors.join("; ")
            )))
        }
    }
    pub fn execute(
        &mut self,
        action: &Action,
        profile: &Profile,
        backend: &mut dyn PlatformBackend,
        stop: &StopToken,
        generation: u64,
    ) -> Result<()> {
        self.click_sent = false;
        if !self.owned.is_empty() || self.mouse.is_some() {
            return Err(PlatformError(
                "Previous owned inputs must be released first.".into(),
            ));
        }
        self.expected_cursor = None;
        let baseline = backend.inspect()?;
        backend.check_input_access()?;
        self.guard(backend, baseline, stop, generation, None)?;
        if [
            1, 2, 0x10, 0x11, 0x12, 0xA2, 0xA3, 0x51, 0x45, 0x57, 0x41, 0x53, 0x44,
        ]
        .into_iter()
        .any(|key| backend.key_held(key))
        {
            return Err(PlatformError(
                "Release mouse buttons and control keys before starting.".into(),
            ));
        }
        let result = (|| match &action.kind {
            ActionKind::Click { point } => {
                if point[0] >= baseline.width || point[1] >= baseline.height {
                    return Err(PlatformError(
                        "Click is outside the calibrated frame.".into(),
                    ));
                }
                let target = [
                    baseline.left + point[0] as i32,
                    baseline.top + point[1] as i32,
                ];
                let deadline = Instant::now() + Duration::from_millis(500);
                let mut gain = 2.0_f64;
                let mut previous: Option<([i32; 2], [i32; 2])> = None;
                loop {
                    self.guard(backend, baseline, stop, generation, Some(target))?;
                    let actual = backend.cursor()?;
                    if actual[0].abs_diff(target[0]) <= 3 && actual[1].abs_diff(target[1]) <= 3 {
                        break;
                    }
                    if Instant::now() > deadline {
                        return Err(PlatformError("Cursor movement was not confirmed.".into()));
                    }
                    if let Some((old, step)) = previous {
                        let moved = actual[0].abs_diff(old[0]).max(actual[1].abs_diff(old[1]));
                        gain = gain.max(
                            (f64::from(moved)
                                / f64::from(step[0].unsigned_abs().max(step[1].unsigned_abs()))
                                + 0.25)
                                .min(8.),
                        );
                    }
                    let dx = target[0] - actual[0];
                    let dy = target[1] - actual[1];
                    let scale =
                        (1. / gain).min(64. / f64::from(dx.unsigned_abs().max(dy.unsigned_abs())));
                    let mut step = [
                        (f64::from(dx) * scale).round_ties_even() as i32,
                        (f64::from(dy) * scale).round_ties_even() as i32,
                    ];
                    if step == [0, 0] {
                        step = if dx.abs() >= dy.abs() {
                            [dx.signum(), 0]
                        } else {
                            [0, dy.signum()]
                        };
                    }
                    backend.move_relative(step)?;
                    previous = Some((actual, step));
                    thread::sleep(Duration::from_millis(5));
                }
                self.expected_cursor = Some(target);
                self.wait(
                    profile.timing.click_hover_seconds,
                    backend,
                    baseline,
                    stop,
                    generation,
                    Some(target),
                )?;
                self.mouse = Some(*point);
                backend.mouse(*point, true)?;
                self.click_sent = true;
                self.wait(
                    profile.timing.click_hold_seconds,
                    backend,
                    baseline,
                    stop,
                    generation,
                    Some(target),
                )
            }
            ActionKind::Rotate => {
                self.down(0x45, backend)?;
                self.wait(
                    profile.timing.rotate_seconds,
                    backend,
                    baseline,
                    stop,
                    generation,
                    None,
                )
            }
            ActionKind::Mount { shortcut } | ActionKind::Skill { shortcut, .. } => {
                let shortcut =
                    Shortcut::parse(shortcut).map_err(|e| PlatformError(e.to_string()))?;
                let mut keys = vec![];
                if shortcut.shift {
                    keys.push(0x10);
                }
                if shortcut.ctrl {
                    keys.push(0x11);
                }
                if shortcut.alt {
                    keys.push(0x12);
                }
                keys.push(match shortcut.key {
                    Key::Letter(c) => c as u16,
                    Key::Digit(n) => u16::from(b'0' + n),
                    Key::Function(n) => 0x6f + u16::from(n),
                });
                if keys.iter().any(|key| backend.key_held(*key)) {
                    return Err(PlatformError(
                        "Release the skill or mount shortcut keys.".into(),
                    ));
                }
                for key in &keys[..keys.len() - 1] {
                    self.guard(backend, baseline, stop, generation, None)?;
                    self.down(*key, backend)?;
                }
                if keys.len() > 1 {
                    self.wait(0.1, backend, baseline, stop, generation, None)?;
                }
                self.guard(backend, baseline, stop, generation, None)?;
                let main = *keys.last().expect("main key");
                self.down(main, backend)?;
                self.wait(
                    profile.buffs.key_hold_seconds,
                    backend,
                    baseline,
                    stop,
                    generation,
                    None,
                )?;
                if keys.len() > 1 {
                    backend.key(main, false, self.owned.contains(&0x12))?;
                    self.owned.pop();
                    self.wait(0.05, backend, baseline, stop, generation, None)?;
                }
                Ok(())
            }
        })();
        let release = self.release(backend);
        match (result, release) {
            (Err(error), Err(cleanup)) => Err(PlatformError(format!("{error}; {cleanup}"))),
            (Err(error), _) => Err(error),
            (_, cleanup) => cleanup,
        }
    }
}
