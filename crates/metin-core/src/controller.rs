use crate::{BuffCycle, Presence, Profile, Result, Session, require};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub point: [u32; 2],
}
#[derive(Debug, Clone, Default)]
pub struct Observation {
    pub ready: bool,
    pub candidates: Vec<Candidate>,
    pub panel_present: bool,
    pub reason: String,
    pub buffs: Vec<Presence>,
}
#[derive(Debug, Clone, PartialEq)]
pub enum ActionKind {
    Click { point: [u32; 2] },
    Rotate,
    Mount { shortcut: String },
    Skill { shortcut: String, index: usize },
}
#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub id: u64,
    pub kind: ActionKind,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Initial,
    Seeking,
    Acquiring,
    Active,
    Settling,
    Buffing,
}
impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::Seeking => "seeking",
            Self::Acquiring => "acquiring",
            Self::Active => "active",
            Self::Settling => "settling",
            Self::Buffing => "buffing",
        }
    }
}
#[derive(Debug, Clone)]
pub struct Controller {
    pub profile: Profile,
    pub session: Session,
    pub buffs: BuffCycle,
    pub running: bool,
    pub phase: Phase,
    pub state: String,
    pub reason: String,
    since: f64,
    present: u32,
    absent: u32,
    last_observation: Option<f64>,
    rotations: u32,
    failures: u32,
    failed: Vec<([u32; 2], f64)>,
    clicked: Option<[u32; 2]>,
    uncertain_since: Option<f64>,
    attempted: HashSet<usize>,
    next_action: u64,
    pending_action: Option<Action>,
}
impl Controller {
    pub fn new(profile: Profile) -> Self {
        Self {
            buffs: BuffCycle::new(profile.buffs.clone()),
            profile,
            session: Session::default(),
            running: false,
            phase: Phase::Initial,
            state: "Stopped".into(),
            reason: "Calibrate the profile and start the bot.".into(),
            since: 0.,
            present: 0,
            absent: 0,
            last_observation: None,
            rotations: 0,
            failures: 0,
            failed: vec![],
            clicked: None,
            uncertain_since: None,
            attempted: HashSet::new(),
            next_action: 0,
            pending_action: None,
        }
    }
    /// Uses the same failure exclusion as acquisition; previews must not invent
    /// a separate target-selection algorithm.
    pub fn next_candidate<'a>(&self, candidates: &'a [Candidate]) -> Option<&'a Candidate> {
        candidates.iter().find(|c| self.available_candidate(c))
    }
    fn same_target(c: &Candidate, point: [u32; 2]) -> bool {
        c.point[0].abs_diff(point[0]) <= 15.max(c.width)
            && c.point[1].abs_diff(point[1]) <= 15.max(c.height)
    }
    fn available_candidate(&self, c: &Candidate) -> bool {
        !self
            .failed
            .iter()
            .any(|(point, _)| Self::same_target(c, *point))
    }
    pub fn next_preview_target(&self, candidates: &[Candidate]) -> Option<[u32; 2]> {
        if !self.running {
            return None;
        }
        candidates
            .iter()
            .find(|c| {
                self.available_candidate(c)
                    && self
                        .selected_target()
                        .is_none_or(|point| !Self::same_target(c, point))
            })
            .map(|c| c.point)
    }
    pub fn selected_target(&self) -> Option<[u32; 2]> {
        if self.running && matches!(self.phase, Phase::Acquiring | Phase::Active) {
            self.clicked
        } else {
            None
        }
    }
    fn status(&mut self, state: &str, reason: impl Into<String>) {
        self.state = state.into();
        self.reason = reason.into();
    }
    pub fn start(&mut self, now: f64) -> Result<()> {
        require(now.is_finite(), "Invalid monotonic time.")?;
        if !self.profile.buffs.ready() {
            self.pause(
                now,
                "Select active icons and a buff area for toggle skills.",
            );
            return Err(crate::ValidationError(self.reason.clone()));
        }
        if self.profile.buffs.enabled && self.buffs.mounted.is_none() {
            self.pause(now, "Mount state is unknown. Set it before starting.");
            return Err(crate::ValidationError(self.reason.clone()));
        }
        self.buffs.resume(now);
        self.running = true;
        self.phase = Phase::Initial;
        self.since = now;
        self.present = 0;
        self.absent = 0;
        self.last_observation = None;
        self.rotations = 0;
        self.failures = 0;
        self.failed.clear();
        self.clicked = None;
        self.uncertain_since = None;
        self.attempted.clear();
        self.pending_action = None;
        self.status(
            "Checking target",
            "Checking whether a target panel is already present.",
        );
        Ok(())
    }
    pub fn pause(&mut self, now: f64, reason: impl Into<String>) {
        if self.running {
            self.buffs.pause(now);
        }
        self.session.pause(Some(now));
        self.running = false;
        self.pending_action = None;
        self.status("Paused", reason);
    }
    pub fn remaining(&self, now: f64) -> Option<f64> {
        if !self.running {
            return None;
        }
        let t = &self.profile.timing;
        let limit = match self.phase {
            Phase::Buffing => return Some((self.buffs.wait_until - now).max(0.)),
            Phase::Initial | Phase::Acquiring => t.acquire_timeout,
            Phase::Seeking => t.search_timeout,
            Phase::Active => t.attack_timeout,
            Phase::Settling => t.settle_seconds,
        };
        Some((limit - (now - self.since)).max(0.))
    }
    fn active(&mut self, now: f64) {
        self.phase = Phase::Active;
        self.since = now;
        self.rotations = 0;
        self.failures = 0;
        self.status(
            "Target active",
            "Panel confirmed; waiting for the target to end.",
        );
    }
    fn issue(&mut self, kind: ActionKind) -> Option<Action> {
        self.next_action += 1;
        let a = Action {
            id: self.next_action,
            kind,
        };
        self.pending_action = Some(a.clone());
        Some(a)
    }
    pub fn ack(&mut self, now: f64, id: u64, success: bool) -> Result<()> {
        require(
            self.pending_action.as_ref().is_some_and(|a| a.id == id),
            "Stale or duplicate action acknowledgement.",
        )?;
        let a = self.pending_action.take().expect("checked action");
        if !success {
            if matches!(a.kind, ActionKind::Mount { .. }) {
                self.buffs.mounted = None;
            }
            self.pause(now, "Input action failed; check the game before resuming.");
            return Ok(());
        }
        if self.phase == Phase::Buffing {
            self.buffs.acknowledge(now)?;
        }
        Ok(())
    }
    fn buff_step(&mut self, now: f64, states: &[Presence]) -> Option<Action> {
        if let Some(step) = self.buffs.step(now, states) {
            if let Some(i) = step.skill_index {
                self.attempted.insert(i);
            }
            self.status(
                "Buffing",
                format!("{}; waiting for the animation to finish.", step.label),
            );
            return self.issue(if step.mounted_after.is_some() {
                ActionKind::Mount {
                    shortcut: step.shortcut,
                }
            } else {
                ActionKind::Skill {
                    shortcut: step.shortcut,
                    index: step.skill_index.expect("skill step"),
                }
            });
        }
        if !self.buffs.active {
            self.phase = Phase::Settling;
            self.since = now;
            self.present = 0;
            self.absent = 0;
            self.status(
                "Target ended",
                format!(
                    "Buffing complete; letting the frame settle. {}",
                    self.buffs.skipped.join(", ")
                ),
            );
        }
        None
    }
    pub fn step(&mut self, now: f64, o: &Observation) -> Option<Action> {
        if !self.running {
            return None;
        }
        if !now.is_finite() || !o.ready {
            self.pause(
                now,
                if o.reason.is_empty() {
                    "Invalid frame."
                } else {
                    &o.reason
                },
            );
            return None;
        }
        if self.phase == Phase::Buffing {
            if o.panel_present {
                self.pause(
                    now,
                    "A target panel appeared while buffing. Check the game before resuming.",
                );
                return None;
            }
            return self.buff_step(now, &o.buffs);
        }
        if self.pending_action.is_some() {
            return None;
        }
        if !self.last_observation.is_some_and(|last| {
            now > last && now - last <= 1.0_f64.max(self.profile.timing.sample_interval * 1.5)
        }) {
            self.present = 0;
            self.absent = 0;
        }
        self.last_observation = Some(now);
        if o.panel_present {
            self.present += 1;
            self.absent = 0;
        } else {
            self.absent += 1;
            self.present = 0;
        }
        let required = self.profile.panel.stable_frames;
        let t = self.profile.timing.clone();
        if !matches!(self.phase, Phase::Active | Phase::Acquiring) {
            if self.present < required && self.absent < required {
                let since = *self.uncertain_since.get_or_insert(now);
                if now - since >= t.acquire_timeout {
                    self.pause(
                        now,
                        "Target panel detection is uncertain. Check the template.",
                    );
                    return None;
                }
            } else {
                self.uncertain_since = None;
            }
        }
        if self.phase == Phase::Active {
            if let Some(remaining) = self.buffs.remaining(now, false) {
                self.status(
                    "Target active",
                    if remaining == 0. {
                        "Scheduled buffs are waiting for the target to end.".into()
                    } else {
                        format!("Scheduled buffs in {remaining:.0} s.")
                    },
                );
            }
            if now - self.since >= t.attack_timeout {
                self.session.failed += 1;
                self.pause(now, "Target timed out. Check the attack or an obstruction.");
                return None;
            }
            if self.absent >= required {
                self.session.completed += 1;
                self.phase = Phase::Settling;
                self.since = now;
                self.status(
                    "Target ended",
                    "Panel disappeared; letting the frame settle before the next target.",
                );
            }
            return None;
        }
        if self.phase == Phase::Acquiring {
            if self.present >= required {
                self.active(now);
            } else if now - self.since >= t.acquire_timeout {
                self.session.failed += 1;
                self.failures += 1;
                if self.failures >= t.max_rotations {
                    self.pause(
                        now,
                        "Target panel was not confirmed within the click retry limit.",
                    );
                    return None;
                }
                if let Some(point) = self.clicked {
                    self.failed.push((point, now + t.failed_cooldown));
                }
                self.phase = Phase::Seeking;
                self.since = now;
                self.status(
                    "Searching",
                    "Click did not confirm a panel; temporarily skipping this point.",
                );
            }
            return None;
        }
        if o.panel_present {
            if self.present >= required {
                self.active(now);
            }
            return None;
        }
        if self.phase == Phase::Initial {
            let frames = if self.profile.buffs.enabled {
                required.max(3)
            } else {
                required
            };
            if self.absent < frames {
                return None;
            }
            self.phase = Phase::Seeking;
        }
        if self.phase == Phase::Settling {
            if now - self.since < t.settle_seconds {
                return None;
            }
            self.phase = Phase::Seeking;
            self.since = now;
        }
        if self.absent < required {
            self.status(
                "Checking panel",
                "Waiting for confirmed target panel absence.",
            );
            return None;
        }
        if self.buffs.needed(now, &o.buffs, &self.attempted) {
            self.buffs.begin(now, &o.buffs, &self.attempted);
            self.phase = Phase::Buffing;
            return self.buff_step(now, &o.buffs);
        }
        self.failed.retain(|(_, until)| now < *until);
        if let Some(c) = self.next_candidate(&o.candidates) {
            let point = c.point;
            self.clicked = Some(point);
            self.attempted.clear();
            self.phase = Phase::Acquiring;
            self.since = now;
            self.present = 0;
            self.absent = 0;
            self.status(
                "Acquiring target",
                "Click sent; waiting for a stable target panel.",
            );
            return self.issue(ActionKind::Click { point });
        }
        self.status(
            "Searching",
            "Waiting for a stable color candidate in the search area.",
        );
        if now - self.since >= t.search_timeout {
            if self.rotations >= t.max_rotations {
                self.pause(now, "No Metin found within the search limit.");
                return None;
            }
            self.rotations += 1;
            self.phase = Phase::Settling;
            self.since = now;
            self.failed.clear();
            self.status(
                "Rotating",
                format!("Search {}/{}.", self.rotations, t.max_rotations),
            );
            return self.issue(ActionKind::Rotate);
        }
        None
    }
}
