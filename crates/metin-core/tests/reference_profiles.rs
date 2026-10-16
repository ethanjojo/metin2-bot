use metin_core::Profile;
#[test]
fn reference_schema_boundaries_and_normalization_match() {
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../tests/parity/profiles/schema-cases.json"
    ))
    .unwrap();
    for case in cases {
        let result = Profile::from_json(&case["input"].to_string());
        if case["expected"].is_null() {
            assert!(
                result.is_err(),
                "accepted reference rejection: {}",
                case["label"]
            );
        } else {
            let expected = Profile::from_json(&case["expected"].to_string()).unwrap();
            assert_eq!(
                result.unwrap_or_else(|e| panic!("{}: {e}", case["label"])),
                expected,
                "{}",
                case["label"]
            );
        }
    }
}
