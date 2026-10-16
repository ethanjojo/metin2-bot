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

#[test]
fn preview_next_target_excludes_current_and_failed_candidates_without_changing_acquisition() {
    let mut p = Profile::from_json(include_str!(
        "../../../tests/parity/profiles/calibrated.json"
    ))
    .unwrap();
    p.buffs.enabled = false;
    p.panel.stable_frames = 1;
    let mut c = Controller::new(p);
    c.start(0.).unwrap();
    c.phase = metin_core::Phase::Seeking;
    let candidates = vec![
        Candidate {
            x: 10,
            y: 10,
            width: 20,
            height: 20,
            point: [20, 20],
        },
        Candidate {
            x: 100,
            y: 100,
            width: 20,
            height: 20,
            point: [110, 110],
        },
    ];
    let o = Observation {
        ready: true,
        candidates: candidates.clone(),
        ..Default::default()
    };
    assert_eq!(c.next_preview_target(&candidates), Some([20, 20]));
    let action = c.step(0.1, &o).unwrap();
    assert_eq!(action.kind, ActionKind::Click { point: [20, 20] });
    c.ack(0.1, action.id, true).unwrap();
    assert_eq!(c.selected_target(), Some([20, 20]));
    assert_eq!(c.next_preview_target(&candidates), Some([110, 110]));
    c.step(c.profile.timing.acquire_timeout + 0.2, &o);
    assert_eq!(c.next_preview_target(&candidates), Some([110, 110]));
    assert_eq!(c.next_candidate(&candidates).unwrap().point, [110, 110]);
    c.pause(20., "Test pause");
    assert!(c.selected_target().is_none());
    assert!(c.next_preview_target(&candidates).is_none());
}
