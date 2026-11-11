use crate::{BuffConfig, BuffMode, Result, require};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Presence {
    Active,
    Absent,
    Unknown,
}
#[derive(Debug, Clone)]
pub struct BuffStep {
    pub shortcut: String,
    pub wait_seconds: f64,
    pub label: String,
    pub mounted_after: Option<bool>,
    pub skill_index: Option<usize>,
}
#[derive(Debug, Clone)]
pub struct BuffCycle {
    pub config: BuffConfig,
    pub mounted: Option<bool>,
    pub wait_until: f64,
    wait_started: f64,
    pub active: bool,
    pub pending: Option<BuffStep>,
    // Acknowledgement moves the owned step here for its animation wait.
    waiting_step: Option<BuffStep>,
    pub skipped: Vec<String>,
    due: HashMap<usize, f64>,
    paused_remaining: Option<HashMap<usize, f64>>,
    steps: VecDeque<BuffStep>,
}
impl BuffCycle {
    pub fn new(config: BuffConfig) -> Self {
        Self {
            config,
            mounted: None,
            wait_until: 0.,
            wait_started: 0.,
            active: false,
            pending: None,
            waiting_step: None,
            skipped: vec![],
            due: HashMap::new(),
            paused_remaining: None,
            steps: VecDeque::new(),
        }
    }
    pub fn resume(&mut self, now: f64) {
        if let Some(remaining) = self.paused_remaining.take() {
            self.due = remaining
                .into_iter()
                .map(|(i, left)| (i, now + left))
                .collect();
        }
        for (i, s) in self.config.skills.iter().enumerate() {
            if s.mode == BuffMode::Refresh {
                self.due.entry(i).or_insert(now);
            }
        }
    }
    pub fn pause(&mut self, now: f64) {
        self.paused_remaining = Some(
            self.due
                .iter()
                .map(|(i, due)| (*i, (due - now).max(0.)))
                .collect(),
        );
        self.steps.clear();
        self.pending = None;
        self.waiting_step = None;
        self.active = false;
    }
    pub fn skill_remaining(&self, now: f64, paused: bool) -> Vec<Option<f64>> {
        self.config
            .skills
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if self.config.enabled && s.mode == BuffMode::Refresh {
                    if paused {
                        self.paused_remaining
                            .as_ref()
                            .and_then(|r| r.get(&i).copied())
                    } else {
                        self.due.get(&i).map(|due| (due - now).max(0.))
                    }
                } else {
                    None
                }
            })
            .collect()
    }
    pub fn remaining(&self, now: f64, paused: bool) -> Option<f64> {
        self.skill_remaining(now, paused)
            .into_iter()
            .flatten()
            .min_by(f64::total_cmp)
    }
    pub fn reset_timer(&mut self, index: usize, now: f64, paused: bool) -> Result<()> {
        require(
            self.config
                .skills
                .get(index)
                .is_some_and(|s| s.mode == BuffMode::Refresh),
            "Only regular skill timers can be reset.",
        )?;
        self.due.insert(index, now);
        if paused {
            self.paused_remaining
                .get_or_insert_default()
                .insert(index, 0.);
        }
        Ok(())
    }
    pub fn reset_timers(&mut self, now: f64) {
        self.pause(now);
        for (index, skill) in self.config.skills.iter().enumerate() {
            if skill.mode == BuffMode::Refresh {
                self.due.insert(index, now);
                self.paused_remaining
                    .get_or_insert_default()
                    .insert(index, 0.);
            }
        }
        self.wait_until = now;
        self.wait_started = now;
    }
    pub fn wait_started(&self) -> f64 {
        self.wait_started
    }
    pub fn active_skill(&self, now: f64) -> Option<usize> {
        if !self.active || !now.is_finite() {
            return None;
        }
        self.pending
            .as_ref()
            .or_else(|| {
                (now < self.wait_until)
                    .then_some(self.waiting_step.as_ref())
                    .flatten()
            })
            .and_then(|step| step.skill_index)
    }
    fn indices(&self, now: f64, states: &[Presence], attempted: &HashSet<usize>) -> Vec<usize> {
        self.config
            .skills
            .iter()
            .enumerate()
            .filter(|(i, s)| {
                !attempted.contains(i)
                    && match s.mode {
                        BuffMode::Refresh => self.due.get(i).is_some_and(|due| now >= *due),
                        BuffMode::Inactive => states.get(*i) == Some(&Presence::Absent),
                    }
            })
            .map(|(i, _)| i)
            .collect()
    }
    pub fn needed(&self, now: f64, states: &[Presence], attempted: &HashSet<usize>) -> bool {
        self.config.enabled
            && (self.mounted != Some(self.config.fight_mounted)
                || !self.indices(now, states, attempted).is_empty())
    }
    pub fn begin(&mut self, now: f64, states: &[Presence], attempted: &HashSet<usize>) {
        self.skipped.clear();
        self.steps.clear();
        self.pending = None;
        self.waiting_step = None;
        self.active = true;
        self.wait_until = now;
        let indices = self.indices(now, states, attempted);
        if indices
            .iter()
            .any(|i| self.config.skills[*i].mode == BuffMode::Refresh)
        {
            for (i, s) in self.config.skills.iter().enumerate() {
                if s.mode == BuffMode::Inactive && !indices.contains(&i) && !attempted.contains(&i)
                {
                    self.skipped.push(format!(
                        "{}: {}",
                        i + 1,
                        if states.get(i) == Some(&Presence::Active) {
                            "already active"
                        } else {
                            "uncertain detection"
                        }
                    ));
                }
            }
        }
        let mut mounted = self.mounted == Some(true);
        if mounted && (!indices.is_empty() || !self.config.fight_mounted) {
            self.steps.push_back(BuffStep {
                shortcut: self.config.mount_shortcut.clone(),
                wait_seconds: self.config.mount_wait_seconds,
                label: "Dismounting".into(),
                mounted_after: Some(false),
                skill_index: None,
            });
            mounted = false;
        }
        for i in indices {
            let s = &self.config.skills[i];
            self.steps.push_back(BuffStep {
                shortcut: s.shortcut.clone(),
                wait_seconds: s.wait_seconds,
                label: format!(
                    "{}: {}",
                    if s.name.is_empty() {
                        format!("Buff {}", i + 1)
                    } else {
                        s.name.clone()
                    },
                    s.shortcut
                ),
                mounted_after: None,
                skill_index: Some(i),
            });
        }
        if !mounted && self.config.fight_mounted {
            self.steps.push_back(BuffStep {
                shortcut: self.config.mount_shortcut.clone(),
                wait_seconds: self.config.mount_wait_seconds,
                label: "Mounting".into(),
                mounted_after: Some(true),
                skill_index: None,
            });
        }
    }
    pub fn step(&mut self, now: f64, states: &[Presence]) -> Option<BuffStep> {
        if self.pending.is_some() || now < self.wait_until {
            return None;
        }
        self.waiting_step = None;
        while let Some(step) = self.steps.front() {
            if let Some(i) = step.skill_index
                && self.config.skills[i].mode == BuffMode::Inactive
                && states.get(i) != Some(&Presence::Absent)
            {
                self.skipped.push(format!(
                    "{}: {}",
                    i + 1,
                    if states.get(i) == Some(&Presence::Active) {
                        "already active"
                    } else {
                        "uncertain detection"
                    }
                ));
                self.steps.pop_front();
                continue;
            }
            break;
        }
        self.pending = self.steps.pop_front();
        if self.pending.is_none() {
            self.active = false;
        }
        self.pending.clone()
    }
    pub fn acknowledge(&mut self, now: f64) -> Result<()> {
        require(self.pending.is_some(), "No pending buff step.")?;
        let step = self.pending.take().expect("checked pending");
        if let Some(mounted) = step.mounted_after {
            self.mounted = Some(mounted);
        }
        if let Some(i) = step.skill_index {
            let s = &self.config.skills[i];
            if s.mode == BuffMode::Refresh {
                self.due.insert(
                    i,
                    now + step.wait_seconds
                        + s.interval_seconds.expect("validated refresh interval"),
                );
            }
        }
        self.wait_until = now + step.wait_seconds;
        self.wait_started = now;
        self.waiting_step = Some(step);
        Ok(())
    }
}
