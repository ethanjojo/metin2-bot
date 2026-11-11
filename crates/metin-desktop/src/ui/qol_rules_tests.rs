// Included from tests.rs: exercise the real edited profile and native controls.
#[test]
fn qol_action_catalog_distinguishes_disabled_incomplete_and_draft_enabled() {
    use action_summary::{ActionState, summaries};
    let mut profile = Profile::create("Draft");
    let initial = summaries(&profile);
    assert_eq!(initial.len(), 7);
    assert_eq!(initial[0].state, ActionState::SetupNeeded);
    let loot = initial
        .iter()
        .find(|row| row.page == ActionsPage::Loot)
        .unwrap();
    assert_eq!(
        (loot.state, loot.enabled, loot.total),
        (ActionState::Off, 0, 1)
    );
    assert!(loot.detail.contains('Z'));
    profile.loot.enabled = true;
    profile.loot.shortcut = "Y".into();
    let edited = summaries(&profile);
    let loot = edited
        .iter()
        .find(|row| row.page == ActionsPage::Loot)
        .unwrap();
    assert_eq!((loot.state, loot.enabled), (ActionState::On, 1));
    assert!(loot.detail.contains('Y'));
    assert!(!loot.detail.contains("Applied"));
    profile
        .periodic_actions
        .push(metin_core::PeriodicAction::new("Click"));
    profile.periodic_actions[0].input = metin_core::ActionInput::Click { region: None };
    let edited = summaries(&profile);
    let periodic = edited
        .iter()
        .find(|row| row.page == ActionsPage::Periodic)
        .unwrap();
    assert_eq!(
        (periodic.state, periodic.enabled, periodic.total),
        (ActionState::SetupNeeded, 0, 1)
    );
    profile.periodic_actions[0].input = Default::default();
    let edited = summaries(&profile);
    let periodic = edited
        .iter()
        .find(|row| row.page == ActionsPage::Periodic)
        .unwrap();
    assert_eq!(periodic.state, ActionState::Off);
    profile.periodic_actions[0].enabled = true;
    let mut incomplete = metin_core::PeriodicAction::new("Unfinished click");
    incomplete.input = metin_core::ActionInput::Click { region: None };
    profile.periodic_actions.push(incomplete);
    let edited = summaries(&profile);
    let periodic = edited
        .iter()
        .find(|row| row.page == ActionsPage::Periodic)
        .unwrap();
    assert_eq!(
        (periodic.state, periodic.enabled, periodic.total),
        (ActionState::On, 1, 2)
    );
    assert!(periodic.detail.contains("1 need setup"));
    profile.periodic_actions[1].enabled = true;
    let edited = summaries(&profile);
    let periodic = edited
        .iter()
        .find(|row| row.page == ActionsPage::Periodic)
        .unwrap();
    assert_eq!(periodic.state, ActionState::SetupNeeded);
}

#[test]
fn qol_rule_choices_preserve_notification_and_bounded_execution_values() {
    let (mut app, ctx, _dir) = setup();
    let mut rule = metin_core::SafetyRule::new("Dialog");
    rule.reaction.pause = false;
    rule.reaction.execute = true;
    rule.execution.wait_seconds = 2.;
    rule.execution.timeout_seconds = 15.;
    rule.execution.attempts = 3;
    app.editor.profile.safety.rules.push(rule);
    app.navigate(Tab::Actions);
    app.actions_page = ActionsPage::VisualRules;
    app.actions_detail = true;
    app.visual_detail = true;
    click_label(&mut app, &ctx, "Wait");
    click_label(&mut app, &ctx, "Continue");
    let rule = &app.editor.profile.safety.rules[0];
    assert!(rule.reaction.notify && rule.reaction.execute && !rule.reaction.pause);
    assert_eq!(rule.execution.verification, metin_core::Verification::Delay);
    assert_eq!(
        rule.execution.after_success,
        metin_core::AfterSuccess::Continue
    );
    assert_eq!(
        (
            rule.execution.wait_seconds,
            rule.execution.timeout_seconds,
            rule.execution.attempts
        ),
        (2., 15., 3)
    );
    click_label(&mut app, &ctx, "Verify");
    click_label(&mut app, &ctx, "Pause");
    click_label(&mut app, &ctx, "Notify");
    let rule = &app.editor.profile.safety.rules[0];
    assert!(!rule.reaction.notify && rule.reaction.execute && !rule.reaction.pause);
    assert_eq!(
        rule.execution.verification,
        metin_core::Verification::ReferenceGone
    );
    assert_eq!(
        rule.execution.after_success,
        metin_core::AfterSuccess::Pause
    );
    assert_eq!(
        (
            rule.execution.wait_seconds,
            rule.execution.timeout_seconds,
            rule.execution.attempts
        ),
        (2., 15., 3)
    );
    app.runtime.shutdown().unwrap();
}

#[test]
fn qol_schedule_keeps_applied_countdowns_in_run() {
    let (mut app, ctx, _dir) = setup();
    app.navigate(Tab::Safety);
    app.safety_page = SafetyPage::Schedule;
    // A tall viewport exposes the whole settings body, so an old collapsed
    // duplicate cannot pass merely because it is below a small scroll viewport.
    let output = compact_settle(&mut app, &ctx, egui::vec2(400., 1000.));
    let text = text_regions(&output);
    assert!(
        !text
            .iter()
            .any(|(value, _)| value == "Advanced applied schedule")
    );
    assert!(
        !text
            .iter()
            .any(|(value, _)| value.contains("window in Recognition"))
    );
    assert!(
        text.iter()
            .any(|(value, _)| value == "Live countdowns are in Run. Save to apply schedule edits.")
    );
    app.runtime.shutdown().unwrap();
}
