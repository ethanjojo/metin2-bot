use metin_core::{BuffConfig, Presence, Profile};
use metin_vision::{BuffDetector, Frame, crop, load_image, template_score};

fn fixture() -> (BuffConfig, Frame, Frame, Frame) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/parity/vision");
    let data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("expected.json")).unwrap())
            .unwrap();
    let case = &data["detectors"][5];
    let config = Profile::from_json(&case["profile"].to_string())
        .unwrap()
        .buffs;
    let frame = |i: usize| {
        load_image(
            &root
                .join("frames")
                .join(case["events"][i]["frame"].as_str().unwrap()),
        )
        .unwrap()
    };
    let active = frame(0);
    let inactive = frame(1);
    let icon =
        metin_vision::decode_png(&config.skills[0].icon.as_ref().unwrap().png_bytes().unwrap())
            .unwrap();
    let weak = (1..10)
        .find_map(|level| {
            let fraction = f64::from(level) / 10.;
            let bgr = active
                .bgr
                .iter()
                .enumerate()
                .map(|(i, &v)| {
                    (f64::from(v) * (1. - fraction) + ((i * 73 + 19) % 251) as f64 * fraction) as u8
                })
                .collect();
            let candidate = Frame::new(active.width, active.height, bgr).unwrap();
            let area = crop(&candidate, config.icon_region.unwrap()).unwrap();
            let score = template_score(&area, &icon, false).unwrap();
            (score > 0.66 && score < 0.89).then_some(candidate)
        })
        .expect("A genuinely ambiguous, non-flat frame");
    (config, active, inactive, weak)
}
fn detector(config: BuffConfig, frame: &Frame) -> BuffDetector {
    BuffDetector::new(config, [frame.width, frame.height], 0.25).unwrap()
}
#[test]
fn read_only_diagnostic_uses_actual_appearance_thresholds_without_extending_active_hold() {
    let (config, active, inactive, weak) = fixture();
    let diagnostic = metin_vision::inspect_buff(&active, &config, 0).unwrap();
    assert!(diagnostic.active_match);
    assert!(diagnostic.active_score.unwrap() >= metin_vision::BUFF_MATCH_THRESHOLD);
    let uncertain = metin_vision::inspect_buff(&weak, &config, 0).unwrap();
    assert!(!uncertain.active_match && !uncertain.inactive_match);
    assert!(uncertain.active_score.unwrap() < metin_vision::BUFF_MATCH_THRESHOLD);
    let inactive_check = metin_vision::inspect_buff(&inactive, &config, 0).unwrap();
    assert!(inactive_check.inactive_match);
    let mut detector = detector(config, &active);
    detector.observe(&active, 0.).unwrap();
    assert_eq!(detector.last_active_ages(0.7), vec![Some(0.7)]);
    detector.reset();
    assert_eq!(detector.last_active_ages(0.8), vec![None]);
}

#[test]
fn zero_disables_hold_and_custom_duration_is_per_skill() {
    let (mut config, active, _, weak) = fixture();
    config.skills[0].active_hold_seconds = 0.;
    let mut d = detector(config.clone(), &active);
    d.observe(&active, 0.).unwrap();
    assert_eq!(d.observe(&weak, 0.1).unwrap(), [Presence::Unknown]);
    config.skills[0].active_hold_seconds = 0.4;
    let mut second = config.skills[0].clone();
    second.active_hold_seconds = 2.;
    config.skills.push(second);
    let mut d = detector(config, &active);
    d.observe(&active, 0.).unwrap();
    d.observe(&weak, 0.25).unwrap();
    assert_eq!(
        d.observe(&weak, 0.5).unwrap(),
        [Presence::Unknown, Presence::Active]
    );
}

#[test]
fn recent_active_survives_ambiguous_animation_but_expires_without_renewal() {
    let (config, active, _, weak) = fixture();
    let mut d = detector(config, &active);
    assert_eq!(d.observe(&active, 0.).unwrap(), [Presence::Active]);
    for time in [0.25, 0.5, 0.75, 1., 1.25, 1.49] {
        assert_eq!(
            d.observe(&weak, time).unwrap(),
            [Presence::Active],
            "at {time}"
        );
    }
    assert_eq!(d.observe(&weak, 1.5).unwrap(), [Presence::Unknown]);
}

#[test]
fn only_a_real_match_renews_the_hold_and_confirmed_inactive_wins_earlier() {
    let (config, active, inactive, weak) = fixture();
    let mut d = detector(config, &active);
    d.observe(&active, 0.).unwrap();
    d.observe(&weak, 0.5).unwrap();
    d.observe(&active, 0.9).unwrap();
    for time in [1.4, 1.9, 2.3] {
        assert_eq!(d.observe(&weak, time).unwrap(), [Presence::Active]);
    }
    assert_eq!(d.observe(&inactive, 2.35).unwrap(), [Presence::Active]);
    assert_eq!(d.observe(&inactive, 2.5).unwrap(), [Presence::Unknown]);
    assert_eq!(d.observe(&inactive, 2.6).unwrap(), [Presence::Absent]);
    // Inactive must retire the previous active evidence, even within its hold.
    d.observe(&active, 3.).unwrap();
    d.observe(&inactive, 3.1).unwrap();
    d.observe(&inactive, 3.2).unwrap();
    assert_eq!(d.observe(&inactive, 3.3).unwrap(), [Presence::Absent]);
    assert_eq!(d.observe(&weak, 3.4).unwrap(), [Presence::Unknown]);
}

#[test]
fn missing_active_without_an_inactive_reference_waits_for_hold_expiry() {
    let (mut config, active, _, _) = fixture();
    let inactive = Frame::new(
        active.width,
        active.height,
        (0..active.bgr.len())
            .map(|i| ((i * 73 + 19) % 251) as u8)
            .collect(),
    )
    .unwrap();
    config.skills[0].inactive_icon = None;
    let mut d = detector(config, &active);
    d.observe(&active, 0.).unwrap();
    for time in [0.25, 0.5, 0.75, 1., 1.25] {
        assert_eq!(d.observe(&inactive, time).unwrap(), [Presence::Active]);
    }
    assert_eq!(d.observe(&inactive, 1.5).unwrap(), [Presence::Absent]);
}

#[test]
fn reset_gap_nonmonotonic_time_and_invalid_pixels_retire_held_active() {
    let (config, active, _, weak) = fixture();
    let mut d = detector(config, &active);
    for case in 0..5 {
        d.reset();
        d.observe(&active, 10.).unwrap();
        match case {
            0 => d.reset(),
            1 => {
                assert_eq!(d.observe(&weak, 11.1).unwrap(), [Presence::Unknown]);
            }
            2 => {
                assert_eq!(d.observe(&weak, 10.).unwrap(), [Presence::Unknown]);
            }
            3 => {
                let flat =
                    Frame::new(active.width, active.height, vec![0; active.bgr.len()]).unwrap();
                assert_eq!(d.observe(&flat, 10.1).unwrap(), [Presence::Unknown]);
            }
            _ => {
                let other = Frame::new(6, 6, vec![70; 108]).unwrap();
                assert_eq!(d.observe(&other, 10.1).unwrap(), [Presence::Unknown]);
            }
        }
        assert_eq!(
            d.observe(&weak, if case == 1 { 11.2 } else { 10.2 })
                .unwrap(),
            [Presence::Unknown],
            "case {case}"
        );
    }
}
