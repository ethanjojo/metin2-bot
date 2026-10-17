use metin_core::*;
fn obs(panel: bool) -> Observation {
    Observation {
        ready: true,
        panel_present: panel,
        ..Default::default()
    }
}
fn profile() -> Profile {
    let mut p = Profile::create("Combat");
    p.panel.stable_frames = 2;
    p
}
#[test]
fn puller_start_burst_does_not_retrigger_from_flicker_or_pause() {
    let mut p = profile();
    p.puller.enabled = true;
    p.puller.presses = 2;
    p.puller.press_interval_seconds = 0.3;
    let mut c = Controller::new(p);
    c.start(0.).unwrap();
    assert!(c.step(0., &obs(true)).is_none());
    c.step(0.25, &obs(true));
    let a = c.step(0.5, &obs(true)).unwrap();
    assert!(matches!(a.kind, ActionKind::Pull { .. }));
    c.ack(0.5, a.id, true).unwrap();
    assert!(c.step(0.7, &obs(true)).is_none());
    let a = c.step(0.85, &obs(true)).unwrap();
    c.ack(0.85, a.id, true).unwrap();
    c.step(1., &obs(false));
    c.step(1.25, &obs(true));
    assert!(c.step(1.5, &obs(true)).is_none());
    c.pause(1.6, "F8");
    c.start(5.).unwrap();
    c.step(5., &obs(true));
    c.step(5.25, &obs(true));
    assert!(c.step(5.5, &obs(true)).is_none());
}
#[test]
fn target_end_attack_precedes_loot_and_does_not_count_an_extra_target() {
    let mut p = profile();
    p.out_of_target.after_combat = true;
    p.out_of_target.after_combat_seconds = 1.;
    p.loot.enabled = true;
    let mut c = Controller::new(p);
    c.start(0.).unwrap();
    c.step(0., &obs(true));
    c.step(0.25, &obs(true));
    c.step(0.5, &obs(false));
    let a = c.step(0.75, &obs(false)).unwrap();
    assert!(matches!(a.kind, ActionKind::Attack { .. }));
    c.ack(0.75, a.id, true).unwrap();
    assert_eq!(c.session.completed, 1);
    assert!(c.step(1., &obs(false)).is_none());
    let a = c.step(1.8, &obs(false)).unwrap();
    assert!(matches!(a.kind, ActionKind::ReleaseAttack));
    c.ack(1.8, a.id, true).unwrap();
    assert_eq!(c.phase, Phase::Looting);
    assert_eq!(c.session.completed, 1);
}
#[test]
fn attack_stops_immediately_when_a_panel_appears() {
    let mut p = profile();
    p.out_of_target.after_combat = true;
    let mut c = Controller::new(p);
    c.start(0.).unwrap();
    c.step(0., &obs(true));
    c.step(0.25, &obs(true));
    c.step(0.5, &obs(false));
    let a = c.step(0.75, &obs(false)).unwrap();
    c.ack(0.75, a.id, true).unwrap();
    let a = c.step(1., &obs(true)).unwrap();
    assert!(matches!(a.kind, ActionKind::ReleaseAttack));
    assert_ne!(c.phase, Phase::Looting);
}
#[test]
fn legacy_puller_modes_load_as_combat_periodic_and_space_roundtrips() {
    let mut json = serde_json::to_value(profile()).unwrap();
    for mode in ["searching", "combat", "both"] {
        json["puller"]["mode"] = mode.into();
        let p = Profile::from_json(&json.to_string()).unwrap();
        assert_eq!(p.puller.mode, PullerMode::CombatPeriodic);
    }
    assert_eq!(Shortcut::parse("Space").unwrap().to_string(), "Space");
}

#[test]
fn periodic_puller_stops_at_target_end_and_never_fires_while_searching() {
    let mut p = profile();
    p.puller.enabled = true;
    p.puller.mode = PullerMode::CombatPeriodic;
    p.puller.timing.interval_seconds = 0.5;
    p.puller.timing.wait_seconds = 0.2;
    let mut c = Controller::new(p);
    c.start(0.).unwrap();
    c.step(0., &obs(true));
    c.step(0.25, &obs(true));
    let a = c.step(0.5, &obs(true)).unwrap();
    c.ack(0.5, a.id, true).unwrap();
    assert!(c.step(0.75, &obs(true)).is_none());
    assert!(c.step(1., &obs(true)).is_none());
    let a = c.step(1.25, &obs(true)).unwrap();
    assert!(matches!(a.kind, ActionKind::Pull { .. }));
    c.ack(1.25, a.id, true).unwrap();
    c.step(1.5, &obs(false));
    c.step(1.75, &obs(false));
    for now in [2., 2.25, 2.5, 2.75, 3., 3.25] {
        assert!(c.step(now, &obs(false)).is_none());
    }
    assert_eq!(c.action_timers()[0].remaining_seconds, None);
}
#[test]
fn long_search_attacks_are_bounded_and_do_not_count_as_kills_or_loot() {
    let mut p = profile();
    p.timing.search_timeout = 120.;
    p.out_of_target.during_search = true;
    p.out_of_target.search_after_seconds = 1.;
    p.out_of_target.search_attack_seconds = 0.5;
    p.out_of_target.max_search_attacks = 2;
    p.loot.enabled = true;
    let mut c = Controller::new(p);
    c.start(0.).unwrap();
    let mut holds = 0;
    let mut releases = 0;
    for i in 0..40 {
        let now = f64::from(i) * 0.25;
        if let Some(a) = c.step(now, &obs(false)) {
            match a.kind {
                ActionKind::Attack { .. } => holds += 1,
                ActionKind::ReleaseAttack => releases += 1,
                _ => panic!("Unexpected action {:?}", a.kind),
            }
            c.ack(now, a.id, true).unwrap();
        }
        if !c.running {
            break;
        }
    }
    assert_eq!((holds, releases), (2, 2));
    assert!(!c.running);
    assert_eq!(c.session.completed, 0);
    assert!(c.reason.contains("search attacks"));
}
#[test]
fn new_candidate_interrupts_search_attack_before_selection() {
    let mut p = profile();
    p.timing.search_timeout = 120.;
    p.out_of_target.during_search = true;
    p.out_of_target.search_after_seconds = 1.;
    let mut c = Controller::new(p);
    c.start(0.).unwrap();
    let mut started = false;
    for i in 0..12 {
        let now = f64::from(i) * 0.25;
        if let Some(a) = c.step(now, &obs(false)) {
            c.ack(now, a.id, true).unwrap();
            started = true;
            break;
        }
    }
    assert!(started);
    let mut o = obs(false);
    o.candidates.push(Candidate {
        x: 1,
        y: 1,
        width: 10,
        height: 10,
        point: [5, 5],
    });
    let a = c.step(3., &o).unwrap();
    assert!(matches!(a.kind, ActionKind::ReleaseAttack));
    assert_eq!(c.session.clicks, 0);
}
#[test]
fn invalid_attack_options_and_conflicting_keys_are_rejected() {
    let mut p = profile();
    p.out_of_target.after_combat = true;
    p.out_of_target.after_combat_seconds = f64::NAN;
    assert!(p.validate().is_err());
    p.out_of_target.after_combat_seconds = 3.;
    p.puller.enabled = true;
    p.out_of_target.shortcut = p.puller.timing.shortcut.clone();
    assert!(validate_scheduled(&p).is_err());
}
