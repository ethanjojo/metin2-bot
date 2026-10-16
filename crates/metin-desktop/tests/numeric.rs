use metin_desktop::ui_controls::step_number;
#[test]
fn uses_domain_limits_and_step() {
    assert_eq!(step_number("interval_seconds", 86400., 1).unwrap(), 86400.);
    assert_eq!(step_number("sample_interval", 0.25, -1).unwrap(), 0.2);
    assert!(step_number("missing", 0., 1).is_err());
}
