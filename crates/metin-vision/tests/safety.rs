use base64::{Engine, engine::general_purpose::STANDARD};
use metin_core::{BuffIcon, Region, SafetyRule};
use metin_platform::Frame;
#[test]
fn safety_templates_match_only_the_calibrated_region_and_validate_real_pngs() {
    let frame = Frame::new(
        200,
        200,
        (0..120000)
            .map(|n| ((n * 17 + n / 113) % 251) as u8)
            .collect(),
    )
    .unwrap();
    let region = Region {
        left: 0.1,
        top: 0.1,
        right: 0.2,
        bottom: 0.2,
    };
    let crop = metin_vision::crop(&frame, region).unwrap();
    let mut rule = SafetyRule::new("Death");
    rule.enabled = true;
    rule.reference = Some(BuffIcon {
        png: STANDARD.encode(metin_vision::encode_png(&crop).unwrap()),
        region: Some(region),
    });
    let detector = metin_vision::SafetyDetector::new(&[rule.clone()]).unwrap();
    assert!(detector.observe(&frame).unwrap()[0].1 > 0.99);
    let blank = Frame::new(200, 200, vec![0; 120000]).unwrap();
    assert!(detector.observe(&blank).unwrap()[0].1 < rule.threshold);
    rule.reference.as_mut().unwrap().png = "bad".into();
    assert!(metin_vision::SafetyDetector::new(&[rule]).is_err());
}
