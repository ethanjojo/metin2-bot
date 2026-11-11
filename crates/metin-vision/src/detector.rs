use crate::{Frame, Result, mat, validate_template};
use metin_core::{Candidate, Profile, Region};
use opencv::{core, imgproc, prelude::*};

/// Detect targets using only the explicit user masks.
pub fn detect_candidates(image: &Frame, profile: &Profile) -> Result<Vec<Candidate>> {
    detect_candidates_excluding(image, profile, None)
}

/// Also exclude the actual confirmed fixed-detail match, when the caller has one.
/// The caller applies the HP similarity threshold before passing that match.
pub fn detect_candidates_excluding(
    image: &Frame,
    profile: &Profile,
    matched_detail: Option<&TemplateMatch>,
) -> Result<Vec<Candidate>> {
    let (Some(c), Some(region)) = (&profile.color, profile.region) else {
        return Ok(vec![]);
    };
    let mut hsv = core::Mat::default();
    imgproc::cvt_color_def(&mat(image)?, &mut hsv, imgproc::COLOR_BGR2HSV)?;
    let mut selected = core::Mat::new_rows_cols_with_default(
        image.height as i32,
        image.width as i32,
        core::CV_8UC1,
        core::Scalar::all(0.),
    )?;
    let [x1, y1, x2, y2] = region.pixels(image.width, image.height);
    let mut masks: Vec<_> = profile
        .masks
        .iter()
        .map(|r| r.pixels(image.width, image.height))
        .collect();
    if let Some(found) = matched_detail {
        masks.push(found.bounds());
    }
    let data = hsv.data_bytes()?;
    let pixels = selected.data_bytes_mut()?;
    for y in y1..=y2 {
        for x in x1..=x2 {
            if masks
                .iter()
                .any(|[a, b, d, e]| x >= *a && x <= *d && y >= *b && y <= *e)
            {
                continue;
            }
            let offset = (y * image.width + x) as usize;
            let h = (i32::from(data[offset * 3]) - i32::from(c.hsv[0])).abs();
            if h.min(180 - h) <= c.hue_tolerance as i32
                && (i32::from(data[offset * 3 + 1]) - i32::from(c.hsv[1])).abs()
                    <= c.saturation_tolerance as i32
                && (i32::from(data[offset * 3 + 2]) - i32::from(c.hsv[2])).abs()
                    <= c.value_tolerance as i32
            {
                pixels[offset] = 255;
            }
        }
    }
    let kernel = core::Mat::ones(3, 3, core::CV_8U)?.to_mat()?;
    let mut joined = core::Mat::default();
    imgproc::morphology_ex(
        &selected,
        &mut joined,
        imgproc::MORPH_CLOSE,
        &kernel,
        core::Point::new(-1, -1),
        1,
        core::BORDER_CONSTANT,
        imgproc::morphology_default_border_value()?,
    )?;
    // Closing can expand into excluded regions: clip again exactly as the reference.
    let joined_data = joined.data_bytes_mut()?;
    for y in 0..image.height {
        for x in 0..image.width {
            if x < x1
                || x > x2
                || y < y1
                || y > y2
                || masks
                    .iter()
                    .any(|[a, b, d, e]| x >= *a && x <= *d && y >= *b && y <= *e)
            {
                joined_data[(y * image.width + x) as usize] = 0;
            }
        }
    }
    let (mut labels, mut stats, mut centers) = (
        core::Mat::default(),
        core::Mat::default(),
        core::Mat::default(),
    );
    let count = imgproc::connected_components_with_stats(
        &joined,
        &mut labels,
        &mut stats,
        &mut centers,
        8,
        core::CV_32S,
    )?;
    let mut found = vec![];
    for label in 1..count {
        let x = *stats.at_2d::<i32>(label, 0)? as u32;
        let y = *stats.at_2d::<i32>(label, 1)? as u32;
        let w = *stats.at_2d::<i32>(label, 2)? as u32;
        let h = *stats.at_2d::<i32>(label, 3)? as u32;
        let area = *stats.at_2d::<i32>(label, 4)? as u32;
        if w < c.min_width
            || h < c.min_height
            || area < c.min_area
            || f64::from(area) > f64::from(image.width) * f64::from(image.height) * c.max_area_ratio
            || !(0.2..=5.).contains(&(f64::from(w) / f64::from(h)))
        {
            continue;
        }
        let cx = *centers.at_2d::<f64>(label, 0)?;
        let cy = *centers.at_2d::<f64>(label, 1)?;
        let mut best: Option<(f64, [u32; 2])> = None;
        for py in y..y + h {
            for px in x..x + w {
                if *labels.at_2d::<i32>(py as i32, px as i32)? == label
                    && pixels_at(&selected, py, px)? != 0
                {
                    let distance = (f64::from(px) - cx).powi(2) + (f64::from(py) - cy).powi(2);
                    if best.as_ref().is_none_or(|(old, _)| distance < *old) {
                        best = Some((distance, [px, py]));
                    }
                }
            }
        }
        if let Some((_, point)) = best {
            found.push(Candidate {
                x,
                y,
                width: w,
                height: h,
                point,
            });
        }
    }
    let center = [f64::from(x1 + x2) / 2., f64::from(y1 + y2) / 2.];
    let distance = |c: &Candidate| {
        (f64::from(c.point[0]) - center[0]).powi(2) + (f64::from(c.point[1]) - center[1]).powi(2)
    };
    found.sort_by(|a, b| {
        distance(a)
            .total_cmp(&distance(b))
            .then(a.x.cmp(&b.x))
            .then(a.y.cmp(&b.y))
    });
    Ok(found)
}
fn pixels_at(m: &core::Mat, y: u32, x: u32) -> Result<u8> {
    Ok(*m.at_2d::<u8>(y as i32, x as i32)?)
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemplateMatch {
    pub score: f64,
    /// Top-left pixel in the connected client frame.
    pub position: [u32; 2],
    pub width: u32,
    pub height: u32,
}
impl TemplateMatch {
    pub fn bounds(self) -> [u32; 4] {
        [
            self.position[0],
            self.position[1],
            self.position[0] + self.width - 1,
            self.position[1] + self.height - 1,
        ]
    }
    pub fn region(self, frame_width: u32, frame_height: u32) -> Region {
        let [left, top, right, bottom] = self.bounds();
        Region {
            left: f64::from(left) / f64::from(frame_width),
            top: f64::from(top) / f64::from(frame_height),
            right: f64::from(right) / f64::from(frame_width),
            bottom: f64::from(bottom) / f64::from(frame_height),
        }
    }
}

fn match_template(image: &Frame, template: &Frame, grayscale: bool) -> Result<TemplateMatch> {
    if template.width > image.width || template.height > image.height {
        return Err(crate::VisionError::Invalid(
            "Template is larger than search region.".into(),
        ));
    }
    let (mut area, mut needle) = (mat(image)?, mat(template)?);
    if grayscale {
        let (mut a, mut n) = (core::Mat::default(), core::Mat::default());
        imgproc::cvt_color_def(&area, &mut a, imgproc::COLOR_BGR2GRAY)?;
        imgproc::cvt_color_def(&needle, &mut n, imgproc::COLOR_BGR2GRAY)?;
        area = a;
        needle = n;
    }
    let mut matches = core::Mat::default();
    imgproc::match_template(
        &area,
        &needle,
        &mut matches,
        imgproc::TM_CCOEFF_NORMED,
        &core::no_array(),
    )?;
    let mut max = 0.;
    let mut position = core::Point::default();
    core::min_max_loc(
        &matches,
        None,
        Some(&mut max),
        None,
        Some(&mut position),
        &core::no_array(),
    )?;
    Ok(TemplateMatch {
        score: max,
        position: [position.x as u32, position.y as u32],
        width: template.width,
        height: template.height,
    })
}

pub fn template_score(image: &Frame, template: &Frame, grayscale: bool) -> Result<f64> {
    Ok(match_template(image, template, grayscale)?.score)
}

/// Find an HP detail anywhere in the complete connected game-client frame.
/// Callers supply the captured client, never a desktop or monitor screenshot.
pub fn panel_match(image: &Frame, template: &Frame) -> Result<TemplateMatch> {
    validate_template(template)?;
    match_template(image, template, true)
}

/// Score a fixed reference inside an explicit region, used by visual safety rules.
pub fn region_template_score(image: &Frame, template: &Frame, region: Region) -> Result<f64> {
    validate_template(template)?;
    let area = crate::crop(image, region)?;
    Ok(match_template(&area, template, true)?.score)
}
