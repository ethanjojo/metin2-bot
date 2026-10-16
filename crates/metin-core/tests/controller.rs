use metin_core::{ActionKind, Candidate, Controller, Observation, Profile};

#[test]
fn target_cycle_trace_matches_python() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/parity");
    let p = Profile::from_json(
        &std::fs::read_to_string(root.join("profiles/calibrated.json")).unwrap(),
    )
    .unwrap();
    let trace: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("traces/target-cycle.json")).unwrap(),
    )
    .unwrap();
    let candidates: Vec<Candidate> = serde_json::from_value(
        serde_json::from_str::<serde_json::Value>(
            &std::fs::read_to_string(root.join("frames/expected.json")).unwrap(),
        )
        .unwrap()[0]["candidates"]
            .clone(),
    )
    .unwrap();
    let mut c = Controller::new(p);
    c.start(0.).unwrap();
    for row in trace.as_array().unwrap() {
        let action = c.step(
            row["now"].as_f64().unwrap(),
            &Observation {
                ready: true,
                candidates: candidates.clone(),
                panel_present: row["panel"].as_bool().unwrap(),
                reason: String::new(),
                buffs: vec![],
            },
        );
        assert_eq!(c.phase.as_str(), row["phase"].as_str().unwrap());
        assert_eq!(c.running, row["running"].as_bool().unwrap());
        assert_eq!(
            c.session.completed,
            row["stats"]["completed"].as_u64().unwrap()
        );
        match (action, row["action"].is_null()) {
            (None, true) => {}
            (Some(action), false) => {
                assert!(matches!(action.kind, ActionKind::Click { .. }));
                if let ActionKind::Click { point } = action.kind {
                    assert_eq!(serde_json::json!(point), row["action"]["point"]);
                }
                c.ack(row["now"].as_f64().unwrap(), action.id, true)
                    .unwrap();
            }
            other => panic!("action mismatch {other:?}"),
        }
    }
}

#[test]
fn stale_ack_cannot_advance_pending_action() {
    let mut p = Profile::create("test");
    p.buffs.enabled = true;
    p.buffs.skills.push(Default::default());
    let mut c = Controller::new(p);
    c.buffs.mounted = Some(false);
    c.start(0.).unwrap();
    let o = Observation {
        ready: true,
        candidates: vec![],
        panel_present: false,
        reason: String::new(),
        buffs: vec![],
    };
    c.step(0., &o);
    c.step(0.25, &o);
    let a = c.step(0.5, &o).unwrap();
    assert!(c.ack(0.5, a.id + 1, true).is_err());
    assert!(c.buffs.pending.is_some());
    c.ack(0.5, a.id, true).unwrap();
    assert!(c.buffs.pending.is_none());
}
