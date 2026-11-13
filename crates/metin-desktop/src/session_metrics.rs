//! Presentation-only rates derived from the canonical active session statistics.
use metin_core::Statistics;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateDisplay {
    #[default]
    PerMinute,
    SecondsPerTarget,
}

impl RateDisplay {
    pub fn toggled(self) -> Self {
        match self {
            Self::PerMinute => Self::SecondsPerTarget,
            Self::SecondsPerTarget => Self::PerMinute,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::PerMinute => "metins/min",
            Self::SecondsPerTarget => "seconds/metin",
        }
    }

    pub fn value(self, rate: SessionRate) -> String {
        let value = match self {
            Self::PerMinute => rate.per_minute,
            Self::SecondsPerTarget => rate.seconds_per_target,
        };
        value.map_or_else(|| "—".into(), |value| format!("{value:.1}"))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SessionRate {
    pub per_minute: Option<f64>,
    pub seconds_per_target: Option<f64>,
}

impl SessionRate {
    pub fn from_statistics(statistics: &Statistics) -> Self {
        let seconds = statistics.active_seconds;
        if !seconds.is_finite() || seconds <= 0. {
            return Self::default();
        }
        let completed = statistics.completed as f64;
        let per_minute = completed * 60. / seconds;
        let average = seconds / completed;
        Self {
            per_minute: per_minute.is_finite().then_some(per_minute),
            seconds_per_target: (statistics.completed > 0 && average.is_finite())
                .then_some(average),
        }
    }

    pub fn tooltip(self) -> String {
        format!(
            "{} metins/min · {} seconds/metin\nUses the entire active cycle: searching, combat, loot, buffs and retries; excludes manual pauses and scheduled breaks. Targets ended counts confirmed target-panel disappearances, not independent proof of kills. Click to switch the displayed rate.",
            RateDisplay::PerMinute.value(self),
            RateDisplay::SecondsPerTarget.value(self),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_uses_the_entire_canonical_active_cycle_and_respects_pause_and_reset() {
        let mut session = metin_core::Session::default();
        session.advance(0., true);
        session.advance(30., true); // searching, combat, loot and other active work
        session.completed = 2;
        session.pause(Some(40.));
        session.advance(400., false); // a manual pause or break adds no active time
        let rate = SessionRate::from_statistics(&session.snapshot());
        assert_eq!(rate.per_minute, Some(3.));
        assert_eq!(rate.seconds_per_target, Some(20.));
        session.advance(400., true);
        session.advance(420., true);
        let rate = SessionRate::from_statistics(&session.snapshot());
        assert_eq!(rate.per_minute, Some(2.));
        assert_eq!(rate.seconds_per_target, Some(30.));
        session.reset();
        assert_eq!(
            SessionRate::from_statistics(&session.snapshot()),
            SessionRate::default()
        );
    }

    #[test]
    fn invalid_or_zero_time_never_produces_nonfinite_metrics_and_zero_targets_has_no_average() {
        for active_seconds in [0., -1., f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                SessionRate::from_statistics(&Statistics {
                    active_seconds,
                    completed: 4,
                    ..Default::default()
                }),
                SessionRate::default()
            );
        }
        assert_eq!(
            SessionRate::from_statistics(&Statistics {
                active_seconds: 60.,
                ..Default::default()
            }),
            SessionRate {
                per_minute: Some(0.),
                seconds_per_target: None
            }
        );
        let extreme = SessionRate::from_statistics(&Statistics {
            active_seconds: f64::MIN_POSITIVE,
            completed: u64::MAX,
            ..Default::default()
        });
        assert!(extreme.per_minute.is_none());
        assert!(extreme.seconds_per_target.is_none_or(f64::is_finite));
    }
}
