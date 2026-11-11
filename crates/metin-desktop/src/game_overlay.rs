//! Native game annotations and frozen, input-free calibration selection.

use metin_core::{Candidate, Region};
use metin_platform::{Frame, TargetState, TargetToken};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct OverlayScene {
    pub geometry: TargetState,
    pub frame: Frame,
    pub captured_at: Instant,
    pub candidates: Vec<Candidate>,
    pub current: Option<[u32; 2]>,
    pub next: Option<[u32; 2]>,
    pub regions: Vec<(Region, String)>,
    pub show_sizes: bool,
    /// Presentation cadence only; fresh frame and safety checks remain independent.
    pub redraw_interval: std::time::Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    Region,
    Point,
}

#[derive(Debug, Clone)]
pub struct SelectionRequest {
    /// Caller generation: discard results if profile, editor or target changed.
    pub id: u64,
    /// Human context, e.g. the buff name and active/inactive template state.
    pub label: String,
    pub geometry: TargetState,
    pub frame: Frame,
    pub kind: SelectionKind,
    /// Existing shape mapped to the freshly captured source, for move/resize.
    pub initial_region: Option<Region>,
    /// Preserved draft shapes, displayed only; they never change selection/profile data.
    pub context_regions: Vec<(Region, String)>,
    /// Only the color tool previews detections using these canonical settings.
    pub color_preview: Option<Box<metin_core::Profile>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlPanelLink {
    pub window: usize,
    pub target: TargetToken,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OverlayEvent {
    Selected {
        id: u64,
        region: Option<Region>,
        point: [u32; 2],
    },
    Cancelled {
        id: u64,
    },
    Failed {
        id: Option<u64>,
        message: String,
    },
}

#[cfg(any(windows, test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RegionAnnotation {
    pixels: [u32; 4],
    outside_image: bool,
}

/// Clip only presentation bounds; preserve invalid draft coordinates for review.
#[cfg(any(windows, test))]
fn region_annotation(region: Region, width: u32, height: u32) -> Option<RegionAnnotation> {
    if width == 0
        || height == 0
        || ![region.left, region.top, region.right, region.bottom]
            .iter()
            .all(|value| value.is_finite())
        || region.left >= region.right
        || region.top >= region.bottom
    {
        return None;
    }
    let clipped = Region {
        left: region.left.clamp(0., 1.),
        top: region.top.clamp(0., 1.),
        right: region.right.clamp(0., 1.),
        bottom: region.bottom.clamp(0., 1.),
    };
    Some(RegionAnnotation {
        pixels: clipped.pixels(width, height),
        outside_image: region.validate().is_err(),
    })
}

#[cfg(test)]
mod region_annotation_tests {
    use super::*;

    #[test]
    fn valid_region_keeps_canonical_pixel_endpoints_and_normal_annotation_color() {
        let region = Region {
            left: 0.1,
            top: 0.2,
            right: 0.8,
            bottom: 0.9,
        };
        assert_eq!(
            region_annotation(region, 100, 200),
            Some(RegionAnnotation {
                pixels: [10, 40, 80, 180],
                outside_image: false,
            })
        );
        assert_eq!(
            region_annotation(
                Region {
                    left: 0.,
                    top: 0.,
                    right: 1.,
                    bottom: 1.
                },
                100,
                200
            ),
            Some(RegionAnnotation {
                pixels: [0, 0, 99, 199],
                outside_image: false,
            })
        );
    }

    #[test]
    fn outside_draft_is_marked_invalid_and_only_presentation_bounds_are_clipped() {
        let draft = Region {
            left: -0.2,
            top: 0.4,
            right: 1.1,
            bottom: 1.3,
        };
        assert_eq!(
            region_annotation(draft, 100, 200),
            Some(RegionAnnotation {
                pixels: [0, 80, 99, 199],
                outside_image: true,
            })
        );
        assert_eq!(
            draft,
            Region {
                left: -0.2,
                top: 0.4,
                right: 1.1,
                bottom: 1.3
            }
        );
        // A wholly outside region remains visible as an edge marker and warning.
        assert_eq!(
            region_annotation(
                Region {
                    left: 1.1,
                    top: 0.2,
                    right: 1.4,
                    bottom: 0.5
                },
                100,
                200
            ),
            Some(RegionAnnotation {
                pixels: [99, 40, 99, 100],
                outside_image: true,
            })
        );
    }

    #[test]
    fn malformed_regions_and_empty_images_have_no_annotation() {
        let valid = Region {
            left: 0.1,
            top: 0.2,
            right: 0.8,
            bottom: 0.9,
        };
        for region in [
            Region {
                left: f64::NAN,
                ..valid
            },
            Region {
                bottom: f64::INFINITY,
                ..valid
            },
            Region {
                left: 0.8,
                right: 0.1,
                ..valid
            },
            Region {
                top: 0.9,
                bottom: 0.2,
                ..valid
            },
            Region {
                right: 0.1,
                ..valid
            },
        ] {
            assert_eq!(region_annotation(region, 100, 200), None);
        }
        assert_eq!(region_annotation(valid, 0, 200), None);
        assert_eq!(region_annotation(valid, 100, 0), None);
    }
}

/// All coordinates are physical client pixels, including on mixed-DPI monitors.
/// The divide-by-size normalization intentionally matches `Region::pixels`.
#[cfg(any(windows, test))]
fn selection_region(a: [u32; 2], b: [u32; 2], width: u32, height: u32) -> Result<Region, String> {
    if width == 0 || height == 0 {
        return Err("The captured client is empty.".into());
    }
    let x = |v: u32| f64::from(v.min(width - 1)) / f64::from(width);
    let y = |v: u32| f64::from(v.min(height - 1)) / f64::from(height);
    let region = Region {
        left: x(a[0].min(b[0])),
        top: y(a[1].min(b[1])),
        right: x(a[0].max(b[0])),
        bottom: y(a[1].max(b[1])),
    };
    region.validate().map_err(|error| error.to_string())?;
    Ok(region)
}

#[cfg(any(windows, test))]
fn client_point(screen: [i32; 2], origin: [i32; 2], size: [u32; 2]) -> [u32; 2] {
    std::array::from_fn(|i| {
        (i64::from(screen[i]) - i64::from(origin[i])).clamp(0, i64::from(size[i].saturating_sub(1)))
            as u32
    })
}

#[cfg(windows)]
fn validate_frame(geometry: TargetState, frame: &Frame) -> Result<(), String> {
    if !geometry.visible
        || geometry.width != frame.width
        || geometry.height != frame.height
        || frame.width == 0
        || frame.height == 0
        || frame.width > 16384
        || frame.height > 16384
        || frame.bgr.len() != frame.width as usize * frame.height as usize * 3
    {
        return Err("Capture dimensions no longer match the visible game client.".into());
    }
    Ok(())
}

#[cfg(windows)]
mod native;
#[cfg(any(windows, test))]
mod selection;
#[cfg(windows)]
pub use native::{GameOverlay, inspect_target};

#[cfg(not(windows))]
pub fn inspect_target(_target: TargetToken) -> Option<TargetState> {
    None
}

#[cfg(not(windows))]
pub struct GameOverlay;
#[cfg(not(windows))]
impl GameOverlay {
    pub fn new() -> Result<Self, String> {
        Err("Native game overlays require Windows.".into())
    }
    pub fn show_annotations(&self, _scene: OverlayScene) -> Result<(), String> {
        Err("Native game overlays require Windows.".into())
    }
    /// Call only after automation has stopped AND its input-release acknowledgement arrived.
    pub fn begin_selection_after_release(&self, _request: SelectionRequest) -> Result<(), String> {
        Err("Native game overlays require Windows.".into())
    }
    pub fn poll(&self) -> Option<OverlayEvent> {
        None
    }
    pub fn hide(&self) {}
    pub fn set_control_panel(&self, _link: Option<ControlPanelLink>) -> Result<(), String> {
        Ok(())
    }
    pub fn annotation_window_handle(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inclusive_selection_round_trips_in_reverse_and_clamps() {
        let region = selection_region([1900, 1000], [100, 10], 1920, 1080).unwrap();
        assert_eq!(region.pixels(1920, 1080), [100, 10, 1900, 1000]);
        let region = selection_region([0, 0], [u32::MAX, u32::MAX], 1920, 1080).unwrap();
        assert_eq!(region.pixels(1920, 1080), [0, 0, 1919, 1079]);
    }

    #[test]
    fn physical_client_coordinates_preserve_negative_monitor_origins() {
        assert_eq!(
            client_point([-1820, -410], [-1920, -420], [1920, 1080]),
            [100, 10]
        );
        assert_eq!(
            client_point([-2200, 990], [-1920, -420], [1920, 1080]),
            [0, 1079]
        );
        // Physical coordinates already incorporate per-monitor DPI; never scale them twice.
        assert_eq!(
            client_point([1700, 850], [200, 100], [1500, 900]),
            [1499, 750]
        );
    }

    #[test]
    fn degenerate_regions_are_rejected_by_the_canonical_validation() {
        assert!(selection_region([9, 9], [9, 9], 100, 100).is_err());
        assert!(selection_region([0, 0], [2, 2], 0, 0).is_err());
    }

    #[test]
    fn small_high_resolution_selections_retain_exact_pixel_endpoints() {
        let region = selection_region([500, 600], [505, 605], 16384, 16384).unwrap();
        assert_eq!(region.pixels(16384, 16384), [500, 600, 505, 605]);
    }
}
