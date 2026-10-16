use metin_platform::Frame;
use metin_vision::validate_template;

#[test]
fn five_pixel_crop_is_accepted_when_distinctive_and_errors_explain_the_real_reason() {
    let pixels = (0..25)
        .flat_map(|n| [if n % 2 == 0 { 0 } else { 255 }; 3])
        .collect();
    assert!(validate_template(&Frame::new(5, 5, pixels).unwrap()).is_ok());
    let flat = validate_template(&Frame::new(5, 5, vec![80; 75]).unwrap())
        .unwrap_err()
        .to_string();
    assert!(flat.contains("contrast"), "{flat}");
    assert!(
        !flat.contains("at least"),
        "Valid size must not be reported as too small: {flat}"
    );
    let small = validate_template(&Frame::new(4, 5, vec![80; 60]).unwrap())
        .unwrap_err()
        .to_string();
    assert!(small.contains("4 × 5"), "{small}");
    assert!(small.contains("at least 5 × 5"), "{small}");
}
