use crate::{Frame, Result, VisionError, crop, decode_png, stddev, template_score};
use metin_core::{BuffConfig, Presence};
pub struct BuffDetector {
    config: BuffConfig,
    size: [u32; 2],
    max_gap: f64,
    last: Option<f64>,
    absent: Vec<u32>,
    active: Vec<Option<Frame>>,
    inactive: Vec<Option<Frame>>,
}
impl BuffDetector {
    pub fn new(config: BuffConfig, size: [u32; 2], interval: f64) -> Result<Self> {
        let decode = |icon: &Option<metin_core::BuffIcon>| -> Result<Option<Frame>> {
            icon.as_ref()
                .map(|i| {
                    i.png_bytes()
                        .map_err(|e| VisionError::Invalid(e.to_string()))
                        .and_then(|bytes| decode_png(&bytes))
                })
                .transpose()
        };
        let active = config
            .skills
            .iter()
            .map(|s| decode(&s.icon))
            .collect::<Result<Vec<_>>>()?;
        let inactive = config
            .skills
            .iter()
            .map(|s| decode(&s.inactive_icon))
            .collect::<Result<Vec<_>>>()?;
        let n = config.skills.len();
        Ok(Self {
            config,
            size,
            max_gap: 1.0_f64.max(interval * 1.5),
            last: None,
            absent: vec![0; n],
            active,
            inactive,
        })
    }
    pub fn reset(&mut self) {
        self.last = None;
        self.absent.fill(0);
    }
    pub fn observe(&mut self, image: &Frame, now: f64) -> Result<Vec<Presence>> {
        if !self
            .last
            .is_some_and(|last| now > last && now - last <= self.max_gap)
        {
            self.absent.fill(0);
        }
        self.last = Some(now);
        let unknown = vec![Presence::Unknown; self.active.len()];
        let Some(region) = self.config.icon_region else {
            self.absent.fill(0);
            return Ok(unknown);
        };
        if [image.width, image.height] != self.size {
            self.absent.fill(0);
            return Ok(unknown);
        }
        let area = crop(image, region)?;
        if stddev(&area.bgr) < 5. {
            self.absent.fill(0);
            return Ok(unknown);
        }
        let score = |template: &Option<Frame>| -> Result<Option<f64>> {
            if let Some(t) = template {
                if t.width > area.width || t.height > area.height {
                    return Ok(None);
                }
                let n = template_score(&area, t, false)?;
                Ok(n.is_finite().then_some(n))
            } else {
                Ok(None)
            }
        };
        let mut states = vec![];
        for i in 0..self.active.len() {
            let a = score(&self.active[i])?;
            let b = score(&self.inactive[i])?;
            let active = a.is_some_and(|a| a >= 0.90)
                && (self.inactive[i].is_none() || a.zip(b).is_some_and(|(a, b)| a - b >= 0.08));
            let absent = if self.inactive[i].is_some() {
                a.zip(b).is_some_and(|(a, b)| b >= 0.90 && b - a >= 0.08)
            } else {
                a.is_some_and(|a| a <= 0.65)
            };
            if !absent {
                self.absent[i] = 0;
            }
            if active {
                states.push(Presence::Active);
            } else if absent {
                self.absent[i] += 1;
                states.push(if self.absent[i] >= 3 {
                    Presence::Absent
                } else {
                    Presence::Unknown
                });
            } else {
                states.push(Presence::Unknown);
            }
        }
        Ok(states)
    }
}
