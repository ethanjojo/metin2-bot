use crate::{Frame, Result, decode_png, region_template_score, validate_template};
use metin_core::{Region, SafetyRule};
use uuid::Uuid;
pub struct SafetyDetector {
    references: Vec<(Uuid, Region, Frame)>,
}
impl SafetyDetector {
    pub fn new(rules: &[SafetyRule]) -> Result<Self> {
        let mut references = vec![];
        for rule in rules.iter().filter(|r| r.enabled) {
            rule.validate()
                .map_err(|e| crate::VisionError::Invalid(e.to_string()))?;
            let reference = rule.reference.as_ref().expect("validated reference");
            let image = decode_png(
                &reference
                    .png_bytes()
                    .map_err(|e| crate::VisionError::Invalid(e.to_string()))?,
            )?;
            validate_template(&image)?;
            references.push((rule.id, reference.region.expect("validated region"), image));
        }
        Ok(Self { references })
    }
    pub fn observe(&self, frame: &Frame) -> Result<Vec<(Uuid, f64)>> {
        self.references
            .iter()
            .map(|(id, region, template)| {
                Ok((*id, region_template_score(frame, template, *region)?))
            })
            .collect()
    }
}
