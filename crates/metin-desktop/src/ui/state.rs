use super::*;
pub(super) fn matches_process(filename: &str, hint: &str) -> bool {
    filename.eq_ignore_ascii_case(hint)
}

pub(super) struct Blocker {
    pub message: String,
    pub tab: Tab,
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
        self.runtime.stop();
        self.gesture = None;
        if let Err(error) = self.editor.delete(self.mode) {
            self.error = Some(error);
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
        self.live = false;
        self.texture_source = None;
        self.gesture = None;
        self.binding = None;
        self.units.fill(false);
        self.selected_skill = 0;
        self.return_to_buffs = false;
        self.snapshot_requested = None;
        self.mode = EditMode::World;
        self.editing = false;
        self.calibration_controls = true;
        self.viewport.reset();
        self.detection_cache = None;
        self.icon_textures.clear();
    }
    pub(super) fn calibration_ready(&self, mode: EditMode) -> bool {
        match mode {
            EditMode::Template => self.editor.template.is_some(),
            EditMode::Color => self.editor.profile.color.is_some(),
            _ => self.editor.region(mode).is_some(),
        }
    }
    pub(super) fn start_blocker(&self) -> Option<Blocker> {
        let block = |message: &str, tab| {
            Some(Blocker {
                message: message.into(),
                tab,
            })
        };
        if !self.snapshot.capabilities.automation_ready() {
            return block(
                if self.snapshot.capabilities.reason.is_empty() {
                    "Checking platform capture and input support."
                } else {
                    &self.snapshot.capabilities.reason
                },
                Tab::Settings,
            );
        }
        if self.target.is_none() {
            return block(
                "Select a game window to connect this profile.",
                Tab::Calibration,
            );
        }
        if self.editor.profile.name.trim().is_empty() {
            return block("Enter a profile name in Calibration.", Tab::Calibration);
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
                return block(label, Tab::Calibration);
            }
        }
        let profile = self.profile_for_save();
        if let Err(error) = profile.buffs.validate() {
            return block(&error.to_string(), Tab::Buffs);
        }
        if !profile.buffs.ready() {
            return block(
                "Finish buff recognition: set its area and the active icon for each toggle skill.",
                Tab::Buffs,
            );
        }
        if let Err(error) = profile.validate() {
            return block(
                &error.to_string(),
                if error.to_string().to_lowercase().contains("skill") {
                    Tab::Buffs
                } else {
                    Tab::Settings
                },
            );
        }
        if !profile.ready() {
            return block(
                "Take a calibrated snapshot and save the profile.",
                Tab::Calibration,
            );
        }
        None
    }
    pub(super) fn navigate(&mut self, tab: Tab) {
        if self.tab != tab {
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
        self.calibration_controls = true;
        self.return_to_buffs = matches!(mode, EditMode::BuffArea | EditMode::BuffIcon { .. });
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
