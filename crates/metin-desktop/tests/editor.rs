use metin_core::{Profile, Region};
use metin_desktop::editor::{EditMode, Editor};
use metin_platform::Frame;
fn distinctive_frame(shift: u8) -> Frame {
    Frame::new(
        200,
        200,
        (0..200 * 200 * 3)
            .map(|n| ((n % 251) as u8) ^ shift)
            .collect(),
    )
    .unwrap()
}
fn area(left: f64, top: f64, right: f64, bottom: f64) -> Region {
    Region {
        left,
        top,
        right,
        bottom,
    }
}
#[test]
fn reference_crop_accepts_exactly_five_source_pixels_through_the_editor() {
    let mut editor = Editor::new(Profile::create("Five pixels"), None);
    editor.freeze(distinctive_frame(0)).unwrap();
    editor
        .set_region(
            EditMode::Template,
            area(20. / 200., 20. / 200., 24. / 200., 24. / 200.),
        )
        .unwrap();
    let template = editor.template.as_ref().unwrap();
    assert_eq!([template.width, template.height], [5, 5]);
}
#[test]
fn history_is_limited_to_32_states_skips_noops_and_discards_redo_branch() {
    let mut e = Editor::new(Profile::create("Test"), None);
    for n in 1..=40 {
        e.profile.name = format!("Name {n}");
        e.record();
        e.record();
    }
    for _ in 0..100 {
        e.undo();
    }
    assert_eq!(e.profile.name, "Name 9");
    assert!(!e.can_undo());
    e.redo();
    assert_eq!(e.profile.name, "Name 10");
    e.profile.name = "New branch".into();
    e.record();
    assert!(!e.can_redo());
}
#[test]
fn template_history_preserves_reference_while_other_areas_change() {
    let mut e = Editor::new(Profile::create("Test"), None);
    e.freeze(distinctive_frame(0)).unwrap();
    e.set_region(EditMode::Template, area(0.2, 0.2, 0.3, 0.3))
        .unwrap();
    let pixels = e.template.clone();
    e.undo();
    assert!(e.template.is_none());
    e.redo();
    assert_eq!(e.template, pixels);
    e.set_region(EditMode::World, area(0., 0., 0.9, 0.9))
        .unwrap();
    e.delete(EditMode::World).unwrap();
    assert_eq!(e.template, pixels);
    e.undo();
    assert_eq!(e.template, pixels);
}

#[test]
fn invalid_reference_confirmation_preserves_all_calibration_and_source_pixels() {
    let mut e = Editor::new(Profile::create("Fixed detail"), None);
    e.freeze(distinctive_frame(0)).unwrap();
    e.set_region(EditMode::World, area(0.1, 0.1, 0.9, 0.9))
        .unwrap();
    e.color([20, 20]).unwrap();
    e.set_region(EditMode::Template, area(0.1, 0.1, 0.2, 0.2))
        .unwrap();
    let before = e.profile.clone();
    let pixels = e.template.clone();
    let frozen = e.frozen.clone();
    let capture = e
        .begin_capture(EditMode::Template, distinctive_frame(128))
        .unwrap();
    assert!(
        capture
            .commit(&mut e, Some(area(0.1, 0.1, 0.11, 0.11)), [0, 0])
            .is_err()
    );
    assert_eq!(e.profile, before);
    assert_eq!(e.template, pixels);
    assert_eq!(e.frozen, frozen);
}

#[test]
fn active_inactive_crops_are_independent_and_reuse_recrops_current_frozen_pixels() {
    let mut p = Profile::create("Test");
    p.buffs.skills.push(Default::default());
    let mut e = Editor::new(p, None);
    e.freeze(distinctive_frame(0)).unwrap();
    e.set_region(EditMode::BuffArea, area(0., 0., 0.5, 0.5))
        .unwrap();
    let active = area(0.1, 0.1, 0.15, 0.15);
    e.set_region(
        EditMode::BuffIcon {
            index: 0,
            inactive: false,
        },
        active,
    )
    .unwrap();
    let original = e.profile.buffs.skills[0].icon.clone().unwrap();
    e.set_region(
        EditMode::BuffIcon {
            index: 0,
            inactive: true,
        },
        area(0.2, 0.2, 0.3, 0.3),
    )
    .unwrap();
    let inactive = e.profile.buffs.skills[0].inactive_icon.as_ref().unwrap();
    let old_size = metin_vision::decode_png(&original.png_bytes().unwrap())
        .unwrap()
        .width;
    assert_ne!(
        metin_vision::decode_png(&inactive.png_bytes().unwrap())
            .unwrap()
            .width,
        old_size
    );
    e.freeze(distinctive_frame(128)).unwrap();
    e.set_region(
        EditMode::BuffIcon {
            index: 0,
            inactive: true,
        },
        active,
    )
    .unwrap();
    let reused = e.profile.buffs.skills[0].inactive_icon.as_ref().unwrap();
    assert_eq!(reused.region, Some(active));
    assert_ne!(reused.png, original.png);
    assert_eq!(e.profile.buffs.skills[0].icon, Some(original));
    let before = e.profile.clone();
    e.set_region(
        EditMode::BuffIcon {
            index: 0,
            inactive: true,
        },
        area(0.6, 0.6, 0.7, 0.7),
    )
    .unwrap();
    assert!(
        e.profile
            .buffs
            .icon_region
            .unwrap()
            .contains(area(0.6, 0.6, 0.7, 0.7))
    );
    e.undo();
    assert_eq!(e.profile, before);
}

#[test]
fn single_buff_icon_capture_derives_clamped_search_area_and_undoes_atomically() {
    let mut p = Profile::create("Test");
    p.buffs.skills.push(Default::default());
    let mut e = Editor::new(p, None);
    e.freeze(distinctive_frame(0)).unwrap();
    let before = e.profile.clone();
    let icon = area(0., 0., 0.05, 0.05);
    e.set_region(
        EditMode::BuffIcon {
            index: 0,
            inactive: false,
        },
        icon,
    )
    .unwrap();
    let search = e.profile.buffs.icon_region.unwrap();
    assert!(search.contains(icon));
    assert_eq!([search.left, search.top], [0., 0.]);
    assert!(search.right > icon.right && search.bottom > icon.bottom);
    assert!(e.profile.buffs.skills[0].icon.is_some());
    e.undo();
    assert_eq!(e.profile, before);
}
#[test]
fn gesture_commit_history_and_single_mask_delete() {
    let mut e = Editor::new(Profile::create("Test"), None);
    e.freeze(Frame::new(200, 200, vec![50; 200 * 200 * 3]).unwrap())
        .unwrap();
    let r = Region {
        left: 0.1,
        top: 0.1,
        right: 0.5,
        bottom: 0.5,
    };
    e.set_region(EditMode::World, r).unwrap();
    e.set_region(EditMode::Mask(None), r).unwrap();
    e.set_region(EditMode::Mask(None), Region { left: 0.6, ..r })
        .unwrap_err();
    assert_eq!(e.profile.masks.len(), 1);
    e.delete(EditMode::Mask(Some(0))).unwrap();
    assert!(e.profile.masks.is_empty());
    e.undo();
    assert_eq!(e.profile.masks.len(), 1);
    e.redo();
    assert!(e.profile.masks.is_empty());
}

#[test]
fn new_frame_dimensions_are_rejected_without_erasing_calibration_or_history() {
    let mut e = Editor::new(Profile::create("Test"), None);
    e.freeze(distinctive_frame(0)).unwrap();
    e.set_region(EditMode::World, area(0.1, 0.1, 0.8, 0.8))
        .unwrap();
    e.color([20, 20]).unwrap();
    e.set_region(EditMode::Template, area(0.2, 0.2, 0.4, 0.4))
        .unwrap();
    let before = e.profile.clone();
    let pixels = e.template.clone();
    let frozen = e.frozen.clone();
    let frame = Frame::new(300, 200, vec![50; 300 * 200 * 3]).unwrap();
    let error = e.freeze(frame.clone()).unwrap_err();
    assert!(error.contains("200 × 200") && error.contains("300 × 200"));
    assert!(e.begin_capture(EditMode::World, frame).is_err());
    assert_eq!(e.profile, before);
    assert_eq!(e.template, pixels);
    assert_eq!(e.frozen, frozen);
    e.undo();
    assert!(e.template.is_none());
    assert!(e.profile.color.is_some() && e.profile.region.is_some());
    e.redo();
    assert_eq!(e.profile, before);
}

#[test]
fn safety_capture_supports_undo_and_keeps_reference_after_rejected_geometry_change() {
    let mut e = Editor::new(Profile::create("Safety"), None);
    e.profile
        .safety
        .rules
        .push(metin_core::SafetyRule::new("Disconnected"));
    e.record();
    e.freeze(distinctive_frame(0)).unwrap();
    e.set_region(EditMode::SafetyReference(0), area(0.1, 0.1, 0.2, 0.2))
        .unwrap();
    assert!(e.profile.safety.rules[0].ready());
    e.profile.safety.rules[0].enabled = true;
    e.record();
    e.delete(EditMode::SafetyReference(0)).unwrap();
    assert!(!e.profile.safety.rules[0].enabled);
    assert!(!e.profile.safety.rules[0].ready());
    e.undo();
    assert!(e.profile.safety.rules[0].enabled);
    assert!(e.profile.safety.rules[0].ready());
    let before = e.profile.clone();
    assert!(
        e.freeze(Frame::new(300, 300, vec![0; 300 * 300 * 3]).unwrap())
            .is_err()
    );
    assert_eq!(e.profile, before);
    assert!(e.profile.safety.rules[0].enabled && e.profile.safety.rules[0].ready());
}

#[test]
fn capture_cancel_preserves_all_tools_frozen_pixels_dirty_state_and_redo() {
    let mut p = Profile::create("All tools");
    p.buffs.skills.push(Default::default());
    p.safety.rules.push(metin_core::SafetyRule::new("Visual"));
    p.periodic_actions
        .push(metin_core::PeriodicAction::new("Periodic"));
    let id = p.periodic_actions[0].id;
    let mut e = Editor::new(p, None);
    e.freeze(distinctive_frame(0)).unwrap();
    let r = area(0.1, 0.1, 0.2, 0.2);
    let modes = [
        EditMode::World,
        EditMode::Template,
        EditMode::BuffArea,
        EditMode::BuffIcon {
            index: 0,
            inactive: false,
        },
        EditMode::BuffIcon {
            index: 0,
            inactive: true,
        },
        EditMode::SafetyReference(0),
        EditMode::VisualClick(0),
        EditMode::PeriodicClick(id),
    ];
    for (i, mode) in modes.into_iter().enumerate() {
        e.set_region(mode, if i == 4 { area(0.3, 0.3, 0.4, 0.4) } else { r })
            .unwrap();
    }
    e.set_region(EditMode::Mask(None), r).unwrap();
    e.color([20, 20]).unwrap();
    e.profile.name = "Committed".into();
    e.record();
    e.mark_saved(e.profile.clone());
    e.profile.name = "Redo branch".into();
    e.record();
    e.undo();
    let before = e.profile.clone();
    let pixels = e.template.clone();
    let source = e.frozen.clone();
    let dirty = e.dirty();
    for mode in modes.into_iter().chain([
        EditMode::Mask(Some(0)),
        EditMode::Mask(None),
        EditMode::Color,
    ]) {
        let capture = e.begin_capture(mode, distinctive_frame(128)).unwrap();
        assert_eq!(capture.initial_region(), e.region(mode));
        assert_eq!(capture.frame(), &distinctive_frame(128));
        drop(capture); // Escape/F8, a failed native window, or closing all take this path.
        assert_eq!(e.profile, before);
        assert_eq!(e.template, pixels);
        assert_eq!(e.frozen, source);
        assert_eq!(e.dirty(), dirty);
        assert!(e.can_redo());
    }
    e.redo();
    assert_eq!(e.profile.name, "Redo branch");
}

#[test]
fn confirmed_capture_commits_one_edit_and_stale_capture_never_overwrites_changes() {
    let mut e = Editor::new(Profile::create("Draft"), None);
    let before = e.profile.clone();
    let capture = e
        .begin_capture(EditMode::World, distinctive_frame(0))
        .unwrap();
    assert_eq!(e.profile, before);
    assert!(e.frozen.is_none());
    let r = area(0.1, 0.1, 0.5, 0.5);
    capture.commit(&mut e, Some(r), [0, 0]).unwrap();
    assert_eq!(e.profile.region, Some(r));
    assert_eq!(e.profile.frame_size, Some([200, 200]));
    e.undo();
    assert_eq!(e.profile, before);
    e.redo();
    assert_eq!(e.profile.region, Some(r));
    let capture = e
        .begin_capture(EditMode::Color, distinctive_frame(128))
        .unwrap();
    e.profile.name = "Changed while selecting".into();
    e.record();
    let latest = e.profile.clone();
    assert!(capture.commit(&mut e, None, [20, 20]).is_err());
    assert_eq!(e.profile, latest);
    let capture = e
        .begin_capture(EditMode::Color, distinctive_frame(128))
        .unwrap();
    capture.commit(&mut e, None, [20, 20]).unwrap();
    assert!(e.profile.color.is_some());
    assert_eq!(e.profile.region, Some(r));
}

#[test]
fn size_review_preserves_physical_coordinates_and_cancel_restores_the_transaction() {
    let mut e = Editor::new(Profile::create("Review"), None);
    e.freeze(distinctive_frame(0)).unwrap();
    e.set_region(EditMode::World, area(0.1, 0.1, 0.7, 0.7))
        .unwrap();
    e.set_region(EditMode::Template, area(0.2, 0.2, 0.4, 0.4))
        .unwrap();
    e.mark_saved(e.profile.clone());
    e.profile.name = "Redo preserved".into();
    e.record();
    e.undo();
    let before = e.profile.clone();
    let pixels = e.template.clone();
    let frozen = e.frozen.clone();
    let old = e.region(EditMode::World).unwrap().pixels(200, 200);
    e.begin_size_review(Frame::new(200, 185, vec![50; 200 * 185 * 3]).unwrap())
        .unwrap();
    assert!(e.is_recalibrating());
    assert_eq!(e.reviewing_dimensions(), Some(([200, 200], [200, 185])));
    assert_eq!(e.region(EditMode::World).unwrap().pixels(200, 185), old);
    assert_eq!(
        e.template, pixels,
        "Reference pixels are never scaled or recaptured"
    );
    assert!(e.review_issues().is_empty());
    e.cancel_size_review();
    assert_eq!(e.profile, before);
    assert_eq!(e.frozen, frozen);
    assert_eq!(e.template, pixels);
    assert!(!e.dirty() && e.can_redo());
    e.redo();
    assert_eq!(e.profile.name, "Redo preserved");
}

#[test]
fn size_review_flags_out_of_bounds_allows_incremental_repairs_and_confirms_one_edit() {
    let mut e = Editor::new(Profile::create("Review"), None);
    e.freeze(distinctive_frame(0)).unwrap();
    e.set_region(EditMode::World, area(0.1, 0.94, 0.7, 0.99))
        .unwrap();
    e.set_region(EditMode::Mask(None), area(0.1, 0.95, 0.3, 0.99))
        .unwrap();
    e.mark_saved(e.profile.clone());
    let before = e.profile.clone();
    let frame = Frame::new(200, 185, vec![50; 200 * 185 * 3]).unwrap();
    e.begin_size_review(frame.clone()).unwrap();
    assert_eq!(e.review_issues().len(), 2);
    assert!(e.finish_size_review().is_err());
    assert!(e.is_recalibrating());
    let capture = e.begin_capture(EditMode::World, frame.clone()).unwrap();
    capture
        .commit(&mut e, Some(area(0.1, 0.1, 0.7, 0.8)), [0, 0])
        .unwrap();
    assert_eq!(
        e.review_issues().len(),
        1,
        "One repair must work while another region still needs attention"
    );
    e.delete(EditMode::Mask(Some(0))).unwrap();
    assert!(e.review_issues().is_empty());
    e.finish_size_review().unwrap();
    assert!(!e.is_recalibrating());
    assert_eq!(e.profile.frame_size, Some([200, 185]));
    e.undo();
    assert_eq!(e.profile, before);
    assert!(
        e.frozen
            .as_ref()
            .is_none_or(|f| e.profile.frame_size == Some([f.width, f.height]))
    );
    e.redo();
    assert_eq!(e.profile.frame_size, Some([200, 185]));
    assert!(e.profile.masks.is_empty());
}

#[test]
fn size_review_rejects_stale_selection_and_nested_or_changed_geometry_without_mutation() {
    let mut e = Editor::new(Profile::create("Review"), None);
    e.freeze(distinctive_frame(0)).unwrap();
    let before = e.profile.clone();
    let frame = Frame::new(200, 185, vec![50; 200 * 185 * 3]).unwrap();
    e.begin_size_review(frame.clone()).unwrap();
    assert!(
        e.begin_size_review(Frame::new(200, 180, vec![50; 200 * 180 * 3]).unwrap())
            .is_err()
    );
    assert!(
        e.begin_capture(EditMode::World, distinctive_frame(0))
            .is_err()
    );
    let capture = e.begin_capture(EditMode::World, frame).unwrap();
    e.cancel_size_review();
    assert!(
        capture
            .commit(&mut e, Some(area(0.1, 0.1, 0.7, 0.8)), [0, 0])
            .is_err()
    );
    assert_eq!(e.profile, before);
}

#[test]
fn size_review_marks_an_endpoint_exactly_one_pixel_beyond_the_new_client_as_outside() {
    let mut e = Editor::new(Profile::create("Edge"), None);
    e.freeze(distinctive_frame(0)).unwrap();
    e.set_region(EditMode::World, area(0.1, 0.1, 0.8, 185. / 200.))
        .unwrap();
    e.begin_size_review(Frame::new(200, 185, vec![40; 200 * 185 * 3]).unwrap())
        .unwrap();
    assert_eq!(
        e.review_issues().len(),
        1,
        "Pixel185 is outside a185px image and cannot be silently clamped to184"
    );
}

#[test]
fn size_review_selection_cannot_commit_after_the_review_has_finished() {
    let mut e = Editor::new(Profile::create("Closed review"), None);
    e.freeze(distinctive_frame(0)).unwrap();
    let frame = Frame::new(200, 185, vec![40; 200 * 185 * 3]).unwrap();
    e.begin_size_review(frame.clone()).unwrap();
    let capture = e.begin_capture(EditMode::World, frame).unwrap();
    e.finish_size_review().unwrap();
    let before = e.profile.clone();
    assert!(
        capture
            .commit(&mut e, Some(area(0.1, 0.1, 0.7, 0.7)), [0, 0])
            .is_err()
    );
    assert_eq!(e.profile, before);
}
