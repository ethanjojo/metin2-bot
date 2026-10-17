use crate::{Profile, Result, Shortcut, require, validation::number};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TimedShortcut {
    pub shortcut: String,
    pub interval_seconds: f64,
    pub wait_seconds: f64,
    pub hold_seconds: f64,
}
impl Default for TimedShortcut {
    fn default() -> Self {
        Self {
            shortcut: "F4".into(),
            interval_seconds: 120.,
            wait_seconds: 1.,
            hold_seconds: 0.15,
        }
    }
}
impl TimedShortcut {
    pub fn validate(&self) -> Result<()> {
        Shortcut::parse(&self.shortcut)?;
        number("interval_seconds", self.interval_seconds)?;
        number("wait_seconds", self.wait_seconds)?;
        number("key_hold_seconds", self.hold_seconds)
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstRun {
    Immediate,
    #[default]
    AfterInterval,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StoredPeriodic")]
pub struct PeriodicAction {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub input: crate::ActionInput,
    pub interval_seconds: f64,
    pub wait_seconds: f64,
    pub first_run: FirstRun,
}
// Read old saved profiles once; serialization only writes the canonical schema.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredPeriodic {
    id: Uuid,
    name: String,
    enabled: bool,
    #[serde(default)]
    timing: Option<TimedShortcut>,
    #[serde(default)]
    input: Option<crate::ActionInput>,
    #[serde(default)]
    interval_seconds: Option<f64>,
    #[serde(default)]
    wait_seconds: Option<f64>,
    #[serde(default)]
    first_run: Option<FirstRun>,
}
impl TryFrom<StoredPeriodic> for PeriodicAction {
    type Error = String;
    fn try_from(v: StoredPeriodic) -> std::result::Result<Self, String> {
        if let Some(t) = v.timing {
            if v.input.is_some()
                || v.interval_seconds.is_some()
                || v.wait_seconds.is_some()
                || v.first_run.is_some()
            {
                return Err("Mixed periodic action formats.".into());
            }
            Ok(Self {
                id: v.id,
                name: v.name,
                enabled: v.enabled,
                input: crate::ActionInput::Key {
                    shortcut: t.shortcut,
                    hold_seconds: t.hold_seconds,
                },
                interval_seconds: t.interval_seconds,
                wait_seconds: t.wait_seconds,
                first_run: FirstRun::Immediate,
            })
        } else {
            Ok(Self {
                id: v.id,
                name: v.name,
                enabled: v.enabled,
                input: v.input.ok_or("Missing action input")?,
                interval_seconds: v.interval_seconds.ok_or("Missing interval")?,
                wait_seconds: v.wait_seconds.ok_or("Missing action wait")?,
                first_run: v.first_run.unwrap_or_default(),
            })
        }
    }
}
impl PeriodicAction {
    pub fn new(name: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            enabled: false,
            input: Default::default(),
            interval_seconds: 120.,
            wait_seconds: 1.,
            first_run: Default::default(),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScheduledKey {
    Periodic(Uuid),
    Puller,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ActionTimer {
    pub key: ScheduledKey,
    pub name: String,
    pub remaining_seconds: Option<f64>,
}
#[derive(Debug, Clone, Default)]
pub struct ActionScheduler {
    due: HashMap<ScheduledKey, f64>,
}
impl ActionScheduler {
    pub fn rebase(&mut self, old_clock: f64) {
        for due in self.due.values_mut() {
            *due = (*due - old_clock).max(0.);
        }
    }
    pub fn reconfigure(&mut self, old: &Profile, new: &Profile) {
        self.due.retain(|key, _| match key {
            ScheduledKey::Puller => false,
            ScheduledKey::Periodic(id) => old
                .periodic_actions
                .iter()
                .find(|a| a.id == *id)
                .zip(new.periodic_actions.iter().find(|a| a.id == *id))
                .is_some_and(|(a, b)| a == b),
        });
    }
    pub fn initialize(&mut self, profile: &Profile, active: f64) {
        for a in profile.periodic_actions.iter().filter(|a| a.enabled) {
            self.due
                .entry(ScheduledKey::Periodic(a.id))
                .or_insert_with(|| {
                    active
                        + if a.first_run == FirstRun::Immediate {
                            0.
                        } else {
                            a.interval_seconds
                        }
                });
        }
    }
    pub fn reset(&mut self) {
        self.due.clear();
    }
    pub fn timers(&self, profile: &Profile, active: f64) -> Vec<ActionTimer> {
        profile
            .periodic_actions
            .iter()
            .filter(|a| a.enabled)
            .map(|a| {
                let key = ScheduledKey::Periodic(a.id);
                let remaining = self
                    .due
                    .get(&key)
                    .map(|due| (*due - active).max(0.))
                    .unwrap_or(if a.first_run == FirstRun::Immediate {
                        0.
                    } else {
                        a.interval_seconds
                    });
                ActionTimer {
                    key,
                    name: a.name.clone(),
                    remaining_seconds: Some(remaining),
                }
            })
            .collect()
    }
    pub fn next(
        &self,
        profile: &Profile,
        active: f64,
    ) -> Option<(ScheduledKey, String, PeriodicAction)> {
        let periodic = profile
            .periodic_actions
            .iter()
            .filter(|a| a.enabled)
            .map(|a| (ScheduledKey::Periodic(a.id), a.name.clone(), a.clone()));
        periodic
            .filter(|(key, _, _)| active >= self.due.get(key).copied().unwrap_or(0.))
            .min_by(|a, b| {
                self.due
                    .get(&a.0)
                    .copied()
                    .unwrap_or(0.)
                    .total_cmp(&self.due.get(&b.0).copied().unwrap_or(0.))
            })
    }

    pub fn complete(&mut self, key: ScheduledKey, active: f64, interval: f64) {
        self.due.insert(key, active + interval);
    }
}

pub fn validate_scheduled(profile: &Profile) -> Result<()> {
    validate_scheduled_setup(profile).map_err(|issue| issue.error)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledGroup {
    Buffs,
    Loot,
    Periodic,
    Puller,
    OutOfTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledIssue {
    pub group: ScheduledGroup,
    pub error: crate::ValidationError,
}

/// The canonical scheduled-action checks, with the editor responsible for a failure.
pub fn validate_scheduled_setup(profile: &Profile) -> std::result::Result<(), ScheduledIssue> {
    let mut group = ScheduledGroup::Periodic;
    let result = (|| -> Result<()> {
        require(
            profile.periodic_actions.len() <= 16,
            "Configure at most 16 periodic actions.",
        )?;
        let mut ids = HashSet::new();
        let mut keys = HashSet::new();
        if profile.buffs.enabled {
            group = ScheduledGroup::Buffs;
            keys.insert(Shortcut::parse(&profile.buffs.mount_shortcut)?);
            for s in &profile.buffs.skills {
                keys.insert(Shortcut::parse(&s.shortcut)?);
            }
        }
        if profile.loot.enabled {
            group = ScheduledGroup::Loot;
            keys.insert(Shortcut::parse(&profile.loot.shortcut)?);
        }
        group = ScheduledGroup::Periodic;
        for action in &profile.periodic_actions {
            require(
                !action.id.is_nil() && ids.insert(action.id),
                "Periodic actions require unique nonempty IDs.",
            )?;
            require(
                !action.name.trim().is_empty()
                    && action.name.chars().count() <= 60
                    && !action.name.chars().any(char::is_control),
                "Periodic action name must contain 1–60 printable characters.",
            )?;
            number("interval_seconds", action.interval_seconds)?;
            number("wait_seconds", action.wait_seconds)?;
            action.input.validate(action.enabled, profile.frame_size)?;
            if action.enabled
                && let Some(shortcut) = action.input.shortcut()
            {
                require(
                    keys.insert(Shortcut::parse(shortcut)?),
                    "Periodic shortcut conflicts with another enabled action.",
                )?;
            }
        }
        group = ScheduledGroup::Puller;
        profile.puller.validate()?;
        group = ScheduledGroup::OutOfTarget;
        profile.out_of_target.validate()?;
        if profile.puller.enabled {
            group = ScheduledGroup::Puller;
            require(
                keys.insert(Shortcut::parse(&profile.puller.timing.shortcut)?),
                "Auto-puller shortcut conflicts with another enabled action.",
            )?;
        }
        if profile.out_of_target.enabled() {
            group = ScheduledGroup::OutOfTarget;
            require(
                keys.insert(Shortcut::parse(&profile.out_of_target.shortcut)?),
                "Out-of-target attack shortcut conflicts with another enabled action.",
            )?;
        }
        Ok(())
    })();
    result.map_err(|error| ScheduledIssue { group, error })
}
