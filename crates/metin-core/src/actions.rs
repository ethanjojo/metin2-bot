use crate::{Profile, Result, Shortcut, require, validation::number};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LootConfig {
    pub enabled: bool,
    pub shortcut: String,
    pub presses: u32,
    pub delay_seconds: f64,
    pub interval_seconds: f64,
    pub hold_seconds: f64,
}
impl Default for LootConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            shortcut: "Z".into(),
            presses: 3,
            delay_seconds: 0.5,
            interval_seconds: 0.3,
            hold_seconds: 0.1,
        }
    }
}
impl LootConfig {
    pub fn validate(&self, profile: &Profile) -> Result<()> {
        number("loot_presses", self.presses.into())?;
        number("loot_delay_seconds", self.delay_seconds)?;
        number("loot_interval_seconds", self.interval_seconds)?;
        number("key_hold_seconds", self.hold_seconds)?;
        let key = Shortcut::parse(&self.shortcut)?;
        if self.enabled && profile.buffs.enabled {
            require(
                key != Shortcut::parse(&profile.buffs.mount_shortcut)?,
                "Loot shortcut conflicts with the mount shortcut.",
            )?;
            for skill in &profile.buffs.skills {
                require(
                    key != Shortcut::parse(&skill.shortcut)?,
                    "Loot shortcut conflicts with a buff shortcut.",
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SafetyConfig {
    pub stop_enabled: bool,
    pub stop_after_seconds: f64,
    pub clock_enabled: bool,
    /// Minutes since local midnight; the next occurrence is fixed when a run starts.
    pub stop_at_minute: u32,
    pub breaks_enabled: bool,
    pub break_every_seconds: f64,
    pub break_seconds: f64,
    pub rules: Vec<crate::SafetyRule>,
    pub recovery_enabled: bool,
    pub recovery_attempts: u32,
    pub recovery_wait_seconds: f64,
    pub sound: bool,
    pub system_notification: bool,
}
impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            stop_enabled: false,
            stop_after_seconds: 14400.,
            clock_enabled: false,
            stop_at_minute: 23 * 60,
            breaks_enabled: false,
            break_every_seconds: 2700.,
            break_seconds: 300.,
            rules: vec![],
            recovery_enabled: false,
            recovery_attempts: 2,
            recovery_wait_seconds: 2.,
            sound: false,
            system_notification: false,
        }
    }
}
impl SafetyConfig {
    pub fn schedule_eq(&self, other: &Self) -> bool {
        self.stop_enabled == other.stop_enabled
            && self.stop_after_seconds == other.stop_after_seconds
            && self.clock_enabled == other.clock_enabled
            && self.stop_at_minute == other.stop_at_minute
            && self.breaks_enabled == other.breaks_enabled
            && self.break_every_seconds == other.break_every_seconds
            && self.break_seconds == other.break_seconds
    }
    pub fn validate(&self) -> Result<()> {
        self.validate_schedule()?;
        crate::validate_rules(&self.rules)
    }
    pub fn validate_schedule(&self) -> Result<()> {
        number("stop_after_seconds", self.stop_after_seconds)?;
        number("break_every_seconds", self.break_every_seconds)?;
        number("break_seconds", self.break_seconds)?;
        number("recovery_attempts", self.recovery_attempts.into())?;
        number("wait_seconds", self.recovery_wait_seconds)?;
        require(
            self.stop_at_minute < 1440,
            "Safety stop time must be a valid local time.",
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScheduleSnapshot {
    pub active_seconds: f64,
    pub stop_remaining: Option<f64>,
    pub clock_remaining: Option<f64>,
    pub clock_at_minute: Option<u32>,
    pub break_remaining: Option<f64>,
    pub break_ends_in: Option<f64>,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum EventLevel {
    #[default]
    Info,
    Warning,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct RunEvent {
    pub id: u64,
    pub at_seconds: f64,
    pub level: EventLevel,
    pub activity: String,
    pub message: String,
    pub destination: crate::EventDestination,
    pub subject: Option<uuid::Uuid>,
    pub notify: bool,
}
