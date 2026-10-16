use metin_core::*;
use serde_json::Value;
fn check(c: &Controller, expected: &Value, now: f64, scenario: usize, event: usize) {
    let label = format!("scenario {scenario}, event {event}");
    assert_eq!(c.running, expected["running"].as_bool().unwrap(), "{label}");
    if let Some(phase) = expected["phase"].as_str() {
        assert_eq!(c.phase.as_str(), phase, "{label}");
    }
    assert_eq!(
        serde_json::json!(c.buffs.mounted),
        expected["mounted"],
        "{label}"
    );
    assert_eq!(
        c.session.completed,
        expected["stats"]["completed"].as_u64().unwrap(),
        "{label}"
    );
    assert_eq!(
        c.session.failed,
        expected["stats"]["failed"].as_u64().unwrap(),
        "{label}"
    );
    let timers = c.buffs.skill_remaining(now, !c.running);
    let reference = expected["timers"].as_array().unwrap();
    assert_eq!(timers.len(), reference.len(), "{label}");
    for (a, b) in timers.iter().zip(reference) {
        match (a, b.as_f64()) {
            (Some(a), Some(b)) => assert!((a - b).abs() < 1e-8, "{label}: timer {a} != {b}"),
            (None, None) => {}
            other => panic!("{label}: {other:?}"),
        }
    }
}
#[test]
fn all_original_controller_test_replays_match() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../tests/parity/traces/reference-controller-tests.json"
    ))
    .unwrap();
    for (scenario, source) in data["scenarios"].as_array().unwrap().iter().enumerate() {
        let profile = Profile::from_json(&source["profile"].to_string()).unwrap();
        let mut c = Controller::new(profile);
        let mut pending = None;
        for (event, row) in source["events"].as_array().unwrap().iter().enumerate() {
            let now = row["now"].as_f64().unwrap();
            match row["kind"].as_str().unwrap() {
                "start" => {
                    c.buffs.mounted = row["mounted"].as_bool();
                    let _ = c.start(now);
                }
                "pause" => c.pause(now, row["reason"].as_str().unwrap()),
                "reset_timer" => c
                    .buffs
                    .reset_timer(
                        row["index"].as_u64().unwrap() as usize,
                        now,
                        row["paused"].as_bool().unwrap(),
                    )
                    .unwrap(),
                "ack" => {
                    let id = pending.take().expect("pending buff action");
                    c.ack(now, id, true).unwrap();
                }
                "step" => {
                    c.buffs.mounted = row["mounted"].as_bool();
                    let o = &row["observation"];
                    let observation = Observation {
                        ready: o["ready"].as_bool().unwrap(),
                        candidates: serde_json::from_value(o["candidates"].clone()).unwrap(),
                        panel_present: o["panel_present"].as_bool().unwrap(),
                        reason: o["reason"].as_str().unwrap().into(),
                        buffs: o["buffs"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| match v.as_str().unwrap() {
                                "active" => Presence::Active,
                                "absent" => Presence::Absent,
                                _ => Presence::Unknown,
                            })
                            .collect(),
                    };
                    let action = c.step(now, &observation);
                    let encoded = action.as_ref().map(|a| match &a.kind {
                        ActionKind::Click { point } => {
                            serde_json::json!({"kind":"click","point":point,"shortcut":null})
                        }
                        ActionKind::Rotate => {
                            serde_json::json!({"kind":"rotate","point":null,"shortcut":null})
                        }
                        ActionKind::Mount { shortcut } => {
                            serde_json::json!({"kind":"mount","point":null,"shortcut":shortcut})
                        }
                        ActionKind::Skill { shortcut, .. } => {
                            serde_json::json!({"kind":"skill","point":null,"shortcut":shortcut})
                        }
                    });
                    assert_eq!(
                        encoded.unwrap_or(Value::Null),
                        row["action"],
                        "scenario {scenario}, event {event}"
                    );
                    if let Some(action) = action {
                        if matches!(action.kind, ActionKind::Click { .. } | ActionKind::Rotate) {
                            c.ack(now, action.id, true).unwrap();
                        } else {
                            pending = Some(action.id);
                        }
                    }
                }
                other => panic!("Unknown replay operation {other}"),
            }
            if !row["expected"].is_null() {
                check(&c, &row["expected"], now, scenario, event);
            }
        }
    }
}
