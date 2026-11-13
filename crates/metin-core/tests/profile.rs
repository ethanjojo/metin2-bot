use metin_core::{Profile, Region, Shortcut};

#[test]
fn preset_provenance_is_optional_and_never_replaces_self_contained_color() {
    let old = serde_json::to_value(Profile::create("Existing profile")).unwrap();
    assert!(old.get("color_preset_id").is_none());
    let mut saved = old;
    saved["color"] = serde_json::to_value(metin_core::ColorConfig::default()).unwrap();
    saved["color"]["hsv"] = serde_json::json!([90, 210, 180]);
    saved["color_preset_id"] = serde_json::json!("8591a09f-25dc-4ca4-b858-cafe68231dfc");
    let loaded = Profile::from_json(&saved.to_string()).unwrap();
    let encoded = serde_json::to_value(&loaded).unwrap();
    assert_eq!(encoded["color"], saved["color"]);
    assert_eq!(encoded["color_preset_id"], saved["color_preset_id"]);
    assert!(loaded.validate_settings().is_ok());
}

#[test]
fn legacy_exclusions_gain_names_without_changing_geometry() {
    let mut saved = serde_json::to_value(Profile::create("Existing profile")).unwrap();
    let regions = serde_json::json!([
        {"left":0.1,"top":0.2,"right":0.3,"bottom":0.4},
        {"left":0.5,"top":0.6,"right":0.7,"bottom":0.8}
    ]);
    saved["masks"] = regions.clone();
    let migrated = Profile::from_json(&saved.to_string()).unwrap();
    let canonical = serde_json::to_value(&migrated).unwrap();
    assert_eq!(canonical["masks"][0]["name"], "Excluded area 1");
    assert_eq!(canonical["masks"][1]["name"], "Excluded area 2");
    assert_eq!(canonical["masks"][0]["region"], regions[0]);
    assert_eq!(canonical["masks"][1]["region"], regions[1]);
    assert_eq!(
        Profile::from_json(&canonical.to_string()).unwrap(),
        migrated
    );
}

#[test]
fn named_exclusions_reject_blank_control_and_oversized_names() {
    let mut saved = serde_json::to_value(Profile::create("Named exclusions")).unwrap();
    for name in [
        "".to_owned(),
        " ".to_owned(),
        "Chat\nwindow".to_owned(),
        "a".repeat(81),
    ] {
        saved["masks"] = serde_json::json!([{"name":name,"region":{"left":0.1,"top":0.2,"right":0.3,"bottom":0.4}}]);
        assert!(Profile::from_json(&saved.to_string()).is_err(), "{name:?}");
    }
    saved["masks"][0]["name"] = serde_json::json!("Chat");
    assert!(Profile::from_json(&saved.to_string()).is_ok());
}

#[test]
fn display_icons_allow_only_the_complete_four_class_catalog() {
    let mut count = 0;
    for id in 0..=u16::MAX {
        let allowed = matches!(id, 1..=5 | 16..=20 | 31..=35 | 46..=50 | 61..=66 | 76..=81 | 91..=96 | 106..=111);
        let parsed = metin_core::SkillIconId::new(id);
        assert_eq!(parsed.is_ok(), allowed, "{id}");
        let saved = serde_json::from_value::<metin_core::SkillIconId>(serde_json::json!(id));
        assert_eq!(saved.is_ok(), allowed, "{id}");
        if let Ok(icon) = parsed {
            count += 1;
            assert_eq!(icon.get(), id);
            assert_eq!(saved.unwrap(), icon);
            assert_eq!(serde_json::to_value(icon).unwrap(), serde_json::json!(id));
        }
    }
    assert_eq!(count, 44);
    for value in [
        serde_json::json!("4"),
        serde_json::json!(-1),
        serde_json::json!(65536),
        serde_json::json!(true),
    ] {
        assert!(serde_json::from_value::<metin_core::SkillIconId>(value).is_err());
    }
}

#[test]
fn presentation_icon_migrates_and_roundtrips_without_affecting_buff_execution() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let reference = |width: u32| {
        // Core validates headers; actual PNG decoding belongs to vision/storage tests.
        let mut header = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        header.extend(width.to_be_bytes());
        header.extend(16u32.to_be_bytes());
        metin_core::BuffIcon {
            png: STANDARD.encode(header),
            region: Some(Region {
                left: 0.1,
                top: 0.1,
                right: 0.2,
                bottom: 0.2,
            }),
        }
    };
    let mut profile = Profile::create("Icons");
    profile.buffs.skills.push(metin_core::BuffSkill {
        icon: Some(reference(16)),
        inactive_icon: Some(reference(17)),
        ..Default::default()
    });
    let old = serde_json::to_value(&profile).unwrap();
    assert!(old["buffs"]["skills"][0].get("display_icon").is_none());
    let loaded = Profile::from_json(&old.to_string()).unwrap();
    assert_eq!(loaded.buffs.skills[0].display_icon, None);
    let mut decorated = loaded.clone();
    decorated.buffs.skills[0].display_icon = Some(metin_core::SkillIconId::new(96).unwrap());
    decorated.buffs.skills[0].name = "Custom skill name".into();
    assert_ne!(decorated, loaded);
    assert!(decorated.buffs.execution_eq(&loaded.buffs));
    assert_eq!(decorated.buffs.skills[0].icon, loaded.buffs.skills[0].icon);
    assert_eq!(
        decorated.buffs.skills[0].inactive_icon,
        loaded.buffs.skills[0].inactive_icon
    );
    assert_eq!(
        Profile::from_json(&serde_json::to_string(&decorated).unwrap()).unwrap(),
        decorated
    );
    let mut changed_reference = decorated.clone();
    changed_reference.buffs.skills[0].icon = None;
    assert!(!changed_reference.buffs.execution_eq(&decorated.buffs));
    let mut invalid = old;
    invalid["buffs"]["skills"][0]["display_icon"] = serde_json::json!(130);
    assert!(Profile::from_json(&invalid.to_string()).is_err());
}

#[test]
fn buff_hold_defaults_for_saved_profiles_roundtrips_and_rejects_invalid_values() {
    let mut value = serde_json::to_value(metin_core::BuffSkill::default()).unwrap();
    value.as_object_mut().unwrap().remove("active_hold_seconds");
    let mut skill: metin_core::BuffSkill = serde_json::from_value(value).unwrap();
    assert_eq!(skill.active_hold_seconds, 1.5);
    skill.active_hold_seconds = 2.;
    let loaded: metin_core::BuffSkill =
        serde_json::from_str(&serde_json::to_string(&skill).unwrap()).unwrap();
    assert_eq!(loaded, skill);
    for seconds in [-0.1, 5.1, f64::NAN, f64::INFINITY] {
        skill.active_hold_seconds = seconds;
        assert!(skill.validate().is_err());
    }
    skill.active_hold_seconds = 0.;
    assert!(skill.validate().is_ok());
}

#[test]
fn calibrated_profile_is_ready_and_roundtrips_without_hp_search_bounds() {
    let p = Profile::from_json(include_str!(
        "../../../tests/parity/profiles/calibrated.json"
    ))
    .unwrap();
    assert!(p.ready());
    let encoded = serde_json::to_value(&p).unwrap();
    assert!(encoded["panel"].get("region").is_none());
    let roundtrip = Profile::from_json(&encoded.to_string()).unwrap();
    assert_eq!(roundtrip, p);
    assert!(roundtrip.ready());
}

#[test]
fn saved_hp_search_bounds_are_discarded_without_changing_other_configuration() {
    let mut canonical = Profile::from_json(include_str!(
        "../../../tests/parity/profiles/calibrated.json"
    ))
    .unwrap();
    canonical.template_region = Some(Region {
        left: 0.1,
        top: 0.1,
        right: 0.2,
        bottom: 0.2,
    });
    canonical.masks.push(
        Region {
            left: 0.7,
            top: 0.7,
            right: 0.9,
            bottom: 0.9,
        }
        .into(),
    );
    canonical.panel.threshold = 0.93;
    canonical.panel.stable_frames = 4;
    canonical.validate().unwrap();
    for retired in [
        serde_json::json!({"left": 0.7, "top": 0.7, "right": 0.71, "bottom": 0.71}),
        serde_json::Value::Null,
    ] {
        let mut saved = serde_json::to_value(&canonical).unwrap();
        saved["panel"]["region"] = retired;
        let migrated = Profile::from_json(&saved.to_string()).unwrap();
        assert_eq!(migrated, canonical);
        assert_eq!(migrated.template_file, canonical.template_file);
        assert_eq!(migrated.template_region, canonical.template_region);
        assert!(
            serde_json::to_value(migrated).unwrap()["panel"]
                .get("region")
                .is_none()
        );
    }
    let mut unexpected = serde_json::to_value(&canonical).unwrap();
    unexpected["panel"]["unexpected"] = serde_json::json!(true);
    assert!(Profile::from_json(&unexpected.to_string()).is_err());
}

#[test]
fn profiles_discard_old_window_hints_and_never_save_a_connection() {
    let profile = Profile::create("Main profile");
    let mut old = serde_json::to_value(&profile).unwrap();
    old["process_name"] = serde_json::json!("custom-client.exe");
    let loaded = Profile::from_json(&old.to_string()).unwrap();
    assert_eq!(loaded, profile);
    assert!(
        serde_json::to_value(loaded)
            .unwrap()
            .get("process_name")
            .is_none()
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
fn precise_five_pixel_regions_at_eight_k_are_valid_but_zero_extent_is_rejected() {
    let region = Region {
        left: 100. / 8192.,
        top: 200. / 4320.,
        right: 104. / 8192.,
        bottom: 204. / 4320.,
    };
    assert!(region.validate().is_ok());
    assert_eq!(region.pixels(8192, 4320), [100, 200, 104, 204]);
    assert!(
        Region {
            right: region.left,
            ..region
        }
        .validate()
        .is_err()
    );
    assert!(
        Region {
            bottom: region.top,
            ..region
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
