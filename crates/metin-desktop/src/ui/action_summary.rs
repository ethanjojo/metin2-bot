use super::ActionsPage;
use metin_core::{BuffMode, Profile, ScheduledGroup};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ActionState {
    On,
    Off,
    SetupNeeded,
}

impl ActionState {
    pub fn label(self) -> &'static str {
        match self {
            Self::On => "On",
            Self::Off => "Off",
            Self::SetupNeeded => "Setup needed",
        }
    }
}

pub(super) struct ActionSummary {
    pub page: ActionsPage,
    pub state: ActionState,
    pub enabled: usize,
    pub total: usize,
    pub detail: String,
}

impl ActionSummary {
    fn new(
        page: ActionsPage,
        enabled: usize,
        total: usize,
        incomplete: usize,
        enabled_incomplete: bool,
        detail: String,
    ) -> Self {
        let state = if enabled_incomplete || total == 0 || (enabled == 0 && incomplete > 0) {
            ActionState::SetupNeeded
        } else if enabled > 0 {
            ActionState::On
        } else {
            ActionState::Off
        };
        Self {
            page,
            state,
            enabled,
            total,
            detail: if incomplete > 0 {
                format!("{incomplete} need setup · {detail}")
            } else {
                detail
            },
        }
    }

    pub fn text(&self) -> String {
        format!("{}/{} enabled · {}", self.enabled, self.total, self.detail)
    }
}

/// Catalog configuration is always derived from the edited profile, never a stale
/// runtime snapshot. "On" describes the draft switch, not runtime acknowledgement.
pub(super) fn summaries(profile: &Profile) -> [ActionSummary; 7] {
    let scheduled_issue = metin_core::validate_scheduled_setup(profile)
        .err()
        .map(|issue| issue.group);
    let group_ok = |group| scheduled_issue != Some(group);
    let single = |page, enabled: bool, ready: bool, detail: String| {
        ActionSummary::new(
            page,
            usize::from(enabled),
            1,
            usize::from(!ready),
            enabled && !ready,
            detail,
        )
    };

    // Buff readiness deliberately checks the enabled form too: ready() accepts
    // disabled configurations, which would hide a missing icon in the catalog.
    let mut buffs = profile.buffs.clone();
    buffs.enabled = true;
    let buffs_ready = buffs.validate().is_ok() && buffs.ready() && group_ok(ScheduledGroup::Buffs);
    let timed = buffs
        .skills
        .iter()
        .filter(|skill| skill.mode == BuffMode::Refresh)
        .count();
    let buff_modes = if timed == buffs.skills.len() {
        "Timed"
    } else if timed == 0 {
        "Icon-based"
    } else {
        "Timed + icon-based"
    };
    let buff_count = buffs.skills.len();

    let visual_incomplete = |rule: &&metin_core::SafetyRule| {
        !rule.ready()
            || !rule.reaction.any()
            || rule.validate().is_err()
            || (rule.reaction.execute
                && rule
                    .execution
                    .input
                    .validate(true, profile.frame_size)
                    .is_err())
    };
    let rules = &profile.safety.rules;
    let visual_enabled = rules.iter().filter(|rule| rule.enabled).count();
    let visual_missing = rules.iter().filter(visual_incomplete).count();
    let visual_invalid = rules
        .iter()
        .filter(|rule| rule.enabled)
        .any(|rule| visual_incomplete(&rule))
        || profile.validate_visual_actions().is_err();
    let periodic = &profile.periodic_actions;
    let periodic_missing = periodic
        .iter()
        .filter(|action| action.input.validate(true, profile.frame_size).is_err())
        .count();
    let periodic_enabled = periodic.iter().filter(|action| action.enabled).count();
    let periodic_invalid = !group_ok(ScheduledGroup::Periodic)
        || periodic.iter().any(|action| {
            action.enabled && action.input.validate(true, profile.frame_size).is_err()
        });
    let periodic_keys = periodic
        .iter()
        .filter(|action| action.input.shortcut().is_some())
        .count();
    let periodic_detail = if let [action] = periodic.as_slice() {
        format!(
            "{} · every {} s",
            action.input.shortcut().unwrap_or("Click"),
            action.interval_seconds
        )
    } else {
        format!(
            "{periodic_keys} key · {} click",
            periodic.len() - periodic_keys
        )
    };
    let attack = &profile.out_of_target;
    [
        ActionSummary::new(
            ActionsPage::Buffs,
            if profile.buffs.enabled { buff_count } else { 0 },
            buff_count,
            usize::from(!buffs_ready),
            profile.buffs.enabled && !buffs_ready,
            buff_modes.into(),
        ),
        single(
            ActionsPage::Loot,
            profile.loot.enabled,
            profile.loot.validate(profile).is_ok() && group_ok(ScheduledGroup::Loot),
            format!(
                "{} · {} presses",
                profile.loot.shortcut, profile.loot.presses
            ),
        ),
        single(
            ActionsPage::Mount,
            profile.buffs.enabled,
            metin_core::Shortcut::parse(&profile.buffs.mount_shortcut).is_ok()
                && (!profile.buffs.enabled || buffs_ready),
            format!(
                "{} · {}",
                profile.buffs.mount_shortcut,
                if profile.buffs.fight_mounted {
                    "Fight mounted"
                } else {
                    "Fight on foot"
                }
            ),
        ),
        ActionSummary::new(
            ActionsPage::VisualRules,
            visual_enabled,
            rules.len(),
            visual_missing,
            visual_invalid,
            format!(
                "{} notify · {} input",
                rules.iter().filter(|rule| rule.reaction.notify).count(),
                rules.iter().filter(|rule| rule.reaction.execute).count()
            ),
        ),
        ActionSummary::new(
            ActionsPage::Periodic,
            periodic_enabled,
            periodic.len(),
            periodic_missing,
            periodic_invalid,
            periodic_detail,
        ),
        single(
            ActionsPage::OutOfTarget,
            attack.enabled(),
            attack.validate().is_ok() && group_ok(ScheduledGroup::OutOfTarget),
            format!(
                "{} · {}",
                attack.shortcut,
                match (attack.after_combat, attack.during_search) {
                    (true, true) => "After combat + search",
                    (true, false) => "After combat",
                    (false, true) => "During search",
                    (false, false) => "After combat / search",
                }
            ),
        ),
        single(
            ActionsPage::Puller,
            profile.puller.enabled,
            profile.puller.validate().is_ok() && group_ok(ScheduledGroup::Puller),
            format!(
                "{} · {}",
                profile.puller.timing.shortcut,
                match profile.puller.mode {
                    metin_core::PullerMode::CombatStart => "Combat start".into(),
                    metin_core::PullerMode::CombatPeriodic =>
                        format!("Repeat {} s", profile.puller.timing.interval_seconds),
                }
            ),
        ),
    ]
}
