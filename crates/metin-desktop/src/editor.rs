use base64::{Engine, engine::general_purpose::STANDARD};
use metin_core::{ActionInput, BuffIcon, ColorConfig, PanelConfig, Profile, Region};
use metin_platform::Frame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    World,
    Mask(Option<usize>),
    Color,
    Panel,
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
        let s = self.history[self.index].clone();
        self.profile = s.profile;
        self.template = s.template;
    }
    pub fn can_undo(&self) -> bool {
        self.index > 0
    }
    pub fn can_redo(&self) -> bool {
        self.index + 1 < self.history.len()
    }
    pub fn freeze(&mut self, frame: Frame) -> Result<()> {
        let size = [frame.width, frame.height];
        let size_changed = self.profile.frame_size != Some(size);
        if self.profile.frame_size.is_some_and(|s| s != size) {
            self.profile.region = None;
            self.profile.masks.clear();
            self.profile.color = None;
            self.profile.panel = PanelConfig::default();
            self.profile.template_file = None;
            self.profile.template_region = None;
            self.profile.buffs.icon_region = None;
            for s in &mut self.profile.buffs.skills {
                s.icon = None;
                s.inactive_icon = None;
            }
            self.template = None;
            for action in &mut self.profile.periodic_actions {
                if let ActionInput::Click { region } = &mut action.input {
                    *region = None;
                    action.enabled = false;
                }
            }
            for rule in &mut self.profile.safety.rules {
                if let ActionInput::Click { region } = &mut rule.execution.input {
                    *region = None;
                }
                rule.reference = None;
                rule.enabled = false;
            }
        }
        self.profile.frame_size = Some(size);
        self.frozen = Some(frame);
        if size_changed {
            self.history = vec![self.snapshot()];
            self.index = 0;
        } else {
            self.record();
        }
        Ok(())
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
            EditMode::Panel => self.profile.panel.region,
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
            EditMode::Panel => {
                if !profile.template_region.is_some_and(|r| region.contains(r)) {
                    profile.template_file = None;
                    profile.template_region = None;
                    template = None;
                }
                profile.panel.region = Some(region);
            }
            EditMode::Template => {
                if !profile.panel.region.is_some_and(|r| r.contains(region)) {
                    return Err("The fixed crop must be inside the target panel area.".into());
                }
                let image = metin_vision::crop(frame, region).map_err(|e| e.to_string())?;
                metin_vision::validate_template(&image).map_err(|e| e.to_string())?;
                template = Some(image);
                profile.template_region = Some(region);
                profile.template_file = Some("pending.png".into());
            }
            EditMode::BuffArea => profile.buffs.icon_region = Some(region),
            EditMode::BuffIcon { index, inactive } => {
                if !profile
                    .buffs
                    .icon_region
                    .is_some_and(|r| r.contains(region))
                {
                    return Err("Select the icon inside the buff area.".into());
                }
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
            }
            EditMode::Color => return Err("Pick a color with a click.".into()),
        }
        profile.validate().map_err(|e| e.to_string())?;
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
            EditMode::Panel => {
                self.profile.panel.region = None;
                self.profile.template_region = None;
                self.profile.template_file = None;
                self.template = None;
            }
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

#[derive(Debug, Clone)]
pub struct Viewport {
    pub size: [u32; 2],
    pub canvas: [f64; 2],
    pub zoom: f64,
    pub offset: [f64; 2],
}
impl Default for Viewport {
    fn default() -> Self {
        Self {
            size: [1, 1],
            canvas: [1., 1.],
            zoom: 1.,
            offset: [0., 0.],
        }
    }
}
impl Viewport {
    pub fn fit(&mut self, size: [u32; 2], canvas: [f64; 2]) {
        if size != self.size {
            self.reset();
        }
        self.size = size;
        self.canvas = canvas;
    }
    pub fn reset(&mut self) {
        self.zoom = 1.;
        self.offset = [0., 0.];
    }
    pub fn scale(&self) -> f64 {
        (self.canvas[0] / f64::from(self.size[0])).min(self.canvas[1] / f64::from(self.size[1]))
            * self.zoom
    }
    pub fn to_canvas(&self, p: [f64; 2]) -> [f64; 2] {
        [
            p[0] * self.scale() + self.offset[0],
            p[1] * self.scale() + self.offset[1],
        ]
    }
    pub fn to_image(&self, p: [f64; 2], clamp: bool) -> Option<[u32; 2]> {
        let x = (p[0] - self.offset[0]) / self.scale();
        let y = (p[1] - self.offset[1]) / self.scale();
        if !clamp
            && (x < 0. || y < 0. || x >= f64::from(self.size[0]) || y >= f64::from(self.size[1]))
        {
            return None;
        }
        Some([
            (x + 1e-8).floor().clamp(0., f64::from(self.size[0] - 1)) as u32,
            (y + 1e-8).floor().clamp(0., f64::from(self.size[1] - 1)) as u32,
        ])
    }
    pub fn zoom_at(&mut self, p: [f64; 2], factor: f64) {
        let old = self.scale();
        let anchored = [(p[0] - self.offset[0]) / old, (p[1] - self.offset[1]) / old];
        self.zoom = (self.zoom * factor).clamp(1., 8.);
        let scale = self.scale();
        self.offset = [p[0] - anchored[0] * scale, p[1] - anchored[1] * scale];
        if self.zoom == 1. {
            self.offset = [0., 0.];
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Gesture {
    Create {
        start: [u32; 2],
    },
    Move {
        start: [u32; 2],
        original: Region,
    },
    Resize {
        start: [u32; 2],
        original: Region,
        handle: [i8; 2],
    },
}
impl Gesture {
    pub fn region(self, end: [u32; 2], size: [u32; 2]) -> Option<Region> {
        let normalized = |p: [u32; 2]| {
            [
                f64::from(p[0]) / f64::from(size[0]),
                f64::from(p[1]) / f64::from(size[1]),
            ]
        };
        let end = normalized(end);
        let r = match self {
            Self::Create { start } => {
                let start = normalized(start);
                Region {
                    left: start[0].min(end[0]),
                    top: start[1].min(end[1]),
                    right: start[0].max(end[0]),
                    bottom: start[1].max(end[1]),
                }
            }
            Self::Move { start, original: r } => {
                let start = normalized(start);
                let dx = (end[0] - start[0]).clamp(-r.left, 1. - r.right);
                let dy = (end[1] - start[1]).clamp(-r.top, 1. - r.bottom);
                Region {
                    left: r.left + dx,
                    right: r.right + dx,
                    top: r.top + dy,
                    bottom: r.bottom + dy,
                }
            }
            Self::Resize {
                start,
                original: mut r,
                handle,
            } => {
                let start = normalized(start);
                let delta = [end[0] - start[0], end[1] - start[1]];
                if handle[0] < 0 {
                    r.left = (r.left + delta[0]).clamp(0., 1.);
                }
                if handle[0] > 0 {
                    r.right = (r.right + delta[0]).clamp(0., 1.);
                }
                if handle[1] < 0 {
                    r.top = (r.top + delta[1]).clamp(0., 1.);
                }
                if handle[1] > 0 {
                    r.bottom = (r.bottom + delta[1]).clamp(0., 1.);
                }
                Region {
                    left: r.left.min(r.right),
                    right: r.left.max(r.right),
                    top: r.top.min(r.bottom),
                    bottom: r.top.max(r.bottom),
                }
            }
        };
        r.validate().ok().map(|()| r)
    }
}
