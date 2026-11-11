use metin_core::Region;
use metin_vision::{Frame, panel_match};

fn needle() -> Frame {
    Frame::new(
        5,
        5,
        (0..25)
            .flat_map(|n| {
                let value = ((n * n * 19 + n * 7) % 251) as u8;
                [value; 3]
            })
            .collect(),
    )
    .unwrap()
}

fn scene(position: [u32; 2], panel_width: u32) -> Frame {
    let mut pixels = vec![35; 120 * 60 * 3];
    // Changing title/HP extent surrounds the immutable detail.
    for y in 10..23 {
        for x in position[0]..(position[0] + panel_width).min(120) {
            let offset = (y * 120 + x) as usize * 3;
            pixels[offset..offset + 3].copy_from_slice(&[50, 60, 170]);
        }
    }
    let template = needle();
    for y in 0..5 {
        for x in 0..5 {
            let offset = ((position[1] + y) * 120 + position[0] + x) as usize * 3;
            let source = (y * 5 + x) as usize * 3;
            pixels[offset..offset + 3].copy_from_slice(&template.bgr[source..source + 3]);
        }
    }
    Frame::new(120, 60, pixels).unwrap()
}

#[test]
fn exact_five_pixel_detail_moves_and_ignores_changing_panel_extent() {
    for (position, width) in [([8, 13], 24), ([72, 13], 45), ([90, 40], 12)] {
        let found = panel_match(&scene(position, width), &needle()).unwrap();
        assert!(found.score > 0.9999);
        assert_eq!(found.position, position);
        assert_eq!(
            found.bounds(),
            [position[0], position[1], position[0] + 4, position[1] + 4]
        );
        assert_eq!(found.region(120, 60).pixels(120, 60), found.bounds());
    }
}

#[test]
fn full_client_search_finds_details_at_every_frame_edge() {
    for position in [[0, 0], [115, 0], [0, 55], [115, 55]] {
        let found = panel_match(&scene(position, 5), &needle()).unwrap();
        assert_eq!(found.position, position);
        assert!(found.score > 0.9999);
        assert_eq!(found.region(120, 60).pixels(120, 60), found.bounds());
    }
}

#[test]
fn hp_detail_larger_than_the_client_is_rejected() {
    let too_small = Frame::new(4, 5, vec![35; 4 * 5 * 3]).unwrap();
    assert!(panel_match(&too_small, &needle()).is_err());
}

#[test]
fn matched_detail_exclusion_removes_only_the_recognized_client_pixels() {
    use metin_core::{ColorConfig, Profile};
    use metin_vision::{TemplateMatch, detect_candidates, detect_candidates_excluding};
    let mut pixels = vec![30; 120 * 60 * 3];
    for (left, top) in [(10, 10), (80, 35)] {
        for y in top..top + 5 {
            for x in left..left + 5 {
                let offset = (y * 120 + x) as usize * 3;
                pixels[offset..offset + 3].copy_from_slice(&[0, 220, 0]);
            }
        }
    }
    let image = Frame::new(120, 60, pixels).unwrap();
    let mut profile = Profile::create("Full client search");
    profile.region = Some(Region {
        left: 0.,
        top: 0.,
        right: 1.,
        bottom: 1.,
    });
    profile.color = Some(ColorConfig {
        hsv: [60, 255, 220],
        min_width: 2,
        min_height: 2,
        min_area: 4,
        max_area_ratio: 0.2,
        ..Default::default()
    });
    // Whole-client HP matching does not exclude target pixels. Only the
    // confirmed matched detail below may be excluded automatically.
    assert_eq!(detect_candidates(&image, &profile).unwrap().len(), 2);
    let found = TemplateMatch {
        score: 1.,
        position: [10, 10],
        width: 5,
        height: 5,
    };
    let remaining = detect_candidates_excluding(&image, &profile, Some(&found)).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!([remaining[0].x, remaining[0].y], [80, 35]);
    profile.masks.push(Region {
        left: 0.6,
        top: 0.5,
        right: 0.9,
        bottom: 0.9,
    });
    let explicit_only = detect_candidates(&image, &profile).unwrap();
    assert_eq!(explicit_only.len(), 1);
    assert_eq!([explicit_only[0].x, explicit_only[0].y], [10, 10]);
    assert!(
        detect_candidates_excluding(&image, &profile, Some(&found))
            .unwrap()
            .is_empty()
    );
}
