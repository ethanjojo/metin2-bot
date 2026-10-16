use metin_desktop::ui_controls::{CapturedKey, capture_shortcut};
#[test]
fn capture_preserves_digit_and_rejects_reserved_keys() {
    assert_eq!(
        capture_shortcut(CapturedKey::Digit(1), true, true, false).unwrap(),
        "Ctrl+Shift+1"
    );
    assert!(capture_shortcut(CapturedKey::Function(7), false, false, false).is_err());
}
