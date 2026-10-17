use crate::{BuffIcon, Result, require, validation::number};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "StoredReactions")]
pub struct RuleReactions {
    pub notify: bool,
    pub pause: bool,
    pub execute: bool,
}
impl Default for RuleReactions {
    fn default() -> Self {
        Self {
            notify: true,
            pause: true,
            execute: false,
        }
    }
}
impl RuleReactions {
    pub fn any(self) -> bool {
        self.notify || self.pause || self.execute
    }
    pub fn validate(self, enabled: bool) -> Result<()> {
        require(
            !(self.pause && self.execute),
            "Immediate pause and performing an action cannot both be selected. Choose pause after completion instead.",
        )?;
        require(
            !enabled || self.any(),
            "Choose at least one reaction before enabling this rule.",
        )
    }
}
// Existing user profiles stored a single reaction string. Read it once;
// all subsequent saves use explicit independent flags.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum StoredReactions {
    Flags {
        notify: bool,
        pause: bool,
        execute: bool,
    },
    Legacy(LegacyReaction),
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum LegacyReaction {
    PauseAndNotify,
    Notify,
    Execute,
}
impl From<StoredReactions> for RuleReactions {
    fn from(value: StoredReactions) -> Self {
        match value {
            StoredReactions::Flags {
                notify,
                pause,
                execute,
            } => Self {
                notify,
                pause,
                execute,
            },
            StoredReactions::Legacy(reaction) => Self {
                notify: true,
                pause: matches!(reaction, LegacyReaction::PauseAndNotify),
                execute: matches!(reaction, LegacyReaction::Execute),
            },
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SafetyRule {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub reference: Option<BuffIcon>,
    pub threshold: f64,
    pub duration_seconds: f64,
    pub reaction: RuleReactions,
    #[serde(default)]
    pub execution: crate::RuleExecution,
}
impl SafetyRule {
    pub fn new(name: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            enabled: false,
            reference: None,
            threshold: 0.92,
            duration_seconds: 3.,
            reaction: Default::default(),
            execution: Default::default(),
        }
    }
    pub fn ready(&self) -> bool {
        self.reference.as_ref().is_some_and(|r| r.region.is_some())
    }
    pub fn validate(&self) -> Result<()> {
        require(!self.id.is_nil(), "Safety rule ID must not be empty.")?;
        require(
            !self.name.trim().is_empty()
                && self.name.chars().count() <= 60
                && !self.name.chars().any(char::is_control),
            "Safety rule name must contain 1–60 printable characters.",
        )?;
        self.reaction.validate(self.enabled)?;
        self.execution.validate()?;
        number("threshold", self.threshold)?;
        number("safety_duration_seconds", self.duration_seconds)?;
        if let Some(reference) = &self.reference {
            reference.png_bytes()?;
        }
        require(
            !self.enabled || self.ready(),
            "Capture a Safety reference before enabling this rule.",
        )
    }
}
pub fn validate_rules(rules: &[SafetyRule]) -> Result<()> {
    require(rules.len() <= 8, "Configure at most eight Safety rules.")?;
    let mut ids = HashSet::new();
    for rule in rules {
        rule.validate()?;
        require(ids.insert(rule.id), "Safety rule IDs must be unique.")?;
    }
    Ok(())
}
#[derive(Debug, Clone, Default)]
struct MatchState {
    since: Option<f64>,
    last: Option<f64>,
    fired: bool,
}
#[derive(Debug, Clone, Default)]
pub struct SafetyMonitor {
    states: HashMap<Uuid, MatchState>,
}
impl SafetyMonitor {
    pub fn reset(&mut self) {
        self.states.clear();
    }
    pub fn update(
        &mut self,
        rules: &[SafetyRule],
        scores: &[(Uuid, f64)],
        now: f64,
        max_gap: f64,
    ) -> Vec<SafetyRule> {
        let mut fired = vec![];
        for rule in rules.iter().filter(|r| r.enabled) {
            let state = self.states.entry(rule.id).or_default();
            let matched = scores
                .iter()
                .any(|(id, score)| *id == rule.id && score.is_finite() && *score >= rule.threshold);
            if !matched {
                *state = Default::default();
                continue;
            }
            if state
                .last
                .is_none_or(|last| now <= last || now - last > max_gap)
            {
                state.since = Some(now);
            }
            state.last = Some(now);
            if !state.fired && now - state.since.unwrap_or(now) >= rule.duration_seconds {
                state.fired = true;
                fired.push(rule.clone());
            }
        }
        fired
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventDestination {
    #[default]
    Run,
    Buffs,
    Loot,
    Mount,
    Periodic,
    Puller,
    OutOfTarget,
    Safety,
    Calibration,
    Settings,
    Recovery,
}
