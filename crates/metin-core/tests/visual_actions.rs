use metin_core::*;
#[test]
fn periodic_old_data_migrates_and_new_actions_wait_first_interval() {
    let id = uuid::Uuid::new_v4();
    let old = serde_json::json!({"id":id,"name":"Item","enabled":true,"timing":{"shortcut":"F5","interval_seconds":60.,"wait_seconds":2.,"hold_seconds":0.2}});
    let a: PeriodicAction = serde_json::from_value(old).unwrap();
    assert_eq!(a.first_run, FirstRun::Immediate);
    assert_eq!(a.input.shortcut(), Some("F5"));
    let saved = serde_json::to_value(&a).unwrap();
    assert!(saved.get("timing").is_none());
    assert_eq!(saved["interval_seconds"], 60.);
    let mut p = Profile::create("New");
    let mut a = PeriodicAction::new("Delayed");
    a.enabled = true;
    a.interval_seconds = 10.;
    p.periodic_actions.push(a);
    let mut scheduler = ActionScheduler::default();
    scheduler.initialize(&p, 0.);
    assert!(scheduler.next(&p, 9.).is_none());
    assert!(scheduler.next(&p, 10.).is_some());
}
#[test]
fn click_configuration_requires_calibration_and_valid_region() {
    let mut input = ActionInput::Click { region: None };
    assert!(input.validate(true, Some([800, 600])).is_err());
    input = ActionInput::Click {
        region: Some(Region {
            left: 0.2,
            top: 0.3,
            right: 0.4,
            bottom: 0.5,
        }),
    };
    assert!(input.validate(true, None).is_err());
    assert!(input.validate(true, Some([800, 600])).is_ok());
}
use base64::{Engine, engine::general_purpose::STANDARD};
fn visual_rule() -> SafetyRule {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend(5u32.to_be_bytes());
    bytes.extend(5u32.to_be_bytes());
    let mut r = SafetyRule::new("Respawn");
    r.enabled = true;
    r.duration_seconds = 0.5;
    r.reference = Some(BuffIcon {
        png: STANDARD.encode(bytes),
        region: Some(Region {
            left: 0.,
            top: 0.,
            right: 0.1,
            bottom: 0.1,
        }),
    });
    r.reaction = RuleReactions {
        notify: true,
        pause: false,
        execute: true,
    };
    r.execution.wait_seconds = 0.25;
    r.execution.timeout_seconds = 0.5;
    r.execution.after_success = AfterSuccess::Continue;
    r
}
fn obs(r: &SafetyRule, score: f64) -> Observation {
    Observation {
        ready: true,
        safety_scores: vec![(r.id, score)],
        ..Default::default()
    }
}
fn controller(r: &SafetyRule) -> Controller {
    let mut p = Profile::create("Visual");
    p.panel.stable_frames = 2;
    p.timing.sample_interval = 0.1;
    p.safety.rules.push(r.clone());
    let mut c = Controller::new(p);
    c.start(-0.5).unwrap();
    c.step(-0.5, &obs(r, 1.));
    c.step(-0.25, &obs(r, 1.));
    c
}
fn send(c: &mut Controller, r: &SafetyRule) {
    let release = c.step(0., &obs(r, 1.)).unwrap();
    assert_eq!(release.kind, ActionKind::ReleaseAttack);
    c.ack(0., release.id, true).unwrap();
    let a = c.step(0.25, &obs(r, 1.)).unwrap();
    assert!(matches!(a.kind, ActionKind::Configured { .. }));
    c.ack(0.25, a.id, true).unwrap();
}
#[test]
fn verified_visual_action_releases_then_sends_once_and_rechecks_fresh_state() {
    let r = visual_rule();
    let mut c = controller(&r);
    send(&mut c, &r);
    assert!(c.step(0.5, &obs(&r, 0.)).is_none());
    assert_eq!(c.phase, Phase::VisualRule);
    assert!(c.step(0.75, &obs(&r, 0.)).is_none());
    assert_eq!(c.phase, Phase::Initial);
    assert!(c.running);
    assert!(
        c.events
            .iter()
            .any(|e| e.message.contains("disappearance confirmed"))
    );
    assert_eq!(c.session.completed, 0);
}
#[test]
fn visual_failure_retries_only_to_limit_then_pauses_with_rule_link() {
    let r = visual_rule();
    let mut c = controller(&r);
    send(&mut c, &r);
    let retry = c.step(1., &obs(&r, 1.)).unwrap();
    assert!(matches!(retry.kind, ActionKind::Configured { .. }));
    c.ack(1., retry.id, true).unwrap();
    assert!(c.step(1.75, &obs(&r, 1.)).is_none());
    assert!(!c.running);
    let event = c.events.back().unwrap();
    assert_eq!(event.subject, Some(r.id));
    assert!(event.notify);
}
#[test]
fn disappearing_event_before_input_never_clicks_old_target() {
    let r = visual_rule();
    let mut c = controller(&r);
    let release = c.step(0., &obs(&r, 1.)).unwrap();
    c.ack(0., release.id, true).unwrap();
    assert!(c.step(0.25, &obs(&r, 0.)).is_none());
    assert!(c.step(0.5, &obs(&r, 0.)).is_none());
    assert_eq!(c.phase, Phase::Initial);
    assert!(c.running);
}
#[test]
fn missing_or_stale_confirmation_does_not_succeed() {
    let r = visual_rule();
    let mut c = controller(&r);
    send(&mut c, &r);
    c.step(0.5, &obs(&r, 0.));
    c.step(3., &obs(&r, 0.));
    assert_eq!(c.phase, Phase::VisualRule);
    assert!(!c.running);
    assert!(
        !c.events
            .iter()
            .any(|e| e.message.contains("disappearance confirmed"))
    );
    let mut c = controller(&r);
    send(&mut c, &r);
    c.step(
        0.5,
        &Observation {
            ready: true,
            ..Default::default()
        },
    );
    assert!(!c.running);
}
#[test]
fn delay_is_explicitly_unverified_and_pause_after_success_is_respected() {
    let mut r = visual_rule();
    r.execution.verification = Verification::Delay;
    r.execution.after_success = AfterSuccess::Pause;
    let mut c = controller(&r);
    send(&mut c, &r);
    c.step(0.5, &obs(&r, 1.));
    assert!(!c.running);
    assert!(
        c.events
            .iter()
            .any(|e| e.message.contains("outcome not verified"))
    );
}
#[test]
fn pause_rule_preempts_visual_input_and_simultaneous_actions_fail_closed() {
    for reaction in [
        RuleReactions {
            notify: true,
            pause: false,
            execute: true,
        },
        RuleReactions {
            notify: true,
            pause: true,
            execute: false,
        },
    ] {
        let r = visual_rule();
        let mut other = visual_rule();
        other.reaction = reaction;
        let mut p = Profile::create("Conflict");
        p.timing.sample_interval = 0.05;
        p.safety.rules = vec![r.clone(), other.clone()];
        let mut c = Controller::new(p);
        c.start(-0.5).unwrap();
        let both = Observation {
            ready: true,
            safety_scores: vec![(r.id, 1.), (other.id, 1.)],
            ..Default::default()
        };
        c.step(-0.5, &both);
        c.step(-0.25, &both);
        assert!(
            c.step(
                0.,
                &Observation {
                    ready: true,
                    safety_scores: vec![(r.id, 1.), (other.id, 1.)],
                    ..Default::default()
                }
            )
            .is_none()
        );
        assert!(!c.running);
    }
}
#[test]
fn pause_cancels_visual_execution_and_rejects_late_ack() {
    let r = visual_rule();
    let mut c = controller(&r);
    let release = c.step(0., &obs(&r, 1.)).unwrap();
    c.pause(0.1, "F8");
    assert!(c.ack(0.2, release.id, true).is_err());
    assert!(c.step(1., &obs(&r, 1.)).is_none());
}
#[test]
fn repeat_event_requires_disappearance_and_new_appearance() {
    let mut r = visual_rule();
    r.execution.verification = Verification::Delay;
    let mut c = controller(&r);
    send(&mut c, &r);
    c.step(0.5, &obs(&r, 1.));
    assert_eq!(c.phase, Phase::Initial);
    assert!(!matches!(
        c.step(0.75, &obs(&r, 1.)).map(|a| a.kind),
        Some(ActionKind::Configured { .. } | ActionKind::ReleaseAttack)
    ));
    c.step(1., &obs(&r, 0.));
    c.step(1.25, &obs(&r, 1.));
    c.step(1.5, &obs(&r, 1.));
    let a = c.step(1.75, &obs(&r, 1.)).unwrap();
    assert_eq!(a.kind, ActionKind::ReleaseAttack);
}
#[test]
fn canonical_periodic_schema_rejects_mixed_formats() {
    let a = PeriodicAction::new("Item");
    let mut v = serde_json::to_value(a).unwrap();
    v["timing"] = serde_json::to_value(TimedShortcut::default()).unwrap();
    assert!(serde_json::from_value::<PeriodicAction>(v).is_err());
}

#[test]
fn impossible_reference_confirmation_timeout_is_rejected_before_start() {
    let r = visual_rule();
    let mut p = Profile::create("Slow capture");
    p.timing.sample_interval = 2.;
    p.safety.rules.push(r);
    assert!(
        Controller::new(p.clone())
            .start(0.)
            .unwrap_err()
            .to_string()
            .contains("verification timeout")
    );
    p.safety.rules[0].execution.timeout_seconds = 15.;
    assert!(Controller::new(p).start(0.).is_ok());
}
#[test]
fn visual_reaction_flags_preserve_independent_notifications_and_reject_conflicts() {
    for notify in [false, true] {
        for (pause, execute) in [(true, false), (false, true), (false, false)] {
            let mut r = visual_rule();
            r.reaction = RuleReactions {
                notify,
                pause,
                execute,
            };
            assert_eq!(r.validate().is_ok(), notify || pause || execute);
            if !notify && !pause && !execute {
                continue;
            }
            let mut c = controller(&r);
            let action = c.step(0., &obs(&r, 1.));
            assert_eq!(c.running, !pause);
            assert_eq!(action.is_some(), execute);
            let event = c
                .events
                .iter()
                .find(|e| e.activity == "Visual rule")
                .unwrap();
            assert_eq!(event.notify, notify);
            assert_eq!(event.subject, Some(r.id));
        }
    }
    let mut r = visual_rule();
    r.reaction = RuleReactions {
        notify: false,
        pause: true,
        execute: true,
    };
    assert!(r.validate().is_err());
}
#[test]
fn old_visual_reactions_migrate_once_and_serialize_explicit_flags() {
    for (stored, expected) in [
        (
            "pause_and_notify",
            RuleReactions {
                notify: true,
                pause: true,
                execute: false,
            },
        ),
        (
            "notify",
            RuleReactions {
                notify: true,
                pause: false,
                execute: false,
            },
        ),
        (
            "execute",
            RuleReactions {
                notify: true,
                pause: false,
                execute: true,
            },
        ),
    ] {
        let flags: RuleReactions = serde_json::from_value(serde_json::json!(stored)).unwrap();
        assert_eq!(flags, expected);
        assert!(serde_json::to_value(flags).unwrap().is_object());
    }
    assert!(
        serde_json::from_value::<RuleReactions>(
            serde_json::json!({"notify":true,"pause":false,"execute":false,"typo":true})
        )
        .is_err()
    );
}
#[test]
fn action_failure_and_pause_after_success_obey_notification_choice() {
    for notify in [false, true] {
        let mut r = visual_rule();
        r.reaction = RuleReactions {
            notify,
            pause: false,
            execute: true,
        };
        let mut c = controller(&r);
        send(&mut c, &r);
        let retry = c.step(1., &obs(&r, 1.)).unwrap();
        c.ack(1., retry.id, true).unwrap();
        c.step(1.75, &obs(&r, 1.));
        assert!(!c.running);
        assert_eq!(c.events.iter().any(|e| e.notify), notify);
        let mut c = controller(&r);
        let release = c.step(0., &obs(&r, 1.)).unwrap();
        c.ack(0., release.id, true).unwrap();
        let a = c.step(0.25, &obs(&r, 1.)).unwrap();
        c.ack(0.25, a.id, false).unwrap();
        assert!(!c.running);
        assert_eq!(c.events.iter().any(|e| e.notify), notify);
        r.execution.after_success = AfterSuccess::Pause;
        let mut c = controller(&r);
        send(&mut c, &r);
        c.step(0.5, &obs(&r, 0.));
        c.step(0.75, &obs(&r, 0.));
        assert!(!c.running);
        assert_eq!(c.events.iter().any(|e| e.notify), notify);
    }
}
#[test]
fn visual_capture_and_focus_errors_respect_notification_flag() {
    for notify in [false, true] {
        let mut r = visual_rule();
        r.reaction.notify = notify;
        let mut c = controller(&r);
        send(&mut c, &r);
        c.step(
            0.5,
            &Observation {
                ready: true,
                ..Default::default()
            },
        );
        assert!(!c.running);
        assert_eq!(c.events.iter().any(|e| e.notify), notify);
        let mut c = controller(&r);
        send(&mut c, &r);
        c.pause_on_runtime_error(0.5, "Game lost focus");
        assert!(!c.running);
        assert_eq!(c.events.iter().any(|e| e.notify), notify);
        assert_eq!(c.events.back().unwrap().subject, Some(r.id));
    }
}
#[test]
fn scheduled_stop_keeps_its_own_alert_during_a_silent_visual_action() {
    let mut r = visual_rule();
    r.reaction.notify = false;
    let mut c = controller(&r);
    send(&mut c, &r);
    c.profile.safety.stop_enabled = true;
    c.profile.safety.stop_after_seconds = 60.;
    c.tick(61.);
    assert!(!c.running);
    let event = c.events.back().unwrap();
    assert!(event.notify);
    assert_eq!(event.destination, EventDestination::Safety);
    assert_eq!(event.subject, None);
}
