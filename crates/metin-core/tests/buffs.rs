use metin_core::{BuffConfig, BuffCycle, BuffSkill, Presence};
use std::collections::HashSet;
#[test]
fn independent_intervals_start_after_own_wait_and_pause_preserves_remaining() {
    let config = BuffConfig {
        enabled: true,
        fight_mounted: false,
        skills: vec![
            BuffSkill {
                wait_seconds: 2.,
                interval_seconds: Some(14400.),
                ..Default::default()
            },
            BuffSkill {
                shortcut: "F2".into(),
                wait_seconds: 1.,
                interval_seconds: Some(60.),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut b = BuffCycle::new(config);
    b.mounted = Some(false);
    b.resume(0.);
    b.begin(0., &[], &HashSet::new());
    assert_eq!(b.step(0., &[]).unwrap().skill_index, Some(0));
    b.acknowledge(0.).unwrap();
    assert!(b.step(1., &[]).is_none());
    assert_eq!(b.step(2., &[]).unwrap().skill_index, Some(1));
    b.acknowledge(2.).unwrap();
    assert_eq!(b.skill_remaining(3., false), vec![Some(14399.), Some(60.)]);
    b.pause(3.);
    b.resume(103.);
    assert_eq!(
        b.skill_remaining(103., false),
        vec![Some(14399.), Some(60.)]
    );
    b.reset_timer(1, 104., false).unwrap();
    assert_eq!(b.skill_remaining(104., false), vec![Some(14398.), Some(0.)]);
}
#[test]
fn uncertain_toggle_is_not_sent_or_dismounted() {
    let config = BuffConfig {
        enabled: true,
        skills: vec![BuffSkill {
            mode: metin_core::BuffMode::Inactive,
            interval_seconds: None,
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut b = BuffCycle::new(config);
    b.mounted = Some(true);
    b.resume(0.);
    assert!(!b.needed(0., &[Presence::Unknown], &HashSet::new()));
    assert!(b.needed(0., &[Presence::Absent], &HashSet::new()));
}
