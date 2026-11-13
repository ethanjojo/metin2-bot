use super::*;
use metin_core::{ActionInput, BuffMode, BuffSkill, FirstRun, PullerMode};

pub(super) fn buff_name(v: &BuffSkill) -> &str {
    if !v.name.trim().is_empty() {
        &v.name
    } else if !v.shortcut.trim().is_empty() {
        &v.shortcut
    } else {
        "Unnamed buff"
    }
}

pub(super) fn input(f: &mut Fields<'_>, a: &ActionInput, b: &ActionInput, g: Geometry) {
    match (a, b) {
        (
            ActionInput::Key {
                shortcut: x,
                hold_seconds: i,
            },
            ActionInput::Key {
                shortcut: y,
                hold_seconds: j,
            },
        ) => {
            f.text("Keyboard shortcut", x, y);
            f.seconds("Key hold", *i, *j);
        }
        (ActionInput::Click { region: x }, ActionInput::Click { region: y }) => {
            g.region(f, "Click target", *x, *y, "Not selected")
        }
        _ => {
            let describe = |v: &ActionInput, side| match v {
                ActionInput::Key {
                    shortcut,
                    hold_seconds,
                } => ("Key", format!("{shortcut} · {hold_seconds} s hold")),
                ActionInput::Click { region } => {
                    ("Click", g.bounds(side, *region, "No click target"))
                }
            };
            let (old, old_detail) = describe(a, 0);
            let (new, new_detail) = describe(b, 1);
            f.compound(
                "Input",
                Value::text(old),
                Value::text(new),
                vec![Field {
                    label: "Input settings".into(),
                    before: Value::text(old_detail),
                    after: Value::text(new_detail),
                }],
            );
        }
    }
}

fn buff(rows: &mut Vec<ProfileChange>, a: &Profile, b: &Profile, g: Geometry) {
    let (old, new) = (&a.buffs, &b.buffs);
    let f = &mut Fields::new(rows, Subject::Buff(None));
    f.toggle("Automatic buffs", old.enabled, new.enabled);
    f.active(new.enabled);
    let ids_in_both = |skills: &[BuffSkill], other: &[BuffSkill]| {
        skills
            .iter()
            .filter(|s| other.iter().any(|v| v.id == s.id))
            .map(|s| s.id)
            .collect::<Vec<_>>()
    };
    let old_ids = ids_in_both(&old.skills, &new.skills);
    let new_ids = ids_in_both(&new.skills, &old.skills);
    if old_ids != new_ids {
        let names = |skills: &[BuffSkill]| {
            skills
                .iter()
                .enumerate()
                .map(|(index, v)| {
                    let position = index + 1;
                    let name = buff_name(v);
                    if !v.shortcut.is_empty() && name != v.shortcut {
                        format!("{position}. {name} ({})", v.shortcut)
                    } else {
                        format!("{position}. {name}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        f.compound(
            "Buff order",
            Value::text(names(&old.skills)),
            Value::text(names(&new.skills)),
            vec![],
        );
    }
    let f = &mut Fields::new(rows, Subject::BuffArea);
    f.active(new.enabled && new.skills.iter().any(|s| s.mode == BuffMode::Inactive));
    g.region(
        f,
        "Buff icon search area",
        old.icon_region,
        new.icon_region,
        "Not selected",
    );
    for item in entities(&old.skills, &new.skills, |v| v.id) {
        let (x, y) = match item {
            Entity::Added(v) => {
                membership(
                    rows,
                    Subject::Buff(Some(v.id)),
                    "Added buff",
                    buff_name(v),
                    true,
                );
                continue;
            }
            Entity::Removed(v) => {
                membership(
                    rows,
                    Subject::Buff(None),
                    "Removed buff",
                    buff_name(v),
                    false,
                );
                continue;
            }
            Entity::Both(x, y) => (x, y),
        };
        let f = &mut Fields::new(rows, Subject::Buff(Some(y.id))).named(buff_name(y));
        f.text("Buff name", &x.name, &y.name);
        let icon = |id: Option<metin_core::SkillIconId>| {
            id.map_or_else(
                || Value::text("Automatic"),
                |id| Value::illustrated(catalog::skill(id).name, Preview::Icon(id)),
            )
        };
        f.field("Display icon", icon(x.display_icon), icon(y.display_icon));
        f.active(new.enabled);
        f.text("Buff shortcut", &x.shortcut, &y.shortcut);
        let interval = |v: Option<f64>| v.map_or_else(|| Value::text("No timer"), seconds);
        if x.mode != y.mode {
            let mode = |v: BuffMode| {
                Value::text(match v {
                    BuffMode::Refresh => "Timed",
                    BuffMode::Inactive => "Icon-based",
                })
            };
            f.compound(
                "Buff mode",
                mode(x.mode),
                mode(y.mode),
                vec![Field {
                    label: "Refresh interval".into(),
                    before: interval(x.interval_seconds),
                    after: interval(y.interval_seconds),
                }],
            );
        } else {
            f.active(new.enabled && y.mode == BuffMode::Refresh);
            f.field(
                "Refresh interval",
                interval(x.interval_seconds),
                interval(y.interval_seconds),
            );
        }
        f.active(new.enabled);
        f.seconds("Wait after buff", x.wait_seconds, y.wait_seconds);
        f.active(new.enabled && y.mode == BuffMode::Inactive);
        f.seconds(
            "Keep Active for",
            x.active_hold_seconds,
            y.active_hold_seconds,
        );
        f.field(
            "Active reference",
            g.capture(0, x.icon.as_ref()),
            g.capture(1, y.icon.as_ref()),
        );
        f.field(
            "Inactive reference",
            g.capture(0, x.inactive_icon.as_ref()),
            g.capture(1, y.inactive_icon.as_ref()),
        );
    }
}
pub(super) fn compare(rows: &mut Vec<ProfileChange>, a: &Profile, b: &Profile, g: Geometry) {
    buff(rows, a, b, g);
    let (x, y) = (&a.buffs, &b.buffs);
    let f = &mut Fields::new(rows, Subject::Mount);
    f.active(y.enabled);
    f.toggle("Fight mounted", x.fight_mounted, y.fight_mounted);
    f.text("Mount shortcut", &x.mount_shortcut, &y.mount_shortcut);
    f.seconds("Key hold duration", x.key_hold_seconds, y.key_hold_seconds);
    f.seconds(
        "Wait after mount toggle",
        x.mount_wait_seconds,
        y.mount_wait_seconds,
    );

    let (x, y) = (&a.loot, &b.loot);
    let f = &mut Fields::new(rows, Subject::Loot);
    f.toggle("Automatic loot", x.enabled, y.enabled);
    f.active(y.enabled);
    f.text("Pickup shortcut", &x.shortcut, &y.shortcut);
    f.count("Pickup presses", x.presses, y.presses);
    f.seconds("Delay before pickup", x.delay_seconds, y.delay_seconds);
    f.active(y.enabled && y.presses > 1);
    f.seconds(
        "Wait between presses",
        x.interval_seconds,
        y.interval_seconds,
    );
    f.active(y.enabled);
    f.seconds("Pickup key hold", x.hold_seconds, y.hold_seconds);

    for item in entities(&a.periodic_actions, &b.periodic_actions, |v| v.id) {
        let (x, y) = match item {
            Entity::Added(v) => {
                membership(
                    rows,
                    Subject::Periodic(Some(v.id)),
                    "Added action",
                    &v.name,
                    true,
                );
                continue;
            }
            Entity::Removed(v) => {
                membership(
                    rows,
                    Subject::Periodic(None),
                    "Removed action",
                    &v.name,
                    false,
                );
                continue;
            }
            Entity::Both(x, y) => (x, y),
        };
        let f = &mut Fields::new(rows, Subject::Periodic(Some(y.id))).named(&y.name);
        f.text("Action name", &x.name, &y.name);
        f.toggle("Enable this action", x.enabled, y.enabled);
        f.active(y.enabled);
        input(f, &x.input, &y.input, g);
        f.seconds("Repeat interval", x.interval_seconds, y.interval_seconds);
        f.seconds("Wait after use", x.wait_seconds, y.wait_seconds);
        let first = |v| match v {
            FirstRun::Immediate => "Immediately",
            FirstRun::AfterInterval => "After interval",
        };
        f.text(
            "First use after Start",
            first(x.first_run),
            first(y.first_run),
        );
    }
    let (x, y) = (&a.puller, &b.puller);
    let f = &mut Fields::new(rows, Subject::Puller);
    f.toggle("Automatic puller", x.enabled, y.enabled);
    f.active(y.enabled);
    f.text("Puller shortcut", &x.timing.shortcut, &y.timing.shortcut);
    let repeat = y.mode == PullerMode::CombatPeriodic;
    let mode = |v| match v {
        PullerMode::CombatStart => "Combat start",
        PullerMode::CombatPeriodic => "Repeat",
    };
    if x.mode != y.mode {
        let usage = [
            x.enabled && x.mode == PullerMode::CombatPeriodic,
            y.enabled && repeat,
        ];
        let details = vec![
            Field::retained_seconds(
                "Repeat interval",
                x.timing.interval_seconds,
                y.timing.interval_seconds,
                usage,
            ),
            Field::retained_seconds(
                "Wait after sequence",
                x.timing.wait_seconds,
                y.timing.wait_seconds,
                usage,
            ),
        ];
        f.compound(
            "When to use",
            Value::text(mode(x.mode)),
            Value::text(mode(y.mode)),
            details,
        );
    }
    f.count("Presses per sequence", x.presses, y.presses);
    f.seconds(
        "Shortcut hold",
        x.timing.hold_seconds,
        y.timing.hold_seconds,
    );
    f.active(y.enabled && repeat);
    if x.mode == y.mode {
        f.seconds(
            "Repeat interval",
            x.timing.interval_seconds,
            y.timing.interval_seconds,
        );
        f.seconds(
            "Wait after sequence",
            x.timing.wait_seconds,
            y.timing.wait_seconds,
        );
    }
    f.active(y.enabled && y.presses > 1);
    f.seconds(
        "Wait between presses",
        x.press_interval_seconds,
        y.press_interval_seconds,
    );

    let (x, y) = (&a.out_of_target, &b.out_of_target);
    let f = &mut Fields::new(rows, Subject::OutOfTarget);
    f.active(y.enabled());
    f.text("Attack shortcut", &x.shortcut, &y.shortcut);
    f.active(true);
    f.toggle("Attack after a target ends", x.after_combat, y.after_combat);
    f.toggle(
        "Attack when searching takes too long",
        x.during_search,
        y.during_search,
    );
    f.active(y.after_combat);
    f.seconds(
        "After-target attack duration",
        x.after_combat_seconds,
        y.after_combat_seconds,
    );
    f.active(y.during_search);
    f.seconds(
        "After searching for",
        x.search_after_seconds,
        y.search_after_seconds,
    );
    f.seconds(
        "Search attack duration",
        x.search_attack_seconds,
        y.search_attack_seconds,
    );
    f.count(
        "Maximum search attacks",
        x.max_search_attacks,
        y.max_search_attacks,
    );
}
