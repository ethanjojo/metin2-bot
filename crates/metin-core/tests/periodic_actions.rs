use metin_core::{ActionKind, Controller, Observation, PeriodicAction, Profile, PullerMode};

fn assert_setup_group(profile: &Profile, group: metin_core::ScheduledGroup) {
    let issue = metin_core::validate_scheduled_setup(profile).unwrap_err();
    assert_eq!(issue.group, group);
    assert_eq!(
        metin_core::validate_scheduled(profile).unwrap_err(),
        issue.error
    );
}

#[test]
fn scheduled_setup_locates_invalid_fields_without_changing_validation_errors() {
    use metin_core::ScheduledGroup as G;
    let base = Profile::create("Setup");
    assert!(metin_core::validate_scheduled_setup(&base).is_ok());

    let mut p = base.clone();
    p.buffs.enabled = true;
    p.buffs.mount_shortcut.clear();
    assert_setup_group(&p, G::Buffs);

    let mut p = base.clone();
    p.loot.enabled = true;
    p.loot.shortcut.clear();
    assert_setup_group(&p, G::Loot);

    let mut p = base.clone();
    p.puller.enabled = true;
    p.periodic_actions.push(PeriodicAction::new(""));
    assert_setup_group(&p, G::Periodic);

    let mut p = base.clone();
    p.puller.timing.shortcut.clear();
    assert_setup_group(&p, G::Puller);

    let mut p = base;
    p.out_of_target.shortcut.clear();
    assert_setup_group(&p, G::OutOfTarget);
}

#[test]
fn scheduled_setup_preserves_conflict_ownership_and_validation_order() {
    use metin_core::ScheduledGroup as G;
    let mut p = Profile::create("Conflicting actions");
    let mut action = PeriodicAction::new("Use item");
    action.enabled = true;
    p.periodic_actions.push(action);
    p.puller.enabled = true;
    assert_setup_group(&p, G::Puller);

    // Field validation precedes cross-action conflicts, as in the runtime API.
    p.out_of_target.shortcut.clear();
    assert_setup_group(&p, G::OutOfTarget);
    p.periodic_actions[0].name.clear();
    assert_setup_group(&p, G::Periodic);

    p.periodic_actions.clear();
    p.out_of_target.shortcut = p.puller.timing.shortcut.clone();
    p.out_of_target.after_combat = true;
    assert_setup_group(&p, G::OutOfTarget);
}

fn observation(panel: bool) -> Observation {
    Observation {
        ready: true,
        panel_present: panel,
        ..Default::default()
    }
}
#[test]
fn periodic_action_waits_between_targets_and_starts_interval_after_ack_and_wait() {
    let mut p = Profile::create("Periodic");
    p.panel.stable_frames = 2;
    let mut action = PeriodicAction::new("Use item");
    action.enabled = true;
    action.first_run = metin_core::FirstRun::Immediate;
    action.wait_seconds = 1.;
    action.interval_seconds = 10.;
    p.periodic_actions.push(action);
    let mut c = Controller::new(p);
    c.start(0.).unwrap();
    c.step(0., &observation(true));
    c.step(0.25, &observation(true));
    assert!(c.step(1., &observation(true)).is_none());
    c.step(1.25, &observation(false));
    c.step(1.5, &observation(false));
    c.step(2., &observation(false));
    let a = c.step(2.5, &observation(false)).unwrap();
    assert!(matches!(a.kind, ActionKind::Scheduled { .. }));
    assert!(c.step(3., &observation(false)).is_none());
    c.ack(3., a.id, true).unwrap();
    assert!(c.step(3.5, &observation(false)).is_none());
    c.step(4., &observation(false));
    assert_eq!(c.action_timers()[0].remaining_seconds, Some(10.));
    c.pause(5., "F8");
    c.start(100.).unwrap();
    assert_eq!(c.action_timers()[0].remaining_seconds, Some(9.));
}
#[test]
fn puller_mode_controls_combat_and_pause_never_bursts_overdue_actions() {
    let mut p = Profile::create("Puller");
    p.panel.stable_frames = 2;
    p.puller.enabled = true;
    p.puller.mode = PullerMode::CombatPeriodic;
    let mut c = Controller::new(p);
    c.start(0.).unwrap();
    c.step(0., &observation(false));
    assert!(c.step(0.25, &observation(false)).is_none());
    c.step(0.5, &observation(true));
    c.step(0.75, &observation(true));
    let a = c.step(1., &observation(true)).unwrap();
    assert!(matches!(a.kind, ActionKind::Pull { .. }));
    c.pause(1.1, "F8");
    assert!(c.ack(1.2, a.id, true).is_err());
    assert!(c.step(100., &observation(true)).is_none());
}

#[test]
fn acknowledged_action_is_not_repeated_after_pause_during_animation() {
    let mut p = Profile::create("Periodic");
    p.panel.stable_frames = 2;
    let mut a = PeriodicAction::new("Item");
    a.enabled = true;
    a.first_run = metin_core::FirstRun::Immediate;
    p.periodic_actions.push(a);
    let mut c = Controller::new(p);
    c.start(0.).unwrap();
    c.step(0., &observation(false));
    let a = c.step(0.25, &observation(false)).unwrap();
    c.ack(0.4, a.id, true).unwrap();
    c.pause(0.5, "F8");
    c.start(100.).unwrap();
    c.step(100., &observation(false));
    assert!(c.step(100.25, &observation(false)).is_none());
    assert!(c.action_timers()[0].remaining_seconds.unwrap() > 100.);
}

#[test]
fn overdue_periodic_actions_are_not_starved_by_shorter_intervals() {
    let mut p = Profile::create("Fair scheduling");
    for name in ["A", "B", "C"] {
        let mut a = PeriodicAction::new(name);
        a.enabled = true;
        a.first_run = metin_core::FirstRun::Immediate;
        a.interval_seconds = 10.;
        a.wait_seconds = 30.;
        p.periodic_actions.push(a);
    }
    p.puller.enabled = true;
    let mut scheduler = metin_core::ActionScheduler::default();
    let mut seen = vec![];
    for now in [0., 30., 60.] {
        let (key, name, _) = scheduler.next(&p, now).unwrap();
        seen.push(name);
        scheduler.complete(key, now + 30., 10.);
    }
    assert_eq!(seen, vec!["A", "B", "C"]);
}
