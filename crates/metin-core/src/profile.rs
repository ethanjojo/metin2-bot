use crate::{Region, Result, Shortcut, ValidationError, require, validation::number};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ColorConfig {
    pub hsv: [u16; 3],
    pub hue_tolerance: u32,
    pub saturation_tolerance: u32,
    pub value_tolerance: u32,
    pub min_area: u32,
    pub min_width: u32,
    pub min_height: u32,
    pub max_area_ratio: f64,
    pub stable_frames: u32,
}
impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            hsv: [0, 200, 200],
            hue_tolerance: 10,
            saturation_tolerance: 65,
            value_tolerance: 65,
            min_area: 120,
            min_width: 12,
            min_height: 18,
            max_area_ratio: 0.03,
            stable_frames: 3,
        }
    }
}
impl ColorConfig {
    pub fn validate(&self) -> Result<()> {
        require(
            self.hsv[0] <= 179 && self.hsv[1] <= 255 && self.hsv[2] <= 255,
            "Invalid HSV color.",
        )?;
        for (name, v) in [
            ("hue_tolerance", self.hue_tolerance),
            ("saturation_tolerance", self.saturation_tolerance),
            ("value_tolerance", self.value_tolerance),
            ("min_area", self.min_area),
            ("min_width", self.min_width),
            ("min_height", self.min_height),
            ("stable_frames", self.stable_frames),
        ] {
            number(name, v.into())?;
        }
        number("max_area_ratio", self.max_area_ratio)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PanelConfig {
    pub region: Option<Region>,
    pub threshold: f64,
    pub stable_frames: u32,
}
impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            region: None,
            threshold: 0.88,
            stable_frames: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TimingConfig {
    pub click_hover_seconds: f64,
    pub click_hold_seconds: f64,
    pub sample_interval: f64,
    pub settle_seconds: f64,
    pub search_timeout: f64,
    pub acquire_timeout: f64,
    pub attack_timeout: f64,
    pub max_rotations: u32,
    pub rotate_seconds: f64,
    pub failed_cooldown: f64,
}
impl Default for TimingConfig {
    fn default() -> Self {
        Self {
            click_hover_seconds: 0.15,
            click_hold_seconds: 0.2,
            sample_interval: 0.25,
            settle_seconds: 1.,
            search_timeout: 5.,
            acquire_timeout: 20.,
            attack_timeout: 120.,
            max_rotations: 4,
            rotate_seconds: 0.35,
            failed_cooldown: 10.,
        }
    }
}
impl TimingConfig {
    pub fn validate(&self) -> Result<()> {
        for (n, v) in [
            ("click_hover_seconds", self.click_hover_seconds),
            ("click_hold_seconds", self.click_hold_seconds),
            ("sample_interval", self.sample_interval),
            ("settle_seconds", self.settle_seconds),
            ("search_timeout", self.search_timeout),
            ("acquire_timeout", self.acquire_timeout),
            ("attack_timeout", self.attack_timeout),
            ("max_rotations", self.max_rotations.into()),
            ("rotate_seconds", self.rotate_seconds),
            ("failed_cooldown", self.failed_cooldown),
        ] {
            number(n, v)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuffIcon {
    pub png: String,
    #[serde(default)]
    pub region: Option<Region>,
}
impl BuffIcon {
    pub fn png_bytes(&self) -> Result<Vec<u8>> {
        require(self.png.len() <= 100000, "Buff icon is too large.")?;
        let bytes = STANDARD
            .decode(&self.png)
            .map_err(|_| ValidationError("Invalid Base64 icon.".into()))?;
        require(
            bytes.len() >= 24 && bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            "The icon must be a PNG image.",
        )?;
        for offset in [16, 20] {
            let n = u32::from_be_bytes(
                bytes[offset..offset + 4]
                    .try_into()
                    .expect("checked header"),
            );
            require(
                (5..=128).contains(&n),
                "Buff icon dimensions must be 5–128 pixels.",
            )?;
        }
        if let Some(r) = self.region {
            r.validate()?;
        }
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BuffMode {
    #[default]
    Refresh,
    Inactive,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BuffSkill {
    pub shortcut: String,
    pub wait_seconds: f64,
    pub mode: BuffMode,
    pub icon: Option<BuffIcon>,
    pub inactive_icon: Option<BuffIcon>,
    pub interval_seconds: Option<f64>,
    pub name: String,
}
impl Default for BuffSkill {
    fn default() -> Self {
        Self {
            shortcut: "F1".into(),
            wait_seconds: 1.5,
            mode: BuffMode::Refresh,
            icon: None,
            inactive_icon: None,
            interval_seconds: Some(120.),
            name: String::new(),
        }
    }
}
impl BuffSkill {
    pub fn validate(&self) -> Result<()> {
        Shortcut::parse(&self.shortcut)?;
        number("wait_seconds", self.wait_seconds)?;
        require(
            self.name.chars().count() <= 60 && !self.name.chars().any(|c| (c as u32) < 32),
            "Skill name must be at most 60 characters without control characters.",
        )?;
        match self.mode {
            BuffMode::Refresh => number(
                "interval_seconds",
                self.interval_seconds
                    .ok_or_else(|| ValidationError("Regular skill needs an interval.".into()))?,
            )?,
            BuffMode::Inactive => require(
                self.interval_seconds.is_none(),
                "Toggle skills do not have intervals.",
            )?,
        };
        for icon in [&self.icon, &self.inactive_icon].into_iter().flatten() {
            icon.png_bytes()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BuffConfig {
    pub enabled: bool,
    pub fight_mounted: bool,
    pub mount_shortcut: String,
    pub mount_wait_seconds: f64,
    pub key_hold_seconds: f64,
    pub skills: Vec<BuffSkill>,
    pub icon_region: Option<Region>,
}
impl Default for BuffConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            fight_mounted: true,
            mount_shortcut: "Ctrl+G".into(),
            mount_wait_seconds: 1.5,
            key_hold_seconds: 0.15,
            skills: vec![],
            icon_region: None,
        }
    }
}
impl BuffConfig {
    pub fn ready(&self) -> bool {
        !self.enabled
            || self.skills.iter().all(|s| {
                s.mode == BuffMode::Refresh || (s.icon.is_some() && self.icon_region.is_some())
            })
    }
    pub fn execution_eq(&self, other: &Self) -> bool {
        let normalize = |c: &Self| {
            let mut c = c.clone();
            for s in &mut c.skills {
                s.name.clear();
            }
            c
        };
        normalize(self) == normalize(other)
    }
    pub fn validate(&self) -> Result<()> {
        let mount = Shortcut::parse(&self.mount_shortcut)?;
        number("mount_wait_seconds", self.mount_wait_seconds)?;
        number("key_hold_seconds", self.key_hold_seconds)?;
        require(
            self.skills.len() <= 8 && (!self.enabled || !self.skills.is_empty()),
            "Configure one to eight skills when buffing is enabled.",
        )?;
        let mut seen = std::collections::HashSet::new();
        for skill in &self.skills {
            skill.validate()?;
            let key = Shortcut::parse(&skill.shortcut)?;
            require(
                key != mount && seen.insert(key),
                "Skill shortcuts must be unique and differ from mounting.",
            )?;
        }
        if let Some(r) = self.icon_region {
            r.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: Uuid,
    pub name: String,
    // Stored process metadata. Desktop window selection uses
    // per-profile UI preferences; this value does not filter the window list.
    #[serde(default = "default_process")]
    pub process_name: String,
    #[serde(default)]
    pub region: Option<Region>,
    #[serde(default)]
    pub masks: Vec<Region>,
    #[serde(default)]
    pub color: Option<ColorConfig>,
    #[serde(default)]
    pub panel: PanelConfig,
    #[serde(default)]
    pub timing: TimingConfig,
    #[serde(default)]
    pub buffs: BuffConfig,
    #[serde(default)]
    pub loot: crate::LootConfig,
    #[serde(default)]
    pub periodic_actions: Vec<crate::PeriodicAction>,
    #[serde(default)]
    pub puller: crate::PullerConfig,
    #[serde(default)]
    pub out_of_target: crate::OutOfTargetConfig,
    #[serde(default)]
    pub safety: crate::SafetyConfig,
    #[serde(default)]
    pub frame_size: Option<[u32; 2]>,
    #[serde(default)]
    pub template_file: Option<String>,
    #[serde(default)]
    pub template_region: Option<Region>,
}
fn default_process() -> String {
    "game-client.exe".into()
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingPreset {
    Careful,
    Normal,
    Responsive,
}
impl TimingPreset {
    pub fn label(self) -> &'static str {
        match self {
            Self::Careful => "Careful",
            Self::Normal => "Normal",
            Self::Responsive => "Responsive",
        }
    }
    pub fn timing(self) -> TimingConfig {
        match self {
            Self::Normal => TimingConfig::default(),
            Self::Careful => TimingConfig {
                sample_interval: 0.5,
                click_hover_seconds: 0.25,
                click_hold_seconds: 0.25,
                settle_seconds: 1.5,
                search_timeout: 7.,
                acquire_timeout: 25.,
                attack_timeout: 150.,
                rotate_seconds: 0.4,
                ..Default::default()
            },
            Self::Responsive => TimingConfig {
                sample_interval: 0.15,
                click_hover_seconds: 0.1,
                click_hold_seconds: 0.1,
                settle_seconds: 0.5,
                search_timeout: 4.,
                acquire_timeout: 15.,
                rotate_seconds: 0.25,
                ..Default::default()
            },
        }
    }
    pub fn infer(t: &TimingConfig) -> Option<Self> {
        [Self::Careful, Self::Normal, Self::Responsive]
            .into_iter()
            .find(|p| p.timing() == *t)
    }
}
impl Profile {
    pub fn create(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            process_name: default_process(),
            region: None,
            masks: vec![],
            color: None,
            panel: PanelConfig::default(),
            timing: TimingConfig::default(),
            buffs: BuffConfig::default(),
            loot: crate::LootConfig::default(),
            periodic_actions: vec![],
            puller: Default::default(),
            out_of_target: Default::default(),
            safety: crate::SafetyConfig::default(),
            frame_size: None,
            template_file: None,
            template_region: None,
        }
    }
    pub fn ready(&self) -> bool {
        self.region.is_some()
            && self.color.is_some()
            && self.panel.region.is_some()
            && self.frame_size.is_some()
            && self.template_file.is_some()
            && self.buffs.ready()
    }
    pub fn validate(&self) -> Result<()> {
        self.validate_calibration()?;
        self.validate_settings()?;
        self.buffs.validate()?;
        self.loot.validate(self)?;
        crate::validate_scheduled(self)?;
        self.safety.validate()?;
        self.validate_visual_actions()
    }
    /// Profile identity and captured geometry, shared by storage and setup UI.
    pub fn validate_calibration(&self) -> Result<()> {
        require(
            !self.name.trim().is_empty() && self.name.chars().count() <= 80,
            "Profile name must contain 1–80 characters.",
        )?;
        require(
            !self.process_name.is_empty() && !self.process_name.contains(['/', '\\', '\0']),
            "Process hint must be a filename without a path.",
        )?;
        require(self.masks.len() <= 30, "Configure at most 30 masks.")?;
        for r in self.masks.iter().chain(
            [self.region, self.panel.region, self.template_region]
                .iter()
                .flatten(),
        ) {
            r.validate()?;
        }
        if let Some(size) = self.frame_size {
            require(
                size.iter().all(|n| (100..=16384).contains(n)),
                "Invalid calibration dimensions.",
            )?;
        }
        if let Some(file) = &self.template_file {
            require(
                file.ends_with(".png") && !file.contains(['/', '\\', ':']) && !file.contains(".."),
                "Invalid template path.",
            )?;
        }
        Ok(())
    }
    /// Detection and timing settings, independent from configured actions.
    pub fn validate_settings(&self) -> Result<()> {
        if let Some(c) = &self.color {
            c.validate()?;
        }
        number("threshold", self.panel.threshold)?;
        number("stable_frames", self.panel.stable_frames.into())?;
        self.timing.validate()?;
        require(
            self.timing.acquire_timeout
                > f64::from(self.panel.stable_frames + 1) * self.timing.sample_interval * 1.5,
            "Confirmation timeout is too short for the required frames.",
        )
    }
    pub fn validate_visual_actions(&self) -> Result<()> {
        for rule in &self.safety.rules {
            if rule.reaction.execute {
                rule.execution
                    .input
                    .validate(rule.enabled, self.frame_size)?;
                if rule.enabled {
                    let frames =
                        if rule.execution.verification == crate::Verification::ReferenceGone {
                            self.panel.stable_frames.max(2) + 1
                        } else {
                            1
                        };
                    let minimum = self.timing.sample_interval * f64::from(frames) * 1.5;
                    require(
                        rule.execution.timeout_seconds >= minimum,
                        &format!(
                            "{}: verification timeout must be at least {minimum:.2} seconds for the capture interval and confirmation frames.",
                            rule.name
                        ),
                    )?;
                }
            }
        }
        Ok(())
    }
    pub fn from_json(text: &str) -> Result<Self> {
        let mut v: serde_json::Value =
            serde_json::from_str(text).map_err(|e| ValidationError(e.to_string()))?;
        let object = v
            .as_object_mut()
            .ok_or_else(|| ValidationError("Profile must be a JSON object.".into()))?;
        if let Some(id) = object.get("id").and_then(|v| v.as_str()) {
            crate::validation::canonical_uuid(id)?;
        }
        if let Some(color) = object.get("color").filter(|v| !v.is_null()) {
            require(
                color.get("hsv").is_some(),
                "Color configuration requires HSV values.",
            )?;
        }
        if let Some(buffs) = object.get_mut("buffs").and_then(|b| b.as_object_mut()) {
            buffs.remove("initial_mounted");
            let legacy = buffs
                .remove("interval_seconds")
                .unwrap_or(serde_json::json!(120));
            let legacy_number = legacy
                .as_f64()
                .ok_or_else(|| ValidationError("Invalid legacy buff interval.".into()))?;
            number("interval_seconds", legacy_number)?;
            if let Some(skills) = buffs.get_mut("skills").and_then(|s| s.as_array_mut()) {
                for skill in skills {
                    if let Some(s) = skill.as_object_mut() {
                        require(
                            s.contains_key("shortcut"),
                            "A saved skill requires a shortcut.",
                        )?;
                        let inactive = s.get("mode").and_then(|m| m.as_str()) == Some("inactive");
                        if !s.contains_key("interval_seconds")
                            || (!inactive && s["interval_seconds"].is_null())
                        {
                            s.insert(
                                "interval_seconds".into(),
                                if inactive {
                                    serde_json::Value::Null
                                } else {
                                    serde_json::json!(legacy_number)
                                },
                            );
                        }
                        if let Some(name) = s
                            .get_mut("name")
                            .and_then(|n| n.as_str().map(|n| n.trim().to_owned()))
                        {
                            require(
                                !s["name"]
                                    .as_str()
                                    .expect("string name")
                                    .chars()
                                    .any(|c| (c as u32) < 32),
                                "Skill names cannot contain control characters.",
                            )?;
                            s.insert("name".into(), serde_json::json!(name));
                        }
                    }
                }
            }
        }
        let profile: Self =
            serde_json::from_value(v).map_err(|e| ValidationError(e.to_string()))?;
        profile.validate()?;
        Ok(profile)
    }
}
