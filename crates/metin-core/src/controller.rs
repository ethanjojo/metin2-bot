#[path = "visual_execution.rs"]
mod visual_execution;
use crate::{
    BuffCycle, EventLevel, Presence, Profile, Result, RunEvent, ScheduleSnapshot, Session, require,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use visual_execution::VisualExecution;

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
    pub safety_scores: Vec<(uuid::Uuid, f64)>,
}
#[derive(Debug, Clone, PartialEq)]
pub enum ActionKind {
    Click {
        point: [u32; 2],
    },
    Rotate,
    Mount {
        shortcut: String,
    },
    Skill {
        shortcut: String,
        index: usize,
    },
    Loot {
        shortcut: String,
    },
    Scheduled {
        input: crate::ActionInput,
        key: crate::ScheduledKey,
    },
    Configured {
        input: crate::ActionInput,
    },
    Key {
        shortcut: String,
        hold_seconds: f64,
    },
    Recover,
    Pull {
        shortcut: String,
        hold_seconds: f64,
    },
    Attack {
        shortcut: String,
        duration_seconds: f64,
    },
    ReleaseAttack,
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
    Looting,
    Break,
    Scheduled,
    Recovering,
    OutOfTarget,
    VisualRule,
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
            Self::Looting => "looting",
            Self::Break => "break",
            Self::Scheduled => "scheduled",
            Self::Recovering => "recovering",
            Self::OutOfTarget => "out_of_target",
            Self::VisualRule => "visual_rule",
        }
    }
}
#[derive(Debug, Clone)]
struct ScheduledExecution {
    key: crate::ScheduledKey,
    name: String,
    timing: crate::PeriodicAction,
    search_elapsed: f64,
    finish_at: Option<f64>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttackReason {
    AfterCombat,
    Search,
}
#[derive(Debug, Clone)]
struct OutOfTargetExecution {
    reason: AttackReason,
    finish_at: Option<f64>,
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
    budget: Session,
    break_base: f64,
    break_until: Option<f64>,
    clock_deadline: Option<f64>,
    stop_reached: bool,
    loot_left: u32,
    loot_ready: f64,
    event_time: f64,
    event_id: u64,
    pub events: std::collections::VecDeque<RunEvent>,
    pub actions: crate::ActionScheduler,
    scheduled: Option<ScheduledExecution>,
    safety_monitor: crate::SafetyMonitor,
    pub recoveries: u32,
    recovery_until: Option<f64>,
    puller: crate::combat::PullerCycle,
    out_of_target: Option<OutOfTargetExecution>,
    search_budget: Session,
    search_attacks: u32,
    visual: Option<VisualExecution>,
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
            budget: Session::default(),
            break_base: 0.,
            break_until: None,
            clock_deadline: None,
            stop_reached: false,
            loot_left: 0,
            loot_ready: 0.,
            event_time: 0.,
            event_id: 0,
            events: Default::default(),
            actions: Default::default(),
            scheduled: None,
            safety_monitor: Default::default(),
            recoveries: 0,
            recovery_until: None,
            puller: Default::default(),
            out_of_target: None,
            search_budget: Default::default(),
            search_attacks: 0,
            visual: None,
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
        self.search_budget.advance(
            self.event_time,
            self.running && matches!(self.phase, Phase::Seeking | Phase::Settling),
        );
        let reason = reason.into();
        if self.state != state || (state == "Paused" && self.reason != reason) {
            self.event(state, &reason, state == "Paused");
        }
        self.state = state.into();
        self.reason = reason;
    }
    pub fn start(&mut self, now: f64) -> Result<()> {
        self.start_with_clock(now, None)
    }
    pub fn start_with_clock(&mut self, now: f64, clock_delay: Option<f64>) -> Result<()> {
        require(now.is_finite(), "Invalid monotonic time.")?;
        self.profile.loot.validate(&self.profile)?;
        self.profile.safety.validate()?;
        self.profile.validate_visual_actions()?;
        crate::validate_scheduled(&self.profile)?;
        if self.stop_reached {
            self.reset_schedule();
            self.actions.reset();
        }
        if self.profile.safety.clock_enabled && self.clock_deadline.is_none() {
            let delay = clock_delay
                .filter(|d| d.is_finite() && *d > 0.)
                .ok_or_else(|| {
                    crate::ValidationError("Safety clock deadline is unavailable.".into())
                })?;
            self.clock_deadline = Some(now + delay);
        }
        self.event_time = now;
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
        self.actions
            .initialize(&self.profile, self.budget.active_seconds);
        self.search_budget.reset();
        self.search_attacks = 0;
        self.out_of_target = None;
        self.visual = None;
        self.safety_monitor.reset();
        self.recoveries = 0;
        self.recovery_until = None;
        self.budget.advance(now, true);
        self.session.advance(now, true);
        self.break_until = None;
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
        self.scheduled = None;
        self.status(
            "Checking target",
            "Checking whether a target panel is already present.",
        );
        Ok(())
    }
    pub fn pause(&mut self, now: f64, reason: impl Into<String>) {
        if now.is_finite() {
            self.event_time = now;
            self.budget.pause(Some(now));
        }
        if self.is_active() {
            self.buffs.pause(now);
        }
        self.session.pause(Some(now));
        self.running = false;
        self.search_budget.pause(Some(now));
        self.puller.cancel();
        self.out_of_target = None;
        self.pending_action = None;
        if self.break_until.take().is_some() {
            self.break_base = self.budget.active_seconds;
        }
        self.loot_left = 0;
        self.scheduled = None;
        self.visual = None;
        self.status("Paused", reason);
    }
    pub fn pause_with_notice(&mut self, now: f64, reason: impl Into<String>) {
        let was_running = self.running;
        let previous = self.event_id;
        self.pause(now, reason);
        if was_running
            && self.event_id > previous
            && let Some(event) = self.events.back_mut()
        {
            event.notify = true;
            event.destination = crate::EventDestination::Calibration;
        }
    }
    /// Runtime errors during a visual action belong to that rule. Schedule
    /// stops use pause_with_notice directly and keep their independent alert.
    pub fn pause_on_runtime_error(&mut self, now: f64, reason: impl Into<String>) {
        let reason = reason.into();
        if let Some(rule) = self.visual.as_ref().map(|v| v.rule.clone()) {
            self.pause(now, reason.clone());
            self.rule_event(&rule, &format!("{}: {}", rule.name, reason), true);
        } else {
            self.pause_with_notice(now, reason);
        }
    }
    pub fn event(&mut self, activity: &str, message: &str, warning: bool) {
        self.event_id += 1;
        if self.events.len() == 200 {
            self.events.pop_front();
        }
        self.events.push_back(RunEvent {
            id: self.event_id,
            at_seconds: self.event_time,
            level: if warning {
                EventLevel::Warning
            } else {
                EventLevel::Info
            },
            activity: activity.into(),
            message: message.into(),
            destination: match activity {
                "Buff" => crate::EventDestination::Buffs,
                "Mount" => crate::EventDestination::Mount,
                "Loot" => crate::EventDestination::Loot,
                "Scheduled action" => crate::EventDestination::Periodic,
                "Auto-puller" => crate::EventDestination::Puller,
                "Out-of-target attack" => crate::EventDestination::OutOfTarget,
                "Recovery" => crate::EventDestination::Recovery,
                "Visual rule" => crate::EventDestination::Safety,
                _ => crate::EventDestination::Run,
            },
            subject: None,
            notify: false,
        });
    }
    /// Reconfiguration explicitly starts a fresh safety budget; statistics remain independent.
    pub fn reset_schedule(&mut self) {
        self.actions.rebase(self.budget.active_seconds);
        self.budget.reset();
        self.break_base = 0.;
        self.break_until = None;
        self.clock_deadline = None;
        self.stop_reached = false;
    }
    pub fn is_active(&self) -> bool {
        self.running && self.phase != Phase::Break
    }
    pub fn schedule(&self, now: f64) -> ScheduleSnapshot {
        let p = &self.profile.safety;
        let active = self.budget.active_seconds;
        ScheduleSnapshot {
            active_seconds: active,
            stop_remaining: p
                .stop_enabled
                .then_some((p.stop_after_seconds - active).max(0.)),
            clock_remaining: self.clock_deadline.map(|d| (d - now).max(0.)),
            clock_at_minute: p.clock_enabled.then_some(p.stop_at_minute),
            break_remaining: p
                .breaks_enabled
                .then_some((p.break_every_seconds - (active - self.break_base)).max(0.)),
            break_ends_in: self.break_until.map(|d| (d - now).max(0.)),
        }
    }
    /// Called independently of capture cadence so safety limits also apply during slow captures.
    pub fn tick(&mut self, now: f64) {
        if !now.is_finite() {
            self.pause(self.event_time, "Invalid monotonic time.");
            return;
        }
        self.event_time = now;
        let active = self.is_active();
        self.search_budget.advance(
            now,
            self.running && matches!(self.phase, Phase::Seeking | Phase::Settling),
        );
        self.budget.advance(now, active);
        self.session.advance(now, active);
        let schedule = self.schedule(now);
        if !self.running {
            if schedule.stop_remaining == Some(0.) || schedule.clock_remaining == Some(0.) {
                self.stop_reached = true;
            }
            return;
        }
        if schedule.stop_remaining == Some(0.) || schedule.clock_remaining == Some(0.) {
            self.pause_with_notice(
                now,
                "Automatic stop: the configured safety limit was reached.",
            );
            self.stop_reached = true;
            if let Some(event) = self.events.back_mut() {
                event.destination = crate::EventDestination::Safety;
            }
            return;
        }
        if self.break_until.is_some_and(|until| now >= until) {
            self.break_until = None;
            self.break_base = self.budget.active_seconds;
            self.phase = Phase::Initial;
            self.since = now;
            self.present = 0;
            self.absent = 0;
            self.last_observation = None;
            self.uncertain_since = None;
            self.buffs.resume(now);
            self.budget.advance(now, true);
            self.session.advance(now, true);
            self.status(
                "Checking target",
                "Break finished; checking a fresh target panel before continuing.",
            );
        }
    }
    pub fn input_budget(&self, now: f64) -> Option<f64> {
        let s = self.schedule(now);
        [s.stop_remaining, s.clock_remaining]
            .into_iter()
            .flatten()
            .reduce(f64::min)
    }
    pub fn remaining(&self, now: f64) -> Option<f64> {
        if !self.running {
            return None;
        }
        let t = &self.profile.timing;
        let limit = match self.phase {
            Phase::Buffing => return Some((self.buffs.wait_until - now).max(0.)),
            Phase::OutOfTarget => {
                return self
                    .out_of_target
                    .as_ref()
                    .and_then(|a| a.finish_at)
                    .map(|end| (end - now).max(0.));
            }
            Phase::Scheduled => {
                return self
                    .scheduled
                    .as_ref()
                    .and_then(|s| s.finish_at)
                    .map(|end| (end - now).max(0.));
            }
            Phase::VisualRule => {
                return self.visual.as_ref().map(|v| {
                    let duration = if v.sent_at.is_some() {
                        v.rule.execution.wait_seconds
                            + if v.rule.execution.verification == crate::Verification::ReferenceGone
                            {
                                v.rule.execution.timeout_seconds
                            } else {
                                0.
                            }
                    } else {
                        v.rule.execution.timeout_seconds
                    };
                    (v.sent_at.unwrap_or(self.since) + duration - now).max(0.)
                });
            }
            Phase::Recovering => return self.recovery_until.map(|end| (end - now).max(0.)),
            Phase::Looting => return Some((self.loot_ready - now).max(0.)),
            Phase::Break => return self.break_until.map(|d| (d - now).max(0.)),
            Phase::Initial | Phase::Acquiring => t.acquire_timeout,
            Phase::Seeking => t.search_timeout,
            Phase::Active => t.attack_timeout,
            Phase::Settling => t.settle_seconds,
        };
        Some((limit - (now - self.since)).max(0.))
    }
    fn active(&mut self, now: f64) {
        self.puller.begin(now, &self.profile.puller);
        self.search_budget.reset();
        self.search_attacks = 0;
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
    pub fn visual_rule_for_input(&self, action: &Action) -> Option<&crate::SafetyRule> {
        if matches!(action.kind, ActionKind::Configured { .. }) {
            self.visual.as_ref().map(|v| &v.rule)
        } else {
            None
        }
    }
    pub fn ack(&mut self, now: f64, id: u64, success: bool) -> Result<()> {
        require(
            self.pending_action.as_ref().is_some_and(|a| a.id == id),
            "Stale or duplicate action acknowledgement.",
        )?;
        let a = self.pending_action.take().expect("checked action");
        self.event_time = now;
        if !success {
            if matches!(a.kind, ActionKind::Mount { .. }) {
                self.buffs.mounted = None;
            }
            self.pause_on_runtime_error(
                now,
                "Input action failed; check the game before resuming.",
            );
            return Ok(());
        }
        if matches!(a.kind, ActionKind::Attack { .. }) {
            if let Some(attack) = &mut self.out_of_target {
                let duration = match a.kind {
                    ActionKind::Attack {
                        duration_seconds, ..
                    } => duration_seconds,
                    _ => unreachable!(),
                };
                attack.finish_at = Some(now + duration);
            }
            self.event(
                "Out-of-target attack",
                "Attack key held; recognition and safety monitoring continue.",
                false,
            );
        }
        if matches!(a.kind, ActionKind::ReleaseAttack) {
            self.event(
                "Out-of-target attack",
                "Attack key released before continuing.",
                false,
            );
        }
        if matches!(a.kind, ActionKind::Pull { .. }) {
            self.puller.acknowledge(now, &self.profile.puller);
            self.event(
                "Auto-puller",
                "Puller shortcut sent for the confirmed combat.",
                false,
            );
        }
        if self.phase == Phase::Buffing {
            self.buffs.acknowledge(now)?;
        }
        if matches!(a.kind, ActionKind::Configured { .. }) {
            self.visual_ack(now);
        }
        if matches!(a.kind, ActionKind::Recover) {
            self.recovery_until = Some(now + self.profile.safety.recovery_wait_seconds);
        }
        let mut active_clock = self.budget.clone();
        active_clock.advance(now, self.is_active());
        if let Some(scheduled) = &mut self.scheduled {
            scheduled.finish_at = Some(now + scheduled.timing.wait_seconds);
            self.actions.complete(
                scheduled.key,
                active_clock.active_seconds,
                scheduled.timing.wait_seconds + scheduled.timing.interval_seconds,
            );
            let message = format!(
                "{} ({}) sent; waiting for completion.",
                scheduled.name,
                if scheduled.timing.input.shortcut().is_some() {
                    "key"
                } else {
                    "click"
                }
            );
            self.event("Scheduled action", &message, false);
            if let Some(event) = self.events.back_mut() {
                match self.scheduled.as_ref().expect("scheduled action").key {
                    crate::ScheduledKey::Periodic(id) => event.subject = Some(id),
                    crate::ScheduledKey::Puller => {}
                }
            }
        }
        if matches!(a.kind, ActionKind::Loot { .. }) {
            self.loot_left = self.loot_left.saturating_sub(1);
            self.loot_ready = now + self.profile.loot.interval_seconds;
            self.event(
                "Loot",
                &format!("Pickup shortcut sent; {} remaining.", self.loot_left),
                false,
            );
        } else if let ActionKind::Skill { shortcut, index } = &a.kind {
            let name = self
                .profile
                .buffs
                .skills
                .get(*index)
                .map(|s| s.name.as_str())
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Skill {}", index + 1));
            self.event("Buff", &format!("{name} ({shortcut}) sent."), false);
        } else if let ActionKind::Mount { shortcut } = &a.kind {
            let state = if self.buffs.mounted == Some(true) {
                "mounted"
            } else {
                "on foot"
            };
            self.event(
                "Mount",
                &format!("Shortcut {shortcut} sent; expected state: {state}."),
                false,
            );
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
        self.tick(now);
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
        if self.profile.safety.rules.iter().any(|r| {
            r.enabled
                && !o
                    .safety_scores
                    .iter()
                    .any(|(id, s)| *id == r.id && s.is_finite())
        }) {
            self.pause_on_runtime_error(
                now,
                "Visual monitoring is unavailable; check reference calibration.",
            );
            return None;
        }
        let rules = self.safety_monitor.update(
            &self.profile.safety.rules,
            &o.safety_scores,
            now,
            1.0_f64.max(self.profile.timing.sample_interval * 1.5),
        );
        let mut stop = false;
        let mut execute = vec![];
        for rule in rules {
            self.rule_event(
                &rule,
                &format!(
                    "{} persisted for {:.1} s.",
                    rule.name, rule.duration_seconds
                ),
                true,
            );
            if rule.reaction.pause {
                stop = true;
            } else if rule.reaction.execute {
                execute.push(rule);
            }
        }
        if stop {
            self.pause(
                now,
                "Visual rule triggered. Check the game before resuming.",
            );
            return None;
        }
        if execute.len() > 1
            || (self.visual.is_some()
                && execute
                    .iter()
                    .any(|r| Some(r.id) != self.visual.as_ref().map(|v| v.rule.id)))
        {
            self.pause(
                now,
                "Conflicting visual actions detected. Check the game before resuming.",
            );
            return None;
        }
        if self.visual.is_none()
            && let Some(rule) = execute.pop()
        {
            return self.begin_visual(now, rule);
        }
        if self.phase == Phase::VisualRule {
            return self.visual_step(now, o);
        }
        if self.phase == Phase::OutOfTarget {
            if self.pending_action.is_some() {
                return None;
            }
            let attack = self.out_of_target.as_ref().expect("attack phase");
            let finished = attack.finish_at.is_some_and(|end| now >= end);
            let interrupted = o.panel_present
                || (attack.reason == AttackReason::Search && !o.candidates.is_empty());
            if finished || interrupted {
                let reason = self.out_of_target.take().expect("attack phase").reason;
                self.present = 0;
                self.absent = 0;
                self.last_observation = None;
                self.since = now;
                self.search_budget.reset();
                if o.panel_present {
                    self.phase = Phase::Initial;
                    self.status(
                        "Checking target",
                        "Attack released; confirming the new target panel.",
                    );
                } else if reason == AttackReason::AfterCombat {
                    self.begin_loot_or_settle(now);
                } else {
                    self.phase = Phase::Seeking;
                    self.status(
                        "Searching",
                        "Attack finished; checking a fresh frame before selecting a target.",
                    );
                }
                return self.issue(ActionKind::ReleaseAttack);
            }
            return None;
        }
        if self.phase == Phase::Recovering {
            if self.recovery_until.is_some_and(|end| now >= end) {
                self.phase = Phase::Initial;
                self.since = now;
                self.present = 0;
                self.absent = 0;
                self.last_observation = None;
                self.uncertain_since = None;
                self.rotations = 0;
                self.failures = 0;
                self.clicked = None;
                self.status(
                    "Checking target",
                    "Recovery finished; checking fresh target state.",
                );
            }
            return None;
        }
        if self.phase == Phase::Break {
            return None;
        }
        if self.phase == Phase::Scheduled {
            if o.panel_present {
                self.pause(now,"A target panel appeared during a periodic action. Check the game before resuming.");
                return None;
            }
            if self
                .scheduled
                .as_ref()
                .is_some_and(|s| s.finish_at.is_some_and(|end| now >= end))
            {
                let ScheduledExecution {
                    name,
                    search_elapsed,
                    ..
                } = self.scheduled.take().expect("scheduled action");
                self.phase = Phase::Seeking;
                self.since = now - search_elapsed;
                self.status("Searching", format!("{name} finished; continuing."));
                self.last_observation = None;
                self.present = 0;
                self.absent = 0;
            }
            return None;
        }
        if self.phase == Phase::Looting {
            if o.panel_present {
                self.pause(
                    now,
                    "A target panel appeared while looting. Check the game before resuming.",
                );
                return None;
            }
            if self.pending_action.is_some() {
                return None;
            }
            if self.loot_left == 0 {
                self.phase = Phase::Settling;
                self.since = now;
                self.status(
                    "Target ended",
                    "Loot finished; waiting for the frame to settle.",
                );
            } else if now >= self.loot_ready {
                return self.issue(ActionKind::Loot {
                    shortcut: self.profile.loot.shortcut.clone(),
                });
            }
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
                return self.recover_or_pause(
                    now,
                    "Target timed out. Check the attack or an obstruction.",
                );
            }
            if self.absent >= required {
                self.session.completed += 1;
                self.recoveries = 0;
                self.phase = Phase::Settling;
                self.since = now;
                self.status(
                    "Target ended",
                    "Panel disappeared; letting the frame settle before the next target.",
                );
                self.puller.end();
                self.search_budget.reset();
                if self.profile.out_of_target.after_combat {
                    return self.begin_out_of_target(AttackReason::AfterCombat);
                }
                self.begin_loot_or_settle(now);
            } else if o.panel_present
                && self.present >= required
                && self.puller.due(now, &self.profile.puller)
            {
                self.status("Auto-puller", "Using the puller during confirmed combat.");
                return self.issue(ActionKind::Pull {
                    shortcut: self.profile.puller.timing.shortcut.clone(),
                    hold_seconds: self.profile.puller.timing.hold_seconds,
                });
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
                    return self.recover_or_pause(
                        now,
                        "Target panel was not confirmed within the click retry limit.",
                    );
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
        self.puller.end();
        if self.schedule(now).break_remaining == Some(0.) {
            self.phase = Phase::Break;
            self.break_until = Some(now + self.profile.safety.break_seconds);
            self.buffs.pause(now);
            self.budget.advance(now, false);
            self.session.advance(now, false);
            self.status(
                "On break",
                "No inputs during the break. Pause or F8 cancels automatic continuation.",
            );
            return None;
        }
        self.failed.retain(|(_, until)| now < *until);
        // A completed buff pass must yield to an available target, even when
        // another refresh expired during a long animation. While no target is
        // available, refresh timers must keep working instead of being blocked
        // until the next click. Inactive toggles retain their attempt latch.
        if self.next_candidate(&o.candidates).is_none() {
            self.attempted
                .retain(|i| self.profile.buffs.skills[*i].mode == crate::BuffMode::Inactive);
        }
        if self.buffs.needed(now, &o.buffs, &self.attempted) {
            self.buffs.begin(now, &o.buffs, &self.attempted);
            self.phase = Phase::Buffing;
            return self.buff_step(now, &o.buffs);
        }
        if let Some(action) = self.scheduled_step() {
            return Some(action);
        }
        if let Some(c) = self.next_candidate(&o.candidates) {
            let point = c.point;
            self.clicked = Some(point);
            self.search_budget.reset();
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
        if self.profile.out_of_target.during_search
            && self.search_budget.active_seconds >= self.profile.out_of_target.search_after_seconds
        {
            if self.search_attacks >= self.profile.out_of_target.max_search_attacks {
                self.pause_with_notice(
                    now,
                    "No target found after the configured out-of-target search attacks.",
                );
                if let Some(event) = self.events.back_mut() {
                    event.destination = crate::EventDestination::OutOfTarget;
                }
                return None;
            }
            self.search_attacks += 1;
            return self.begin_out_of_target(AttackReason::Search);
        }
        if now - self.since >= t.search_timeout {
            if self.rotations >= t.max_rotations {
                return self.recover_or_pause(now, "No Metin found within the search limit.");
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
    pub fn action_timers(&self) -> Vec<crate::ActionTimer> {
        let mut timers = self
            .actions
            .timers(&self.profile, self.budget.active_seconds);
        if self.profile.puller.enabled {
            timers.push(crate::ActionTimer {
                key: crate::ScheduledKey::Puller,
                name: "Auto-puller".into(),
                remaining_seconds: self.puller.remaining(self.event_time, &self.profile.puller),
            });
        }
        timers
    }
    fn begin_loot_or_settle(&mut self, now: f64) {
        self.since = now;
        self.phase = Phase::Settling;
        if self.profile.loot.enabled {
            self.phase = Phase::Looting;
            self.loot_left = self.profile.loot.presses;
            self.loot_ready = now + self.profile.loot.delay_seconds;
            self.status("Looting", "Target end confirmed; collecting nearby loot.");
        }
    }
    fn begin_out_of_target(&mut self, reason: AttackReason) -> Option<Action> {
        self.phase = Phase::OutOfTarget;
        self.out_of_target = Some(OutOfTargetExecution {
            reason,
            finish_at: None,
        });
        let duration = if reason == AttackReason::AfterCombat {
            self.profile.out_of_target.after_combat_seconds
        } else {
            self.profile.out_of_target.search_attack_seconds
        };
        self.status(
            "Out-of-target attack",
            if reason == AttackReason::AfterCombat {
                "Timed attack after the target ended; loot follows."
            } else {
                "Timed attack during a long search; no target completion is assumed."
            },
        );
        self.issue(ActionKind::Attack {
            shortcut: self.profile.out_of_target.shortcut.clone(),
            duration_seconds: duration,
        })
    }
    fn scheduled_step(&mut self) -> Option<Action> {
        let (key, name, timing) = self
            .actions
            .next(&self.profile, self.budget.active_seconds)?;
        let action = ActionKind::Scheduled {
            input: timing.input.clone(),
            key,
        };
        self.scheduled = Some(ScheduledExecution {
            key,
            name: name.clone(),
            timing,
            search_elapsed: (self.event_time - self.since).max(0.),
            finish_at: None,
        });
        self.phase = Phase::Scheduled;
        self.status("Scheduled action", format!("Using {name}."));
        self.issue(action)
    }
    fn recover_or_pause(&mut self, now: f64, reason: &str) -> Option<Action> {
        if self.profile.safety.recovery_enabled
            && self.recoveries < self.profile.safety.recovery_attempts
        {
            self.recoveries += 1;
            self.phase = Phase::Recovering;
            self.recovery_until = None;
            self.status(
                "Recovering",
                format!(
                    "{reason} Clearing target and rotating camera ({}/{}).",
                    self.recoveries, self.profile.safety.recovery_attempts
                ),
            );
            self.event(
                "Recovery",
                &format!(
                    "Attempt {}/{}: clear target and rotate camera.",
                    self.recoveries, self.profile.safety.recovery_attempts
                ),
                true,
            );
            self.issue(ActionKind::Recover)
        } else {
            self.pause_with_notice(now, reason);
            None
        }
    }
}
