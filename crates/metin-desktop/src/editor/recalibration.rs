//! An explicit size review is a private editor draft until confirmed.
use super::*;

#[derive(Debug, Clone)]
pub struct ReviewRegion {
    pub mode: EditMode,
    pub label: String,
    pub region: Region,
}

impl Editor {
    pub fn is_recalibrating(&self) -> bool {
        self.review_base.is_some()
    }
    pub fn reviewing_dimensions(&self) -> Option<([u32; 2], [u32; 2])> {
        Some((
            self.review_base.as_ref()?.profile.frame_size?,
            self.profile.frame_size?,
        ))
    }
    pub fn begin_size_review(&mut self, frame: Frame) -> Result<()> {
        if self.is_recalibrating() {
            return Err("Finish or cancel the current calibration review first.".into());
        }
        let previous = self
            .profile
            .frame_size
            .ok_or("There are no saved calibration dimensions to review.")?;
        let current = [frame.width, frame.height];
        if previous == current {
            return Err("The game dimensions have not changed.".into());
        }
        let mut profile = self.profile.clone();
        // Convert the old inclusive physical endpoints, never scale to the new layout.
        remap_regions(&mut profile, previous, current);
        profile.frame_size = Some(current);
        let mut draft = Editor::new(profile, self.template.clone());
        draft.saved = self.saved.clone();
        draft.frozen = Some(frame);
        let original = std::mem::replace(self, draft);
        self.review_base = Some(Box::new(original));
        Ok(())
    }
    pub fn cancel_size_review(&mut self) {
        if let Some(original) = self.review_base.take() {
            *self = *original;
        }
    }
    pub fn finish_size_review(&mut self) -> Result<()> {
        if !self.is_recalibrating() {
            return Err("There is no calibration review to confirm.".into());
        }
        if !self.review_issues().is_empty() {
            return Err("Some calibration areas are outside the new image. Edit or remove the highlighted areas before confirming.".into());
        }
        self.profile.validate().map_err(|e| e.to_string())?;
        let mut original = *self.review_base.take().expect("checked review");
        original.profile = self.profile.clone();
        original.template = self.template.clone();
        original.frozen = self.frozen.clone();
        original.record();
        *self = original;
        Ok(())
    }
    pub fn review_regions(&self) -> Vec<ReviewRegion> {
        let mut modes = vec![EditMode::World, EditMode::Template, EditMode::BuffArea];
        modes.extend((0..self.profile.masks.len()).map(|i| EditMode::Mask(Some(i))));
        for index in 0..self.profile.buffs.skills.len() {
            modes.push(EditMode::BuffIcon {
                index,
                inactive: false,
            });
            modes.push(EditMode::BuffIcon {
                index,
                inactive: true,
            });
        }
        for index in 0..self.profile.safety.rules.len() {
            modes.push(EditMode::SafetyReference(index));
            modes.push(EditMode::VisualClick(index));
        }
        modes.extend(
            self.profile
                .periodic_actions
                .iter()
                .map(|a| EditMode::PeriodicClick(a.id)),
        );
        modes
            .into_iter()
            .filter_map(|mode| {
                self.region(mode).map(|region| ReviewRegion {
                    mode,
                    region,
                    label: match mode {
                        EditMode::World => "Search area".into(),
                        EditMode::Template => "Target HP detail".into(),
                        EditMode::BuffArea => "Buff detection area".into(),
                        EditMode::Mask(Some(i)) => self.profile.masks[i].name.clone(),
                        EditMode::BuffIcon { index, inactive } => format!(
                            "Skill {} {} icon",
                            index + 1,
                            if inactive { "Inactive" } else { "Active" }
                        ),
                        EditMode::SafetyReference(i) => format!("Visual rule {} reference", i + 1),
                        EditMode::VisualClick(i) => format!("Visual rule {} click target", i + 1),
                        EditMode::PeriodicClick(id) => format!(
                            "Periodic action: {}",
                            self.profile
                                .periodic_actions
                                .iter()
                                .find(|a| a.id == id)
                                .map_or("Action", |a| a.name.as_str())
                        ),
                        _ => unreachable!("only region modes are listed"),
                    },
                })
            })
            .collect()
    }
    pub fn review_issues(&self) -> Vec<ReviewRegion> {
        if !self.is_recalibrating() {
            return vec![];
        }
        self.review_regions()
            .into_iter()
            .filter(|r| r.region.validate().is_err())
            .collect()
    }
}

fn remap_regions(profile: &mut Profile, old: [u32; 2], new: [u32; 2]) {
    let coordinate = |pixel: u32, size: u32| {
        let normal = f64::from(pixel) / f64::from(size);
        // Region accepts 1.0 as an inclusive frame edge, but an OLD pixel
        // equal to the NEW size is already outside. Keep it invalid in the
        // draft rather than silently clamping it to size-1.
        if pixel >= size {
            normal.max(1. + f64::EPSILON)
        } else {
            normal
        }
    };
    let convert = |slot: &mut Region| {
        let [l, t, r, b] = slot.pixels(old[0], old[1]);
        *slot = Region {
            left: coordinate(l, new[0]),
            top: coordinate(t, new[1]),
            right: coordinate(r, new[0]),
            bottom: coordinate(b, new[1]),
        };
    };
    for slot in [
        &mut profile.region,
        &mut profile.template_region,
        &mut profile.buffs.icon_region,
    ]
    .into_iter()
    .flatten()
    {
        convert(slot);
    }
    for slot in &mut profile.masks {
        convert(&mut slot.region);
    }
    for skill in &mut profile.buffs.skills {
        for icon in [&mut skill.icon, &mut skill.inactive_icon]
            .into_iter()
            .flatten()
        {
            if let Some(slot) = &mut icon.region {
                convert(slot);
            }
        }
    }
    for rule in &mut profile.safety.rules {
        if let Some(reference) = &mut rule.reference
            && let Some(slot) = &mut reference.region
        {
            convert(slot);
        }
        if let ActionInput::Click { region: Some(slot) } = &mut rule.execution.input {
            convert(slot);
        }
    }
    for action in &mut profile.periodic_actions {
        if let ActionInput::Click { region: Some(slot) } = &mut action.input {
            convert(slot);
        }
    }
}
