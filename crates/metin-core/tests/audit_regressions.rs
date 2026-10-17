use metin_core::*;

fn empty() -> Observation {
    Observation {
        ready: true,
        ..Default::default()
    }
}

#[test]
fn refresh_buffs_repeat_during_uninterrupted_search() {
    let mut p = Profile::create("Refresh during search");
    p.buffs.enabled = true;
    p.buffs.fight_mounted = false;
    p.buffs.skills.push(BuffSkill {
        interval_seconds: Some(10.),
        wait_seconds: 0.2,
        ..Default::default()
    });
    p.timing.search_timeout = 120.;
    let mut c = Controller::new(p);
    c.buffs.mounted = Some(false);
    c.start(0.).unwrap();
    let mut casts = Vec::new();
    for i in 0..100 {
        let now = f64::from(i) * 0.25;
        if let Some(a) = c.step(now, &empty()) {
            if matches!(a.kind, ActionKind::Skill { .. }) {
                casts.push(now);
            }
            c.ack(now, a.id, true).unwrap();
        }
    }
    assert_eq!(casts.len(), 3, "Refresh should recur: {casts:?}");
    assert!(c.running);
}

#[test]
fn periodic_animation_does_not_consume_search_timeout() {
    let mut p = Profile::create("Search budget");
    p.timing.search_timeout = 5.;
    let mut action = PeriodicAction::new("Long animation");
    action.enabled = true;
    action.first_run = FirstRun::Immediate;
    action.wait_seconds = 30.;
    p.periodic_actions.push(action);
    let mut c = Controller::new(p);
    c.start(0.).unwrap();
    let mut rotations = Vec::new();
    for i in 0..149 {
        let now = f64::from(i) * 0.25;
        if let Some(a) = c.step(now, &empty()) {
            if matches!(a.kind, ActionKind::Rotate) {
                rotations.push(now);
            }
            c.ack(now, a.id, true).unwrap();
        }
    }
    assert_eq!(
        rotations.len(),
        1,
        "One complete search allowance after the animation: {rotations:?}"
    );
    assert!(rotations[0] >= 35. && rotations[0] <= 36., "{rotations:?}");
}

#[test]
fn expired_failed_target_gets_priority_after_a_long_refresh_animation() {
    let mut p = Profile::create("Cooldown during buff");
    p.buffs.enabled = true;
    p.buffs.fight_mounted = false;
    p.buffs.skills.push(BuffSkill {
        interval_seconds: Some(10.),
        wait_seconds: 30.,
        ..Default::default()
    });
    p.buffs.skills.push(BuffSkill {
        shortcut: "F2".into(),
        interval_seconds: Some(10.),
        wait_seconds: 30.,
        ..Default::default()
    });
    p.timing.search_timeout = 120.;
    p.timing.acquire_timeout = 3.;
    p.timing.failed_cooldown = 10.;
    let mut c = Controller::new(p);
    c.buffs.mounted = Some(false);
    c.start(0.).unwrap();
    let mut o = empty();
    o.candidates.push(Candidate {
        x: 40,
        y: 50,
        width: 10,
        height: 10,
        point: [45, 55],
    });
    let mut clicks = Vec::new();
    for i in 0..801 {
        let now = f64::from(i) * 0.25;
        if let Some(a) = c.step(now, &o) {
            if matches!(a.kind, ActionKind::Click { .. }) {
                clicks.push(now);
            }
            c.ack(now, a.id, true).unwrap();
        }
    }
    assert!(
        clicks.len() >= 2,
        "Expired target cannot be starved by refresh: {clicks:?}"
    );
}
