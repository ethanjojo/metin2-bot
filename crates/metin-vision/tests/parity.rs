use metin_core::Profile;
use metin_vision::{detect_candidates, load_image, region_template_score};

#[test]
fn golden_candidates_and_region_template_scores_match_reference() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/parity");
    let saved = std::fs::read_to_string(root.join("profiles/calibrated.json")).unwrap();
    let p = Profile::from_json(&saved).unwrap();
    // Preserve the historical cropped-image golden calls. Current HP matching
    // always searches the full client and is covered separately in panel_match.
    let historical: serde_json::Value = serde_json::from_str(&saved).unwrap();
    let region = serde_json::from_value(historical["panel"]["region"].clone()).unwrap();
    let expected: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("frames/expected.json")).unwrap())
            .unwrap();
    let template = load_image(&root.join("frames/template.png")).unwrap();
    for case in expected.as_array().unwrap() {
        let frame = load_image(&root.join("frames").join(case["frame"].as_str().unwrap())).unwrap();
        let found = detect_candidates(&frame, &p).unwrap();
        assert_eq!(serde_json::to_value(found).unwrap(), case["candidates"]);
        let score = region_template_score(&frame, &template, region).unwrap();
        assert!(
            (score - case["panel_score"].as_f64().unwrap()).abs() <= 1e-5,
            "score {score}"
        );
    }
}
