use crate::{Result, require};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Region {
    pub fn validate(self) -> Result<()> {
        require(
            [self.left, self.top, self.right, self.bottom]
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
            "Region coordinates must be finite and within 0–1.",
        )?;
        require(
            self.right - self.left >= 0.001 && self.bottom - self.top >= 0.001,
            "Region must have positive dimensions of at least 0.1%.",
        )
    }
    /// Inclusive pixel endpoints; intentionally matches Python's half-even round.
    pub fn pixels(self, width: u32, height: u32) -> [u32; 4] {
        let x =
            |v: f64| ((v * f64::from(width)).round_ties_even() as u32).min(width.saturating_sub(1));
        let y = |v: f64| {
            ((v * f64::from(height)).round_ties_even() as u32).min(height.saturating_sub(1))
        };
        [x(self.left), y(self.top), x(self.right), y(self.bottom)]
    }
    pub fn contains(self, inner: Self) -> bool {
        self.left <= inner.left
            && self.top <= inner.top
            && self.right >= inner.right
            && self.bottom >= inner.bottom
    }
}
