use metin_platform::{HidKey, HidKeyboard, scan_to_hid};

#[test]
fn physical_scan_mapping_preserves_layout_and_extended_modifiers() {
    for (scan, usage) in [
        (0x22, 0x0a),
        (0x39, 0x2c),
        (0x01, 0x29),
        (0x15, 0x1c),
        (0x2c, 0x1d),
        (0x02, 0x1e),
        (0x0b, 0x27),
        (0x3b, 0x3a),
        (0x58, 0x45),
    ] {
        assert_eq!(scan_to_hid(scan).unwrap(), HidKey::Usage(usage));
    }
    for (scan, bit) in [
        (0x1d, 1),
        (0x2a, 2),
        (0x38, 4),
        (0xe01d, 16),
        (0x36, 32),
        (0xe038, 64),
    ] {
        assert_eq!(scan_to_hid(scan).unwrap(), HidKey::Modifier(bit));
    }
    assert!(scan_to_hid(0).is_err());
    assert!(scan_to_hid(0xffff).is_err());
}

#[test]
fn complete_reports_hold_shortcuts_until_their_individual_release() {
    let mut report = HidKeyboard::default();
    report.set(HidKey::Modifier(1), true).unwrap();
    report.set(HidKey::Modifier(4), true).unwrap();
    report.set(HidKey::Usage(0x0a), true).unwrap();
    report.set(HidKey::Usage(0x0a), true).unwrap();
    assert_eq!(report.keys, [0x0a]);
    assert_eq!(report.modifiers, 5);
    report.set(HidKey::Modifier(1), false).unwrap();
    assert_eq!(report.modifiers, 4);
    assert_eq!(report.keys, [0x0a]);
    report.set(HidKey::Usage(0x0a), false).unwrap();
    report.set(HidKey::Modifier(4), false).unwrap();
    assert_eq!(report, HidKeyboard::default());
    // Retrying a release after a delivery failure must still send a neutral report.
    report.set(HidKey::Usage(0x0a), false).unwrap();
    assert_eq!(report, HidKeyboard::default());
}

#[test]
fn rollover_and_invalid_reports_are_rejected_without_losing_held_keys() {
    let mut report = HidKeyboard::default();
    for key in 4..10 {
        report.set(HidKey::Usage(key), true).unwrap();
    }
    let before = report.clone();
    assert!(report.set(HidKey::Usage(10), true).is_err());
    assert!(report.set(HidKey::Usage(0), true).is_err());
    assert!(report.set(HidKey::Modifier(3), true).is_err());
    assert_eq!(report, before);
}
