use metin_core::{Profile, Region};
use metin_desktop::editor::{EditMode, Editor, Viewport};
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
        .set_region(EditMode::Panel, area(0., 0., 0.9, 0.9))
        .unwrap();
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
fn template_history_retains_pixels_and_panel_containment_invalidates_only_when_needed() {
    let mut e = Editor::new(Profile::create("Test"), None);
    e.freeze(distinctive_frame(0)).unwrap();
    e.set_region(EditMode::Panel, area(0.1, 0.1, 0.9, 0.5))
        .unwrap();
    e.set_region(EditMode::Template, area(0.2, 0.2, 0.3, 0.3))
        .unwrap();
    let template = e.template.clone().unwrap();
    e.undo();
    assert!(e.template.is_none());
    e.redo();
    assert_eq!(e.template, Some(template.clone()));
    e.set_region(EditMode::Panel, area(0.0, 0.0, 0.95, 0.8))
        .unwrap();
    assert_eq!(e.template, Some(template.clone()));
    e.set_region(EditMode::Panel, area(0.5, 0.5, 0.9, 0.9))
        .unwrap();
    assert!(e.template.is_none());
    assert!(e.profile.template_file.is_none());
    e.undo();
    assert_eq!(e.template, Some(template));
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
    assert!(
        e.set_region(
            EditMode::BuffIcon {
                index: 0,
                inactive: true
            },
            area(0.6, 0.6, 0.7, 0.7)
        )
        .is_err()
    );
    assert_eq!(e.profile, before);
}
#[test]
fn viewport_zoom_anchor_and_padding() {
    let mut v = Viewport::default();
    v.fit([800, 600], [400., 400.]);
    assert_eq!(v.to_image([100., 100.], false), Some([200, 200]));
    assert!(v.to_image([10., 390.], false).is_none());
    v.zoom_at([100., 100.], 2.);
    assert_eq!(v.to_image([100., 100.], false), Some([200, 200]));
    v.zoom_at([0., 0.], 100.);
    assert_eq!(v.zoom, 8.);
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
fn new_frame_dimensions_retire_history_from_previous_geometry() {
    let mut e = Editor::new(Profile::create("Test"), None);
    e.freeze(Frame::new(200, 200, vec![50; 200 * 200 * 3]).unwrap())
        .unwrap();
    e.set_region(
        EditMode::World,
        Region {
            left: 0.1,
            top: 0.1,
            right: 0.8,
            bottom: 0.8,
        },
    )
    .unwrap();
    e.freeze(Frame::new(300, 200, vec![50; 300 * 200 * 3]).unwrap())
        .unwrap();
    e.undo();
    assert_eq!(e.profile.frame_size, Some([300, 200]));
    assert!(e.profile.region.is_none());
}

#[test]
fn safety_capture_supports_undo_and_disables_after_geometry_changes() {
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
    e.freeze(Frame::new(300, 300, vec![0; 300 * 300 * 3]).unwrap())
        .unwrap();
    assert!(!e.profile.safety.rules[0].enabled);
    assert!(!e.profile.safety.rules[0].ready());
}
