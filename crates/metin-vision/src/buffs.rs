use crate::{Frame, Result, VisionError, crop, decode_png, stddev};
use metin_core::{BuffConfig, Presence};
pub const BUFF_MATCH_THRESHOLD: f64 = 0.90;
const BUFF_MATCH_MARGIN: f64 = 0.08;
const BUFF_MISSING_THRESHOLD: f64 = 0.65;

fn decode_icon(icon: &Option<metin_core::BuffIcon>) -> Result<Option<Frame>> {
    icon.as_ref()
        .map(|icon| {
            icon.png_bytes()
                .map_err(|error| VisionError::Invalid(error.to_string()))
                .and_then(|bytes| decode_png(&bytes))
        })
        .transpose()
}
struct Appearance {
    active_score: Option<f64>,
    inactive_score: Option<f64>,
    active: bool,
    absent: bool,
    best_match: Option<crate::TemplateMatch>,
}
fn appearance(
    area: &Frame,
    active: &Option<Frame>,
    inactive: &Option<Frame>,
) -> Result<Appearance> {
    let find = |template: &Option<Frame>| -> Result<Option<crate::TemplateMatch>> {
        let Some(template) = template
            .as_ref()
            .filter(|template| template.width <= area.width && template.height <= area.height)
        else {
            return Ok(None);
        };
        let found = crate::detector::match_template(area, template, false)?;
        Ok(found.score.is_finite().then_some(found))
    };
    let active_found = find(active)?;
    let inactive_found = find(inactive)?;
    let a = active_found.map(|found| found.score);
    let b = inactive_found.map(|found| found.score);
    Ok(Appearance {
        active_score: a,
        inactive_score: b,
        active: a.is_some_and(|value| value >= BUFF_MATCH_THRESHOLD)
            && (inactive.is_none() || a.zip(b).is_some_and(|(a, b)| a - b >= BUFF_MATCH_MARGIN)),
        absent: if inactive.is_some() {
            a.zip(b)
                .is_some_and(|(a, b)| b >= BUFF_MATCH_THRESHOLD && b - a >= BUFF_MATCH_MARGIN)
        } else {
            a.is_some_and(|a| a <= BUFF_MISSING_THRESHOLD)
        },
        best_match: match (active_found, inactive_found) {
            (Some(a), Some(b)) => Some(if a.score >= b.score { a } else { b }),
            (a, b) => a.or(b),
        },
    })
}
pub struct BuffDiagnostic {
    pub area: Frame,
    pub current: Frame,
    pub active: Option<Frame>,
    pub inactive: Option<Frame>,
    pub active_score: Option<f64>,
    pub inactive_score: Option<f64>,
    pub active_match: bool,
    pub inactive_match: bool,
    pub message: String,
}
/// Inspect appearance evidence without advancing temporal confirmation or hold
/// state. The same matcher and thresholds are used by the live detector.
pub fn inspect_buff(image: &Frame, config: &BuffConfig, index: usize) -> Result<BuffDiagnostic> {
    let skill = config
        .skills
        .get(index)
        .ok_or_else(|| VisionError::Invalid("The selected buff no longer exists.".into()))?;
    let region = config
        .icon_region
        .ok_or_else(|| VisionError::Invalid("Select the buff detection area first.".into()))?;
    let area = crop(image, region)?;
    let active = decode_icon(&skill.icon)?;
    let inactive = decode_icon(&skill.inactive_icon)?;
    let evidence = appearance(&area, &active, &inactive)?;
    let contrast = stddev(&area.bgr) >= 5.;
    let active_match = contrast && evidence.active;
    let inactive_match = contrast && evidence.absent && inactive.is_some();
    let message=if !contrast {"Unknown · The current detection area has too little contrast"}
        else if active.is_none(){"Unknown · Capture the Active reference first"}
        else if evidence.active_score.is_none(){"Unknown · The Active reference is larger than the detection area"}
        else if active_match {"Active reference matches this image"}
        else if inactive_match {"Inactive reference matches this image; live confirmation requires consecutive samples"}
        else if evidence.absent && inactive.is_none() {"Active reference was not found; no Inactive reference is configured. Live checks require repeated missing evidence"}
        else if evidence.active_score.zip(evidence.inactive_score).is_some_and(|(a,b)|a>=BUFF_MATCH_THRESHOLD && b>=BUFF_MATCH_THRESHOLD) {"Unknown · Both references match too similarly to distinguish Active from Inactive"}
        else {"Unknown · The references do not confidently distinguish this image"}.into();
    let current = if let Some(found) = evidence.best_match {
        crop(&area, found.region(area.width, area.height))?
    } else {
        area.clone()
    };
    Ok(BuffDiagnostic {
        area,
        current,
        active,
        inactive,
        active_score: evidence.active_score,
        inactive_score: evidence.inactive_score,
        active_match,
        inactive_match,
        message,
    })
}
pub struct BuffDetector {
    config: BuffConfig,
    size: [u32; 2],
    max_gap: f64,
    last: Option<f64>,
    absent: Vec<u32>,
    last_active: Vec<Option<f64>>,
    active: Vec<Option<Frame>>,
    inactive: Vec<Option<Frame>>,
}
impl BuffDetector {
    pub fn new(config: BuffConfig, size: [u32; 2], interval: f64) -> Result<Self> {
        let active = config
            .skills
            .iter()
            .map(|s| decode_icon(&s.icon))
            .collect::<Result<Vec<_>>>()?;
        let inactive = config
            .skills
            .iter()
            .map(|s| decode_icon(&s.inactive_icon))
            .collect::<Result<Vec<_>>>()?;
        let n = config.skills.len();
        Ok(Self {
            config,
            size,
            max_gap: 1.0_f64.max(interval * 1.5),
            last: None,
            absent: vec![0; n],
            last_active: vec![None; n],
            active,
            inactive,
        })
    }
    pub fn reset(&mut self) {
        self.last = None;
        self.absent.fill(0);
        self.last_active.fill(None);
    }
    pub fn last_active_ages(&self, now: f64) -> Vec<Option<f64>> {
        self.last_active
            .iter()
            .map(|last| {
                last.and_then(|last| (now.is_finite() && now >= last).then_some(now - last))
            })
            .collect()
    }
    pub fn observe(&mut self, image: &Frame, now: f64) -> Result<Vec<Presence>> {
        if !now.is_finite() {
            self.reset();
            return Ok(vec![Presence::Unknown; self.active.len()]);
        }
        if !self
            .last
            .is_some_and(|last| now > last && now - last <= self.max_gap)
        {
            self.reset();
        }
        self.last = Some(now);
        let unknown = vec![Presence::Unknown; self.active.len()];
        let Some(region) = self.config.icon_region else {
            self.reset();
            return Ok(unknown);
        };
        if [image.width, image.height] != self.size {
            self.reset();
            return Ok(unknown);
        }
        let area = crop(image, region)?;
        if stddev(&area.bgr) < 5. {
            self.reset();
            return Ok(unknown);
        }
        let mut states = vec![];
        for i in 0..self.active.len() {
            let evidence = appearance(&area, &self.active[i], &self.inactive[i])?;
            let (active, absent) = (evidence.active, evidence.absent);
            if !absent {
                self.absent[i] = 0;
            } else {
                self.absent[i] = self.absent[i].saturating_add(1);
            }
            let confirmed_absent = self.absent[i] >= 3;
            let held = self.last_active[i]
                .is_some_and(|last| now - last < self.config.skills[i].active_hold_seconds);
            if active {
                self.last_active[i] = Some(now);
                states.push(Presence::Active);
            } else if confirmed_absent && self.inactive[i].is_some() {
                // Positive inactive evidence overrides the hold; mere failure to
                // find the active appearance must wait for its hold to expire.
                self.last_active[i] = None;
                states.push(Presence::Absent);
            } else if held {
                states.push(Presence::Active);
            } else if confirmed_absent {
                self.last_active[i] = None;
                states.push(Presence::Absent);
            } else {
                states.push(Presence::Unknown);
            }
        }
        Ok(states)
    }
}
