use crate::{Region, Result, Shortcut, require};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionInput {
    Key { shortcut: String, hold_seconds: f64 },
    Click { region: Option<Region> },
}
impl Default for ActionInput {
    fn default() -> Self {
        Self::Key {
            shortcut: "F4".into(),
            hold_seconds: 0.15,
        }
    }
}
impl ActionInput {
    pub fn shortcut(&self) -> Option<&str> {
        match self {
            Self::Key { shortcut, .. } => Some(shortcut),
            Self::Click { .. } => None,
        }
    }
    pub fn validate(&self, enabled: bool, frame_size: Option<[u32; 2]>) -> Result<()> {
        match self {
            Self::Key {
                shortcut,
                hold_seconds,
            } => {
                Shortcut::parse(shortcut)?;
                crate::validation::number("key_hold_seconds", *hold_seconds)?;
            }
            Self::Click { region } => {
                if let Some(region) = region {
                    region.validate()?;
                }
                if enabled {
                    require(
                        region.is_some() && frame_size.is_some(),
                        "Select a click area on a calibrated snapshot before enabling this action.",
                    )?;
                }
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    #[default]
    ReferenceGone,
    Delay,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AfterSuccess {
    #[default]
    Pause,
    Continue,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuleExecution {
    pub input: ActionInput,
    pub verification: Verification,
    pub wait_seconds: f64,
    pub timeout_seconds: f64,
    pub attempts: u32,
    pub after_success: AfterSuccess,
}
impl Default for RuleExecution {
    fn default() -> Self {
        Self {
            input: Default::default(),
            verification: Default::default(),
            wait_seconds: 1.,
            timeout_seconds: 10.,
            attempts: 2,
            after_success: Default::default(),
        }
    }
}
impl RuleExecution {
    pub fn validate(&self) -> Result<()> {
        self.input.validate(false, None)?;
        crate::validation::number("wait_seconds", self.wait_seconds)?;
        crate::validation::number("visual_timeout_seconds", self.timeout_seconds)?;
        crate::validation::number("visual_attempts", f64::from(self.attempts))
    }
}
