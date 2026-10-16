use metin_core::{Profile, Region, Shortcut};

#[test]
fn default_process_is_generic_and_explicit_process_names_roundtrip() {
    let mut profile = Profile::create("Main profile");
    assert_eq!(profile.process_name, "game-client.exe");
    profile.process_name = "custom-client.exe".into();
    let encoded = serde_json::to_string(&profile).unwrap();
    assert_eq!(
        Profile::from_json(&encoded).unwrap().process_name,
        "custom-client.exe"
    );
}

#[test]
fn example_roundtrip_preserves_every_field() {
    let json = include_str!("../../../profile.example.json");
    let profile = Profile::from_json(json).unwrap();
    let output = serde_json::to_string(&profile).unwrap();
    assert_eq!(profile, Profile::from_json(&output).unwrap());
    assert_eq!(profile.name, "Main profile");
    assert_eq!(
        profile.id.to_string(),
        "8591a09f-25dc-4ca4-b858-cafe68231dfc"
    );
    assert_eq!(profile.timing.attack_timeout, 120.0);
    let input: serde_json::Value = serde_json::from_str(json).unwrap();
    let encoded: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        input.as_object().unwrap().keys().collect::<Vec<_>>(),
        encoded.as_object().unwrap().keys().collect::<Vec<_>>()
    );
    assert!(!profile.ready());
}

#[test]
fn rejects_unknown_fields_wrong_types_and_invalid_ranges() {
    let original: serde_json::Value =
        serde_json::from_str(include_str!("../../../profile.example.json")).unwrap();
    for (field, value) in [
        ("unexpected", serde_json::json!(1)),
        ("masks", serde_json::json!(false)),
    ] {
        let mut data = original.clone();
        data[field] = value;
        assert!(Profile::from_json(&data.to_string()).is_err());
    }
    for value in [
        serde_json::json!(0),
        serde_json::json!(true),
        serde_json::json!(3),
    ] {
        let mut data = original.clone();
        data["timing"]["sample_interval"] = value;
        assert!(Profile::from_json(&data.to_string()).is_err());
    }
}

#[test]
fn half_even_pixel_conversion_matches_reference() {
    let r = Region {
        left: 0.125,
        top: 0.375,
        right: 0.625,
        bottom: 0.875,
    };
    assert_eq!(r.pixels(20, 20), [2, 8, 12, 18]);
    assert!(
        Region {
            left: 0.5,
            right: 0.4,
            ..r
        }
        .validate()
        .is_err()
    );
}

#[test]
fn validates_shortcuts_and_canonical_modifier_order() {
    assert_eq!(
        Shortcut::parse("shift+ctrl+1").unwrap().to_string(),
        "Ctrl+Shift+1"
    );
    for bad in ["F7", "Alt+F8", "Ctrl+Ctrl+G", "Ctrl", "Ctrl++G", "Super+G"] {
        assert!(Shortcut::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn legacy_intervals_normalize_only_refresh_skills() {
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("../../../profile.example.json")).unwrap();
    value["buffs"]["interval_seconds"] = serde_json::json!(14400);
    value["buffs"]["initial_mounted"] = serde_json::json!(true);
    value["buffs"]["skills"] = serde_json::json!([
        {"shortcut":"F1"}, {"shortcut":"F2","mode":"inactive"}
    ]);
    let p = Profile::from_json(&value.to_string()).unwrap();
    assert_eq!(p.buffs.skills[0].interval_seconds, Some(14400.0));
    assert_eq!(p.buffs.skills[1].interval_seconds, None);
    assert!(
        !serde_json::to_string(&p)
            .unwrap()
            .contains("initial_mounted")
    );
}

#[test]
fn required_nested_fields_and_control_characters_are_not_defaulted_away() {
    let original: serde_json::Value =
        serde_json::from_str(include_str!("../../../profile.example.json")).unwrap();
    let mut missing_color = original.clone();
    missing_color["color"] = serde_json::json!({});
    assert!(Profile::from_json(&missing_color.to_string()).is_err());
    let mut missing_key = original.clone();
    missing_key["buffs"]["skills"] = serde_json::json!([{}]);
    assert!(Profile::from_json(&missing_key.to_string()).is_err());
    let mut bad_name = original;
    bad_name["buffs"]["skills"] = serde_json::json!([{"shortcut":"F1","name":"\nName\n"}]);
    assert!(Profile::from_json(&bad_name.to_string()).is_err());
}
