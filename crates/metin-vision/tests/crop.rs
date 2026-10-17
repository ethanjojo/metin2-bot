use metin_core::Region;
use metin_vision::{Frame, crop};
use opencv::{core, prelude::*};
use std::sync::Arc;

fn patterned_frame(width: u32, height: u32) -> Frame {
    Frame::new(
        width,
        height,
        (0..width * height * 3)
            .map(|n| ((n * 37 + n / 7) % 256) as u8)
            .collect(),
    )
    .unwrap()
}

fn opencv_reference(image: &Frame, region: Region) -> Frame {
    let [x1, y1, x2, y2] = region.pixels(image.width, image.height);
    let mut source = core::Mat::new_rows_cols_with_default(
        image.height as i32,
        image.width as i32,
        core::CV_8UC3,
        core::Scalar::all(0.),
    )
    .unwrap();
    source.data_bytes_mut().unwrap().copy_from_slice(&image.bgr);
    let roi = core::Mat::roi(
        &source,
        core::Rect::new(
            x1 as i32,
            y1 as i32,
            (x2 - x1 + 1) as i32,
            (y2 - y1 + 1) as i32,
        ),
    )
    .unwrap()
    .try_clone()
    .unwrap();
    Frame::new(
        roi.cols() as u32,
        roi.rows() as u32,
        roi.data_bytes().unwrap().to_vec(),
    )
    .unwrap()
}

#[test]
fn crops_match_opencv_inclusive_roi_for_tiny_and_rounded_edges() {
    for (width, height) in [(1, 1), (2, 3), (17, 11), (640, 480)] {
        let image = patterned_frame(width, height);
        for [left, top, right, bottom] in [
            [0., 0., 1., 1.],
            [0.1, 0.2, 0.8, 0.9],
            [0.25, 0.25, 0.75, 0.75],
            [0.998, 0.998, 1., 1.],
            [0., 0., 0.002, 0.002],
        ] {
            let region = Region {
                left,
                top,
                right,
                bottom,
            };
            let actual = crop(&image, region).unwrap();
            assert_eq!(actual, opencv_reference(&image, region));
            assert!(!Arc::ptr_eq(&image.bgr, &actual.bgr));
        }
    }
}

#[test]
fn inclusive_endpoints_keep_last_row_column_and_bgr_order() {
    let image = patterned_frame(4, 4);
    let result = crop(
        &image,
        Region {
            left: 0.25,
            top: 0.5,
            right: 1.,
            bottom: 1.,
        },
    )
    .unwrap();
    assert_eq!((result.width, result.height), (3, 2));
    let expected: Vec<u8> = [2, 3]
        .into_iter()
        .flat_map(|y| {
            image.bgr[((y * 4 + 1) * 3)..((y * 4 + 4) * 3)]
                .iter()
                .copied()
        })
        .collect();
    assert_eq!(result.bgr.as_ref(), expected);
}

#[test]
fn invalid_and_reversed_regions_are_rejected_before_crop() {
    let image = patterned_frame(4, 4);
    for [left, top, right, bottom] in [
        [0.8, 0., 0.2, 1.],
        [0., 0.9, 1., 0.1],
        [0., 0., 0.0005, 1.],
        [0., 0., 1., 0.],
        [-0.1, 0., 1., 1.],
        [0., 0., 1.1, 1.],
        [f64::NAN, 0., 1., 1.],
        [0., 0., f64::INFINITY, 1.],
    ] {
        assert!(
            crop(
                &image,
                Region {
                    left,
                    top,
                    right,
                    bottom
                }
            )
            .is_err()
        );
    }
}
