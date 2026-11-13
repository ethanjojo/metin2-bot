use super::*;
use metin_core::{AfterSuccess, RuleReactions, Verification};

fn reactions(v: RuleReactions) -> Value {
    let names: Vec<_> = [
        (v.notify, "Notify"),
        (v.pause, "Pause immediately"),
        (v.execute, "Perform action"),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect();
    Value::text(if names.is_empty() {
        "None".into()
    } else {
        names.join(" + ")
    })
}
pub(super) fn compare(rows: &mut Vec<ProfileChange>, a: &Profile, b: &Profile, g: Geometry) {
    let (x, y) = (&a.safety, &b.safety);
    for item in entities(&x.rules, &y.rules, |v| v.id) {
        let (old, new) = match item {
            Entity::Added(v) => {
                membership(rows, Subject::Rule(Some(v.id)), "Added rule", &v.name, true);
                continue;
            }
            Entity::Removed(v) => {
                membership(rows, Subject::Rule(None), "Removed rule", &v.name, false);
                continue;
            }
            Entity::Both(old, new) => (old, new),
        };
        let f = &mut Fields::new(rows, Subject::Rule(Some(new.id))).named(&new.name);
        f.text("Rule name", &old.name, &new.name);
        f.toggle("Enable this rule", old.enabled, new.enabled);
        f.active(new.enabled);
        f.field(
            "Event reference",
            g.capture(0, old.reference.as_ref()),
            g.capture(1, new.reference.as_ref()),
        );
        f.field(
            "Similarity threshold",
            Value::text(percent(old.threshold)),
            Value::text(percent(new.threshold)),
        );
        f.seconds(
            "Must persist for",
            old.duration_seconds,
            new.duration_seconds,
        );
        f.field(
            "Reactions",
            reactions(old.reaction),
            reactions(new.reaction),
        );
        f.active(new.enabled && new.reaction.execute);
        let (x, y) = (&old.execution, &new.execution);
        super::actions::input(f, &x.input, &y.input, g);
        let verify = |v| match v {
            Verification::ReferenceGone => "Verify disappearance",
            Verification::Delay => "Wait only",
        };
        let verification_changed = x.verification != y.verification;
        if verification_changed {
            let timeout = |e: &metin_core::RuleExecution| {
                let purpose = if e.verification == Verification::Delay {
                    "event confirmation"
                } else {
                    "verification"
                };
                Value::text(format!("{} s ({purpose})", e.timeout_seconds))
            };
            let attempts = |e: &metin_core::RuleExecution| {
                if e.verification == Verification::Delay {
                    Value::text(format!("{} (inactive setting)", e.attempts))
                } else {
                    Value::text(e.attempts.to_string())
                }
            };
            f.compound(
                "After input",
                Value::text(verify(x.verification)),
                Value::text(verify(y.verification)),
                vec![
                    Field {
                        label: "Timeout".into(),
                        before: timeout(x),
                        after: timeout(y),
                    },
                    Field {
                        label: "Maximum attempts".into(),
                        before: attempts(x),
                        after: attempts(y),
                    },
                ],
            );
        }
        f.seconds("Wait after input", x.wait_seconds, y.wait_seconds);
        if !verification_changed {
            f.seconds(
                if y.verification == Verification::Delay {
                    "Event confirmation timeout"
                } else {
                    "Verification timeout"
                },
                x.timeout_seconds,
                y.timeout_seconds,
            );
        }
        let completion = |v| match v {
            AfterSuccess::Pause => "Pause",
            AfterSuccess::Continue => "Continue",
        };
        f.text(
            "After completion",
            completion(x.after_success),
            completion(y.after_success),
        );
        f.active(
            new.enabled && new.reaction.execute && y.verification == Verification::ReferenceGone,
        );
        if !verification_changed {
            f.count("Maximum attempts", x.attempts, y.attempts);
        }
    }
    let f = &mut Fields::new(rows, Subject::Safety(SafetyPage::Schedule));
    f.toggle("Stop after active time", x.stop_enabled, y.stop_enabled);
    f.toggle("Stop at local time", x.clock_enabled, y.clock_enabled);
    f.toggle("Take regular breaks", x.breaks_enabled, y.breaks_enabled);
    let duration = |s: f64| {
        if s % 60. == 0. {
            Value::text(format!("{} min", s / 60.))
        } else {
            seconds(s)
        }
    };
    f.active(y.stop_enabled);
    f.field(
        "Active time limit",
        duration(x.stop_after_seconds),
        duration(y.stop_after_seconds),
    );
    f.active(y.clock_enabled);
    let clock = |m: u32| Value::text(format!("{:02}:{:02}", m / 60, m % 60));
    f.field(
        "Local stop time",
        clock(x.stop_at_minute),
        clock(y.stop_at_minute),
    );
    f.active(y.breaks_enabled);
    f.field(
        "Break every",
        duration(x.break_every_seconds),
        duration(y.break_every_seconds),
    );
    f.field(
        "Break duration",
        duration(x.break_seconds),
        duration(y.break_seconds),
    );
    let f = &mut Fields::new(rows, Subject::Safety(SafetyPage::Recovery));
    f.toggle(
        "Attempt limited recovery",
        x.recovery_enabled,
        y.recovery_enabled,
    );
    f.active(y.recovery_enabled);
    f.count(
        "Maximum recovery attempts",
        x.recovery_attempts,
        y.recovery_attempts,
    );
    f.seconds(
        "Wait after recovery",
        x.recovery_wait_seconds,
        y.recovery_wait_seconds,
    );
    let f = &mut Fields::new(rows, Subject::Safety(SafetyPage::Notifications));
    f.toggle("Play a sound", x.sound, y.sound);
    f.toggle(
        "Show a system notification",
        x.system_notification,
        y.system_notification,
    );
    let f = &mut Fields::new(rows, Subject::Timing);
    let (x, y) = (&a.timing, &b.timing);
    for (name, before, after) in [
        ("Capture interval", x.sample_interval, y.sample_interval),
        ("After target ends", x.settle_seconds, y.settle_seconds),
        ("Search timeout", x.search_timeout, y.search_timeout),
        ("Confirmation timeout", x.acquire_timeout, y.acquire_timeout),
        ("Attack timeout", x.attack_timeout, y.attack_timeout),
        (
            "Cursor settling",
            x.click_hover_seconds,
            y.click_hover_seconds,
        ),
        ("Mouse hold", x.click_hold_seconds, y.click_hold_seconds),
        ("Camera rotation", x.rotate_seconds, y.rotate_seconds),
        (
            "Failed point cooldown",
            x.failed_cooldown,
            y.failed_cooldown,
        ),
    ] {
        f.seconds(name, before, after);
    }
    f.count(
        "Maximum rotations / retries",
        x.max_rotations,
        y.max_rotations,
    );
}
