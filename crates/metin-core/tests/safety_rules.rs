use base64::{Engine, engine::general_purpose::STANDARD};
use metin_core::*;
fn rule() -> SafetyRule {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend(5u32.to_be_bytes());
    bytes.extend(5u32.to_be_bytes());
    let mut r = SafetyRule::new("Disconnected");
    // Domain checks the PNG header; decoding and image content are covered by vision tests.
    r.reference = Some(BuffIcon {
        png: STANDARD.encode(bytes),
        region: Some(Region {
            left: 0.,
            top: 0.,
            right: 0.1,
            bottom: 0.1,
        }),
    });
    r.enabled = true;
    r.duration_seconds = 0.5;
    r
}
#[test]
fn duration_requires_fresh_consecutive_matches_and_latches_until_absent() {
    let r = rule();
    let mut monitor = SafetyMonitor::default();
    let rules = [r.clone()];
    assert!(monitor.update(&rules, &[(r.id, 1.)], 0., 1.).is_empty());
    assert!(monitor.update(&rules, &[(r.id, 1.)], 3., 1.).is_empty());
    assert_eq!(monitor.update(&rules, &[(r.id, 1.)], 3.5, 1.).len(), 1);
    assert!(monitor.update(&rules, &[(r.id, 1.)], 4., 1.).is_empty());
    monitor.update(&rules, &[(r.id, 0.)], 4.25, 1.);
    assert!(monitor.update(&rules, &[(r.id, 1.)], 4.5, 1.).is_empty());
    assert_eq!(monitor.update(&rules, &[(r.id, 1.)], 5., 1.).len(), 1);
}
#[test]
fn pause_rule_preempts_actions_while_notify_only_continues_and_records_destination() {
    for reaction in [
        RuleReactions {
            notify: true,
            pause: false,
            execute: false,
        },
        RuleReactions {
            notify: true,
            pause: true,
            execute: false,
        },
    ] {
        let mut r = rule();
        r.reaction = reaction;
        let mut p = Profile::create("Safety");
        p.safety.rules.push(r.clone());
        let mut c = Controller::new(p);
        c.start(0.).unwrap();
        let o = Observation {
            ready: true,
            panel_present: true,
            safety_scores: vec![(r.id, 1.)],
            ..Default::default()
        };
        c.step(0., &o);
        c.step(0.25, &o);
        c.step(0.5, &o);
        assert_eq!(
            c.running,
            reaction
                == RuleReactions {
                    notify: true,
                    pause: false,
                    execute: false
                }
        );
        let event = c
            .events
            .iter()
            .find(|e| e.activity == "Visual rule")
            .unwrap();
        assert!(event.notify);
        assert_eq!(event.subject, Some(r.id));
        assert_eq!(event.destination, EventDestination::Safety);
    }
}
#[test]
fn missing_monitoring_data_pauses_without_action() {
    let mut p = Profile::create("Safety");
    p.safety.rules.push(rule());
    let mut c = Controller::new(p);
    c.start(0.).unwrap();
    assert!(
        c.step(
            0.,
            &Observation {
                ready: true,
                ..Default::default()
            }
        )
        .is_none()
    );
    assert!(!c.running);
    assert!(c.events.back().unwrap().notify);
}
#[test]
fn recovery_is_bounded_and_never_counts_as_target_end_or_loot() {
    let mut p = Profile::create("Recovery");
    p.panel.stable_frames = 2;
    p.safety.recovery_enabled = true;
    p.safety.recovery_attempts = 1;
    p.safety.recovery_wait_seconds = 0.2;
    p.loot.enabled = true;
    p.timing.attack_timeout = 10.;
    let mut c = Controller::new(p);
    c.start(0.).unwrap();
    let active = Observation {
        ready: true,
        panel_present: true,
        ..Default::default()
    };
    c.step(0., &active);
    c.step(0.25, &active);
    let a = c.step(10.25, &active).unwrap();
    assert!(matches!(a.kind, ActionKind::Recover));
    c.ack(10.3, a.id, true).unwrap();
    c.step(10.5, &active);
    c.step(10.75, &active);
    c.step(11., &active);
    assert!(c.step(21., &active).is_none());
    assert!(!c.running);
    assert_eq!(c.recoveries, 1);
    assert_eq!(c.session.completed, 0);
}
#[test]
fn notification_preferences_do_not_restart_safety_budget_and_presets_infer_custom() {
    let a = SafetyConfig::default();
    let mut b = a.clone();
    b.sound = true;
    assert!(a.schedule_eq(&b));
    b.stop_after_seconds = 60.;
    assert!(!a.schedule_eq(&b));
    for preset in [
        TimingPreset::Careful,
        TimingPreset::Normal,
        TimingPreset::Responsive,
    ] {
        let mut t = preset.timing();
        t.validate().unwrap();
        assert_eq!(TimingPreset::infer(&t), Some(preset));
        t.attack_timeout += 1.;
        assert_eq!(TimingPreset::infer(&t), None);
    }
}
