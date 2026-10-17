use crate::{Result, Shortcut, TimedShortcut, validation::number};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PullerMode {
    #[default]
    CombatStart,
    // Stored profiles from the previous periodic puller normalize on their next save.
    #[serde(alias = "searching", alias = "combat", alias = "both")]
    CombatPeriodic,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PullerConfig {
    pub enabled: bool,
    pub mode: PullerMode,
    pub timing: TimedShortcut,
    pub presses: u32,
    pub press_interval_seconds: f64,
}
impl Default for PullerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: Default::default(),
            timing: Default::default(),
            presses: 1,
            press_interval_seconds: 0.3,
        }
    }
}
impl PullerConfig {
    pub fn validate(&self) -> Result<()> {
        Shortcut::parse(&self.timing.shortcut)?;
        number("puller_interval_seconds", self.timing.interval_seconds)?;
        number("puller_wait_seconds", self.timing.wait_seconds)?;
        number("key_hold_seconds", self.timing.hold_seconds)?;
        number("puller_presses", self.presses.into())?;
        number("puller_press_gap", self.press_interval_seconds)
    }
}
#[derive(Debug, Clone, Default)]
pub(crate) struct PullerCycle {
    combat: bool,
    left: u32,
    ready_at: f64,
}
impl PullerCycle {
    pub fn begin(&mut self, now: f64, c: &PullerConfig) {
        if !self.combat {
            self.combat = true;
            self.left = if c.enabled { c.presses } else { 0 };
            self.ready_at = now;
        }
    }
    pub fn cancel(&mut self) {
        self.left = 0;
    }
    pub fn end(&mut self) {
        *self = Self::default();
    }
    pub fn due(&mut self, now: f64, c: &PullerConfig) -> bool {
        if !c.enabled || !self.combat || now < self.ready_at {
            return false;
        }
        if self.left == 0 && c.mode == PullerMode::CombatPeriodic {
            self.left = c.presses;
        }
        self.left > 0
    }
    pub fn acknowledge(&mut self, now: f64, c: &PullerConfig) {
        self.left = self.left.saturating_sub(1);
        self.ready_at = now
            + if self.left > 0 {
                c.press_interval_seconds
            } else {
                c.timing.wait_seconds + c.timing.interval_seconds
            };
    }
    pub fn remaining(&self, now: f64, c: &PullerConfig) -> Option<f64> {
        (self.combat && c.enabled && (self.left > 0 || c.mode == PullerMode::CombatPeriodic))
            .then_some((self.ready_at - now).max(0.))
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OutOfTargetConfig {
    pub shortcut: String,
    pub after_combat: bool,
    pub after_combat_seconds: f64,
    pub during_search: bool,
    pub search_after_seconds: f64,
    pub search_attack_seconds: f64,
    pub max_search_attacks: u32,
}
impl Default for OutOfTargetConfig {
    fn default() -> Self {
        Self {
            shortcut: "Space".into(),
            after_combat: false,
            after_combat_seconds: 3.,
            during_search: false,
            search_after_seconds: 10.,
            search_attack_seconds: 3.,
            max_search_attacks: 2,
        }
    }
}
impl OutOfTargetConfig {
    pub fn enabled(&self) -> bool {
        self.after_combat || self.during_search
    }
    pub fn validate(&self) -> Result<()> {
        Shortcut::parse(&self.shortcut)?;
        number("attack_hold_seconds", self.after_combat_seconds)?;
        number("attack_hold_seconds", self.search_attack_seconds)?;
        number("idle_search_seconds", self.search_after_seconds)?;
        number("search_attack_attempts", self.max_search_attacks.into())
    }
}
