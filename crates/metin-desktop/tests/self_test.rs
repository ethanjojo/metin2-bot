#[test]
fn self_test_is_isolated_and_reports_components() {
    let dir = tempfile::tempdir().unwrap();
    let report = dir.path().join("report.json");
    metin_desktop::self_test::run(&report).unwrap();
    let data: serde_json::Value = serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    assert_eq!(data["success"], true);
    assert_eq!(data["game_input"], false);
    for name in ["vision", "storage", "light_theme", "dark_theme"] {
        assert_eq!(data["checks"][name], true, "{name}");
    }
}
