//! OpenCV detection, using the same algorithms and fixtures as the reference.
mod buffs;
mod detector;
mod tracking;
pub use buffs::*;
pub use detector::*;
pub use metin_platform::Frame;
use opencv::{core, imgcodecs, prelude::*};
use std::path::Path;
pub use tracking::*;

#[derive(Debug, thiserror::Error)]
pub enum VisionError {
    #[error("{0}")]
    OpenCv(#[from] opencv::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Invalid(String),
}
pub type Result<T> = std::result::Result<T, VisionError>;
pub(crate) fn mat(frame: &Frame) -> Result<core::Mat> {
    let mut m = core::Mat::new_rows_cols_with_default(
        frame.height as i32,
        frame.width as i32,
        core::CV_8UC3,
        core::Scalar::all(0.),
    )?;
    m.data_bytes_mut()?.copy_from_slice(&frame.bgr);
    Ok(m)
}
pub(crate) fn frame(m: &core::Mat) -> Result<Frame> {
    Frame::new(m.cols() as u32, m.rows() as u32, m.data_bytes()?.to_vec())
        .map_err(|e| VisionError::Invalid(e.to_string()))
}
pub fn load_image(path: &Path) -> Result<Frame> {
    decode_png(&std::fs::read(path)?)
}
pub fn decode_png(bytes: &[u8]) -> Result<Frame> {
    if bytes.len() < 24 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(VisionError::Invalid("Expected a PNG image.".into()));
    }
    for offset in [16, 20] {
        let n = u32::from_be_bytes(bytes[offset..offset + 4].try_into().expect("header length"));
        if n == 0 || n > 16384 {
            return Err(VisionError::Invalid("PNG dimensions exceed limits.".into()));
        }
    }
    let m = imgcodecs::imdecode(&core::Vector::from_slice(bytes), imgcodecs::IMREAD_COLOR)?;
    frame(&m)
}
pub fn encode_png(image: &Frame) -> Result<Vec<u8>> {
    let mut output = core::Vector::<u8>::new();
    if !imgcodecs::imencode(".png", &mat(image)?, &mut output, &core::Vector::new())? {
        return Err(VisionError::Invalid("Unable to encode PNG.".into()));
    }
    Ok(output.to_vec())
}
pub fn crop(image: &Frame, region: metin_core::Region) -> Result<Frame> {
    region
        .validate()
        .map_err(|e| VisionError::Invalid(e.to_string()))?;
    let [x1, y1, x2, y2] = region.pixels(image.width, image.height);
    let source = mat(image)?;
    frame(
        &core::Mat::roi(
            &source,
            core::Rect::new(
                x1 as i32,
                y1 as i32,
                (x2 - x1 + 1) as i32,
                (y2 - y1 + 1) as i32,
            ),
        )?
        .try_clone()?,
    )
}
pub(crate) fn stddev(bytes: &[u8]) -> f64 {
    let mean = bytes.iter().map(|v| f64::from(*v)).sum::<f64>() / bytes.len() as f64;
    (bytes
        .iter()
        .map(|v| (f64::from(*v) - mean).powi(2))
        .sum::<f64>()
        / bytes.len() as f64)
        .sqrt()
}
pub fn validate_template(image: &Frame) -> Result<()> {
    let mut gray = core::Mat::default();
    opencv::imgproc::cvt_color_def(&mat(image)?, &mut gray, opencv::imgproc::COLOR_BGR2GRAY)?;
    if image.width < 5 || image.height < 5 || stddev(gray.data_bytes()?) < 5. {
        Err(VisionError::Invalid(
            "Select a distinctive crop of at least 5 × 5 pixels.".into(),
        ))
    } else {
        Ok(())
    }
}
pub fn pick_color(image: &Frame, point: [u32; 2]) -> Result<[u16; 3]> {
    if point[0] >= image.width || point[1] >= image.height {
        return Err(VisionError::Invalid("Color picker outside frame.".into()));
    }
    let mut channels = [vec![], vec![], vec![]];
    for y in point[1].saturating_sub(1)..=(point[1] + 1).min(image.height - 1) {
        for x in point[0].saturating_sub(1)..=(point[0] + 1).min(image.width - 1) {
            for (c, values) in channels.iter_mut().enumerate() {
                values.push(image.bgr[(y * image.width + x) as usize * 3 + c]);
            }
        }
    }
    let mut bgr = vec![];
    for values in &mut channels {
        values.sort_unstable();
        let n = values.len();
        bgr.push(if n % 2 == 0 {
            ((u16::from(values[n / 2 - 1]) + u16::from(values[n / 2])) / 2) as u8
        } else {
            values[n / 2]
        });
    }
    let one = Frame::new(1, 1, bgr).map_err(|e| VisionError::Invalid(e.to_string()))?;
    let mut hsv = core::Mat::default();
    opencv::imgproc::cvt_color_def(&mat(&one)?, &mut hsv, opencv::imgproc::COLOR_BGR2HSV)?;
    let p = hsv.data_bytes()?;
    Ok([p[0].into(), p[1].into(), p[2].into()])
}
