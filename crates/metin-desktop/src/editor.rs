mod recalibration;
use base64::{Engine, engine::general_purpose::STANDARD};
use metin_core::{ActionInput, BuffIcon, ColorConfig, Profile, Region};
use metin_platform::Frame;
pub use recalibration::ReviewRegion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    World,
    Mask(Option<usize>),
    Color,
    Template,
    BuffArea,
    BuffIcon { index: usize, inactive: bool },
    SafetyReference(usize),
    VisualClick(usize),
    PeriodicClick(uuid::Uuid),
}
#[derive(Clone, PartialEq)]
struct Snapshot {
    profile: Profile,
    template: Option<Frame>,
}
pub struct Editor {
    pub profile: Profile,
    pub template: Option<Frame>,
    pub frozen: Option<Frame>,
    history: Vec<Snapshot>,
    index: usize,
    saved: Snapshot,
    review_base: Option<Box<Editor>>,
    change_id: u64,
}
type Result<T> = std::result::Result<T, String>;
impl Editor {
    pub fn new(profile: Profile, template: Option<Frame>) -> Self {
        let saved = Snapshot {
            profile: profile.clone(),
            template: template.clone(),
        };
        Self {
            profile,
            template,
            frozen: None,
            history: vec![saved.clone()],
            index: 0,
            saved,
            review_base: None,
            change_id: 0,
        }
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            profile: self.profile.clone(),
            template: self.template.clone(),
        }
    }
    pub fn dirty(&self) -> bool {
        self.profile != self.saved.profile || self.template != self.saved.template
    }
    pub fn mark_saved(&mut self, profile: Profile) {
        self.profile = profile;
        self.saved = self.snapshot();
        self.record();
    }
    /// Restore the last successfully saved transaction; retains undo history.
    pub fn revert(&mut self) {
        self.profile = self.saved.profile.clone();
        self.template = self.saved.template.clone();
        if self
            .frozen
            .as_ref()
            .is_some_and(|frame| self.profile.frame_size != Some([frame.width, frame.height]))
        {
            self.frozen = None;
        }
        self.record();
    }
    pub fn record(&mut self) {
        let state = self.snapshot();
        if self.history.get(self.index) == Some(&state) {
            return;
        }
        self.history.truncate(self.index + 1);
        self.history.push(state);
        if self.history.len() > 32 {
            self.history.remove(0);
        }
        self.index = self.history.len() - 1;
        self.change_id = self.change_id.wrapping_add(1);
    }
    /// Identifies the current history operation, including undo/redo/branch edits.
    pub fn change_id(&self) -> u64 {
        self.change_id
    }
    pub fn undo(&mut self) {
        if self.index > 0 {
            self.index -= 1;
            self.restore();
        }
    }
    pub fn redo(&mut self) {
        if self.index + 1 < self.history.len() {
            self.index += 1;
            self.restore();
        }
    }
    fn restore(&mut self) {
        self.change_id = self.change_id.wrapping_add(1);
        let s = self.history[self.index].clone();
        self.profile = s.profile;
        self.template = s.template;
        if self
            .frozen
            .as_ref()
            .is_some_and(|f| self.profile.frame_size != Some([f.width, f.height]))
        {
            self.frozen = None;
        }
    }
    pub fn can_undo(&self) -> bool {
        self.index > 0
    }
    pub fn can_redo(&self) -> bool {
        self.index + 1 < self.history.len()
    }
    /// Attach source pixels without ever retiring calibration implicitly.
    pub fn freeze(&mut self, frame: Frame) -> Result<()> {
        let size = [frame.width, frame.height];
        if let Some(previous) = self.profile.frame_size
            && previous != size
        {
            return Err(format!(
                "Game size changed from {} × {} to {} × {} pixels. Calibration was kept. Restore the original game size or create a new profile for this resolution.",
                previous[0], previous[1], size[0], size[1]
            ));
        }
        self.profile.frame_size = Some(size);
        self.frozen = Some(frame);
        self.record();
        Ok(())
    }
    /// Native selection is a draft until explicit confirmation. Dropping it is cancel.
    pub fn begin_capture(&self, mode: EditMode, frame: Frame) -> Result<CaptureEdit> {
        let mut draft = Self::new(self.profile.clone(), self.template.clone());
        draft.freeze(frame)?;
        Ok(CaptureEdit {
            reviewing: self.is_recalibrating(),
            base: self.snapshot(),
            draft,
            mode,
        })
    }
    pub fn region(&self, mode: EditMode) -> Option<Region> {
        match mode {
            EditMode::VisualClick(index) => {
                self.profile
                    .safety
                    .rules
                    .get(index)
                    .and_then(|r| match r.execution.input {
                        ActionInput::Click { region } => region,
                        _ => None,
                    })
            }
            EditMode::PeriodicClick(id) => self
                .profile
                .periodic_actions
                .iter()
                .find(|a| a.id == id)
                .and_then(|a| match a.input {
                    ActionInput::Click { region } => region,
                    _ => None,
                }),
            EditMode::SafetyReference(index) => self
                .profile
                .safety
                .rules
                .get(index)
                .and_then(|r| r.reference.as_ref())
                .and_then(|r| r.region),
            EditMode::World => self.profile.region,
            EditMode::Mask(Some(i)) => self.profile.masks.get(i).copied(),
            EditMode::Template => self.profile.template_region,
            EditMode::BuffArea => self.profile.buffs.icon_region,
            EditMode::BuffIcon { index, inactive } => self
                .profile
                .buffs
                .skills
                .get(index)
                .and_then(|s| {
                    if inactive {
                        s.inactive_icon.as_ref()
                    } else {
                        s.icon.as_ref()
                    }
                })
                .and_then(|i| i.region),
            _ => None,
        }
    }
    pub fn color(&mut self, point: [u32; 2]) -> Result<()> {
        let frame = self
            .frozen
            .as_ref()
            .ok_or("Take a Snapshot before calibration.")?;
        let hsv = metin_vision::pick_color(frame, point).map_err(|e| e.to_string())?;
        self.profile.color = Some(ColorConfig {
            hsv,
            ..self.profile.color.clone().unwrap_or_default()
        });
        self.record();
        Ok(())
    }
    pub fn set_region(&mut self, mode: EditMode, region: Region) -> Result<()> {
        self.set_region_checked(mode, region, !self.is_recalibrating())
    }
    fn set_region_checked(
        &mut self,
        mode: EditMode,
        region: Region,
        validate_profile: bool,
    ) -> Result<()> {
        region.validate().map_err(|e| e.to_string())?;
        let frame = self
            .frozen
            .as_ref()
            .ok_or("Take a Snapshot before editing.")?;
        let mut profile = self.profile.clone();
        let mut template = self.template.clone();
        match mode {
            EditMode::VisualClick(index) => {
                profile
                    .safety
                    .rules
                    .get_mut(index)
                    .ok_or("Visual rule no longer exists.")?
                    .execution
                    .input = ActionInput::Click {
                    region: Some(region),
                };
            }
            EditMode::PeriodicClick(id) => {
                profile
                    .periodic_actions
                    .iter_mut()
                    .find(|a| a.id == id)
                    .ok_or("Periodic action no longer exists.")?
                    .input = ActionInput::Click {
                    region: Some(region),
                };
            }
            EditMode::SafetyReference(index) => {
                let image = metin_vision::crop(frame, region).map_err(|e| e.to_string())?;
                metin_vision::validate_template(&image).map_err(|e| e.to_string())?;
                let icon = BuffIcon {
                    png: STANDARD
                        .encode(metin_vision::encode_png(&image).map_err(|e| e.to_string())?),
                    region: Some(region),
                };
                icon.png_bytes().map_err(|e| e.to_string())?;
                profile
                    .safety
                    .rules
                    .get_mut(index)
                    .ok_or("Visual rule no longer exists.")?
                    .reference = Some(icon);
            }
            EditMode::World => profile.region = Some(region),
            EditMode::Mask(index) => {
                if let Some(i) = index {
                    let slot = profile
                        .masks
                        .get_mut(i)
                        .ok_or("The mask no longer exists.")?;
                    *slot = region;
                } else {
                    profile.masks.push(region);
                }
            }
            EditMode::Template => {
                let image = metin_vision::crop(frame, region).map_err(|e| e.to_string())?;
                metin_vision::validate_template(&image).map_err(|e| e.to_string())?;
                template = Some(image);
                profile.template_region = Some(region);
                profile.template_file = Some("pending.png".into());
            }
            EditMode::BuffArea => profile.buffs.icon_region = Some(region),
            EditMode::BuffIcon { index, inactive } => {
                let image = metin_vision::crop(frame, region).map_err(|e| e.to_string())?;
                metin_vision::validate_template(&image).map_err(|e| e.to_string())?;
                if image.width > 128 || image.height > 128 {
                    return Err("Select an icon of 5–128 pixels.".into());
                }
                let bytes = metin_vision::encode_png(&image).map_err(|e| e.to_string())?;
                let skill = profile
                    .buffs
                    .skills
                    .get_mut(index)
                    .ok_or("The skill no longer exists.")?;
                let other = if inactive {
                    &skill.icon
                } else {
                    &skill.inactive_icon
                };
                if let Some(other) = other {
                    let other_image =
                        metin_vision::decode_png(&other.png_bytes().map_err(|e| e.to_string())?)
                            .map_err(|e| e.to_string())?;
                    if image == other_image {
                        return Err("Active and inactive appearances must be different.".into());
                    }
                }
                let icon = BuffIcon {
                    png: STANDARD.encode(bytes),
                    region: Some(region),
                };
                if inactive {
                    skill.inactive_icon = Some(icon);
                } else {
                    skill.icon = Some(icon);
                }
                if !profile
                    .buffs
                    .icon_region
                    .is_some_and(|area| area.contains(region))
                {
                    let padded = Region {
                        left: (region.left - 6. / f64::from(frame.width)).max(0.),
                        top: (region.top - 6. / f64::from(frame.height)).max(0.),
                        right: (region.right + 6. / f64::from(frame.width)).min(1.),
                        bottom: (region.bottom + 6. / f64::from(frame.height)).min(1.),
                    };
                    profile.buffs.icon_region =
                        Some(profile.buffs.icon_region.map_or(padded, |old| Region {
                            left: old.left.min(padded.left),
                            top: old.top.min(padded.top),
                            right: old.right.max(padded.right),
                            bottom: old.bottom.max(padded.bottom),
                        }));
                }
            }
            EditMode::Color => return Err("Pick a color with a click.".into()),
        }
        if validate_profile {
            profile.validate().map_err(|e| e.to_string())?;
        }
        self.profile = profile;
        self.template = template;
        self.record();
        Ok(())
    }
    pub fn delete(&mut self, mode: EditMode) -> Result<()> {
        match mode {
            EditMode::VisualClick(index) => {
                if let Some(rule) = self.profile.safety.rules.get_mut(index) {
                    rule.execution.input = ActionInput::Click { region: None };
                    rule.enabled = false;
                }
            }
            EditMode::PeriodicClick(id) => {
                if let Some(action) = self
                    .profile
                    .periodic_actions
                    .iter_mut()
                    .find(|a| a.id == id)
                {
                    action.input = ActionInput::Click { region: None };
                    action.enabled = false;
                }
            }
            EditMode::SafetyReference(index) => {
                if let Some(rule) = self.profile.safety.rules.get_mut(index) {
                    rule.reference = None;
                    rule.enabled = false;
                }
            }
            EditMode::World => self.profile.region = None,
            EditMode::Mask(Some(i)) => {
                if i < self.profile.masks.len() {
                    self.profile.masks.remove(i);
                }
            }
            EditMode::Color => self.profile.color = None,
            EditMode::Template => {
                self.profile.template_region = None;
                self.profile.template_file = None;
                self.template = None;
            }
            EditMode::BuffArea => self.profile.buffs.icon_region = None,
            EditMode::BuffIcon { index, inactive } => {
                if let Some(s) = self.profile.buffs.skills.get_mut(index) {
                    if inactive {
                        s.inactive_icon = None;
                    } else {
                        s.icon = None;
                    }
                }
            }
            _ => {}
        }
        self.record();
        Ok(())
    }
}

/// A single frozen source, the previous calibration and a private editing draft.
/// No caller can change the live editor merely by opening or closing the selector.
pub struct CaptureEdit {
    reviewing: bool,
    base: Snapshot,
    draft: Editor,
    mode: EditMode,
}
impl CaptureEdit {
    pub fn current(&self, editor: &Editor) -> bool {
        self.reviewing == editor.is_recalibrating() && self.base == editor.snapshot()
    }
    pub fn frame(&self) -> &Frame {
        self.draft
            .frozen
            .as_ref()
            .expect("capture has source pixels")
    }
    pub fn profile(&self) -> &Profile {
        &self.draft.profile
    }
    pub fn initial_region(&self) -> Option<Region> {
        let region = self.draft.region(self.mode)?;
        if region.validate().is_ok() {
            return Some(region);
        }
        // Visible selection seed only. The private draft retains the invalid
        // original until the user explicitly confirms a repaired rectangle.
        let clipped = Region {
            left: region.left.clamp(0., 1.),
            top: region.top.clamp(0., 1.),
            right: region.right.clamp(0., 1.),
            bottom: region.bottom.clamp(0., 1.),
        };
        clipped.validate().ok().map(|()| clipped)
    }
    pub fn commit(
        mut self,
        editor: &mut Editor,
        region: Option<Region>,
        point: [u32; 2],
    ) -> Result<()> {
        if !self.current(editor) {
            return Err("The profile changed during selection. Open the selection again.".into());
        }
        if self.mode == EditMode::Color {
            self.draft.color(point)?;
        } else {
            self.draft.set_region_checked(
                self.mode,
                region.ok_or("Draw a non-empty area.")?,
                !editor.is_recalibrating(),
            )?;
        }
        editor.profile = self.draft.profile;
        editor.template = self.draft.template;
        editor.frozen = self.draft.frozen;
        editor.record();
        Ok(())
    }
}
