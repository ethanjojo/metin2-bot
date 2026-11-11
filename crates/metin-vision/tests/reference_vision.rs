use metin_core::{Candidate, Profile, Region};
use metin_vision::*;
use serde_json::Value;
#[test]
fn all_reference_image_calls_and_buff_observations_match() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/parity/vision");
    let data: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("expected.json")).unwrap())
            .unwrap();
    let image = |v: &Value| load_image(&root.join("frames").join(v.as_str().unwrap())).unwrap();
    for (i, call) in data["calls"].as_array().unwrap().iter().enumerate() {
        let frame = image(&call["frame"]);
        match call["kind"].as_str().unwrap() {
            "detect" => {
                let profile = Profile::from_json(&call["profile"].to_string()).unwrap();
                assert_eq!(
                    serde_json::to_value(detect_candidates(&frame, &profile).unwrap()).unwrap(),
                    call["expected"],
                    "call {i}"
                );
            }
            "panel" => {
                let region: Region = serde_json::from_value(call["region"].clone()).unwrap();
                let actual =
                    region_template_score(&frame, &image(&call["template"]), region).unwrap();
                assert!(
                    (actual - call["expected"].as_f64().unwrap()).abs() <= 1e-5,
                    "call {i}: {actual}"
                );
            }
            "color" => {
                let point: [u32; 2] = serde_json::from_value(call["point"].clone()).unwrap();
                assert_eq!(
                    serde_json::to_value(pick_color(&frame, point).unwrap()).unwrap(),
                    call["expected"],
                    "call {i}"
                );
            }
            other => panic!("Unknown call {other}"),
        }
    }
    for (i, detector) in data["detectors"].as_array().unwrap().iter().enumerate() {
        let p = Profile::from_json(&detector["profile"].to_string()).unwrap();
        let size: [u32; 2] = serde_json::from_value(detector["size"].clone()).unwrap();
        let mut d =
            BuffDetector::new(p.buffs, size, detector["interval"].as_f64().unwrap()).unwrap();
        for (j, event) in detector["events"].as_array().unwrap().iter().enumerate() {
            let states = d
                .observe(&image(&event["frame"]), event["now"].as_f64().unwrap())
                .unwrap();
            let actual: Vec<_> = states
                .into_iter()
                .map(|s| match s {
                    metin_core::Presence::Active => "active",
                    metin_core::Presence::Absent => "absent",
                    metin_core::Presence::Unknown => "unknown",
                })
                .collect();
            assert_eq!(
                serde_json::json!(actual),
                event["expected"],
                "detector {i}, event {j}"
            );
        }
    }
    for tracker in data["trackers"].as_array().unwrap() {
        let mut t = CandidateTracker::new(
            tracker["required"].as_u64().unwrap() as u32,
            tracker["interval"].as_f64().unwrap(),
        );
        for e in tracker["events"].as_array().unwrap() {
            let candidates: Vec<Candidate> =
                serde_json::from_value(e["candidates"].clone()).unwrap();
            assert_eq!(
                serde_json::json!(t.update(candidates, e["now"].as_f64().unwrap())),
                e["expected"]
            );
        }
    }
}
