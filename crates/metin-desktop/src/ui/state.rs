use super::*;

pub(super) struct Blocker {
    pub message: String,
    pub action: SetupAction,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SetupAction {
    Window,
    Calibration,
    Mount,
    Actions(ActionsPage),
    Safety,
    VisualRules,
    Settings,
}
pub(super) struct RunSetupStep {
    pub title: &'static str,
    pub detail: String,
    pub action: SetupAction,
}
impl MetinApp {
    pub(super) fn undo_profile(&mut self) {
        self.runtime.stop();
        self.gesture = None;
        self.editor.undo();
    }
    pub(super) fn redo_profile(&mut self) {
        self.runtime.stop();
        self.gesture = None;
        self.editor.redo();
    }
    pub(super) fn delete_selected(&mut self) {
        self.delete_area(self.mode);
    }
    pub(super) fn delete_area(&mut self, mode: EditMode) {
        self.runtime.stop();
        self.gesture = None;
        if let Err(error) = self.editor.delete(mode) {
            self.error = Some(error);
            return;
        }
        self.hover_mode = None;
        if let EditMode::Mask(Some(deleted)) = mode
            && let EditMode::Mask(Some(selected)) = self.mode
        {
            if selected == deleted {
                self.mode = EditMode::Mask(None);
                self.editing = false;
            } else if selected > deleted {
                self.mode = EditMode::Mask(Some(selected - 1));
            }
        }
    }

    pub(super) fn has_unsaved_profile(&self) -> bool {
        self.editor.dirty() || !self.profiles.iter().any(|p| p.id == self.editor.profile.id)
    }
    pub(super) fn profile_for_save(&self) -> Profile {
        let mut profile = self.editor.profile.clone();
        profile.buffs.skills = self
            .retained_skill_indices()
            .into_iter()
            .map(|i| self.editor.profile.buffs.skills[i].clone())
            .collect();
        profile
    }
    pub(super) fn retained_skill_indices(&self) -> Vec<usize> {
        self.editor
            .profile
            .buffs
            .skills
            .iter()
            .enumerate()
            .filter(|(_, skill)| !skill.shortcut.trim().is_empty())
            .map(|(i, _)| i)
            .collect()
    }
    pub(super) fn replace_profile(
        &mut self,
        profile: Profile,
        template: Option<metin_platform::Frame>,
    ) {
        self.runtime.stop();
        self.send(Command::Detach);
        self.editor = Editor::new(profile, template);
        self.target = None;
        self.history_open = false;
        self.run_mount_editing = false;
        self.window_search.clear();
        self.target_list_revision = 0;
        self.send(Command::RefreshTargets);
        self.live = false;
        self.texture_source = None;
        self.gesture = None;
        self.binding = None;
        self.units.fill(false);
        self.selected_skill = 0;
        self.return_to_buffs = false;
        self.return_to_safety = false;
        self.selected_safety = 0;
        self.selected_periodic = None;
        self.snapshot_score = None;
        self.preview_snapshot = None;
        self.snapshot_requested = None;
        self.mode = EditMode::World;
        self.editing = false;
        self.viewport.reset();
        self.detection_cache = None;
        self.click_preview = None;
        self.icon_textures.clear();
    }
    pub(super) fn calibration_ready(&self, mode: EditMode) -> bool {
        match mode {
            EditMode::SafetyReference(index) => self
                .editor
                .profile
                .safety
                .rules
                .get(index)
                .is_some_and(|r| r.ready()),
            EditMode::Template => self.editor.template.is_some(),
            EditMode::Color => self.editor.profile.color.is_some(),
            EditMode::BuffIcon { index, inactive } => self
                .editor
                .profile
                .buffs
                .skills
                .get(index)
                .and_then(|skill| {
                    if inactive {
                        skill.inactive_icon.as_ref()
                    } else {
                        skill.icon.as_ref()
                    }
                })
                .is_some(),
            _ => self.editor.region(mode).is_some(),
        }
    }
    pub(super) fn start_blocker(&self) -> Option<Blocker> {
        self.readiness_blockers().into_iter().next()
    }
    // The toolbar and Run checklist consume the same validation results. Run
    // groups related calibration errors into one actionable setup step.
    fn readiness_blockers(&self) -> Vec<Blocker> {
        let mut blockers = Vec::new();
        let mut block = |message: &str, action| {
            blockers.push(Blocker {
                message: message.into(),
                action,
            });
        };
        if !self.snapshot.capabilities.automation_ready() {
            block(
                if self.snapshot.capabilities.reason.is_empty() {
                    "Checking platform capture and input support."
                } else {
                    &self.snapshot.capabilities.reason
                },
                SetupAction::Settings,
            );
        }
        if self.target.is_none() {
            block(
                "Select a game window to connect this profile.",
                SetupAction::Window,
            );
        }
        if self.editor.profile.name.trim().is_empty() {
            block(
                "Enter a profile name in Calibration.",
                SetupAction::Calibration,
            );
        }
        for (mode, label) in [
            (EditMode::World, "Set the search area."),
            (EditMode::Color, "Pick the Metin color."),
            (EditMode::Panel, "Set the target panel area."),
            (
                EditMode::Template,
                "Capture a fixed reference crop inside the target panel.",
            ),
        ] {
            if !self.calibration_ready(mode) {
                block(label, SetupAction::Calibration);
            }
        }
        let profile = self.profile_for_save();
        if let Err(error) = profile.loot.validate(&profile) {
            block(&error.to_string(), SetupAction::Actions(ActionsPage::Loot));
        }
        if let Err(issue) = metin_core::validate_scheduled_setup(&profile) {
            block(
                &issue.error.to_string(),
                SetupAction::Actions(scheduled_page(issue.group)),
            );
        }
        if let Err(error) = profile.safety.validate_schedule() {
            block(&error.to_string(), SetupAction::Safety);
        }
        if let Err(error) = profile.buffs.validate() {
            block(&error.to_string(), SetupAction::Actions(ActionsPage::Buffs));
        }
        if !profile.buffs.ready() {
            block(
                "Finish buff recognition: set its area and the active icon for each toggle skill.",
                SetupAction::Actions(ActionsPage::Buffs),
            );
        }
        if profile.buffs.enabled && self.declared_mount().is_none() {
            block(
                "Confirm On foot or Mounted for this game window in Actions > Mount.",
                SetupAction::Mount,
            );
        }
        if let Err(error) = profile.validate_calibration() {
            block(&error.to_string(), SetupAction::Calibration);
        }
        if let Err(error) = profile.validate_settings() {
            block(&error.to_string(), SetupAction::Settings);
        }
        if let Err(error) = metin_core::validate_rules(&profile.safety.rules)
            .and_then(|()| profile.validate_visual_actions())
        {
            block(&error.to_string(), SetupAction::VisualRules);
        }
        if !profile.ready() {
            block(
                "Take a calibrated snapshot and save the profile.",
                SetupAction::Calibration,
            );
        }
        blockers
    }
    pub(super) fn run_setup_steps(&self) -> Vec<RunSetupStep> {
        let mut steps = Vec::new();
        for blocker in self.readiness_blockers() {
            let action = blocker.action;
            if !steps
                .iter()
                .any(|step: &RunSetupStep| step.action == action)
            {
                steps.push(RunSetupStep {
                    action,
                    detail: blocker.message,
                    title: match action {
                        SetupAction::Window => "Game window",
                        SetupAction::Calibration => "Calibration",
                        SetupAction::Mount => "Mount state",
                        SetupAction::Actions(page) => match page {
                            ActionsPage::Buffs => "Buffs",
                            ActionsPage::Loot => "Loot",
                            ActionsPage::Periodic => "Periodic actions",
                            ActionsPage::Puller => "Auto-puller",
                            ActionsPage::OutOfTarget => "Out-of-target attack",
                            ActionsPage::Mount => "Mount state",
                            ActionsPage::VisualRules => "Visual rules",
                        },
                        SetupAction::Safety => "Safety setup",
                        SetupAction::VisualRules => "Visual rules",
                        SetupAction::Settings => "Platform & settings",
                    },
                });
            }
        }
        steps
    }
    pub(super) fn open_setup(&mut self, action: SetupAction) {
        match action {
            SetupAction::Window | SetupAction::Calibration => self.navigate(Tab::Calibration),
            SetupAction::Mount => {
                self.navigate(Tab::Actions);
                self.actions_page = ActionsPage::Mount;
            }
            SetupAction::Actions(page) => {
                self.navigate(Tab::Actions);
                self.actions_page = page;
            }
            SetupAction::Safety => self.navigate(Tab::Safety),
            SetupAction::VisualRules => {
                self.navigate(Tab::Actions);
                self.actions_page = ActionsPage::VisualRules;
            }
            SetupAction::Settings => self.navigate(Tab::Settings),
        }
    }
    pub(super) fn navigate(&mut self, tab: Tab) {
        if self.tab != tab {
            if tab == Tab::Actions {
                self.actions_page = if self
                    .editor
                    .profile
                    .loot
                    .validate(&self.editor.profile)
                    .is_err()
                {
                    ActionsPage::Loot
                } else if self.editor.profile.buffs.enabled && self.declared_mount().is_none() {
                    ActionsPage::Mount
                } else if self.editor.profile.buffs.validate().is_err()
                    || !self.editor.profile.buffs.ready()
                {
                    ActionsPage::Buffs
                } else if let Err(issue) =
                    metin_core::validate_scheduled_setup(&self.editor.profile)
                {
                    scheduled_page(issue.group)
                } else {
                    self.actions_page
                };
            }
            if tab == Tab::Settings && !self.snapshot.capabilities.automation_ready() {
                self.settings_page = SettingsPage::Diagnostics;
            }
            self.gesture = None;
            self.editing = false;
            self.binding = None;
            self.hover_mode = None;
            self.tab = tab;
        }
    }
    pub(super) fn edit_area(&mut self, mode: EditMode) {
        self.runtime.stop();
        self.gesture = None;
        self.binding = None;
        self.live = false;
        self.return_to_buffs = matches!(mode, EditMode::BuffArea | EditMode::BuffIcon { .. });
        self.return_to_safety = matches!(
            mode,
            EditMode::SafetyReference(_) | EditMode::VisualClick(_) | EditMode::PeriodicClick(_)
        );
        self.mode = mode;
        self.editing = true;
        self.tab = Tab::Calibration;
    }
    pub(super) fn reorder_skill(&mut self, a: usize, b: usize) {
        if a >= self.editor.profile.buffs.skills.len()
            || b >= self.editor.profile.buffs.skills.len()
        {
            return;
        }
        self.runtime.stop();
        self.editor.profile.buffs.skills.swap(a, b);
        self.units.swap(a, b);
        if self.selected_skill == a {
            self.selected_skill = b;
        } else if self.selected_skill == b {
            self.selected_skill = a;
        }
        self.retire_skill_gesture();
        self.editor.record();
    }
    pub(super) fn remove_skill(&mut self, index: usize) {
        if index >= self.editor.profile.buffs.skills.len() {
            return;
        }
        self.runtime.stop();
        self.editor.profile.buffs.skills.remove(index);
        self.units.remove(index);
        self.units.push(false);
        if index < self.selected_skill {
            self.selected_skill -= 1;
        }
        self.selected_skill = self
            .selected_skill
            .min(self.editor.profile.buffs.skills.len().saturating_sub(1));
        self.retire_skill_gesture();
        self.editor.record();
    }
    fn retire_skill_gesture(&mut self) {
        self.binding = None;
        self.gesture = None;
        if matches!(self.mode, EditMode::BuffIcon { .. }) {
            self.mode = EditMode::World;
            self.editing = false;
            self.return_to_buffs = false;
        }
    }
    pub(super) fn add_skill(&mut self) {
        if self.editor.profile.buffs.skills.len() >= 8 {
            return;
        }
        self.runtime.stop();
        let mut skill = metin_core::BuffSkill::default();
        for n in [1, 2, 3, 4, 5, 6, 9, 10, 11, 12] {
            let key = format!("F{n}");
            if key != self.editor.profile.buffs.mount_shortcut
                && !self
                    .editor
                    .profile
                    .buffs
                    .skills
                    .iter()
                    .any(|s| s.shortcut == key)
            {
                skill.shortcut = key;
                break;
            }
        }
        self.selected_skill = self.editor.profile.buffs.skills.len();
        self.editor.profile.buffs.skills.push(skill);
        self.editor.record();
    }
}

fn scheduled_page(group: metin_core::ScheduledGroup) -> ActionsPage {
    use metin_core::ScheduledGroup as G;
    match group {
        G::Buffs => ActionsPage::Buffs,
        G::Loot => ActionsPage::Loot,
        G::Periodic => ActionsPage::Periodic,
        G::Puller => ActionsPage::Puller,
        G::OutOfTarget => ActionsPage::OutOfTarget,
    }
}
