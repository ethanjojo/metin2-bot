use metin_core::{ActionKind, Controller, Observation, Phase, Profile};

fn observation(panel: bool) -> Observation {
    Observation {
        ready: true,
        panel_present: panel,
        ..Default::default()
    }
}
fn controller() -> Controller {
    let mut p = Profile::create("Test");
    p.panel.stable_frames = 2;
    p.loot.enabled = true;
    p.loot.delay_seconds = 0.;
    p.loot.presses = 2;
    p.loot.interval_seconds = 0.2;
    Controller::new(p)
}
#[test]
fn loot_requires_confirmed_target_end_and_acknowledged_presses() {
    let mut c = controller();
    c.start(0.).unwrap();
    assert!(c.step(0., &observation(false)).is_none());
    assert!(c.step(0.25, &observation(false)).is_none());
    assert_ne!(c.phase, Phase::Looting);
    c.step(0.5, &observation(true));
    c.step(0.75, &observation(true));
    assert_eq!(c.phase, Phase::Active);
    c.step(1., &observation(false));
    c.step(1.25, &observation(false));
    assert_eq!(c.phase, Phase::Looting);
    let a = c.step(1.5, &observation(false)).unwrap();
    assert!(matches!(a.kind, ActionKind::Loot { .. }));
    assert!(c.step(1.75, &observation(false)).is_none());
    c.ack(1.75, a.id, true).unwrap();
    assert!(c.step(1.8, &observation(false)).is_none());
    let a = c.step(2., &observation(false)).unwrap();
    c.ack(2.1, a.id, true).unwrap();
    c.step(2.2, &observation(false));
    assert_eq!(c.phase, Phase::Settling);
    assert_eq!(c.session.completed, 1);
}
#[test]
fn automatic_stop_ignores_statistics_reset_and_manual_pause_time() {
    let mut c = controller();
    c.profile.safety.stop_enabled = true;
    c.profile.safety.stop_after_seconds = 60.;
    c.start(0.).unwrap();
    c.tick(20.);
    c.session.reset();
    c.pause(25., "Manual pause");
    c.start(100.).unwrap();
    c.tick(134.);
    assert!(c.running);
    c.tick(135.);
    assert!(!c.running);
    assert!(c.reason.contains("Automatic stop"));
    c.start(200.).unwrap();
    assert_eq!(c.schedule(200.).stop_remaining, Some(60.));
}
#[test]
fn break_excludes_time_and_manual_pause_prevents_resume() {
    let mut c = controller();
    c.profile.safety.breaks_enabled = true;
    c.profile.safety.break_every_seconds = 60.;
    c.profile.safety.break_seconds = 30.;
    c.start(0.).unwrap();
    c.step(60., &observation(false));
    c.step(60.25, &observation(false));
    assert_eq!(c.phase, Phase::Break);
    let active = c.schedule(60.25).active_seconds;
    c.tick(80.);
    assert_eq!(c.schedule(80.).active_seconds, active);
    c.pause(80., "F8");
    c.tick(200.);
    assert!(!c.running);
    c.start(210.).unwrap();
    assert_ne!(c.phase, Phase::Break);
    assert_eq!(c.schedule(210.).break_remaining, Some(60.));
}
#[test]
fn new_profile_options_are_disabled_and_invalid_values_rejected() {
    let mut p = Profile::create("Test");
    assert!(!p.loot.enabled && !p.safety.stop_enabled && !p.safety.breaks_enabled);
    p.loot.enabled = true;
    p.loot.shortcut = "F8".into();
    assert!(p.validate().is_err());
    p.loot.shortcut = "Z".into();
    p.loot.presses = 0;
    assert!(p.validate().is_err());
}

#[test]
fn target_reappearing_during_loot_pauses_without_pickup() {
    let mut c = controller();
    c.start(0.).unwrap();
    c.step(0., &observation(true));
    c.step(0.25, &observation(true));
    c.step(0.5, &observation(false));
    c.step(0.75, &observation(false));
    assert_eq!(c.phase, Phase::Looting);
    assert!(c.step(1., &observation(true)).is_none());
    assert!(!c.running);
    assert!(c.reason.contains("while looting"));
}
#[test]
fn invalid_observation_during_loot_never_sends_input() {
    let mut c = controller();
    c.start(0.).unwrap();
    c.step(0., &observation(true));
    c.step(0.25, &observation(true));
    c.step(0.5, &observation(false));
    c.step(0.75, &observation(false));
    assert!(c.step(1., &Observation::default()).is_none());
    assert!(!c.running);
}
#[test]
fn overdue_break_waits_for_loot_and_settling() {
    let mut c = controller();
    c.profile.safety.breaks_enabled = true;
    c.profile.safety.break_every_seconds = 60.;
    c.start(0.).unwrap();
    c.step(0., &observation(true));
    c.step(0.25, &observation(true));
    c.tick(61.);
    assert_eq!(c.phase, Phase::Active);
    c.step(61.25, &observation(false));
    c.step(61.5, &observation(false));
    for now in [61.75, 62.] {
        let a = c.step(now, &observation(false)).unwrap();
        c.ack(now, a.id, true).unwrap();
    }
    c.step(62.25, &observation(false));
    assert_eq!(c.phase, Phase::Settling);
    c.step(62.5, &observation(false));
    assert_ne!(c.phase, Phase::Break);
    c.step(63., &observation(false));
    c.step(63.5, &observation(false));
    assert_eq!(c.phase, Phase::Break);
}
#[test]
fn break_resumes_with_fresh_confirmation_and_preserves_buff_timers() {
    let mut c = controller();
    c.profile.safety.breaks_enabled = true;
    c.profile.safety.break_every_seconds = 60.;
    c.profile.safety.break_seconds = 30.;
    c.start(0.).unwrap();
    c.step(60., &observation(false));
    c.step(60.25, &observation(false));
    assert_eq!(c.phase, Phase::Break);
    c.tick(90.25);
    assert!(c.running);
    assert_eq!(c.phase, Phase::Initial);
    assert_eq!(c.schedule(90.25).break_remaining, Some(60.));
    c.tick(91.25);
    assert_eq!(c.schedule(91.25).break_remaining, Some(59.));
}
#[test]
fn clock_stop_applies_during_break_and_rearms_only_explicitly() {
    let mut c = controller();
    c.profile.safety.clock_enabled = true;
    assert_eq!(
        c.schedule(0.).clock_at_minute,
        Some(c.profile.safety.stop_at_minute)
    );
    c.profile.safety.breaks_enabled = true;
    c.profile.safety.break_every_seconds = 60.;
    c.profile.safety.break_seconds = 60.;
    assert!(c.start(0.).is_err());
    c.start_with_clock(0., Some(70.)).unwrap();
    c.step(60., &observation(false));
    c.step(60.25, &observation(false));
    c.tick(70.);
    assert!(!c.running);
    assert!(c.reason.contains("Automatic stop"));
    c.tick(130.);
    assert!(!c.running);
    c.start_with_clock(140., Some(86400.)).unwrap();
    assert_eq!(c.schedule(140.).clock_remaining, Some(86400.));
}
#[test]
fn history_is_bounded_and_does_not_repeat_changing_status_countdowns() {
    let mut c = controller();
    c.start(0.).unwrap();
    c.step(0., &observation(true));
    c.step(0.25, &observation(true));
    let count = c.events.len();
    for i in 1..50 {
        c.step(f64::from(i) * 0.25 + 0.25, &observation(true));
    }
    assert_eq!(c.events.len(), count);
    for i in 0..250 {
        c.pause(20., format!("Reason {i}"));
    }
    assert_eq!(c.events.len(), 200);
    assert!(c.events.back().unwrap().message.contains("Reason 249"));
    let remaining = c.schedule(20.);
    c.events.clear();
    assert_eq!(c.schedule(20.), remaining);
}
#[test]
fn stored_profiles_without_new_options_remain_valid_and_roundtrip() {
    let old = include_str!("../../../tests/parity/profiles/calibrated.json");
    let p = Profile::from_json(old).unwrap();
    assert!(!p.loot.enabled && !p.safety.clock_enabled);
    assert_eq!(
        Profile::from_json(&serde_json::to_string(&p).unwrap()).unwrap(),
        p
    );
    let mut value: serde_json::Value = serde_json::from_str(old).unwrap();
    value["safety"] = serde_json::json!({"break_seconds": -1.});
    assert!(Profile::from_json(&value.to_string()).is_err());
    value["safety"] = serde_json::json!({"hidden_magic": true});
    assert!(Profile::from_json(&value.to_string()).is_err());
}

#[test]
fn buff_history_uses_the_skill_name_and_shortcut() {
    let mut c = controller();
    c.profile.buffs.enabled = true;
    c.profile.buffs.fight_mounted = false;
    c.profile.buffs.skills.push(metin_core::BuffSkill {
        name: "Aura".into(),
        ..Default::default()
    });
    c.buffs = metin_core::BuffCycle::new(c.profile.buffs.clone());
    c.buffs.mounted = Some(false);
    c.start(0.).unwrap();
    c.step(0., &observation(false));
    c.step(0.25, &observation(false));
    let a = c.step(0.5, &observation(false)).unwrap();
    c.ack(0.6, a.id, true).unwrap();
    let event = c.events.back().unwrap();
    assert_eq!(event.activity, "Buff");
    assert_eq!(event.message, "Aura (F1) sent.");
}
