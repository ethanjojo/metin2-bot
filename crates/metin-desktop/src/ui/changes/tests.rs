use super::*;
use metin_core::{
    ActionInput, BuffIcon, BuffMode, BuffSkill, ColorConfig, ExcludedArea, PeriodicAction, Region,
    RuleReactions, SafetyRule,
};
fn changes(a: &Profile, b: &Profile) -> Vec<ProfileChange> {
    profile_changes(a, b, [None, None], &[])
}
fn region(x: f64) -> Region {
    Region {
        left: x,
        top: 0.1,
        right: x + 0.2,
        bottom: 0.3,
    }
}
fn skill(name: &str) -> BuffSkill {
    BuffSkill {
        name: name.into(),
        shortcut: name.into(),
        ..Default::default()
    }
}
fn image(value: u8) -> Frame {
    Frame::new(8, 8, vec![value; 8 * 8 * 3]).unwrap()
}
fn icon(payload: &str, x: f64) -> BuffIcon {
    BuffIcon {
        png: payload.into(),
        region: Some(region(x)),
    }
}
#[test]
fn semantic_color_is_one_change() {
    let mut a = Profile::create("Review");
    a.color = Some(ColorConfig::default());
    let mut b = a.clone();
    b.color.as_mut().unwrap().hsv = [42, 100, 83];
    let rows = changes(&a, &b);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].field.after.text, "HSV 42, 100, 83");
    assert!(matches!(
        rows[0].field.after.preview,
        Some(Preview::Color([42, 100, 83]))
    ));
}
#[test]
fn semantic_reorder_and_removal_each_count_once() {
    let mut a = Profile::create("Review");
    a.buffs.skills = ["A", "B", "C"].map(skill).to_vec();
    let mut b = a.clone();
    b.buffs.skills.swap(0, 2);
    let rows = changes(&a, &b);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].field.before.text, "1. A\n2. B\n3. C");
    assert_eq!(rows[0].field.after.text, "1. C\n2. B\n3. A");
    b.buffs.skills.remove(1);
    let rows = changes(&a, &b);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].field.before.text, "1. A\n2. B\n3. C");
    assert_eq!(rows[0].field.after.text, "1. C\n2. A");
    b = a.clone();
    b.buffs.skills.remove(0);
    let rows = changes(&a, &b);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].field.label.contains("A · Removed buff"));
    assert_eq!(rows[0].subject, Subject::Buff(None));
    b.buffs.skills[0].name = "Renamed".into();
    let rows = changes(&a, &b);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].subject, Subject::Buff(Some(a.buffs.skills[1].id)));
    assert_eq!(rows[0].field.before.text, "B");
}
#[test]
fn semantic_hp_image_and_crop_count_once() {
    let mut a = Profile::create("Review");
    a.frame_size = Some([100, 100]);
    let mut b = a.clone();
    b.template_region = Some(region(0.2));
    b.template_file = Some("a-private-filename.png".into());
    let (one, two) = (image(1), image(2));
    let rows = profile_changes(&a, &b, [Some(&one), Some(&two)], &[]);
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].field.after.text,
        "Image 8 × 8 px · Source 21 × 21 px at (20, 10)"
    );
    assert!(matches!(
        rows[0].field.after.preview,
        Some(Preview::Frame(_))
    ));
    assert!(profile_changes(&a, &a, [Some(&one), Some(&one)], &[]).is_empty());
}
#[test]
fn rebase_preserves_source_pixels_across_all_region_families() {
    let mut a = Profile::create("Geometry");
    let bounds = region(0.1);
    a.frame_size = Some([100, 100]);
    a.region = Some(Region {
        left: 0.,
        top: 0.,
        right: 1.,
        bottom: 1.,
    });
    a.template_region = Some(bounds);
    a.masks.push(ExcludedArea::new("Chat", bounds));
    a.buffs.icon_region = Some(bounds);
    let reference = icon("opaque", 0.1);
    let mut s = skill("F1");
    s.icon = Some(reference.clone());
    s.inactive_icon = Some(reference.clone());
    a.buffs.skills.push(s);
    let mut action = PeriodicAction::new("Potion");
    action.input = ActionInput::Click {
        region: Some(bounds),
    };
    a.periodic_actions.push(action.clone());
    let mut rule = SafetyRule::new("Warning");
    rule.reference = Some(reference);
    rule.execution.input = action.input;
    a.safety.rules.push(rule);
    let mut b = a.clone();
    b.frame_size = Some([200, 200]);
    let new = Region {
        left: 0.05,
        top: 0.05,
        right: 0.15,
        bottom: 0.15,
    };
    b.region = Some(Region {
        left: 0.,
        top: 0.,
        right: 0.495,
        bottom: 0.495,
    });
    b.template_region = Some(new);
    b.masks[0].region = new;
    b.buffs.icon_region = Some(new);
    b.buffs.skills[0].icon.as_mut().unwrap().region = Some(new);
    b.buffs.skills[0].inactive_icon.as_mut().unwrap().region = Some(new);
    b.safety.rules[0].reference.as_mut().unwrap().region = Some(new);
    b.safety.rules[0].execution.input = ActionInput::Click { region: Some(new) };
    b.periodic_actions[0].input = ActionInput::Click { region: Some(new) };
    let rows = changes(&a, &b);
    assert_eq!(
        rows.iter()
            .map(|r| r.field.label.as_str())
            .collect::<Vec<_>>(),
        ["Calibrated frame size"]
    );
}
#[test]
fn catalogue_groups_preset_and_mode_descendants_and_keeps_fine_precision() {
    let mut a = Profile::create("A");
    a.color = Some(Default::default());
    a.buffs.skills.push(skill("F3"));
    a.periodic_actions.push(PeriodicAction::new("Potion"));
    a.safety.rules.push(SafetyRule::new("Warning"));
    let mut b = a.clone();
    let c = b.color.as_mut().unwrap();
    c.hsv = [17, 220, 180];
    c.min_width = 77;
    c.min_height = 89;
    let preset = MetinPreset {
        id: Uuid::new_v4(),
        name: "Jade".into(),
        color: c.clone(),
    };
    b.color_preset_id = Some(preset.id);
    b.buffs.skills[0].mode = BuffMode::Inactive;
    b.buffs.skills[0].interval_seconds = None;
    b.periodic_actions[0].input = ActionInput::Click {
        region: Some(region(0.2)),
    };
    b.safety.rules[0].reaction = RuleReactions {
        notify: false,
        pause: false,
        execute: true,
    };
    b.panel.threshold = 0.8800000000000001;
    let rows = profile_changes(&a, &b, [None, None], std::slice::from_ref(&preset));
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[0].field.after.text, "Jade");
    assert_eq!(rows[0].details.len(), 2);
    assert_eq!(rows[0].details[1].after.text, "77 × 89 px");
    assert_eq!(rows[1].field.before.text, "88%");
    assert_eq!(rows[1].field.after.text, "88.00000000000001%");
    assert_eq!(rows[2].details[0].after.text, "No timer");
    assert_eq!(rows[3].field.after.text, "Click");
    assert!(rows[3].details[0].after.text.contains("of frame"));
    assert_eq!(rows[4].field.after.text, "Perform action");
    let mut custom = b.clone();
    custom.color_preset_id = None;
    custom.color.as_mut().unwrap().min_area += 1;
    let rows = changes(&b, &custom);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].field.label, "Minimum candidate area");
}
#[test]
fn enabled_disabled_and_inactive_execution_settings_have_exact_destinations() {
    let mut a = Profile::create("Main");
    a.buffs.skills.push(skill("F2"));
    a.safety.rules.push(SafetyRule::new("Pause on warning"));
    let mut b = a.clone();
    b.buffs.skills[0].display_icon = Some(metin_core::SkillIconId::new(16).unwrap());
    b.buffs.key_hold_seconds = 0.123;
    b.loot.presses = 4;
    b.puller.timing.interval_seconds += 5.;
    b.out_of_target.max_search_attacks += 1;
    b.safety.rules[0].execution.wait_seconds += 1.;
    b.safety.stop_at_minute = 0;
    b.safety.recovery_wait_seconds += 1.;
    b.safety.system_notification = true;
    b.timing.max_rotations += 1;
    let rows = changes(&a, &b);
    assert_eq!(
        rows.iter().map(|r| r.subject).collect::<Vec<_>>(),
        [
            Subject::Buff(Some(a.buffs.skills[0].id)),
            Subject::Mount,
            Subject::Loot,
            Subject::Puller,
            Subject::OutOfTarget,
            Subject::Rule(Some(a.safety.rules[0].id)),
            Subject::Safety(SafetyPage::Schedule),
            Subject::Safety(SafetyPage::Recovery),
            Subject::Safety(SafetyPage::Notifications),
            Subject::Timing
        ]
    );
    assert!(matches!(
        rows[0].field.after.preview,
        Some(Preview::Icon(_))
    ));
    assert_eq!(rows[1].field.after.text, "0.123 s");
    assert!(rows[3].field.label.contains("inactive setting"));
    assert!(rows[5].field.label.contains("inactive setting"));
    assert_eq!(rows[6].field.after.text, "00:00");
}
#[test]
fn image_and_crop_replacements_are_one_change_per_actual_reference() {
    let mut a = Profile::create("Main");
    a.buffs.skills.push(skill("F2"));
    a.safety.rules.push(SafetyRule::new("Alert"));
    a.buffs.skills[0].icon = Some(icon("PRIVATE1", 0.1));
    a.safety.rules[0].reference = Some(icon("PRIVATE1", 0.2));
    let mut b = a.clone();
    b.buffs.skills[0].icon = Some(icon("PRIVATE2", 0.3));
    b.safety.rules[0].reference = Some(icon("PRIVATE2", 0.4));
    let rows = changes(&a, &b);
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert!(!row.field.before.text.contains("PRIVATE"));
        assert!(!row.field.after.text.contains("PRIVATE"));
        assert!(matches!(row.field.after.preview, Some(Preview::Capture(_))));
    }
}
#[test]
fn repeat_and_verification_modes_fold_their_saved_dependent_settings() {
    let mut a = Profile::create("Modes");
    a.puller.enabled = true;
    a.safety.rules.push(SafetyRule::new("Alarm"));
    let mut b = a.clone();
    b.puller.mode = metin_core::PullerMode::CombatPeriodic;
    b.puller.timing.interval_seconds = 60.;
    b.safety.rules[0].execution.verification = metin_core::Verification::Delay;
    b.safety.rules[0].execution.timeout_seconds = 8.;
    b.safety.rules[0].execution.attempts = 4;
    let rows = changes(&a, &b);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].field.after.text, "Repeat");
    assert_eq!(rows[0].details[0].before.text, "120 s (inactive setting)");
    assert_eq!(rows[0].details[0].after.text, "60 s");
    assert_eq!(rows[1].details[0].after.text, "8 s (event confirmation)");
    assert_eq!(rows[1].details[1].after.text, "4 (inactive setting)");
}
#[test]
fn draft_badge_uses_effective_save_and_navigates_by_live_identity() {
    let (mut app, _ctx, _dir) = crate::ui::tests::setup();
    assert_eq!(app.current_changes().caption(), "New profile");
    app.save().unwrap();
    let blank = BuffSkill {
        name: "Shield".into(),
        shortcut: " ".into(),
        ..Default::default()
    };
    let blank_id = blank.id;
    app.editor.profile.buffs.skills.push(blank);
    app.editor.record();
    let draft = app.current_changes();
    assert_eq!(draft.caption(), "Draft");
    assert_eq!(draft.drafts, [(blank_id, "Shield".into())]);
    assert!(draft.rows.is_empty());
    app.editor.profile.buffs.skills.insert(0, skill("F3"));
    app.editor.record();
    let current = app.current_changes();
    assert_eq!(current.caption(), "1 change");
    let row = &current.rows[0];
    app.editor.profile.buffs.skills.swap(0, 1);
    app.open_profile_change(row);
    assert!(app.buff_detail);
    assert_eq!(app.selected_skill, 1);
    app.editor.profile.buffs.skills.pop();
    app.open_profile_change(row);
    assert!(!app.buff_detail);
    app.revert_profile();
    assert!(app.current_changes().drafts.is_empty());
    assert!(app.current_changes().rows.is_empty());
    app.runtime.shutdown().unwrap();
}
#[test]
fn replacing_same_profile_clears_cached_edits_at_matching_revision() {
    let (mut app, _ctx, _dir) = crate::ui::tests::setup();
    app.save().unwrap();
    let baseline = app.editor.profile.clone();
    app.replace_profile(baseline.clone(), None);
    app.editor.profile.loot.delay_seconds += 1.;
    app.editor.record();
    let revision = app.editor.change_id();
    assert_eq!(app.current_changes().rows[0].subject, Subject::Loot);
    app.replace_profile(baseline, None);
    app.editor.profile.timing.rotate_seconds += 1.;
    app.editor.record();
    assert_eq!(app.editor.change_id(), revision);
    assert_eq!(app.current_changes().rows[0].subject, Subject::Timing);
    app.runtime.shutdown().unwrap();
}
#[test]
fn hp_reference_uses_dimensions_and_pixels_independent_of_allocation_and_filename() {
    let a = Profile::create("Review");
    let mut b = a.clone();
    b.template_file = Some("other-reference.png".into());
    let saved = image(2);
    let same = image(2);
    assert!(profile_changes(&a, &b, [Some(&saved), Some(&same)], &[]).is_empty());
    let reshaped = Frame::new(4, 16, saved.bgr.to_vec()).unwrap();
    let rows = profile_changes(&a, &b, [Some(&saved), Some(&reshaped)], &[]);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].field.before.text.starts_with("Image 8 × 8 px"));
    assert!(rows[0].field.after.text.starts_with("Image 4 × 16 px"));
}

#[test]
fn navigation_resolves_named_rules_actions_and_exclusions_after_deletion() {
    let (mut app, _ctx, _dir) = crate::ui::tests::setup();
    let mut a = Profile::create("List editors");
    for name in ["First", "Second"] {
        a.periodic_actions.push(PeriodicAction::new(name));
        a.safety.rules.push(SafetyRule::new(name));
        a.masks.push(ExcludedArea::new(name, region(0.1)));
    }
    let mut b = a.clone();
    b.periodic_actions.remove(0);
    b.periodic_actions[0].wait_seconds += 1.;
    b.safety.rules.remove(0);
    b.safety.rules[0].duration_seconds += 1.;
    b.masks.remove(0);
    b.masks[0].name = "Chat".into();
    let rows = changes(&a, &b);
    assert_eq!(rows.len(), 6);
    app.editor = Editor::new(b, None);
    for row in rows {
        app.open_profile_change(&row);
        match row.subject {
            Subject::Area(Some(id)) => {
                assert_eq!(id, a.masks[1].id);
                assert_eq!(app.mode, EditMode::Mask(Some(0)));
            }
            Subject::Area(None) => assert_eq!(app.mode, EditMode::Mask(None)),
            Subject::Periodic(Some(id)) => assert_eq!(app.selected_periodic, Some(id)),
            Subject::Periodic(None) => assert!(app.selected_periodic.is_none()),
            Subject::Rule(Some(id)) => {
                assert_eq!(id, a.safety.rules[1].id);
                assert!(app.visual_detail);
                assert_eq!(app.selected_safety, 0);
            }
            Subject::Rule(None) => assert!(!app.visual_detail),
            _ => panic!("unexpected change"),
        }
    }
    app.runtime.shutdown().unwrap();
}

#[test]
fn large_change_popup_scrolls_within_compact_panel_and_groups_rows() {
    let (mut app, ctx, _dir) = crate::ui::tests::setup();
    app.save().unwrap();
    for n in 0..12 {
        app.editor
            .profile
            .periodic_actions
            .push(PeriodicAction::new(&format!("Action {n}")));
    }
    app.editor.record();
    let size = egui::vec2(400. / 1.5, 480. / 1.5);
    let render = |app: &mut MetinApp, events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ui| app.changes_badge(ui),
        );
        output.textures_delta.clear();
        output
    };
    let output = render(&mut app, vec![]);
    let center = crate::ui::tests::text_regions(&output)
        .iter()
        .find(|(s, _)| s == "12 changes")
        .expect("badge count")
        .1
        .center();
    for pressed in [true, false] {
        render(
            &mut app,
            vec![
                egui::Event::PointerMoved(center),
                egui::Event::PointerButton {
                    pos: center,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    for _ in 0..3 {
        render(&mut app, vec![]);
    }
    let output = render(&mut app, vec![]);
    let text = crate::ui::tests::text_regions(&output);
    assert!(text.iter().any(|(s, _)| s == "Unsaved changes"));
    assert!(text.iter().any(|(s, _)| s == "Periodic actions"));
    assert!(text.iter().any(|(s, _)| s == "Saved"));
    assert!(text.iter().any(|(s, _)| s == "Edited"));
    assert!(text.iter().all(|(_, r)| r.right() <= size.x + 1.));
    let (up, down, thumb, _, _) = ctx
        .data(|d| {
            d.get_temp::<(egui::Rect, egui::Rect, egui::Rect, f32, egui::Id)>(egui::Id::unique(
                "select-scroll-geometry",
            ))
        })
        .expect("Visible Metin scrollbar");
    assert!(up.top() >= 0. && down.bottom() <= size.y && thumb.height() > 0.);
    app.runtime.shutdown().unwrap();
}
